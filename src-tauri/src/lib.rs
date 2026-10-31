use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

mod ai;
mod ask;
mod capture;
mod computer;
mod dashboard;
mod control;
mod stats;
mod apps;
mod clipboard;
mod wallpaper;
mod dictation;
mod circle;
mod game;
mod island;
mod choco;
mod media;
mod memory;
mod pet;
mod shelf;
mod status;

/// Quit for good. A normal exit can stall on the mic, model or hook threads after it has
/// already let go of the single-instance lock, leaving an island with no dashboard behind
/// that the next launch can't reach; so tidy up, then end the process outright.
fn quit_now(app: &tauri::AppHandle) {
    app.cleanup_before_exit();
    unsafe {
        windows_sys::Win32::System::Threading::TerminateProcess(
            windows_sys::Win32::System::Threading::GetCurrentProcess(),
            0,
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Launching it again opens the dashboard instead of a second island.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| dashboard::show(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().args([dashboard::QUIET_START]).build())
        .manage(island::HitSize(Mutex::new((0.0, 0.0))))
        .manage(island::UserHidden(Arc::new(AtomicBool::new(false))))
        .manage(island::Hovered(AtomicBool::new(false)))
        .manage(capture::LastFrame(Mutex::new(Vec::new())))
        .manage(pet::Pet::default())
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .expect("main window is defined in tauri.conf.json");
            island::place(&window)?;
            capture::exclude_from_capture(&window);
            island::spawn_watcher(app.handle().clone(), window);
            media::spawn_watcher(app.handle().clone());
            status::spawn_watcher(app.handle().clone());
            choco::spawn(app.handle().clone());
            pet::spawn(app.handle().clone());
            game::spawn(app.handle().clone());
            let quiet = std::env::args().any(|a| a == dashboard::QUIET_START);
            dashboard::create(app.handle(), !quiet)?;
            clipboard::start(app.handle().clone());
            dictation::spawn(app.handle().clone());
            circle::spawn(app.handle().clone());

            let open = MenuItem::with_id(app, "dashboard", "Open dashboard", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Hide island", true, None::<&str>)?;
            let voice = MenuItem::with_id(app, "voice", "Hey Choco on/off", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &voice, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Island")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "dashboard" => dashboard::show(app),
                    "toggle" => {
                        let hidden = &app.state::<island::UserHidden>().0;
                        let now_hidden = !hidden.load(Ordering::Relaxed);
                        hidden.store(now_hidden, Ordering::Relaxed);
                        let _ = toggle.set_text(if now_hidden { "Show island" } else { "Hide island" });
                    }
                    // The island remembers the choice and turns the listener on or off.
                    "voice" => {
                        let _ = app.emit("choco-toggle", ());
                    }
                    "quit" => quit_now(app),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            island::set_hit_size,
            island::is_hovered,
            island::set_typing,
            capture::capture_backdrop,
            status::status_current,
            status::set_volume,
            status::toggle_mute,
            choco::choco_set_enabled,
            choco::choco_follow_up,
            choco::choco_configure,
            choco::choco_mics,
            ask::choco_ask,
            ai::ai_status,
            ai::ai_set_provider,
            ai::ai_set_key,
            ai::ai_set_model,
            ai::ai_models,
            computer::computer_run,
            computer::computer_stop,
            computer::computer_answer,
            control::control_state,
            control::control_set_radio,
            control::control_set_dark,
            control::control_set_brightness,
            control::control_set_output,
            control::control_power,
            stats::stats_read,
            stats::stats_stop,
            apps::apps_list,
            apps::apps_launch,
            clipboard::clipboard_history,
            clipboard::clipboard_clear,
            clipboard::clipboard_paste,
            wallpaper::wallpaper_list,
            wallpaper::wallpaper_thumb,
            wallpaper::wallpaper_set,
            wallpaper::wallpaper_open_folder,
            dictation::dictation_set_enabled,
            circle::circle_close,
            circle::circle_stop_listening,
            circle::circle_listen,
            circle::circle_ask,
            pet::pet_configure,
            pet::pet_config,
            pet::pet_wake,
            pet::pet_home,
            pet::pet_context,
            pet::pet_ready,
            shelf::shelf_thumb,
            shelf::shelf_exists,
            shelf::shelf_open,
            shelf::shelf_drag,
            media::media_current,
            media::media_play_pause,
            media::media_next,
            media::media_previous,
            media::media_seek,
            media::media_artwork,
            media::fetch_lyrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dynamic Island");
}
