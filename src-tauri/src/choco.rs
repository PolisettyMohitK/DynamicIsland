//! "Hey Choco": an always-on wake word (openWakeWord front-end with our own "hey choco"
//! model, see tools/wakeword), then the spoken command is recorded until you stop talking
//! and turned into text (Moonshine). Both run offline through ONNX Runtime. The frontend
//! decides what the text means.
//!
//! Events:
//! - `choco` {state: "loading" | "ready" | "off" | "thinking" | "idle"},
//!   {state: "listening", followUp}, {state: "heard", text} or {state: "error", message}
//! - `choco-level` 0–1 microphone level while listening.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

const RATE: usize = 16_000;
/// openWakeWord works on 80ms chunks.
const CHUNK: usize = 1280;
const WAKE_THRESHOLD: f32 = 0.5;
/// Trained by tools/wakeword/train_choco.py; small enough to ship inside the app.
const WAKE_MODEL: &[u8] = include_bytes!("../models/hey_choco.onnx");

const OWW: &str = "https://github.com/dscripka/openWakeWord/releases/download/v0.5.1";
const MOONSHINE: &str = "https://huggingface.co/onnx-community/moonshine-base-ONNX/resolve/main";
/// (file name on disk, where to get it)
const MODELS: [(&str, &str); 5] = [
    ("melspectrogram.onnx", "{OWW}/melspectrogram.onnx"),
    ("embedding_model.onnx", "{OWW}/embedding_model.onnx"),
    ("moonshine_encoder.onnx", "{MOON}/onnx/encoder_model_quantized.onnx"),
    ("moonshine_decoder.onnx", "{MOON}/onnx/decoder_model_quantized.onnx"),
    ("moonshine_tokenizer.json", "{MOON}/tokenizer.json"),
];

/// Recording ends after this much quiet once you've started talking.
const END_SILENCE: Duration = Duration::from_millis(900);
/// Give up if nothing is said this long after the wake word.
const NO_SPEECH: Duration = Duration::from_secs(5);
/// After an answer Choco listens a little for a follow-up, without the wake word.
const NO_FOLLOW_UP: Duration = Duration::from_secs(4);
const MAX_COMMAND: Duration = Duration::from_secs(20);

pub struct Choco {
    enabled: AtomicBool,
    /// Listen once without the wake word (a follow-up question).
    follow_up: AtomicBool,
    /// Wake score needed, as f32 bits. Lower wakes more easily.
    threshold: AtomicU32,
    /// Microphone by name; None is the Windows default.
    mic: Mutex<Option<String>>,
    /// Settings changed: reopen the microphone.
    reopen: AtomicBool,
}

