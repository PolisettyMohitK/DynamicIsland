// Volume, output device, battery and network state for the pop-ups, plus
// brightness changes. The frontend decides what deserves a pop-up.
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_millis(100);
const DEVICE_EVERY: u32 = 10; // ~1s: output device and battery
const NETWORK_EVERY: u32 = 30; // ~3s

#[derive(Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// 0–100
    pub volume: u8,
    pub muted: bool,
    /// Friendly name of the default playback device.
    pub output: String,
    pub battery: Option<Battery>,
    pub network: Option<Network>,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

#[derive(Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub wifi: bool,
    pub name: String,
}

pub fn spawn_watcher(app: AppHandle) {
    let events = app.clone();
    std::thread::spawn(move || imp::watch_status(app));
    std::thread::spawn(move || {
        imp::watch_brightness(|level| {
            let _ = events.emit("brightness", level);
        })
    });
}

#[tauri::command]
pub async fn status_current() -> Status {
    tauri::async_runtime::spawn_blocking(imp::read_once).await.unwrap_or_default()
}

#[tauri::command]
pub async fn set_volume(level: f64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || imp::set_volume(level as f32))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn toggle_mute() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(imp::toggle_mute)
        .await
        .map_err(|e| e.to_string())?
}

fn emit_on_change(app: &AppHandle, last: &mut Status, now: Status) {
    if *last != now {
        *last = now.clone();
        let _ = app.emit("status", now);
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use windows::core::{w, BSTR};
    use windows::Networking::Connectivity::{NetworkConnectivityLevel, NetworkInformation};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{eConsole, eRender, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoTaskMemFree, CLSCTX_ALL,
        COINIT_MULTITHREADED, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, STGM_READ,
    };
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::System::Wmi::{
        IWbemClassObject, IWbemLocator, WbemLocator, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
        WBEM_INFINITE,
    };

    struct Endpoint {
        id: String,
        name: String,
        volume: IAudioEndpointVolume,
    }

    fn com_init() {
        // Fails harmlessly if this thread already has COM.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    }

    fn default_device() -> windows::core::Result<IMMDevice> {
        unsafe {
            let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            devices.GetDefaultAudioEndpoint(eRender, eConsole)
        }
    }

    fn device_id(device: &IMMDevice) -> String {
        unsafe {
            let Ok(p) = device.GetId() else { return String::new() };
            let id = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as _));
            id
        }
    }

    fn endpoint() -> Option<Endpoint> {
        let device = default_device().ok()?;
        unsafe {
            let name = device
                .OpenPropertyStore(STGM_READ)
                .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
                .map(|v| v.to_string())
                .unwrap_or_default();
            Some(Endpoint {
                id: device_id(&device),
                name,
                volume: device.Activate(CLSCTX_ALL, None).ok()?,
            })
        }
    }

    fn battery() -> Option<Battery> {
        let mut s = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut s).ok()? };
        // 128: no battery, 255: unknown
        if s.BatteryFlag == 128 || s.BatteryFlag == 255 || s.BatteryLifePercent > 100 {
            return None;
        }
        Some(Battery { percent: s.BatteryLifePercent, charging: s.ACLineStatus == 1 })
    }

    fn network() -> Option<Network> {
        let profile = NetworkInformation::GetInternetConnectionProfile().ok()?;
        let level = profile.GetNetworkConnectivityLevel().ok()?;
        if level == NetworkConnectivityLevel::None {
            return None;
        }
        Some(Network {
            wifi: profile.IsWlanConnectionProfile().unwrap_or(false),
            name: profile.ProfileName().map(|n| n.to_string()).unwrap_or_default(),
        })
    }

    fn read(ep: &Option<Endpoint>, battery: Option<Battery>, network: Option<Network>) -> Status {
        let (volume, muted) = ep
            .as_ref()
            .map(|e| unsafe {
                (
                    e.volume.GetMasterVolumeLevelScalar().unwrap_or(0.0),
                    e.volume.GetMute().map(|m| m.as_bool()).unwrap_or(false),
                )
            })
            .unwrap_or((0.0, false));
        Status {
            volume: (volume * 100.0).round() as u8,
            muted,
            output: ep.as_ref().map(|e| e.name.clone()).unwrap_or_default(),
            battery,
            network,
        }
    }

    pub fn read_once() -> Status {
        com_init();
        read(&endpoint(), battery(), network())
    }

    pub fn watch_status(app: AppHandle) {
        com_init();
        let mut ep = endpoint();
        let mut bat = battery();
        let mut net = network();
        let mut last = read(&ep, bat.clone(), net.clone());
        let mut tick = 0u32;
        loop {
            std::thread::sleep(if crate::game::on() { crate::game::IDLE_POLL } else { POLL });
            tick = tick.wrapping_add(1);
            if tick % DEVICE_EVERY == 0 {
                // Plugging in headphones switches the default device.
                let current = default_device().ok().map(|d| device_id(&d));
                if current != ep.as_ref().map(|e| e.id.clone()) {
                    ep = endpoint();
                }
                bat = battery();
            }
            if tick % NETWORK_EVERY == 0 {
                net = network();
            }
            emit_on_change(&app, &mut last, read(&ep, bat.clone(), net.clone()));
        }
    }

    pub fn set_volume(level: f32) -> Result<(), String> {
        com_init();
        let ep = endpoint().ok_or("no playback device")?;
        unsafe {
            ep.volume
                .SetMasterVolumeLevelScalar(level.clamp(0.0, 1.0), std::ptr::null())
                .map_err(|e| e.to_string())?;
            if level > 0.0 {
                let _ = ep.volume.SetMute(false, std::ptr::null());
            }
        }
        Ok(())
    }

    pub fn toggle_mute() -> Result<(), String> {
        com_init();
        let ep = endpoint().ok_or("no playback device")?;
        unsafe {
            let muted = ep.volume.GetMute().map_err(|e| e.to_string())?.as_bool();
            ep.volume.SetMute(!muted, std::ptr::null()).map_err(|e| e.to_string())
        }
    }

    /// Blocks on WMI brightness events (built-in laptop panels only).
    /// Returns quietly on machines that don't report them.
    pub fn watch_brightness(emit: impl Fn(u32)) {
        com_init();
        let result: windows::core::Result<()> = (|| unsafe {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_ALL)?;
            let services = locator.ConnectServer(
                &BSTR::from("ROOT\\WMI"),
                &BSTR::new(),
                &BSTR::new(),
                &BSTR::new(),
                0,
                &BSTR::new(),
                None,
            )?;
            CoSetProxyBlanket(
                &services,
                RPC_C_AUTHN_WINNT,
                RPC_C_AUTHZ_NONE,
                None,
                RPC_C_AUTHN_LEVEL_CALL,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
            )?;
            let events = services.ExecNotificationQuery(
                &BSTR::from("WQL"),
                &BSTR::from("SELECT * FROM WmiMonitorBrightnessEvent"),
                WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                None,
            )?;
            loop {
                let mut objs: [Option<IWbemClassObject>; 1] = [None];
                let mut returned = 0u32;
                events.Next(WBEM_INFINITE, &mut objs, &mut returned).ok()?;
                let Some(obj) = objs[0].take() else { continue };
                let mut value = VARIANT::default();
                obj.Get(w!("Brightness"), 0, &mut value, None, None)?;
                if let Ok(level) = u32::try_from(&value) {
                    emit(level);
                }
            }
        })();
        if let Err(e) = result {
            eprintln!("brightness events unavailable: {e}");
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::*;

    pub fn read_once() -> Status {
        Status::default()
    }
    pub fn watch_status(_app: AppHandle) {}
    pub fn set_volume(_level: f32) -> Result<(), String> {
        Err("not supported".into())
    }
    pub fn toggle_mute() -> Result<(), String> {
        Err("not supported".into())
    }
    pub fn watch_brightness(_emit: impl Fn(u32)) {}
}
