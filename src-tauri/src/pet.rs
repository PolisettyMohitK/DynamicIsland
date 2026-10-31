//! Choco as a pet: a small dog that sleeps in the island and, once woken, hops out and
//! follows the cursor around the desktop. It lives in its own small, transparent,
//! click-through window that moves with it; this thread is its legs (where it is and how
//! fast it goes). The pet page draws it and picks what it does while standing still.
//! The window only exists while it's out (and a couple of minutes after, so a quick
//! re-wake is instant): asleep in the island it costs nothing.
//!
//! Events: `pet-motion` {motion, facing, slope, gaze, excited} where motion is walk | run | rest |
//! alert | wake | climb | nudge; `pet-hover` (cursor over the dog); `pet-home` (true while asleep in
//! the island); `pet-config` (the settings, for the pet page).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::island;

/// Logical size of the pet window; the dog stands at its bottom centre.
const WIN_W: f64 = 120.0;
const WIN_H: f64 = 96.0;
/// The part of the window that is the dog, for hovering and clicking.
const DOG_W: f64 = 70.0;
const DOG_H: f64 = 54.0;

/// Ticks: smooth while moving, relaxed while it rests, slow while it sleeps.
const MOVING: Duration = Duration::from_millis(16);
const RESTING: Duration = Duration::from_millis(50);
const ASLEEP: Duration = Duration::from_millis(250);

/// While moving, wait for whole screen refreshes rather than a fixed 16ms, so every move lands
/// on a refresh: a fixed tick drifts against a 144/160Hz screen and the dog judders. Waits at
/// least FRAME_MIN (so 80 moves a second on a 160Hz screen): each move costs the webview work.
#[cfg(windows)]
fn next_frame() {
    const FRAME_MIN: Duration = Duration::from_millis(9);
    let start = Instant::now();
    while start.elapsed() < FRAME_MIN {
        let before = Instant::now();
        unsafe { windows_sys::Win32::Graphics::Dwm::DwmFlush() };
        // Returned at once (nothing composing): fall back to the plain tick rather than spin.
        if before.elapsed() < Duration::from_millis(1) {
            std::thread::sleep(MOVING);
            return;
        }
    }
}
#[cfg(not(windows))]
fn next_frame() {
    std::thread::sleep(MOVING);
}

// Speeds in logical px per second.
const WALK: f64 = 110.0;
const RUN: f64 = 300.0;
const EXCITED: f64 = 420.0;
/// How quickly it reaches the speed it wants (per second); lower is floatier.
const GRIP: f64 = 5.0;
/// Starts moving once the cursor is this much further than its follow distance.
const SLACK: f64 = 45.0;
/// Runs instead of walking past this much further.
const RUN_AFTER: f64 = 280.0;
/// Further than this (say, another monitor) and it just hops over.
const TELEPORT: f64 = 1600.0;

/// Waking: it stretches before hopping out. Going home: the climb into the island.
const STRETCH: Duration = Duration::from_millis(900);
const CLIMB: Duration = Duration::from_millis(650);
/// At night it heads home after this long without the cursor moving.
const NIGHT_IDLE: Duration = Duration::from_secs(30);
/// After resting this long it perks up (ears up, a beat) before setting off.
const ALERT_AFTER: Duration = Duration::from_secs(2);
const ALERT: Duration = Duration::from_millis(300);
/// Home this long (or a game starts) and its window closes.
const KEEP_WARM: Duration = Duration::from_secs(120);
/// Waking opens the window; it shows once the page says it's ready (or after this).
const LOAD_TIMEOUT: Duration = Duration::from_secs(3);

/// A fast side-to-side shake of the cursor gets it excited.
const SHAKE_FLIPS: usize = 4;
const SHAKE_WINDOW: Duration = Duration::from_millis(900);
const SHAKE_SPEED: f64 = 900.0; // logical px/s
const EXCITED_FOR: Duration = Duration::from_secs(3);

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetConfig {
    pub enabled: bool,
    /// Follow distance: close | normal | far.
    pub distance: String,
    /// calm | playful.
    pub energy: String,
    pub sleep_after_min: u32,
    pub night_sleep: bool,
    /// Only the pet page uses this.
    pub sounds: bool,
}

impl Default for PetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            distance: "normal".into(),
            energy: "calm".into(),
            sleep_after_min: 2,
            night_sleep: true,
            sounds: false,
        }
    }
}

