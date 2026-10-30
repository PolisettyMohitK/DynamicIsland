use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager};

mod ask;
mod capture;
mod computer;
mod game;
mod island;
mod choco;
mod media;
mod memory;
mod pet;
mod shelf;
mod status;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
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

            let toggle = MenuItem::with_id(app, "toggle", "Hide island", true, None::<&str>)?;
            let voice = MenuItem::with_id(app, "voice", "Hey Choco on/off", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle, &voice, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Island")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id.as_ref() {
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
                    "quit" => app.exit(0),
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
            ask::choco_key_status,
            ask::choco_set_key,
            computer::computer_run,
            computer::computer_stop,
            computer::computer_answer,
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
