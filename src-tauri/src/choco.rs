//! "Hey Choco": an always-on wake word (openWakeWord front-end with our own "hey choco"
//! model, see tools/wakeword), then the spoken command is recorded until you stop talking
//! and turned into text (Moonshine). Both run offline through ONNX Runtime. The frontend
//! decides what the text means.
//!
//! Events:
//! - `choco` {state: "loading" | "ready" | "off" | "thinking" | "idle"},
//!   {state: "listening", followUp}, {state: "heard", text} or {state: "error", message}
//! - `choco-level` 0–1 microphone level while listening.
//! - `choco-partial` what's been said so far, while you're still talking.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ort::environment::Environment;
use ort::session::Session;
use ort::value::{DynValue, Tensor};
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
    // The merged decoder remembers its work between words instead of starting over each time.
    ("moonshine_decoder_merged.onnx", "{MOON}/onnx/decoder_model_merged_quantized.onnx"),
    ("moonshine_tokenizer.json", "{MOON}/tokenizer.json"),
];

/// Recording ends after this much quiet once you've started talking. Long enough for the
/// pause while you think of the next word.
const END_SILENCE: Duration = Duration::from_millis(1200);
/// Give up if nothing is said this long after the wake word.
const NO_SPEECH: Duration = Duration::from_secs(5);
/// After an answer Choco listens a little for a follow-up, without the wake word.
const NO_FOLLOW_UP: Duration = Duration::from_secs(4);
const MAX_COMMAND: Duration = Duration::from_secs(12);
/// How often what's been said so far is turned into text for the preview.
const PREVIEW_EVERY: Duration = Duration::from_millis(600);
/// Speech is louder than this share of the loudest moment of the command; anything quieter is
/// the room, so steady noise can't keep a command open.
const VOICE_OF_PEAK: f32 = 0.2;
/// Speech is at least this loud (int16 scale). Laptop mic arrays are quiet: a room sits near
/// 30 and soft speech near 150, so a higher bar drops words.
const MIN_VOICE: f32 = 120.0;
/// After this much quiet the command is probably over, so it's turned into text while the
/// rest of END_SILENCE runs out; the answer is ready the moment listening stops.
const SETTLED: Duration = Duration::from_millis(400);

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
    if let Event::Error { message } = &e {
        log_error(app, message);
    }
    let _ = app.emit("choco", e);
}

/// Errors only flash by on the island; choco-errors.log keeps them to look at later.
fn log_error(app: &AppHandle, message: &str) {
    use std::io::Write;
    let Ok(dir) = app.path().app_local_data_dir() else { return };
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("choco-errors.log")) {
        let _ = writeln!(f, "{when} {message}");
    }
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
    let mut wake: Option<Wake> = None;
    loop {
        if !choco.enabled.load(Ordering::Relaxed) || crate::game::on() {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        if wake.is_none() {
            emit(&app, Event::Loading);
            match load(&app) {
                Ok(w) => wake = Some(w),
                Err(e) => {
                    emit(&app, Event::Error { message: format!("Couldn't load voice models: {e}") });
                    choco.enabled.store(false, Ordering::Relaxed);
                    continue;
                }
            }
        }
        let wake = wake.as_mut().unwrap();
        if let Err(e) = listen(&app, &choco, wake) {
            emit(&app, Event::Error { message: format!("Microphone: {e}") });
            // Try again shortly (e.g. a headset was unplugged).
            std::thread::sleep(Duration::from_secs(3));
        }
    }
}

/// Speech-to-text, shared by "Hey Choco" and dictation so Moonshine is only in memory once.
static STT: Mutex<Option<Stt>> = Mutex::new(None);

/// ONNX Runtime allows one environment per process, ever; the wake word and speech-to-text share it.
fn env() -> Res<&'static Environment> {
    static ENV: std::sync::OnceLock<Environment> = std::sync::OnceLock::new();
    static INIT: Mutex<()> = Mutex::new(());
    let _one = INIT.lock().unwrap();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let env = ort::init().with_name("choco").build()?;
    Ok(ENV.get_or_init(|| env))
}

fn load(app: &AppHandle) -> Res<Wake> {
    let dir = fetch(app)?;
    let wake = Wake::new(env()?, &dir, WAKE_MODEL)?;
    load_stt(app)?;
    Ok(wake)
}