#[derive(Default)]
pub struct Pet {
    config: Mutex<PetConfig>,
    wake: Mutex<Option<String>>,
    go_home: AtomicBool,
    /// Focus timer running: it lies down nearby and doesn't chase.
    calm: AtomicBool,
    night: AtomicBool,
    /// The pet page has loaded and is listening.
    ready: AtomicBool,
    /// Clicked: it sits where it is and watches the cursor instead of following.
    stay: AtomicBool,
}

#[derive(Clone, Serialize)]
struct Motion {
    motion: &'static str,
    facing: i8,
    /// How far it leans, in degrees: negative nose-up (going up the screen), positive nose-down.
    slope: i8,
    /// While staying: how far it tips its head to look at the cursor, degrees (negative is up).
    gaze: i8,
    excited: bool,
}

#[tauri::command]
pub fn pet_configure(app: AppHandle, config: PetConfig) {
    *app.state::<Pet>().config.lock().unwrap() = config.clone();
    let _ = app.emit("pet-config", config);
}

#[tauri::command]
pub fn pet_config(app: AppHandle) -> PetConfig {
    app.state::<Pet>().config.lock().unwrap().clone()
}

/// reason: hover | voice | nudge (a reminder: it runs right up to the cursor).
#[tauri::command]
pub fn pet_wake(app: AppHandle, reason: String) {
    *app.state::<Pet>().wake.lock().unwrap() = Some(reason);
}

#[tauri::command]
pub fn pet_home(app: AppHandle) {
    app.state::<Pet>().go_home.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn pet_context(app: AppHandle, calm: bool, night: bool) {
    let pet = app.state::<Pet>();
    pet.calm.store(calm, Ordering::Relaxed);
    pet.night.store(night, Ordering::Relaxed);
}

#[tauri::command]
pub fn pet_stay(app: AppHandle, stay: bool) {
    app.state::<Pet>().stay.store(stay, Ordering::Relaxed);
}

#[tauri::command]
pub fn pet_ready(app: AppHandle) {
    app.state::<Pet>().ready.store(true, Ordering::Relaxed);
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || Legs::new(app).run());
}

fn open_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, "pet", WebviewUrl::App("pet".into()))
        .title("Choco")
        .inner_size(WIN_W, WIN_H)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focusable(false)
        .visible(false)
        // Must match the main window's, or WebView2 refuses the second window.
        .additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required",
        )
        .build()?;
    window.set_ignore_cursor_events(true)?;
    // Choco's computer use screenshots the desktop; the dog shouldn't be in them.
    crate::capture::exclude_from_capture(&window);
    Ok(window)
}

#[derive(PartialEq)]
enum Where {
    Home,
    /// The window is opening.
    Loading(Instant),
    Waking(Instant),
    Out,
    /// Heading home: when it set off, and whether it was sent (double-click) rather than bored.
    Walking(Instant, bool),
    Climbing(Instant),
}

struct Legs {
    app: AppHandle,
    window: Option<WebviewWindow>,
    at: Where,
    /// Feet position and velocity, physical px.
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    facing: i8,
    slope: i8,
    gaze: i8,
    sent: Option<(&'static str, i8, i8, i8, bool)>,
    hovered: bool,
    last_cursor: (i32, i32),
    last_moved: Instant,
    nudge: bool,
    flips: Vec<Instant>,
    last_dir: i8,
    excited_until: Instant,
    rest_since: Instant,
    alert_until: Option<Instant>,
    home_since: Instant,
}

impl Legs {
    fn new(app: AppHandle) -> Self {
        let now = Instant::now();
        Self {
            app,
            window: None,
            at: Where::Home,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            facing: 1,
            slope: 0,
            gaze: 0,
            sent: None,
            hovered: false,
            last_cursor: island::cursor_pos(),
            last_moved: now,
            nudge: false,
            flips: Vec::new(),
            last_dir: 0,
            excited_until: now,
            rest_since: now,
            alert_until: None,
            home_since: now,
        }
    }

    fn pet(&self) -> tauri::State<'_, Pet> {
        self.app.state::<Pet>()
    }

    fn run(mut self) {
        let _ = self.app.emit("pet-home", true);
        let mut last = Instant::now();
        loop {
            let tick = match self.at {
                Where::Home => ASLEEP,
                Where::Loading(_) => RESTING,
                Where::Out if self.vx.hypot(self.vy) < 1.0 => RESTING,
                _ => MOVING,
            };
            if tick == MOVING {
                next_frame();
            } else {
                std::thread::sleep(tick);
            }
            let now = Instant::now();
            let dt = (now - last).as_secs_f64().min(0.1);
            last = now;
            self.step(now, dt);
        }
    }

