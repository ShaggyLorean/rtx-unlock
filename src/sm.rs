use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::download::fetch;
use crate::fg::FgStatus;
use crate::game::{GameInfo, Load, ProxySlot, SM_LABEL};
use crate::gpu::{Gpu, GpuClass};
use crate::sources::{sm_source, SM_PROXIES, SM_REPO, SM_TESTED_DRIVER, SM_VERSION};

pub const MANIFEST_NAME: &str = "rtx-unlock-smooth-motion.json";
pub const PRIVATE_DIR: &str = "nvp_private";
pub const LOG_DIR: &str = "logs";
pub const LOG_PREFIX: &str = "sm86_proxy_";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub tool_version: String,
    pub source_version: String,
    pub proxy: String,
    pub files: Vec<String>,
    pub date_unix: u64,
}

impl Manifest {
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(dir.join(MANIFEST_NAME), text).map_err(|e| format!("cannot write manifest: {e}"))
    }

    pub fn load(dir: &Path) -> Option<Manifest> {
        let text = fs::read_to_string(dir.join(MANIFEST_NAME)).ok()?;
        serde_json::from_str(&text).ok()
    }
}

#[derive(Debug, Clone)]
pub enum SmStatus {
    NotInstalled,
    Installed(Manifest),
    Manual(String),
}

impl SmStatus {
    pub fn label(&self) -> String {
        match self {
            SmStatus::NotInstalled => "not installed".into(),
            SmStatus::Installed(m) => format!("installed as {}, build {}", m.proxy, m.source_version),
            SmStatus::Manual(n) => format!("installed by hand as {n}, remove it by hand first"),
        }
    }

    pub fn present(&self) -> bool {
        !matches!(self, SmStatus::NotInstalled)
    }
}

pub fn status(info: &GameInfo) -> SmStatus {
    if let Some(m) = Manifest::load(&info.proxy_dir) {
        return SmStatus::Installed(m);
    }
    if let Some(s) = info.slots.iter().find(|s| s.owner.as_deref() == Some(SM_LABEL)) {
        return SmStatus::Manual(s.name.to_string());
    }
    if let Some((n, _)) = info.present.iter().find(|(_, o)| o == SM_LABEL) {
        return SmStatus::Manual(n.clone());
    }
    SmStatus::NotInstalled
}

pub fn choose_proxy(slots: &[ProxySlot]) -> Option<&'static str> {
    SM_PROXIES.iter().copied().find(|name| {
        slots
            .iter()
            .any(|s| s.name == *name && s.free() && s.load == Load::Import)
    })
}

pub fn gpu_block(gpu: &Gpu) -> Option<String> {
    match gpu.class {
        GpuClass::Sm86 => None,
        GpuClass::Sm75 => Some("Smooth Motion runs on RTX 30 (Ampere) only; RTX 20 cannot run its kernels".into()),
        GpuClass::NativeFg => Some("RTX 40/50: enable Smooth Motion in the NVIDIA App instead".into()),
        GpuClass::Unsupported => Some(format!("unsupported GPU: {}", gpu.name)),
    }
}

pub fn driver_note(gpu: &Gpu) -> Option<String> {
    (gpu.driver.trim() != SM_TESTED_DRIVER).then(|| {
        format!(
            "tested on driver {SM_TESTED_DRIVER}; on {} it may stay inactive, Check reads the proxy log after a run",
            gpu.driver.trim()
        )
    })
}

