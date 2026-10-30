// Control Centre: the Windows switches the island can flip. Wi-Fi and Bluetooth radios, dark
// mode, screen brightness, which speakers play, and power. Nothing here polls; the frontend
// reads the state when the Control Centre opens.
use serde::Serialize;

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ControlState {
    /// None when the PC has no such radio.
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
    pub dark: bool,
    /// 0–100; None on desktop monitors that don't take brightness from Windows.
    pub brightness: Option<u8>,
    pub outputs: Vec<Output>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    pub id: String,
    pub name: String,
    pub default: bool,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn control_state() -> Result<ControlState, String> {
    blocking(|| Ok(imp::state())).await
}

/// `kind` is "wifi" or "bluetooth".
#[tauri::command]
pub async fn control_set_radio(kind: String, on: bool) -> Result<(), String> {
    blocking(move || imp::set_radio(&kind, on)).await
}

#[tauri::command]
pub async fn control_set_dark(on: bool) -> Result<(), String> {
    blocking(move || imp::set_dark(on)).await
}

#[tauri::command]
pub async fn control_set_brightness(level: u8) -> Result<(), String> {
    blocking(move || imp::set_brightness(level.min(100))).await
}

#[tauri::command]
pub async fn control_set_output(id: String) -> Result<(), String> {
    blocking(move || imp::set_output(&id)).await
}

/// `action` is "lock", "sleep", "restart", "shutdown" or "signout".
#[tauri::command]
pub async fn control_power(action: String) -> Result<(), String> {
    blocking(move || imp::power(&action)).await
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use windows::core::{w, Interface, IUnknown, BSTR, GUID, HRESULT, PCWSTR};
    use windows::Devices::Radios::{Radio, RadioKind, RadioState};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::{
        eCommunications, eConsole, eMultimedia, eRender, ERole, IMMDeviceEnumerator, MMDeviceEnumerator,
        DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoSetProxyBlanket, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
        EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, STGM_READ,
    };
    use windows::Win32::System::Registry::{
        RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD,
    };
    use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::System::Wmi::{
        IWbemClassObject, IWbemLocator, IWbemServices, WbemLocator, WBEM_FLAG_FORWARD_ONLY,
        WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WBEM_INFINITE,
    };

    const PERSONALIZE: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    const NO_WINDOW: u32 = 0x0800_0000;

    fn com_init() {
        // Fails harmlessly if this thread already has COM.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    }

    pub fn state() -> ControlState {
        com_init();
        let (wifi, bluetooth) = radios();
        ControlState { wifi, bluetooth, dark: dark(), brightness: brightness(), outputs: outputs() }
    }

    // Radios -------------------------------------------------------------------------------

    fn find_radio(kind: RadioKind) -> Option<Radio> {
        let all = Radio::GetRadiosAsync().ok()?.get().ok()?;
        all.into_iter().find(|r| r.Kind().ok() == Some(kind))
    }

    fn radios() -> (Option<bool>, Option<bool>) {
        let on = |kind| find_radio(kind).and_then(|r| r.State().ok()).map(|s| s == RadioState::On);
        (on(RadioKind::WiFi), on(RadioKind::Bluetooth))
    }

    pub fn set_radio(kind: &str, on: bool) -> Result<(), String> {
        com_init();
        let kind = match kind {
            "wifi" => RadioKind::WiFi,
            "bluetooth" => RadioKind::Bluetooth,
            _ => return Err(format!("unknown radio {kind}")),
        };
        let _ = Radio::RequestAccessAsync().and_then(|op| op.get());
        let radio = find_radio(kind).ok_or("no such radio")?;
        let state = if on { RadioState::On } else { RadioState::Off };
        radio.SetStateAsync(state).and_then(|op| op.get()).map_err(|e| e.to_string())?;
        Ok(())
    }

    // Dark mode ----------------------------------------------------------------------------

    fn dark() -> bool {
        let mut light = 1u32;
        let mut size = 4u32;
        let err = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PERSONALIZE,
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut light as *mut u32 as _),
                Some(&mut size),
            )
        };
        err.is_ok() && light == 0
    }

    pub fn set_dark(on: bool) -> Result<(), String> {
        let light: u32 = if on { 0 } else { 1 };
        for name in [w!("AppsUseLightTheme"), w!("SystemUsesLightTheme")] {
            let err = unsafe {
                RegSetKeyValueW(HKEY_CURRENT_USER, PERSONALIZE, name, REG_DWORD.0, Some(&light as *const u32 as _), 4)
            };
            err.ok().map_err(|e| e.to_string())?;
        }
        // Tell open windows the theme changed, the way Settings does.
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };
        let area: Vec<u16> = "ImmersiveColorSet\0".encode_utf16().collect();
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                area.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                200,
                std::ptr::null_mut(),
            );
        }
        Ok(())
    }

    // Brightness (built-in panels, through WMI) --------------------------------------------

    fn wmi() -> windows::core::Result<IWbemServices> {
        unsafe {
            let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_ALL)?;
            let services =
                locator.ConnectServer(&BSTR::from("ROOT\\WMI"), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None)?;
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
            Ok(services)
        }
    }

    fn first(services: &IWbemServices, query: &str) -> Option<IWbemClassObject> {
        unsafe {
            let rows = services
                .ExecQuery(
                    &BSTR::from("WQL"),
                    &BSTR::from(query),
                    WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                    None,
                )
                .ok()?;
            let mut objs: [Option<IWbemClassObject>; 1] = [None];
            let mut returned = 0u32;
            rows.Next(WBEM_INFINITE, &mut objs, &mut returned).ok().ok()?;
            objs[0].take()
        }
    }

    fn brightness() -> Option<u8> {
        let services = wmi().ok()?;
        let row = first(&services, "SELECT CurrentBrightness FROM WmiMonitorBrightness")?;
        let mut value = VARIANT::default();
        unsafe { row.Get(w!("CurrentBrightness"), 0, &mut value, None, None).ok()? };
        u32::try_from(&value).ok().map(|v| v.min(100) as u8)
    }

    pub fn set_brightness(level: u8) -> Result<(), String> {
        com_init();
        let run = || -> windows::core::Result<()> {
            let services = wmi()?;
            let row = first(&services, "SELECT * FROM WmiMonitorBrightnessMethods")
                .ok_or_else(|| windows::core::Error::from(HRESULT(-1)))?;
            unsafe {
                let mut path = VARIANT::default();
                row.Get(w!("__PATH"), 0, &mut path, None, None)?;
                let path = BSTR::try_from(&path)?;
                let mut class = None;
                services.GetObject(
                    &BSTR::from("WmiMonitorBrightnessMethods"),
                    WBEM_GENERIC_FLAG_TYPE(0),
                    None,
                    Some(&mut class),
                    None,
                )?;
                let class = class.ok_or_else(|| windows::core::Error::from(HRESULT(-1)))?;
                let mut input = None;
                class.GetMethod(w!("WmiSetBrightness"), 0, &mut input, std::ptr::null_mut())?;
                let args = input.ok_or_else(|| windows::core::Error::from(HRESULT(-1)))?.SpawnInstance(0)?;
                // WMI takes uint32 as VT_I4 and uint8 as VT_UI1.
                args.Put(w!("Timeout"), 0, &VARIANT::from(0i32), 0)?;
                args.Put(w!("Brightness"), 0, &VARIANT::from(level), 0)?;
                services.ExecMethod(&path, &BSTR::from("WmiSetBrightness"), WBEM_GENERIC_FLAG_TYPE(0), None, &args, None, None)?;
            }
            Ok(())
        };
        run().map_err(|e| format!("brightness: {e}"))
    }

    // Audio output -------------------------------------------------------------------------

    fn outputs() -> Vec<Output> {
        let read = || -> windows::core::Result<Vec<Output>> {
            unsafe {
                let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                let default = devices.GetDefaultAudioEndpoint(eRender, eConsole).ok().map(|d| id_of(&d));
                let list = devices.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
                let mut out = Vec::new();
                for i in 0..list.GetCount()? {
                    let device = list.Item(i)?;
                    let id = id_of(&device);
                    let name = device
                        .OpenPropertyStore(STGM_READ)
                        .and_then(|s| s.GetValue(&PKEY_Device_FriendlyName))
                        .map(|v| v.to_string())
                        .unwrap_or_default();
                    out.push(Output { default: default.as_deref() == Some(id.as_str()), id, name });
                }
                Ok(out)
            }
        };
        read().unwrap_or_default()
    }

    fn id_of(device: &windows::Win32::Media::Audio::IMMDevice) -> String {
        unsafe {
            let Ok(p) = device.GetId() else { return String::new() };
            let id = p.to_string().unwrap_or_default();
            CoTaskMemFree(Some(p.0 as _));
            id
        }
    }

    // Windows has no public API for picking the default speakers. Every app that does it
    // (EarTrumpet, SoundSwitch) uses the undocumented IPolicyConfig, stable since Windows 7.
    // Called through its vtable: SetDefaultEndpoint is slot 13 (3 from IUnknown + 10).
    const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
    const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);
    const SET_DEFAULT_ENDPOINT: usize = 13;
    type SetDefaultEndpoint = unsafe extern "system" fn(*mut core::ffi::c_void, PCWSTR, ERole) -> HRESULT;

    pub fn set_output(id: &str) -> Result<(), String> {
        com_init();
        let wide: Vec<u16> = id.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let client: IUnknown = CoCreateInstance(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL).map_err(|e| e.to_string())?;
            let mut raw = std::ptr::null_mut();
            client.query(&IID_POLICY_CONFIG, &mut raw).ok().map_err(|e| e.to_string())?;
            let policy = IUnknown::from_raw(raw); // released on drop
            let vtable = *(raw as *const *const usize);
            let set: SetDefaultEndpoint = std::mem::transmute(*vtable.add(SET_DEFAULT_ENDPOINT));
            for role in [eConsole, eMultimedia, eCommunications] {
                set(raw, PCWSTR(wide.as_ptr()), role).ok().map_err(|e| e.to_string())?;
            }
            drop(policy);
        }
        Ok(())
    }

    // Power --------------------------------------------------------------------------------

    pub fn power(action: &str) -> Result<(), String> {
        let shutdown = |args: &[&str]| {
            std::process::Command::new("shutdown")
                .args(args)
                .creation_flags(NO_WINDOW)
                .spawn()
                .map(|_| ())
                .map_err(|e| e.to_string())
        };
        match action {
            "lock" => unsafe { windows::Win32::System::Shutdown::LockWorkStation() }.map_err(|e| e.to_string()),
            "sleep" => {
                unsafe { windows::Win32::System::Power::SetSuspendState(false, false, false) };
                Ok(())
            }
            "restart" => shutdown(&["/r", "/t", "0"]),
            "shutdown" => shutdown(&["/s", "/t", "0"]),
            "signout" => shutdown(&["/l"]),
            _ => Err(format!("unknown power action {action}")),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::*;
    const NO: &str = "only on Windows";
    pub fn state() -> ControlState {
        ControlState::default()
    }
    pub fn set_radio(_: &str, _: bool) -> Result<(), String> {
        Err(NO.into())
    }
    pub fn set_dark(_: bool) -> Result<(), String> {
        Err(NO.into())
    }
    pub fn set_brightness(_: u8) -> Result<(), String> {
        Err(NO.into())
    }
    pub fn set_output(_: &str) -> Result<(), String> {
        Err(NO.into())
    }
    pub fn power(_: &str) -> Result<(), String> {
        Err(NO.into())
    }
}