    fn step(&mut self, now: Instant, dt: f64) {
        let config = self.pet().config.lock().unwrap().clone();
        let wake = self.pet().wake.lock().unwrap().take();
        let home_asked = self.pet().go_home.swap(false, Ordering::Relaxed);
        let away = !config.enabled
            || crate::game::on()
            || self.app.state::<island::UserHidden>().0.load(Ordering::Relaxed);

        let cursor = island::cursor_pos();
        if cursor != self.last_cursor {
            self.watch_shake(now, cursor, dt);
            self.last_cursor = cursor;
            self.last_moved = now;
        }

        match self.at {
            Where::Home => {
                if self.window.is_some() && (crate::game::on() || now - self.home_since >= KEEP_WARM) {
                    // Closing the window frees its WebView2.
                    if let Some(w) = self.window.take() {
                        let _ = w.destroy();
                    }
                }
                if let Some(reason) = wake.filter(|_| !away) {
                    self.nudge = reason == "nudge";
                    if self.window.is_some() {
                        self.wake_up(now);
                        return;
                    }
                    self.pet().ready.store(false, Ordering::Relaxed);
                    match open_window(&self.app) {
                        Ok(w) => {
                            self.window = Some(w);
                            self.at = Where::Loading(now);
                        }
                        Err(e) => eprintln!("pet window: {e}"),
                    }
                }
            }
            _ if away => self.arrive_home(),
            Where::Loading(since) => {
                if self.pet().ready.load(Ordering::Relaxed) || now - since >= LOAD_TIMEOUT {
                    self.wake_up(now);
                }
            }
            Where::Waking(since) => {
                if now - since >= STRETCH {
                    self.at = Where::Out;
                    self.last_moved = now;
                }
            }
            Where::Out => {
                if wake.as_deref() == Some("nudge") {
                    // A reminder beats being told to stay.
                    self.nudge = true;
                    self.pet().stay.store(false, Ordering::Relaxed);
                }
                let still = now - self.last_moved;
                let night = config.night_sleep && self.pet().night.load(Ordering::Relaxed);
                // Told to stay, it stays (and naps there) until clicked or sent home.
                let stay = self.pet().stay.load(Ordering::Relaxed);
                if home_asked
                    || (!stay && still >= Duration::from_secs(config.sleep_after_min.max(1) as u64 * 60))
                    || (!stay && night && still >= NIGHT_IDLE)
                {
                    self.at = Where::Walking(now, home_asked);
                } else {
                    self.follow(now, dt, &config);
                }
            }
            Where::Walking(since, sent) => {
                let (hx, hy) = self.home_spot();
                // A cursor that moves again calls it back out (but not a double-click send-off).
                if self.last_moved > since && !sent {
                    self.at = Where::Out;
                } else if self.go_to(hx, hy, WALK * self.scale(), dt) < 6.0 {
                    self.at = Where::Climbing(now);
                    self.vx = 0.0;
                    self.vy = 0.0;
                    self.send("climb");
                }
            }
            Where::Climbing(since) => {
                if now - since >= CLIMB {
                    self.arrive_home();
                }
            }
        }
        if !matches!(self.at, Where::Home | Where::Loading(_)) {
            self.place();
            self.watch_hover(cursor);
        }
    }

    fn wake_up(&mut self, now: Instant) {
        let (hx, hy) = self.home_spot();
        self.x = hx;
        self.y = hy;
        self.vx = 0.0;
        self.vy = 0.0;
        self.place();
        if let Some(w) = &self.window {
            let _ = w.show();
        }
        let _ = self.app.emit("pet-home", false);
        self.at = Where::Waking(now);
        self.slope = 0;
        self.sent = None;
        self.send("wake");
    }

    fn arrive_home(&mut self) {
        if let Some(w) = &self.window {
            let _ = w.hide();
            let _ = w.set_ignore_cursor_events(true);
        }
        self.home_since = Instant::now();
        if self.hovered {
            self.hovered = false;
            let _ = self.app.emit("pet-hover", false);
        }
        self.at = Where::Home;
        self.nudge = false;
        self.pet().stay.store(false, Ordering::Relaxed);
        let _ = self.app.emit("pet-home", true);
    }