pub fn no_proxy_message(slots: &[ProxySlot]) -> String {
    let imported: Vec<&str> = slots
        .iter()
        .filter(|s| s.fits_sm() && s.load == Load::Import)
        .map(|s| s.name)
        .collect();
    let taken: Vec<String> = slots
        .iter()
        .filter(|s| s.fits_sm())
        .filter_map(|s| s.owner.as_ref().map(|o| format!("{} ({o})", s.name)))
        .collect();
    format!(
        "no usable name for Smooth Motion. It must be imported at startup by the executable, and none of [{}] is. \
         Imported and usable names: [{}], already taken: [{}].",
        SM_PROXIES.join(", "),
        imported.join(", "),
        taken.join(", ")
    )
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn install(info: &GameInfo, gpu: &Gpu, log: &dyn Fn(String)) -> Result<(), String> {
    if let Some(e) = gpu_block(gpu) {
        return Err(e);
    }
    match crate::fg::status(info) {
        FgStatus::NotInstalled => {}
        _ => return Err("the DLSS-G FG unlock is installed in this game; remove it first, one frame generator at a time".into()),
    }
    match status(info) {
        SmStatus::Installed(_) => return Err("Smooth Motion is already installed; use Remove first".into()),
        SmStatus::Manual(n) => return Err(format!("a hand-made Smooth Motion proxy exists ({n}); remove it by hand first")),
        SmStatus::NotInstalled => {}
    }
    let Some(proxy) = choose_proxy(&info.slots) else {
        return Err(no_proxy_message(&info.slots));
    };
    let src = sm_source(proxy).ok_or_else(|| format!("no source for {proxy}"))?;
    log(format!(
        "GPU: {} (driver {}), Smooth Motion proxy: {proxy}, build {SM_VERSION} from {SM_REPO}",
        gpu.name, gpu.driver
    ));
    if let Some(n) = driver_note(gpu) {
        log(n);
    }
    let cached = fetch(src.url, src.sha256, log)?;
    let dst = info.proxy_dir.join(proxy);
    fs::copy(&cached, &dst).map_err(|e| format!("cannot write {}: {e}", dst.display()))?;
    log(format!("wrote: {}", dst.display()));
    let m = Manifest {
        tool_version: env!("CARGO_PKG_VERSION").into(),
        source_version: SM_VERSION.into(),
        proxy: proxy.into(),
        files: vec![dst.to_string_lossy().into_owned()],
        date_unix: now_unix(),
    };
    if let Err(e) = m.save(&info.proxy_dir) {
        let _ = fs::remove_file(&dst);
        return Err(e);
    }
    log("Smooth Motion installed. Launch the game; the proxy writes logs\\sm86_proxy_<pid>.log next to the executable and Check reads it.".into());
    Ok(())
}

fn remove_runtime_files(dir: &Path, log: &dyn Fn(String)) {
    let private = dir.join(PRIVATE_DIR);
    if private.is_dir() {
        match fs::remove_dir_all(&private) {
            Ok(()) => log(format!("deleted: {}", private.display())),
            Err(e) => log(format!("could not delete {}: {e}", private.display())),
        }
    }
    let logs = dir.join(LOG_DIR);
    if let Ok(rd) = fs::read_dir(&logs) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(LOG_PREFIX) && name.ends_with(".log") {
                let _ = fs::remove_file(e.path());
            }
        }
        if fs::remove_dir(&logs).is_ok() {
            log(format!("deleted: {}", logs.display()));
        }
    }
}

