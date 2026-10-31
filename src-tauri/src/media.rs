// Now playing and playback controls from Windows' media sessions: Spotify,
// browsers, and any player that shows up in the Windows media flyout.
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_millis(500);

#[derive(Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app: String,
    pub playing: bool,
    pub position_ms: f64,
    pub duration_ms: f64,
    /// Unix ms when `position_ms` was measured; extrapolate from here while playing.
    pub updated_at_ms: f64,
}

/// Emits `media` (a `Media` or null) whenever what's playing changes.
pub fn spawn_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            let now = imp::current();
            if now != last {
                let _ = app.emit("media", &now);
                last = now;
            }
            std::thread::sleep(if crate::game::on() { crate::game::IDLE_POLL } else { POLL });
        }
    });
}

// Commands are async so the blocking WinRT calls stay off the UI thread.

#[tauri::command]
pub async fn media_current() -> Option<Media> {
    imp::current()
}

#[tauri::command]
pub async fn media_play_pause() -> bool {
    imp::control(Control::PlayPause)
}

#[tauri::command]
pub async fn media_next() -> bool {
    imp::control(Control::Next)
}

#[tauri::command]
pub async fn media_previous() -> bool {
    imp::control(Control::Previous)
}

#[tauri::command]
pub async fn media_seek(position_ms: f64) -> bool {
    imp::control(Control::Seek(position_ms))
}

/// Album art bytes (usually JPEG or PNG) for the current track; empty if none.
#[tauri::command]
pub async fn media_artwork() -> Response {
    Response::new(imp::artwork().unwrap_or_default())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    synced_lyrics: Option<String>,
}

/// Time-synced lyrics (LRC text) from lrclib.net, or None if it has none.
#[tauri::command]
pub async fn fetch_lyrics(
    title: String,
    artist: String,
    album: String,
    duration_ms: f64,
) -> Result<Option<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("Island/1.0 (personal desktop notch app)")
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let duration = format!("{}", (duration_ms / 1000.0).round());

    // Exact match first, then a looser search (browser players often put the channel name in `artist`).
    let exact = client
        .get("https://lrclib.net/api/get")
        .query(&[
            ("track_name", title.as_str()),
            ("artist_name", artist.as_str()),
            ("album_name", album.as_str()),
            ("duration", duration.as_str()),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if exact.status().is_success() {
        if let Ok(track) = exact.json::<LrclibTrack>().await {
            if track.synced_lyrics.is_some() {
                return Ok(track.synced_lyrics);
            }
        }
    }

    let found: Vec<LrclibTrack> = client
        .get("https://lrclib.net/api/search")
        .query(&[("q", format!("{artist} {title}"))])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(found.into_iter().find_map(|t| t.synced_lyrics))
}

enum Control {
    PlayPause,
    Next,
    Previous,
    Seek(f64),
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{Control, Media};
    use windows::core::Interface;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::{DataReader, IInputStream};

    const TICKS_PER_MS: f64 = 10_000.0;
    const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000; // 1601 -> 1970 in 100ns ticks

    /// Made once: asking Windows for a new one each poll is a round trip to another process.
    fn session() -> Option<Session> {
        static MANAGER: std::sync::OnceLock<Option<Manager>> = std::sync::OnceLock::new();
        MANAGER
            .get_or_init(|| Manager::RequestAsync().ok()?.get().ok())
            .as_ref()?
            .GetCurrentSession()
            .ok()
    }

    pub fn current() -> Option<Media> {
        let s = session()?;
        let props = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let title = props.Title().ok()?.to_string();
        if title.is_empty() {
            return None;
        }
        let timeline = s.GetTimelineProperties().ok()?;
        let start = timeline.StartTime().ok()?.Duration;
        Some(Media {
            title,
            artist: props.Artist().map(|a| a.to_string()).unwrap_or_default(),
            album: props.AlbumTitle().map(|a| a.to_string()).unwrap_or_default(),
            app: s.SourceAppUserModelId().map(|a| a.to_string()).unwrap_or_default(),
            playing: s.GetPlaybackInfo().ok()?.PlaybackStatus().ok()? == Status::Playing,
            position_ms: (timeline.Position().ok()?.Duration - start) as f64 / TICKS_PER_MS,
            duration_ms: (timeline.EndTime().ok()?.Duration - start) as f64 / TICKS_PER_MS,
            updated_at_ms: (timeline.LastUpdatedTime().ok()?.UniversalTime - UNIX_EPOCH_TICKS) as f64
                / TICKS_PER_MS,
        })
    }

    pub fn control(c: Control) -> bool {
        let Some(s) = session() else { return false };
        let op = match c {
            Control::PlayPause => s.TryTogglePlayPauseAsync(),
            Control::Next => s.TrySkipNextAsync(),
            Control::Previous => s.TrySkipPreviousAsync(),
            Control::Seek(ms) => {
                let start = s
                    .GetTimelineProperties()
                    .and_then(|t| t.StartTime())
                    .map(|t| t.Duration)
                    .unwrap_or(0);
                s.TryChangePlaybackPositionAsync(start + (ms * TICKS_PER_MS) as i64)
            }
        };
        op.and_then(|op| op.get()).unwrap_or(false)
    }

    pub fn artwork() -> Option<Vec<u8>> {
        let props = session()?.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.get().ok()?;
        let size = stream.Size().ok()? as u32;
        let reader = DataReader::CreateDataReader(&stream.cast::<IInputStream>().ok()?).ok()?;
        reader.LoadAsync(size).ok()?.get().ok()?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        Some(bytes)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{Control, Media};

    pub fn current() -> Option<Media> {
        None
    }

    pub fn control(_c: Control) -> bool {
        false
    }

    pub fn artwork() -> Option<Vec<u8>> {
        None
    }
}