    fn follow(&mut self, now: Instant, dt: f64, config: &PetConfig) {
        let scale = self.scale();
        let (cx, cy) = (self.last_cursor.0 as f64, self.last_cursor.1 as f64);
        if self.pet().stay.load(Ordering::Relaxed) {
            // Staying: stock still, turning to keep its eyes on the cursor.
            self.vx = 0.0;
            self.vy = 0.0;
            self.slope = 0;
            self.alert_until = None;
            if (cx - self.x).abs() > 6.0 * scale {
                self.facing = if cx >= self.x { 1 } else { -1 };
            }
            let (dx, dy) = ((cx - self.x).abs().max(1.0), cy - (self.y - 38.0 * scale));
            self.gaze = ((dy.atan2(dx).to_degrees().clamp(-40.0, 30.0) / 3.0).round() * 3.0) as i8;
            self.send("rest");
            return;
        }
        let calm = self.pet().calm.load(Ordering::Relaxed);
        let excited = now < self.excited_until && !calm;
        let mut keep = match config.distance.as_str() {
            "close" => 60.0,
            "far" => 190.0,
            _ => 115.0,
        } * scale;
        if calm {
            keep *= 1.6;
        }
        if excited || self.nudge {
            keep = 28.0 * scale;
        }
        // Stand just below the cursor, so it isn't on top of what you're pointing at.
        let (tx, ty) = (cx, cy + 34.0 * scale);
        let d = (tx - self.x).hypot(ty - self.y);

        if d > TELEPORT * scale {
            self.x = tx - self.facing as f64 * keep;
            self.y = ty;
            self.send("wake");
            return;
        }
        let pep = if config.energy == "playful" { 1.25 } else { 0.85 };
        let moving = self.vx.hypot(self.vy) > 1.0;
        let want = if d < keep || (!moving && d < keep + SLACK * scale) {
            0.0
        } else if excited {
            EXCITED
        } else if d > keep + RUN_AFTER * scale && !calm {
            RUN * pep
        } else {
            WALK * pep
        } * scale;

        // Anticipation: a rested dog perks up first instead of sliding off at once.
        if want > 0.0 && !moving && self.alert_until.is_none() && now - self.rest_since >= ALERT_AFTER {
            self.alert_until = Some(now + ALERT);
        }
        if let Some(until) = self.alert_until {
            if now < until {
                self.facing = if tx >= self.x { 1 } else { -1 };
                self.send("alert");
                return;
            }
            self.alert_until = None;
            self.rest_since = now;
        }

        let (ux, uy) = if d > 0.0 { ((tx - self.x) / d, (ty - self.y) / d) } else { (0.0, 0.0) };
        let k = (GRIP * dt).min(1.0);
        self.vx += (ux * want - self.vx) * k;
        self.vy += (uy * want - self.vy) * k;
        if want == 0.0 && self.vx.hypot(self.vy) < 12.0 * scale {
            self.vx = 0.0;
            self.vy = 0.0;
        }
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.clamp();

        let speed = self.vx.hypot(self.vy) / scale;
        if speed >= 1.0 {
            self.rest_since = now;
        }
        if self.vx.abs() > 8.0 * scale {
            self.facing = if self.vx > 0.0 { 1 } else { -1 };
        } else if speed < 1.0 {
            // Resting: face the cursor.
            self.facing = if cx >= self.x { 1 } else { -1 };
        }
        self.update_slope();
        if speed < 1.0 {
            if self.nudge {
                self.nudge = false;
                self.send("nudge");
            } else {
                self.send("rest");
            }
        } else {
            self.send(if speed > WALK * 1.4 { "run" } else { "walk" });
        }
    }

    /// Head for a point at `speed`; returns the distance left.
    fn go_to(&mut self, tx: f64, ty: f64, speed: f64, dt: f64) -> f64 {
        let d = (tx - self.x).hypot(ty - self.y);
        let step = (speed * dt).min(d);
        if d > 0.0 {
            self.vx = (tx - self.x) / d * speed;
            self.vy = (ty - self.y) / d * speed;
            self.x += (tx - self.x) / d * step;
            self.y += (ty - self.y) / d * step;
        }
        if self.vx.abs() > 1.0 {
            self.facing = if self.vx > 0.0 { 1 } else { -1 };
        }
        self.update_slope();
        self.send("walk");
        d - step
    }