/// Downloads any missing models; returns their folder.
fn fetch(app: &AppHandle) -> Res<PathBuf> {
    // Two callers at once would download the same files twice.
    static FETCHING: Mutex<()> = Mutex::new(());
    let _one = FETCHING.lock().unwrap();
    let dir = app.path().app_local_data_dir()?.join("models");
    std::fs::create_dir_all(&dir)?;
    // Replaced by moonshine_decoder_merged.onnx.
    let _ = std::fs::remove_file(dir.join("moonshine_decoder.onnx"));
    for (name, url) in MODELS {
        let path = dir.join(name);
        if !path.exists() {
            let url = url.replace("{OWW}", OWW).replace("{MOON}", MOONSHINE);
            tauri::async_runtime::block_on(download(&url, &path))?;
        }
    }
    Ok(dir)
}

/// Loads Moonshine if it isn't already.
pub(crate) fn load_stt(app: &AppHandle) -> Res<()> {
    if STT.lock().unwrap().is_some() {
        return Ok(());
    }
    let dir = fetch(app)?;
    let stt = Stt::new(env()?, &dir)?;
    STT.lock().unwrap().get_or_insert(stt);
    Ok(())
}

/// 16kHz mono audio (int16 scale) to text. Call `load_stt` first.
pub(crate) fn transcribe(audio: &[f32]) -> Res<String> {
    STT.lock().unwrap().as_mut().ok_or("speech model isn't loaded")?.transcribe(audio)
}

/// The microphone picked in Choco's settings, or None for the Windows default.
pub(crate) fn mic(app: &AppHandle) -> Option<String> {
    app.state::<Arc<Choco>>().mic.lock().unwrap().clone()
}

async fn download(url: &str, to: &Path) -> Res<()> {
    let bytes = reqwest::get(url).await?.error_for_status()?.bytes().await?;
    // Written under a temporary name so a half-finished download is never loaded.
    let part = to.with_extension("part");
    std::fs::write(&part, &bytes)?;
    std::fs::rename(part, to)?;
    Ok(())
}

/// The wake word gets one thread, so always listening never competes with what you're doing.
/// Speech-to-text runs only while you wait for it, so it gets a few.
fn builder(env: &Environment, threads: usize) -> Res<ort::session::builder::SessionBuilder> {
    Ok(Session::builder(env)?.with_intra_threads(threads).map_err(ort::Error::<()>::from)?)
}

fn session(env: &Environment, path: PathBuf, threads: usize) -> Res<Session> {
    Ok(builder(env, threads)?.commit_from_file(path)?)
}

/// Runs until Choco is turned off or its settings change (Ok), or the microphone fails (Err).
fn listen(app: &AppHandle, choco: &Choco, wake: &mut Wake) -> Res<()> {
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
                    // Track the room's background noise so "talking" is relative to it: down
                    // quickly, up slowly, so a fan or music becomes the new quiet but a word doesn't.
                    let rate = if level < floor { 0.05 } else { 0.01 };
                    floor = (floor * (1.0 - rate) + level * rate).max(50.0);
                    if wake.push(&chunk, level > floor * 1.5)? >f32::from_bits(choco.threshold.load(Ordering::Relaxed)) {
                        command = Some(Command::start(app, false));
                    }
                }
                Some(c) => {
                    let _ = app.emit("choco-level", (level / 3000.0).min(1.0));
                    if let Some(done) = c.push(&chunk, level, floor) {
                        command = None;
                        PREVIEW_FOR.fetch_add(1, Ordering::Relaxed);
                        wake.reset();
                        match done {
                            Some(audio) => {
                                emit(app, Event::Thinking);
                                let text = transcribe(&audio)?;
                                emit(app, Event::Heard { text });
                            }
                            None => emit(app, Event::Idle),
                        }
                        // Whatever was said meanwhile is stale; start fresh.
                        while rx.try_recv().is_ok() {}
                        pending.clear();
                        break;
                    }
                    if c.preview_due() && preview(app, c.audio.clone()) {
                        c.last_preview = Instant::now();
                    }
                }
            }
        }
    }
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

/// Bumped when a command ends, so a preview still being worked out for it is dropped.
static PREVIEW_FOR: AtomicU32 = AtomicU32::new(0);
/// A preview is being worked out; one at a time.
static PREVIEWING: AtomicBool = AtomicBool::new(false);

/// Turns what's been said so far into text on another thread, so listening never stalls,
/// and sends it as `choco-partial`. False if one is already being worked out.
fn preview(app: &AppHandle, audio: Vec<f32>) -> bool {
    if PREVIEWING.swap(true, Ordering::AcqRel) {
        return false;
    }
    let command = PREVIEW_FOR.load(Ordering::Relaxed);
    let app = app.clone();
    std::thread::spawn(move || {
        if let Ok(text) = transcribe(&audio) {
            if !text.is_empty() && PREVIEW_FOR.load(Ordering::Relaxed) == command {
                let _ = app.emit("choco-partial", text);
            }
        }
        PREVIEWING.store(false, Ordering::Release);
    });
    true
}

