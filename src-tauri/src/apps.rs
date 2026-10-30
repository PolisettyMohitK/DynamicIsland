// Control Centre > Apps: everything in the Start menu, and launching it.
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone)]
pub struct App {
    #[serde(rename(deserialize = "Name"))]
    pub name: String,
    #[serde(rename(deserialize = "AppID"))]
    pub id: String,
}

/// Get-StartApps takes a second, so the list is read once per run.
static CACHE: Mutex<Option<Vec<App>>> = Mutex::new(None);

#[tauri::command]
pub async fn apps_list() -> Result<Vec<App>, String> {
    if let Some(apps) = CACHE.lock().unwrap().clone() {
        return Ok(apps);
    }
    let apps = tauri::async_runtime::spawn_blocking(read).await.map_err(|e| e.to_string())??;
    *CACHE.lock().unwrap() = Some(apps.clone());
    Ok(apps)
}

#[tauri::command]
pub fn apps_launch(id: String) -> Result<(), String> {
    // AppsFolder starts desktop and Store apps alike from their Start menu id.
    std::process::Command::new("explorer.exe")
        .arg(format!("shell:AppsFolder\\{id}"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn read() -> Result<Vec<App>, String> {
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-StartApps | Select-Object Name,AppID | ConvertTo-Json -Compress",
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    let mut apps: Vec<App> = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    // Uninstallers and help links aren't things anyone opens on purpose.
    apps.retain(|a| !a.id.starts_with("http") && !a.name.to_lowercase().contains("uninstall"));
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.name == b.name);
    Ok(apps)
}