    /// Lean into the way it's going: the angle of travel, eased so straight up or down is a
    /// steep slope rather than standing on end. In steps of SLOPE_STEP so it isn't an event a frame.
    fn update_slope(&mut self) {
        const SLOPE_STEP: f64 = 2.0;
        let speed = self.vx.hypot(self.vy) / self.scale();
        self.slope = if speed < 1.0 {
            0
        } else {
            // Side-on, so only the up/down part matters: 0° level, ±90° straight up/down.
            let angle = self.vy.atan2(self.vx.abs()).to_degrees();
            let tilt = (angle * 0.45).clamp(-38.0, 38.0);
            ((tilt / SLOPE_STEP).round() * SLOPE_STEP) as i8
        };
    }

    /// Just under the island, where it hops out and climbs back in.
    fn home_spot(&self) -> (f64, f64) {
        let Some(main) = self.app.get_webview_window("main") else { return (self.x, self.y) };
        let (Ok(pos), Ok(size), Ok(scale)) = (main.outer_position(), main.outer_size(), main.scale_factor())
        else {
            return (self.x, self.y);
        };
        (pos.x as f64 + size.width as f64 / 2.0, pos.y as f64 + (30.0 + DOG_H) * scale)
    }

    fn scale(&self) -> f64 {
        let main = self.app.get_webview_window("main");
        self.window.as_ref().or(main.as_ref()).and_then(|w| w.scale_factor().ok()).unwrap_or(1.0)
    }

    /// Keep its feet inside the work area (off the taskbar) of the cursor's monitor.
    fn clamp(&mut self) {
        let Some((l, t, r, b)) = work_area(self.last_cursor) else { return };
        let s = self.scale();
        self.x = self.x.clamp(l + DOG_W / 2.0 * s, r - DOG_W / 2.0 * s);
        self.y = self.y.clamp(t + DOG_H * s, b - 2.0 * s);
    }

    fn place(&self) {
        let Some(w) = &self.window else { return };
        // By its real size: Windows may round the window up from what was asked.
        let Ok(size) = w.outer_size() else { return };
        let _ = w.set_position(tauri::PhysicalPosition::new(
            (self.x - size.width as f64 / 2.0).round() as i32,
            (self.y - size.height as f64).round() as i32,
        ));
    }

    /// Clicks reach the dog itself; everywhere else they pass through.
    fn watch_hover(&mut self, (cx, cy): (i32, i32)) {
        let s = self.scale();
        let (cx, cy) = (cx as f64, cy as f64);
        let inside = (cx - self.x).abs() <= DOG_W / 2.0 * s && cy <= self.y && cy >= self.y - DOG_H * s;
        if inside != self.hovered {
            self.hovered = inside;
            if let Some(w) = &self.window {
                let _ = w.set_ignore_cursor_events(!inside);
            }
            let _ = self.app.emit("pet-hover", inside);
        }
    }

    fn watch_shake(&mut self, now: Instant, (x, _): (i32, i32), dt: f64) {
        let dx = (x - self.last_cursor.0) as f64;
        let s = self.scale();
        if dt <= 0.0 || dx.abs() / dt < SHAKE_SPEED * s {
            return;
        }
        let dir = if dx > 0.0 { 1 } else { -1 };
        if dir != self.last_dir {
            self.last_dir = dir;
            self.flips.push(now);
        }
        self.flips.retain(|t| now - *t < SHAKE_WINDOW);
        if self.flips.len() >= SHAKE_FLIPS {
            self.flips.clear();
            self.excited_until = now + EXCITED_FOR;
        }
    }

    fn send(&mut self, motion: &'static str) {
        let excited = Instant::now() < self.excited_until;
        let next = (motion, self.facing, self.slope, self.gaze, excited);
        if self.sent == Some(next) {
            return;
        }
        self.sent = Some(next);
        let _ = self.app.emit_to(
            "pet",
            "pet-motion",
            Motion { motion, facing: self.facing, slope: self.slope, gaze: self.gaze, excited },
        );
    }
}

#[cfg(target_os = "windows")]
fn work_area((x, y): (i32, i32)) -> Option<(f64, f64, f64, f64)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    unsafe {
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST), &mut info) == 0 {
            return None;
        }
        let r = info.rcWork;
        Some((r.left as f64, r.top as f64, r.right as f64, r.bottom as f64))
    }
}

#[cfg(not(target_os = "windows"))]
fn work_area(_: (i32, i32)) -> Option<(f64, f64, f64, f64)> {
    None
}