/// The spoken command after the wake word.
struct Command {
    audio: Vec<f32>,
    started: Instant,
    last_voice: Option<Instant>,
    /// The loudest it's been, so quiet after talking counts as quiet even in a noisy room.
    peak: f32,
    last_preview: Instant,
    /// How long to wait for the first word.
    patience: Duration,
}

impl Command {
    fn start(app: &AppHandle, follow_up: bool) -> Self {
        emit(app, Event::Listening { follow_up });
        let patience = if follow_up { NO_FOLLOW_UP } else { NO_SPEECH };
        let now = Instant::now();
        Command { audio: Vec::new(), started: now, last_voice: None, peak: 0.0, last_preview: now, patience }
    }

    /// Some(Some(audio)) when finished, Some(None) when nothing was said.
    fn push(&mut self, chunk: &[f32], level: f32, floor: f32) -> Option<Option<Vec<f32>>> {
        self.audio.extend_from_slice(chunk);
        self.peak = self.peak.max(level);
        if level > (floor * 3.0).max(MIN_VOICE).max(self.peak * VOICE_OF_PEAK) {
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

    /// Time to update the preview: now and then while you're talking, and once when you've
    /// gone quiet, so the text is worked out (and remembered) before listening stops.
    fn preview_due(&self) -> bool {
        let Some(voice) = self.last_voice else { return false };
        if voice.elapsed() < SETTLED {
            self.last_preview.elapsed() >= PREVIEW_EVERY
        } else {
            self.last_preview < voice + SETTLED
        }
    }
}

/// A microphone (by name, else the default) as 16kHz mono, in int16 scale (what openWakeWord expects).
pub(crate) fn open_mic(name: Option<&str>) -> Res<(cpal::Stream, Receiver<Vec<f32>>)> {
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

/// Quiet chunks are held rather than worked through, unless there was sound this recently:
/// the wake word's score peaks a little after it's said.
const WAKE_HANGOVER: usize = 16;
/// Enough held audio to rebuild everything the detector looks at once sound comes back:
/// 16 embeddings, the first of which looks back 76 mel frames (8 per chunk).
const WAKE_BACKLOG: usize = 16 + 76 / 8 + 1;

/// openWakeWord: audio → mel frames → speech embeddings → "hey choco" score.
/// In a quiet room it does nothing until there's sound, then catches up on what it held.
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
    held: VecDeque<Vec<f32>>,
    /// Chunks since the last one with sound in it.
    quiet: usize,
}

impl Wake {
    fn new(env: &Environment, dir: &Path, model: &[u8]) -> Res<Self> {
        let word = builder(env, 1)?.commit_from_memory(model)?;
        Ok(Wake {
            mel: session(env, dir.join("melspectrogram.onnx"), 1)?,
            embed: session(env, dir.join("embedding_model.onnx"), 1)?,
            word_in: word.inputs()[0].name().to_string(),
            word_out: word.outputs()[0].name().to_string(),
            word,
            raw: VecDeque::new(),
            frames: VecDeque::new(),
            embeddings: VecDeque::new(),
            held: VecDeque::new(),
            quiet: 0,
        })
    }

    fn reset(&mut self) {
        self.raw.clear();
        self.frames = std::iter::repeat([1.0; 32]).take(76).collect();
        self.embeddings.clear();
        self.held.clear();
        self.quiet = 0;
    }

    /// The wake score after this chunk; `sound` says whether it's louder than the room.
    fn push(&mut self, chunk: &[f32], sound: bool) -> Res<f32> {
        self.quiet = if sound { 0 } else { self.quiet + 1 };
        if self.quiet > WAKE_HANGOVER {
            self.held.push_back(chunk.to_vec());
            if self.held.len() > WAKE_BACKLOG {
                self.held.pop_front();
            }
            return Ok(0.0);
        }
        while let Some(c) = self.held.pop_front() {
            self.score(&c)?;
        }
        self.score(chunk)
    }

    fn score(&mut self, chunk: &[f32]) -> Res<f32> {
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
    /// The decoder's remembered keys and values, one input per layer and kind.
    past: Vec<String>,
    vocab: Vec<String>,
    /// The last audio it worked out and what it said, so the same speech isn't done twice
    /// (a command's final text is usually ready from the preview taken as you went quiet).
    last: Option<(Vec<f32>, String)>,
}

/// Silence kept either side of the speech, so soft first and last sounds aren't cut.
const PAD: usize = RATE / 4;
/// Quiet speech is turned up to peak here (of full scale), by at most MAX_GAIN.
const LOUDNESS: f32 = 0.7;
const MAX_GAIN: f32 = 8.0;

/// Audio (int16 scale) as Moonshine wants it: only the speech, without the quiet around it
/// (less to work through, and no silence to imagine words in), centred and turned up if it's quiet.
/// Empty when it's all silence.
fn prepare(audio: &[f32]) -> Vec<f32> {
    const FRAME: usize = RATE / 50;
    let levels: Vec<f32> = audio
        .chunks(FRAME)
        .map(|f| {
            let mean = f.iter().sum::<f32>() / f.len() as f32;
            (f.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / f.len() as f32).sqrt()
        })
        .collect();
    let loudest = levels.iter().copied().fold(0.0, f32::max);
    let speech = |l: &f32| *l > loudest * 0.1;
    let (Some(first), Some(last)) = (levels.iter().position(speech), levels.iter().rposition(speech)) else {
        return Vec::new();
    };
    let clip = &audio[(first * FRAME).saturating_sub(PAD)..((last + 1) * FRAME + PAD).min(audio.len())];
    let mean = clip.iter().sum::<f32>() / clip.len() as f32;
    let peak = clip.iter().map(|v| (v - mean).abs()).fold(1.0, f32::max) / 32768.0;
    let gain = (LOUDNESS / peak).min(MAX_GAIN) / 32768.0;
    clip.iter().map(|v| (v - mean) * gain).collect()
}

/// Half the cores, up to 4.
fn stt_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).clamp(1, 4))
}

