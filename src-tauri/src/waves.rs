// Music waves for Now Playing: what the speakers are playing, as a few loudness bands from
// bass to treble. Listens to the default output (loopback, never the microphone), and only
// between `waves_start` and `waves_stop`, which the page calls while it shows the waves.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{AppHandle, Emitter};

const BANDS: usize = 24;
/// Lowest and highest band centres, Hz; spaced evenly in pitch between.
const LOW_HZ: f32 = 50.0;
const HIGH_HZ: f32 = 10_000.0;
/// Samples looked at per update.
const WINDOW: usize = 2048;
/// Updates a second.
const RATE: usize = 30;
/// The loudest band sets the scale, and the scale eases back down this much per update, so
/// quiet songs fill the line as much as loud ones.
const PEAK_DECAY: f32 = 0.992;
/// Below this the music is silence, not something to scale up.
const MIN_PEAK: f32 = 0.002;

/// Set to false to stop the running listener.
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

#[tauri::command]
pub fn waves_start(app: AppHandle) {
    let mut running = RUNNING.lock().unwrap();
    if running.is_some() {
        return;
    }
    let alive = Arc::new(AtomicBool::new(true));
    *running = Some(alive.clone());
    std::thread::spawn(move || {
        if let Err(e) = listen(&app, &alive) {
            eprintln!("[waves] {e}");
        }
        let mut running = RUNNING.lock().unwrap();
        if running.as_ref().is_some_and(|a| Arc::ptr_eq(a, &alive)) {
            *running = None;
        }
    });
}

#[tauri::command]
pub fn waves_stop() {
    if let Some(alive) = RUNNING.lock().unwrap().take() {
        alive.store(false, Ordering::Relaxed);
    }
}

fn listen(app: &AppHandle, alive: &AtomicBool) -> Result<(), Box<dyn std::error::Error>> {
    let device = cpal::default_host().default_output_device().ok_or("no speakers found")?;
    let config = device.default_output_config()?;
    let channels = config.channels() as usize;
    let rate = config.sample_rate().0 as f32;
    let (tx, rx) = sync_channel::<Vec<f32>>(64);
    let err = |e| eprintln!("[waves] {e}");
    // An input stream on an output device records what it plays (WASAPI loopback).
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.config(),
            move |data: &[f32], _| {
                let _ = tx.try_send(data.chunks(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect());
            },
            err,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.config(),
            move |data: &[i16], _| {
                let mono = data.chunks(channels).map(|f| f.iter().map(|&s| s as f32 / 32768.0).sum::<f32>() / channels as f32);
                let _ = tx.try_send(mono.collect());
            },
            err,
            None,
        )?,
        f => return Err(format!("unsupported speaker format {f:?}").into()),
    };
    stream.play()?;

    let hop = (rate as usize / RATE).max(1);
    let window: Vec<f32> = (0..WINDOW)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WINDOW as f32).cos())
        .collect();
    let centres: Vec<f32> = (0..BANDS)
        .map(|b| LOW_HZ * (HIGH_HZ / LOW_HZ).powf(b as f32 / (BANDS - 1) as f32))
        .collect();
    let mut samples: Vec<f32> = Vec::with_capacity(WINDOW * 2);
    let mut fresh = 0;
    let mut peak = MIN_PEAK;
    while alive.load(Ordering::Relaxed) {
        // Nothing playing sends nothing, so wake now and then to notice being stopped.
        let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
        fresh += chunk.len();
        samples.extend(chunk);
        if samples.len() > WINDOW {
            samples.drain(..samples.len() - WINDOW);
        }
        if fresh < hop || samples.len() < WINDOW {
            continue;
        }
        fresh = 0;
        let mut bands: Vec<f32> = centres
            .iter()
            .map(|&f| {
                // Treble carries less energy than bass; tilt it up so the line moves across.
                goertzel(&samples, &window, f / rate) * (f / LOW_HZ).sqrt()
            })
            .collect();
        let loudest = bands.iter().cloned().fold(0.0, f32::max);
        peak = (peak * PEAK_DECAY).max(loudest).max(MIN_PEAK);
        for b in &mut bands {
            *b = (*b / peak).min(1.0);
        }
        let _ = app.emit("music-waves", bands);
    }
    Ok(())
}

/// Strength of one frequency (cycles per sample) in the windowed samples.
fn goertzel(samples: &[f32], window: &[f32], freq: f32) -> f32 {
    let coeff = 2.0 * (std::f32::consts::TAU * freq).cos();
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    for (x, w) in samples.iter().zip(window) {
        let s = x * w + coeff * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt() / samples.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goertzel_finds_its_tone() {
        let rate = 48_000.0;
        let tone: Vec<f32> = (0..WINDOW).map(|i| (std::f32::consts::TAU * 1000.0 * i as f32 / rate).sin()).collect();
        let window = vec![1.0; WINDOW];
        let on = goertzel(&tone, &window, 1000.0 / rate);
        let off = goertzel(&tone, &window, 4000.0 / rate);
        assert!(on > off * 20.0, "on {on}, off {off}");
    }
}