pub fn remove(info: &GameInfo, log: &dyn Fn(String)) -> Result<(), String> {
    let Some(m) = Manifest::load(&info.proxy_dir) else {
        return Err("no manifest: this Smooth Motion proxy was not installed by rtx-unlock, remove it by hand".into());
    };
    let root = info.root.canonicalize().unwrap_or(info.root.clone());
    for f in &m.files {
        let p = PathBuf::from(f);
        let inside = p.canonicalize().map(|c| c.starts_with(&root)).unwrap_or(false);
        if !inside {
            log(format!("skipped (outside the game folder): {f}"));
            continue;
        }
        match fs::remove_file(&p) {
            Ok(()) => log(format!("deleted: {f}")),
            Err(e) => log(format!("could not delete {f}: {e}")),
        }
    }
    remove_runtime_files(&info.proxy_dir, log);
    fs::remove_file(info.proxy_dir.join(MANIFEST_NAME)).map_err(|e| format!("cannot delete manifest: {e}"))?;
    log("Smooth Motion removed.".into());
    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunFacts {
    pub file: String,
    pub attached: bool,
    pub nvp_init: bool,
    pub wrapped: bool,
    pub passthrough: bool,
    pub load_failures: usize,
    pub presents: u64,
    pub graphs: u64,
}

impl RunFacts {
    pub fn summary(&self) -> String {
        if !self.attached {
            return format!("{}: the proxy did not start", self.file);
        }
        if self.wrapped && self.graphs > 0 {
            let mut s = format!(
                "{}: working, the swapchain was wrapped and {} frames were generated ({} presents)",
                self.file, self.graphs, self.presents
            );
            if self.load_failures > 0 {
                s.push_str(&format!("; {} CUDA module loads failed", self.load_failures));
            }
            return s;
        }
        if !self.nvp_init {
            return format!("{}: NvPresent64 did not initialise, the driver build is probably not supported", self.file);
        }
        if self.load_failures > 0 {
            return format!("{}: {} CUDA module loads failed, no frames generated", self.file, self.load_failures);
        }
        if self.passthrough && !self.wrapped {
            return format!(
                "{}: the game's swapchain was not wrapped (passthrough); the game may create its device before the proxy loads",
                self.file
            );
        }
        format!("{}: the proxy started but no frame was generated yet", self.file)
    }
}

fn number_after(line: &str, key: &str) -> Option<u64> {
    let i = line.find(key)? + key.len();
    let digits: String = line[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

pub fn parse_log(file: &str, text: &str) -> RunFacts {
    let mut f = RunFacts { file: file.to_string(), ..Default::default() };
    for line in text.lines() {
        if line.contains("[ATTACH]") {
            f.attached = true;
        }
        if line.contains("NVP_Init_D3D() -> TRUE") {
            f.nvp_init = true;
        }
        if line.contains("verified as NvPresent proxy") {
            f.wrapped = true;
        }
        if line.contains("passthrough active") {
            f.passthrough = true;
        }
        if line.contains("LOAD FAILED") {
            f.load_failures += 1;
        }
        if line.contains("HookedPresent #") {
            if let Some(n) = number_after(line, "HookedPresent #") {
                f.presents = f.presents.max(n);
            }
            if let Some(g) = number_after(line, "graphs=") {
                f.graphs = f.graphs.max(g);
            }
        }
    }
    f
}

pub fn read_last_run(proxy_dir: &Path) -> Option<RunFacts> {
    let rd = fs::read_dir(proxy_dir.join(LOG_DIR)).ok()?;
    let newest = rd
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with(LOG_PREFIX) && n.ends_with(".log")
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
    let bytes = fs::read(newest.path()).ok()?;
    Some(parse_log(
        &newest.file_name().to_string_lossy(),
        &String::from_utf8_lossy(&bytes),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(name: &'static str, load: Load, owner: Option<&str>) -> ProxySlot {
        ProxySlot { name, load, known_dll: false, owner: owner.map(str::to_string) }
    }

    #[test]
    fn choice_needs_a_startup_import() {
        let mut slots = vec![
            slot("version.dll", Load::Import, None),
            slot("winmm.dll", Load::Import, None),
            slot("dbghelp.dll", Load::DelayLoad, None),
            slot("dsound.dll", Load::Import, None),
            slot("dxgi.dll", Load::Import, None),
        ];
        assert_eq!(choose_proxy(&slots), Some("winmm.dll"));
        slots[1].owner = Some("OptiScaler".into());
        assert_eq!(choose_proxy(&slots), Some("version.dll"));
        slots[0].owner = Some("dlssg_sm86 (FG unlock)".into());
        assert_eq!(choose_proxy(&slots), Some("dsound.dll"));
        slots[3].known_dll = true;
        assert_eq!(choose_proxy(&slots), None);
        assert!(no_proxy_message(&slots).contains("winmm.dll (OptiScaler)"));
    }

    #[test]
    fn gpu_gate_and_driver_note() {
        let g = |name: &str, driver: &str| Gpu {
            name: name.into(),
            driver: driver.into(),
            class: crate::gpu::classify(name),
        };
        assert!(gpu_block(&g("NVIDIA GeForce RTX 3090", SM_TESTED_DRIVER)).is_none());
        assert!(gpu_block(&g("NVIDIA GeForce RTX 2080", SM_TESTED_DRIVER)).is_some());
        assert!(gpu_block(&g("NVIDIA GeForce RTX 4090", SM_TESTED_DRIVER)).unwrap().contains("NVIDIA App"));
        assert!(driver_note(&g("NVIDIA GeForce RTX 3090", SM_TESTED_DRIVER)).is_none());
        assert!(driver_note(&g("NVIDIA GeForce RTX 3090", "617.10")).unwrap().contains("617.10"));
    }

    #[test]
    fn log_facts() {
        let working = "[1] [MILESTONE 1/14] [ATTACH] DllMain\n\
            [2] NVP_Init_D3D() -> TRUE\n\
            [3] SwapChain 1 is native DXGI or interposer (vtable=2), passthrough active\n\
            [4] SwapChain 3 verified as NvPresent proxy (wrapper @ 4)\n\
            [5] HookedPresent #120: sync=0 -> hr=0 (graphs=60)\n\
            [6] HookedPresent #240: sync=0 -> hr=0 (graphs=120)\n";
        let f = parse_log("a.log", working);
        assert!(f.attached && f.nvp_init && f.wrapped && f.passthrough);
        assert_eq!((f.presents, f.graphs, f.load_failures), (240, 120, 0));
        assert!(f.summary().contains("working"));
        let stuck = "[ATTACH]\nNVP_Init_D3D() -> TRUE\npassthrough active\nHookedPresent #9: (graphs=0)\n";
        assert!(parse_log("b.log", stuck).summary().contains("passthrough"));
        let broken = "[ATTACH]\nNVP_Init_D3D() -> TRUE\nrc=300 (?)  <-- LOAD FAILED\n";
        assert!(parse_log("c.log", broken).summary().contains("1 CUDA module loads failed"));
        assert!(parse_log("d.log", "").summary().contains("did not start"));
    }

    #[test]
    fn remove_cleans_runtime_files_and_manifest() {
        let dir = std::env::temp_dir().join(format!("rtxu-sm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(PRIVATE_DIR)).unwrap();
        fs::create_dir_all(dir.join(LOG_DIR)).unwrap();
        fs::write(dir.join("game.exe"), b"").unwrap();
        fs::write(dir.join("winmm.dll"), b"proxy").unwrap();
        fs::write(dir.join(PRIVATE_DIR).join("NvPresent64.dll"), b"x").unwrap();
        fs::write(dir.join(LOG_DIR).join("sm86_proxy_1.log"), b"x").unwrap();
        let m = Manifest {
            tool_version: "0.6.0".into(),
            source_version: SM_VERSION.into(),
            proxy: "winmm.dll".into(),
            files: vec![dir.join("winmm.dll").to_string_lossy().into_owned()],
            date_unix: 1,
        };
        m.save(&dir).unwrap();
        let info = GameInfo {
            root: dir.clone(),
            exe: dir.join("game.exe"),
            proxy_dir: dir.clone(),
            engine: crate::game::Engine::Unknown,
            slots: Vec::new(),
            present: Vec::new(),
            dlssg: false,
            anticheat: None,
        };
        assert!(matches!(status(&info), SmStatus::Installed(_)));
        remove(&info, &|_| {}).unwrap();
        assert!(!dir.join("winmm.dll").exists());
        assert!(!dir.join(PRIVATE_DIR).exists());
        assert!(!dir.join(LOG_DIR).exists());
        assert!(!dir.join(MANIFEST_NAME).exists());
        assert!(matches!(status(&info), SmStatus::NotInstalled));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore]
    fn e2e_install_remove() {
        let Ok(dir) = std::env::var("RTXU_E2E_GAME") else {
            eprintln!("RTXU_E2E_GAME not set, skipped");
            return;
        };
        let log = |s: String| eprintln!("  {s}");
        let root = Path::new(&dir);
        let gpu = crate::gpu::detect().unwrap();
        let info = crate::game::analyze(root).unwrap();
        assert!(matches!(status(&info), SmStatus::NotInstalled));
        install(&info, &gpu, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        let SmStatus::Installed(m) = status(&info) else { panic!("not reported as installed") };
        let slot = info.slots.iter().find(|s| s.name == m.proxy).unwrap();
        assert_eq!(slot.owner.as_deref(), Some(SM_LABEL));
        remove(&info, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        assert!(matches!(status(&info), SmStatus::NotInstalled));
        assert!(!info.proxy_dir.join(&m.proxy).exists());
    }
}