impl Default for Choco {
    fn default() -> Self {
        Choco {
            enabled: AtomicBool::new(false),
            follow_up: AtomicBool::new(false),
            threshold: AtomicU32::new(WAKE_THRESHOLD.to_bits()),
            mic: Mutex::new(None),
            reopen: AtomicBool::new(false),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(tag = "state", rename_all = "camelCase")]
enum Event {
    Loading,
    Ready,
    Off,
    Listening {
        #[serde(rename = "followUp")]
        follow_up: bool,
    },
    Thinking,
    Idle,
    Heard { text: String },
    Error { message: String },
}

fn emit(app: &AppHandle, e: Event) {
    let _ = app.emit("choco", e);
}

pub fn spawn(app: AppHandle) {
    let choco = Arc::new(Choco::default());
    app.manage(choco.clone());
    std::thread::spawn(move || run(app, choco));
}

#[tauri::command]
pub fn choco_set_enabled(choco: State<'_, Arc<Choco>>, on: bool) {
    choco.enabled.store(on, Ordering::Relaxed);
}

/// Listen once more without the wake word, right after an answer.
#[tauri::command]
pub fn choco_follow_up(choco: State<'_, Arc<Choco>>) {
    choco.follow_up.store(true, Ordering::Relaxed);
}

/// `threshold` 0–1: the wake score needed. `mic`: a name from `choco_mics`, or null for the default.
#[tauri::command]
pub fn choco_configure(choco: State<'_, Arc<Choco>>, threshold: f32, mic: Option<String>) {
    choco.threshold.store(threshold.clamp(0.05, 0.95).to_bits(), Ordering::Relaxed);
    let mut current = choco.mic.lock().unwrap();
    if *current != mic {
        *current = mic;
        choco.reopen.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub fn choco_mics() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|d| d.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

fn run(app: AppHandle, choco: Arc<Choco>) {
    let mut models: Option<(Wake, Stt)> = None;
    loop {
        if !choco.enabled.load(Ordering::Relaxed) || crate::game::on() {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        if models.is_none() {
            emit(&app, Event::Loading);
            match load(&app) {
                Ok(m) => models = Some(m),
                Err(e) => {
                    emit(&app, Event::Error { message: format!("Couldn't load voice models: {e}") });
                    choco.enabled.store(false, Ordering::Relaxed);
                    continue;
                }
            }
        }
        let (wake, stt) = models.as_mut().unwrap();
        if let Err(e) = listen(&app, &choco, wake, stt) {
            emit(&app, Event::Error { message: format!("Microphone: {e}") });
            // Try again shortly (e.g. a headset was unplugged).
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

fn load(app: &AppHandle) -> Res<(Wake, Stt)> {
    let dir = app.path().app_local_data_dir()?.join("models");
    std::fs::create_dir_all(&dir)?;
    for (name, url) in MODELS {
        let path = dir.join(name);
        if !path.exists() {
            let url = url.replace("{OWW}", OWW).replace("{MOON}", MOONSHINE);
            tauri::async_runtime::block_on(download(&url, &path))?;
        }
    }
    let env = ort::init().with_name("choco").build()?;
    Ok((Wake::new(&env, &dir, WAKE_MODEL)?, Stt::new(&env, &dir)?))
}

async fn download(url: &str, to: &Path) -> Res<()> {
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    // Written under a temporary name so a half-finished download is never loaded.
    let part = to.with_extension("part");
    std::fs::write(&part, &bytes)?;
    std::fs::rename(part, to)?;
    Ok(())
}

/// One thread per model: Choco should never compete with what you're doing.
fn builder(env: &Environment) -> Res<ort::session::builder::SessionBuilder> {
    Ok(Session::builder(env)?.with_intra_threads(1).map_err(ort::Error::<()>::from)?)
}

fn session(env: &Environment, path: PathBuf) -> Res<Session> {
    Ok(builder(env)?.commit_from_file(path)?)
}

/// Runs until Choco is turned off or its settings change (Ok), or the microphone fails (Err).
fn listen(app: &AppHandle, choco: &Choco, wake: &mut Wake, stt: &mut Stt) -> Res<()> {
    choco.reopen.store(false, Ordering::Relaxed);
    let mic = choco.mic.lock().unwrap().clone();
    let (_stream, rx) = open_mic(mic.as_deref())?;
    emit(app, Event::Ready);
    wake.reset();

    let mut pending: Vec<f32> = Vec::with_capacity(CHUNK * 2);
    let mut floor = 200.0f32;
    let mut command: Option<Command> = None;

    loop {
        if !choco.enabled.load(Ordering::Relaxed) {
            emit(app, Event::Off);
            return Ok(());
        }
        // A game came up: let go of the microphone quietly; listening resumes after.
        if crate::game::on() {
            return Ok(());
        }
        if choco.reopen.load(Ordering::Relaxed) {
            return Ok(());
        }
        if command.is_none() && choco.follow_up.swap(false, Ordering::Relaxed) {
            command = Some(Command::start(app, true));
        }
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(samples) => pending.extend(samples),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Err("audio stream stopped".into()),
        }

        while pending.len() >= CHUNK {
            let chunk: Vec<f32> = pending.drain(..CHUNK).collect();
            let level = rms(&chunk);

            match command.as_mut() {
                None => {
                    // Track the room's background noise so "talking" is relative to it.
                    if level < floor * 2.0 {
                        floor = (floor * 0.95 + level * 0.05).max(50.0);
                    }
                    if wake.push(&chunk)? > f32::from_bits(choco.threshold.load(Ordering::Relaxed)) {
                        command = Some(Command::start(app, false));
                    }
                }
                Some(c) => {
                    let _ = app.emit("choco-level", (level / 3000.0).min(1.0));
                    if let Some(done) = c.push(&chunk, level, floor) {
                        command = None;
                        wake.reset();
                        match done {
                            Some(audio) => {
                                emit(app, Event::Thinking);
                                let text = stt.transcribe(&audio)?;
                                emit(app, Event::Heard { text });
                            }
                            None => emit(app, Event::Idle),
                        }
                        // Whatever was said meanwhile is stale; start fresh.
                        while rx.try_recv().is_ok() {}
                        pending.clear();
                        break;
                    }
                }
            }
        }
    }
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

/// The spoken command after the wake word.
struct Command {
    audio: Vec<f32>,
    started: Instant,
    last_voice: Option<Instant>,
    /// How long to wait for the first word.
    patience: Duration,
}

impl Command {
    fn start(app: &AppHandle, follow_up: bool) -> Self {
        emit(app, Event::Listening { follow_up });
        let patience = if follow_up { NO_FOLLOW_UP } else { NO_SPEECH };
        Command { audio: Vec::new(), started: Instant::now(), last_voice: None, patience }
    }

    /// Some(Some(audio)) when finished, Some(None) when nothing was said.
    fn push(&mut self, chunk: &[f32], level: f32, floor: f32) -> Option<Option<Vec<f32>>> {
        self.audio.extend_from_slice(chunk);
        if level > (floor * 2.5).max(300.0) {
            self.last_voice = Some(Instant::now());
        }
        match self.last_voice {
            None if self.started.elapsed() > self.patience => Some(None),
            Some(t) if t.elapsed() > END_SILENCE || self.started.elapsed() > MAX_COMMAND => {
                Some(Some(std::mem::take(&mut self.audio)))
            }
            _ => None,
        }
    }
}

/// A microphone (by name, else the default) as 16kHz mono, in int16 scale (what openWakeWord expects).
fn open_mic(name: Option<&str>) -> Res<(cpal::Stream, Receiver<Vec<f32>>)> {
    let host = cpal::default_host();
    let named = name.and_then(|n| host.input_devices().ok()?.find(|d| d.name().is_ok_and(|d| d == n)));
    // A chosen mic that's unplugged falls back to the default.
    let device = named.or_else(|| host.default_input_device()).ok_or("no microphone found")?;
    let config = device.default_input_config()?;
    let channels = config.channels() as usize;
    let ratio = config.sample_rate().0 as f32 / RATE as f32;
    let (tx, rx) = sync_channel::<Vec<f32>>(64);

    // Turns mono frames from the callback into 16kHz chunks for the worker.
    let feeder = || {
        let mut down = Downsample::new(ratio);
        let tx = tx.clone();
        move |mono: &mut dyn Iterator<Item = f32>| {
            // If the worker falls behind, drop audio rather than block the audio thread.
            let _ = tx.try_send(down.run(mono));
        }
    };
    let err = |e| eprintln!("[choco] mic error: {e}");
    let cfg = config.config();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut feed = feeder();
            device.build_input_stream(
            &cfg,
            move |data: &[f32], _| {
                feed(&mut data.chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32 * 32768.0))
            },
            err,
            None,
        )?
        }
        cpal::SampleFormat::I16 => {
            let mut feed = feeder();
            device.build_input_stream(
            &cfg,
            move |data: &[i16], _| {
                feed(&mut data.chunks(channels).map(|f| f.iter().map(|&s| s as f32).sum::<f32>() / channels as f32))
            },
            err,
            None,
        )?
        }
        f => return Err(format!("unsupported microphone format {f:?}").into()),
    };
    stream.play()?;
    Ok((stream, rx))
}

/// Box-filter downsampling: averages each run of input samples into one output sample.
/// Plenty for speech, and handles uneven ratios like 44.1k → 16k.
struct Downsample {
    ratio: f32,
    phase: f32,
    sum: f32,
    n: f32,
}

impl Downsample {
    fn new(ratio: f32) -> Self {
        Downsample { ratio, phase: 0.0, sum: 0.0, n: 0.0 }
    }

    fn run(&mut self, input: &mut dyn Iterator<Item = f32>) -> Vec<f32> {
        let mut out = Vec::new();
        for x in input {
            self.sum += x;
            self.n += 1.0;
            self.phase += 1.0;
            if self.phase >= self.ratio {
                out.push(self.sum / self.n);
                self.sum = 0.0;
                self.n = 0.0;
                self.phase -= self.ratio;
            }
        }
        out
    }
}

/// openWakeWord: audio → mel frames → speech embeddings → "hey choco" score.
struct Wake {
    mel: Session,
    embed: Session,
    word: Session,
    word_in: String,
    word_out: String,
    /// Last CHUNK samples plus 3 hops of context, so mel frames line up between chunks.
    raw: VecDeque<f32>,
    frames: VecDeque<[f32; 32]>,
    embeddings: VecDeque<Vec<f32>>,
}

impl Wake {
    fn new(env: &Environment, dir: &Path, model: &[u8]) -> Res<Self> {
        let word = builder(env)?.commit_from_memory(model)?;
        Ok(Wake {
            mel: session(env, dir.join("melspectrogram.onnx"))?,
            embed: session(env, dir.join("embedding_model.onnx"))?,
            word_in: word.inputs()[0].name().to_string(),
            word_out: word.outputs()[0].name().to_string(),
            word,
            raw: VecDeque::new(),
            frames: VecDeque::new(),
            embeddings: VecDeque::new(),
        })
    }

    fn reset(&mut self) {
        self.raw.clear();
        self.frames = std::iter::repeat([1.0; 32]).take(76).collect();
        self.embeddings.clear();
    }

    fn push(&mut self, chunk: &[f32]) -> Res<f32> {
        self.raw.extend(chunk);
        while self.raw.len() > CHUNK + 480 {
            self.raw.pop_front();
        }
        let audio: Vec<f32> = self.raw.iter().copied().collect();
        let out = self.mel.run(ort::inputs!["input" => Tensor::from_array(([1, audio.len()], audio))?])?;
        let (_, mel) = out["output"].try_extract_tensor::<f32>()?;
        for f in mel.chunks_exact(32) {
            let mut frame = [0.0; 32];
            for (d, s) in frame.iter_mut().zip(f) {
                *d = s / 10.0 + 2.0;
            }
            self.frames.push_back(frame);
        }
        while self.frames.len() > 76 {
            self.frames.pop_front();
        }

        let window: Vec<f32> = self.frames.iter().flatten().copied().collect();
        let out = self.embed.run(ort::inputs!["input_1" => Tensor::from_array(([1, 76, 32, 1], window))?])?;
        let (_, emb) = out["conv2d_19"].try_extract_tensor::<f32>()?;
        self.embeddings.push_back(emb.to_vec());
        if self.embeddings.len() > 16 {
            self.embeddings.pop_front();
        }
        if self.embeddings.len() < 16 {
            return Ok(0.0);
        }

        let feats: Vec<f32> = self.embeddings.iter().flatten().copied().collect();
        let out = self.word.run(ort::inputs![self.word_in.as_str() => Tensor::from_array(([1, 16, 96], feats))?])?;
        let (_, score) = out[self.word_out.as_str()].try_extract_tensor::<f32>()?;
        Ok(score[0])
    }
}

/// Moonshine speech-to-text with greedy decoding.
struct Stt {
    encoder: Session,
    decoder: Session,
    vocab: Vec<String>,
}

const BOS: i64 = 1;
const EOS: i64 = 2;

impl Stt {
    fn new(env: &Environment, dir: &Path) -> Res<Self> {
        let tok: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("moonshine_tokenizer.json"))?)?;
        let map = tok["model"]["vocab"].as_object().ok_or("tokenizer has no vocab")?;
        let mut vocab = vec![String::new(); map.len()];
        for (piece, id) in map {
            if let Some(slot) = id.as_u64().and_then(|i| vocab.get_mut(i as usize)) {
                *slot = piece.clone();
            }
        }
        Ok(Stt {
            encoder: session(env, dir.join("moonshine_encoder.onnx"))?,
            decoder: session(env, dir.join("moonshine_decoder.onnx"))?,
            vocab,
        })
    }

    fn transcribe(&mut self, audio: &[f32]) -> Res<String> {
        let input: Vec<f32> = audio.iter().map(|s| s / 32768.0).collect();
        let len = input.len();
        let out = self.encoder.run(ort::inputs!["input_values" => Tensor::from_array(([1, len], input))?])?;
        let (shape, hidden) = out["last_hidden_state"].try_extract_tensor::<f32>()?;
        let hidden = Tensor::from_array(([1, shape[1] as usize, shape[2] as usize], hidden.to_vec()))?;
        drop(out);

        // About 6 tokens per second of speech is generous; stops runaway loops.
        let max = len / RATE * 6 + 10;
        let mut ids = vec![BOS];
        while ids.len() < max {
            let n = ids.len();
            let out = self.decoder.run(ort::inputs![
                "input_ids" => Tensor::from_array(([1, n], ids.clone()))?,
                "encoder_hidden_states" => hidden.view(),
            ])?;
            let (shape, logits) = out["logits"].try_extract_tensor::<f32>()?;
            let last = &logits[logits.len() - shape[2] as usize..];
            let next = last
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, _)| i as i64)
                .unwrap_or(EOS);
            if next == EOS {
                break;
            }
            ids.push(next);
        }
        Ok(self.decode(&ids[1..]))
    }

    /// SentencePiece-style pieces: "▁" is a space, "<0xNN>" is a raw byte.
    fn decode(&self, ids: &[i64]) -> String {
        let mut bytes = Vec::new();
        for &id in ids {
            let piece = self.vocab.get(id as usize).map(String::as_str).unwrap_or("");
            if piece.len() == 6 && piece.starts_with("<0x") && piece.ends_with('>') {
                if let Ok(b) = u8::from_str_radix(&piece[3..5], 16) {
                    bytes.push(b);
                    continue;
                }
            }
            if piece.starts_with('<') && piece.ends_with('>') {
                continue; // special tokens
            }
            bytes.extend(piece.replace('\u{2581}', " ").as_bytes());
        }
        String::from_utf8_lossy(&bytes).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs the models and a 16kHz mono test.wav saying "... Hey Choco. <command>" in
    /// CHOCO_MODELS: `CHOCO_MODELS=path cargo test choco -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn wake_then_transcribe() {
        let dir = PathBuf::from(std::env::var("CHOCO_MODELS").unwrap());
        let wav = std::fs::read(dir.join("test.wav")).unwrap();
        let audio: Vec<f32> = wav[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32).collect();
        let env = ort::init().build().unwrap();
        let mut wake = Wake::new(&env, &dir, WAKE_MODEL).unwrap();
        let mut stt = Stt::new(&env, &dir).unwrap();
        wake.reset();
        let hit = audio
            .chunks_exact(CHUNK)
            .position(|c| wake.push(c).unwrap() > WAKE_THRESHOLD)
            .expect("wake word not heard");
        let after = &audio[(hit + 1) * CHUNK..];
        let t = Instant::now();
        let text = stt.transcribe(after).unwrap();
        println!("wake at {:.2}s, heard {text:?} in {:?}", (hit * CHUNK) as f32 / RATE as f32, t.elapsed());
        assert!(text.to_lowercase().contains("focus"));
    }
}
