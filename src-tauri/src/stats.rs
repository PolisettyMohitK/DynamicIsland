// Control Centre > Stats: CPU, GPU, memory, disk, network and the hungriest apps.
// Sampled only while the tab is open: the frontend asks once a second and calls
// `stats_stop` when it closes, which drops everything this holds.
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub cpu: f32,
    pub cpu_name: String,
    pub cores: usize,
    pub gpu: Option<f32>,
    pub gpu_name: String,
    /// Dedicated video memory, bytes.
    pub vram_used: Option<u64>,
    pub vram_total: Option<u64>,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    /// Bytes per second since the last sample.
    pub down: u64,
    pub up: u64,
    pub uptime: u64,
    pub top: Vec<App>,
}

#[derive(Serialize)]
pub struct App {
    pub name: String,
    pub memory: u64,
}

struct Sampler {
    sys: System,
    networks: Networks,
    last: Instant,
    gpu: Option<gpu::Gpu>,
}

static SAMPLER: Mutex<Option<Sampler>> = Mutex::new(None);
const TOP: usize = 4;

#[tauri::command]
pub async fn stats_read() -> Stats {
    tauri::async_runtime::spawn_blocking(read).await.unwrap_or_default()
}

#[tauri::command]
pub fn stats_stop() {
    SAMPLER.lock().unwrap().take();
}

fn read() -> Stats {
    let mut guard = SAMPLER.lock().unwrap();
    let s = guard.get_or_insert_with(|| Sampler {
        sys: System::new_with_specifics(RefreshKind::nothing()),
        networks: Networks::new_with_refreshed_list(),
        last: Instant::now(),
        gpu: gpu::Gpu::open(),
    });

    s.sys.refresh_cpu_usage();
    s.sys.refresh_memory();
    s.sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_memory());
    s.networks.refresh(true);
    let secs = s.last.elapsed().as_secs_f64().max(0.001);
    s.last = Instant::now();

    // Apps by memory, with their processes added together (browsers run dozens).
    let mut by_name: HashMap<String, u64> = HashMap::new();
    for p in s.sys.processes().values() {
        let name = p.name().to_string_lossy().trim_end_matches(".exe").to_string();
        *by_name.entry(name).or_default() += p.memory();
    }
    let mut top: Vec<App> = by_name.into_iter().map(|(name, memory)| App { name, memory }).collect();
    top.sort_by(|a, b| b.memory.cmp(&a.memory));
    top.truncate(TOP);

    // The system drive.
    let disks = Disks::new_with_refreshed_list();
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let disk = disks
        .iter()
        .find(|d| d.mount_point().to_string_lossy().starts_with(&system_drive))
        .or_else(|| disks.iter().next());

    let (down, up) = s
        .networks
        .values()
        .fold((0, 0), |(d, u), n| (d + n.received(), u + n.transmitted()));

    let gpu = s.gpu.as_mut().map(|g| g.sample());

    Stats {
        cpu: s.sys.global_cpu_usage(),
        cpu_name: s.sys.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
        cores: s.sys.cpus().len(),
        gpu: gpu.as_ref().map(|g| g.load),
        gpu_name: s.gpu.as_ref().map(|g| g.name.clone()).unwrap_or_default(),
        vram_used: gpu.as_ref().map(|g| g.memory),
        vram_total: s.gpu.as_ref().map(|g| g.total).filter(|&t| t > 0),
        ram_used: s.sys.used_memory(),
        ram_total: s.sys.total_memory(),
        disk_used: disk.map(|d| d.total_space() - d.available_space()).unwrap_or(0),
        disk_total: disk.map(|d| d.total_space()).unwrap_or(0),
        down: (down as f64 / secs) as u64,
        up: (up as f64 / secs) as u64,
        uptime: System::uptime(),
        top,
    }
}

#[cfg(target_os = "windows")]
mod gpu {
    use windows::core::w;
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_FMT_LARGE, PDH_HCOUNTER, PDH_HQUERY,
    };

    pub struct Gpu {
        pub name: String,
        pub total: u64,
        query: PDH_HQUERY,
        engines: PDH_HCOUNTER,
        memory: PDH_HCOUNTER,
    }

    pub struct Sample {
        pub load: f32,
        pub memory: u64,
    }

    // Only touched behind the sampler's mutex.
    unsafe impl Send for Gpu {}

    impl Gpu {
        pub fn open() -> Option<Gpu> {
            unsafe {
                // The first real (non-software) adapter: its name and video memory.
                let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
                let mut name = String::new();
                let mut total = 0;
                for i in 0.. {
                    let Ok(adapter) = factory.EnumAdapters1(i) else { break };
                    let Ok(desc) = adapter.GetDesc1() else { continue };
                    if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                        continue;
                    }
                    let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
                    name = String::from_utf16_lossy(&desc.Description[..len]);
                    total = desc.DedicatedVideoMemory as u64;
                    break;
                }

                // Task Manager's numbers: 3D engine use per process, and dedicated memory per adapter.
                let mut query = PDH_HQUERY::default();
                if PdhOpenQueryW(None, 0, &mut query) != 0 {
                    return None;
                }
                let mut engines = PDH_HCOUNTER::default();
                let mut memory = PDH_HCOUNTER::default();
                PdhAddEnglishCounterW(query, w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage"), 0, &mut engines);
                PdhAddEnglishCounterW(query, w!("\\GPU Adapter Memory(*)\\Dedicated Usage"), 0, &mut memory);
                // Rates need two readings; the first sample after this one has a real value.
                PdhCollectQueryData(query);
                Some(Gpu { name, total, query, engines, memory })
            }
        }

        pub fn sample(&mut self) -> Sample {
            unsafe { PdhCollectQueryData(self.query) };
            let load = values(self.engines, false).iter().sum::<f64>().min(100.0) as f32;
            let memory = values(self.memory, true).into_iter().fold(0.0, f64::max) as u64;
            Sample { load, memory }
        }
    }

    impl Drop for Gpu {
        fn drop(&mut self) {
            unsafe { PdhCloseQuery(self.query) };
        }
    }

    /// Every instance of a wildcard counter.
    fn values(counter: PDH_HCOUNTER, large: bool) -> Vec<f64> {
        let format = if large { PDH_FMT_LARGE } else { PDH_FMT_DOUBLE };
        unsafe {
            let mut size = 0u32;
            let mut count = 0u32;
            PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, None);
            if size == 0 {
                return Vec::new();
            }
            // u64s keep the buffer aligned for the items at its start.
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, Some(items)) != 0 {
                return Vec::new();
            }
            std::slice::from_raw_parts(items, count as usize)
                .iter()
                .map(|i| if large { i.FmtValue.Anonymous.largeValue as f64 } else { i.FmtValue.Anonymous.doubleValue })
                .collect()
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod gpu {
    pub struct Gpu {
        pub name: String,
        pub total: u64,
    }
    pub struct Sample {
        pub load: f32,
        pub memory: u64,
    }
    impl Gpu {
        pub fn open() -> Option<Gpu> {
            None
        }
        pub fn sample(&mut self) -> Sample {
            Sample { load: 0.0, memory: 0 }
        }
    }
}