const BOS: i64 = 1;
/// moonshine-base's attention heads and their size, for the decoder's empty memory.
const HEADS: usize = 8;
const HEAD_DIM: usize = 52;
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
        let decoder = session(env, dir.join("moonshine_decoder_merged.onnx"), stt_threads())?;
        let past = decoder
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .filter(|n| n.starts_with("past_key_values."))
            .collect();
        Ok(Stt {
            encoder: session(env, dir.join("moonshine_encoder.onnx"), stt_threads())?,
            decoder,
            past,
            vocab,
            last: None,
        })
    }

    fn transcribe(&mut self, audio: &[f32]) -> Res<String> {
        let input = prepare(audio);
        if input.is_empty() {
            return Ok(String::new());
        }
        if let Some((heard, text)) = &self.last {
            if *heard == input {
                return Ok(text.clone());
            }
        }
        let text = self.run(&input)?;
        self.last = Some((input, text.clone()));
        Ok(text)
    }

    fn run(&mut self, input: &[f32]) -> Res<String> {
        let len = input.len();
        let input = input.to_vec();
        let out = self.encoder.run(ort::inputs!["input_values" => Tensor::from_array(([1, len], input))?])?;
        let (shape, hidden) = out["last_hidden_state"].try_extract_tensor::<f32>()?;
        let hidden = Tensor::from_array(([1, shape[1] as usize, shape[2] as usize], hidden.to_vec()))?;
        drop(out);

        // About 6 tokens per second of speech is generous; stops runaway loops.
        let max = len / RATE * 6 + 10;
        let mut ids = vec![BOS];
        // Nothing remembered before the first word.
        let mut past: Vec<(String, DynValue)> = Vec::with_capacity(self.past.len());
        for name in &self.past {
            past.push((name.clone(), Tensor::<f32>::from_array(([1, HEADS, 0, HEAD_DIM], Vec::new()))?.into_dyn()));
        }
        while ids.len() < max {
            let cached = ids.len() > 1;
            let mut inputs: Vec<(std::borrow::Cow<str>, ort::session::SessionInputValue)> = vec![
                ("input_ids".into(), Tensor::from_array(([1, 1], vec![ids[ids.len() - 1]]))?.into()),
                ("encoder_hidden_states".into(), hidden.view().into()),
                ("use_cache_branch".into(), Tensor::from_array(([1], vec![cached]))?.into()),
            ];
            inputs.extend(past.iter().map(|(n, v)| (n.as_str().into(), v.view().into())));
            let mut out = self.decoder.run(inputs)?;
            for (name, value) in past.iter_mut() {
                // What it makes of the speech is worked out on the first word and kept.
                if cached && name.contains(".encoder.") {
                    continue;
                }
                let present = name.replacen("past_key_values", "present", 1);
                *value = out.remove(&present).ok_or_else(|| format!("decoder gave no {present}"))?;
            }
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
            .position(|c| wake.push(c, true).unwrap() > WAKE_THRESHOLD)
            .expect("wake word not heard");
        let after = &audio[(hit + 1) * CHUNK..];
        let t = Instant::now();
        let text = stt.transcribe(after).unwrap();
        println!("wake at {:.2}s, heard {text:?} in {:?}", (hit * CHUNK) as f32 / RATE as f32, t.elapsed());
        assert!(text.to_lowercase().contains("focus"));
    }

    /// Holding quiet audio costs next to nothing and still hears the wake word as it comes.
    /// Needs wake.wav ("Hey Choco. ...") in CHOCO_MODELS.
    /// `CHOCO_MODELS=path cargo test --release choco::tests::quiet_room -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn quiet_room() {
        let dir = PathBuf::from(std::env::var("CHOCO_MODELS").unwrap());
        let wav = std::fs::read(dir.join("wake.wav")).unwrap();
        // Ten seconds of a quiet room, then the wake word.
        let mut audio: Vec<f32> = (0..10 * RATE).map(|i| ((i * 7919 % 61) as f32 - 30.0) * 2.0).collect();
        audio.extend(wav[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32));
        audio.extend(std::iter::repeat(0.0).take(2 * RATE));
        let env = ort::init().build().unwrap();
        let mut wake = Wake::new(&env, &dir, WAKE_MODEL).unwrap();
        let mut scores = |gate: bool| {
            wake.reset();
            let t = Instant::now();
            let s: Vec<f32> = audio.chunks_exact(CHUNK).map(|c| wake.push(c, !gate || rms(c) > 150.0).unwrap()).collect();
            (s, t.elapsed())
        };
        let (always, always_t) = scores(false);
        let (gated, gated_t) = scores(true);
        let hit = |s: &[f32]| s.iter().position(|&v| v > WAKE_THRESHOLD);
        println!("always on: hit at chunk {:?} in {always_t:?}; gated: hit at chunk {:?} in {gated_t:?}", hit(&always), hit(&gated));
        assert!(hit(&always).is_some());
        assert_eq!(hit(&always), hit(&gated));
        // From the first sound on, the gate sees exactly what always-on does.
        let start = 10 * RATE / CHUNK;
        for (a, g) in always[start..].iter().zip(&gated[start..]) {
            assert!((a - g).abs() < 0.02, "{a} vs {g}");
        }
    }

    /// Soft speech on a quiet laptop mic, with a thinking pause mid-sentence, is kept whole.
    #[test]
    fn quiet_speech_with_a_pause() {
        let room = 30.0;
        let mut c = Command { audio: Vec::new(), started: Instant::now(), last_voice: None, peak: 0.0, last_preview: Instant::now(), patience: NO_SPEECH };
        let chunk = vec![0.0; CHUNK];
        let say = |c: &mut Command, level: f32, ms: u64| {
            let end = Instant::now() + Duration::from_millis(ms);
            while Instant::now() < end {
                if let Some(done) = c.push(&chunk, level, room) {
                    return Some(done);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            None
        };
        assert!(say(&mut c, 180.0, 600).is_none(), "ended while talking");
        assert!(say(&mut c, room, 900).is_none(), "ended in a short pause");
        assert!(say(&mut c, 160.0, 400).is_none(), "ended while talking again");
        assert!(matches!(say(&mut c, room, 2000), Some(Some(_))), "didn't end after going quiet");
    }

    /// How long a command takes to become text, with the quiet a real one ends with.
    /// `CHOCO_MODELS=path cargo test --release choco::tests::speed -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn speed() {
        let dir = PathBuf::from(std::env::var("CHOCO_MODELS").unwrap());
        let wav = std::fs::read(dir.join("test.wav")).unwrap();
        let mut audio: Vec<f32> = wav[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32).collect();
        audio.extend(std::iter::repeat(0.0).take(RATE));
        let env = ort::init().build().unwrap();
        let mut stt = Stt::new(&env, &dir).unwrap();
        for _ in 0..3 {
            stt.last = None;
            let t = Instant::now();
            let text = stt.transcribe(&audio).unwrap();
            println!("{:?} in {:?}", text, t.elapsed());
        }
        // Going on being quiet doesn't change the speech, so the text is already known.
        let first = stt.transcribe(&audio).unwrap();
        audio.extend(std::iter::repeat(0.0).take(RATE / 2));
        let t = Instant::now();
        assert_eq!(stt.transcribe(&audio).unwrap(), first);
        println!("again after more quiet in {:?}", t.elapsed());
        // A quiet microphone is turned up.
        let soft: Vec<f32> = audio.iter().map(|v| v / 20.0).collect();
        println!("at 1/20 volume: {:?}", stt.transcribe(&soft).unwrap());
    }
}
