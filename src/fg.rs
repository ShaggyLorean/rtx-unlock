use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::download::fetch;
use crate::game::{Engine, GameInfo, Load, ProxySlot};
use crate::gpu::Gpu;
use crate::sources::{fg_source, Runtime, SDLI_COMMIT};

pub const MANIFEST_NAME: &str = "rtx-unlock.json";
pub const INI_NAME: &str = "dlssg_sm86.ini";
pub const LOG_DIR: &str = "dlssg_sm86";

pub fn choose_proxy(slots: &[ProxySlot], runtime: Runtime) -> Option<&'static str> {
    let allowed = runtime.proxies();
    let pick = |family: bool, load: Load| {
        slots
            .iter()
            .find(|s| allowed.contains(&s.name) && s.free() && s.is_family() == family && s.load == load)
            .map(|s| s.name)
    };
    pick(true, Load::Import)
        .or_else(|| pick(false, Load::Import))
        .or_else(|| pick(true, Load::DelayLoad))
        .or_else(|| pick(false, Load::DelayLoad))
}

pub fn clamp_frames(runtime: Runtime, frames: u32) -> u32 {
    frames.clamp(1, runtime.max_generated_frames())
}

pub fn make_ini(runtime: Runtime, max_frames: u32) -> String {
    let frames = clamp_frames(runtime, max_frames);
    match runtime {
        Runtime::Legacy010 => format!(
            "; Written by rtx-unlock for dlssg_for_sm86 0.1.0. Restart the game after changing this file.\n\
             [General]\n\
             Enabled=1\n\
             \n\
             [FrameGeneration]\n\
             MaxGeneratedFrames={frames}\n\
             \n\
             [Logging]\n\
             Level=1\n\
             File=1\n\
             DebugOutput=0\n\
             EvaluateEvery=120\n\
             Directory=dlssg_sm86\\logs\n\
             \n\
             [Compatibility]\n\
             KernelImage=Auto\n\
             ForceSM86Route=0\n\
             SimulateAmpere=0\n\
             \n\
             [Runtime]\n\
             Mode=Bundled\n\
             CacheDirectory=\n\
             Path=\n"
        ),
        _ => format!(
            "; Written by rtx-unlock for dlssg_for_sm86 0.3.5 ({}). Restart the game after changing this file.\n\
             [General]\n\
             Enabled=1\n\
             \n\
             [FrameGeneration]\n\
             Optimized=1\n\
             MaxGeneratedFrames={frames}\n\
             \n\
             [Compatibility]\n\
             Preset=Auto\n\
             \n\
             [Logging]\n\
             Level=1\n\
             Directory=dlssg_sm86\\logs\n\
             \n\
             [Runtime]\n\
             Mode=Bundled\n\
             CacheDirectory=\n",
            runtime.tag()
        ),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub tool_version: String,
    pub source_version: String,
    pub source_commit: String,
    pub proxy: String,
    pub router: String,
    #[serde(default)]
    pub legacy: bool,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub max_frames: u32,
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

    pub fn build_label(&self) -> String {
        if !self.runtime.is_empty() {
            return format!("{} ({})", self.source_version, self.runtime);
        }
        if self.legacy {
            return "0.1.0".into();
        }
        self.source_version.clone()
    }
}

#[derive(Debug, Clone)]
pub enum FgStatus {
    NotInstalled,
    Installed(Manifest),
    Manual(PathBuf),
}

impl FgStatus {
    pub fn label(&self) -> String {
        match self {
            FgStatus::NotInstalled => "not installed".into(),
            FgStatus::Installed(m) => {
                let frames = if m.max_frames > 0 { format!(", up to {}X", m.max_frames + 1) } else { String::new() };
                format!("installed as {}, build {}{frames}", m.proxy, m.build_label())
            }
            FgStatus::Manual(p) => format!("installed by hand ({}), remove it by hand first", p.display()),
        }
    }
}

fn reframework_plugins(root: &Path) -> PathBuf {
    root.join("reframework").join("plugins")
}

fn ensure_reframework(root: &Path, log: &dyn Fn(String)) -> Result<Vec<PathBuf>, String> {
    use std::io::Read;
    let plugins = reframework_plugins(root);
    let dinput = root.join("dinput8.dll");
    if dinput.exists() {
        if plugins.is_dir() {
            return Ok(Vec::new());
        }
        let owner = crate::winutil::original_filename(&dinput).unwrap_or_else(|| "unknown".into());
        if !owner.to_ascii_lowercase().contains("reframework") && owner != "dinput8.dll" {
            return Err(format!(
                "dinput8.dll already exists ({owner}) but reframework\\plugins does not; install REFramework by hand"
            ));
        }
        log("dinput8.dll present, treating it as REFramework; creating the plugins folder".into());
        fs::create_dir_all(&plugins).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }
    log("RE Engine: REFramework missing, fetching the nightly build".into());
    let api = format!(
        "https://api.github.com/repos/{}/releases/latest",
        crate::sources::REFRAMEWORK_REPO
    );
    let json = crate::download::fetch_text(&api)?;
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("bad release JSON: {e}"))?;
    let url = v["assets"]
        .as_array()
        .and_then(|a| {
            a.iter().find_map(|x| {
                (x["name"].as_str() == Some(crate::sources::REFRAMEWORK_ASSET))
                    .then(|| x["browser_download_url"].as_str().map(str::to_string))
                    .flatten()
            })
        })
        .ok_or("REFramework.zip asset not found")?;
    let zip_path = crate::download::fetch_unpinned(&url, log)?;
    let file = fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("cannot open zip: {e}"))?;
    let mut written = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name().eq_ignore_ascii_case("dinput8.dll") {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            fs::write(&dinput, bytes).map_err(|e| format!("cannot write dinput8.dll: {e}"))?;
            written.push(dinput.clone());
            log(format!("wrote: {} (REFramework)", dinput.display()));
        }
    }
    if written.is_empty() {
        return Err("zip contains no dinput8.dll".into());
    }
    fs::create_dir_all(&plugins).map_err(|e| e.to_string())?;
    Ok(written)
}

pub fn status(info: &GameInfo) -> FgStatus {
    if let Some(m) = Manifest::load(&info.proxy_dir) {
        return FgStatus::Installed(m);
    }
    for dir in [info.proxy_dir.clone(), reframework_plugins(&info.root)] {
        let ini = dir.join(INI_NAME);
        if ini.exists() {
            return FgStatus::Manual(ini);
        }
    }
    FgStatus::NotInstalled
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn slot_report(slots: &[ProxySlot]) -> Vec<String> {
    slots
        .iter()
        .map(|s| {
            let owner = s.owner.as_deref().map(|o| format!(", in folder: {o}")).unwrap_or_default();
            format!("{:12} {}{owner}: {}", s.name, s.load.label(), s.verdict())
        })
        .collect()
}

pub fn no_proxy_message(slots: &[ProxySlot], runtime: Runtime) -> String {
    let taken: Vec<String> = slots
        .iter()
        .filter_map(|s| s.owner.as_ref().map(|o| format!("{} ({o})", s.name)))
        .collect();
    let loaded: Vec<&str> = slots.iter().filter(|s| s.loads()).map(|s| s.name).collect();
    let mut msg = format!(
        "no usable proxy name for build {}. Names this executable loads: [{}], already taken: [{}].",
        runtime.label(),
        loaded.join(", "),
        taken.join(", ")
    );
    if runtime == Runtime::Legacy010 {
        msg.push_str(" The 0.1.0 build exists only as version.dll; pick a 0.3.5 build for other names.");
    } else {
        msg.push_str(" Workaround: install Ultimate ASI Loader and add version.dll as an .asi by hand.");
    }
    msg
}

pub fn install(
    info: &GameInfo,
    gpu: &Gpu,
    runtime: Runtime,
    max_frames: u32,
    log: &dyn Fn(String),
) -> Result<(), String> {
    let router = gpu.class.router().ok_or_else(|| match gpu.class {
        crate::gpu::GpuClass::NativeFg => {
            "RTX 40/50: no unlock needed, the game's own DLSS Frame Generation already runs".to_string()
        }
        _ => format!("unsupported GPU: {}", gpu.name),
    })?;
    if !info.dlssg {
        return Err(
            "the game ships no DLSS Frame Generation DLL (sl.dlss_g.dll / nvngx_dlssg.dll); there is nothing to unlock"
                .into(),
        );
    }
    match status(info) {
        FgStatus::Installed(_) => return Err("FG unlock is already installed; use Remove first".into()),
        FgStatus::Manual(p) => {
            return Err(format!("a hand-made dlssg_sm86 install exists ({}); remove it by hand first", p.display()))
        }
        FgStatus::NotInstalled => {}
    }
    for line in slot_report(&info.slots) {
        log(line);
    }

    let mut written: Vec<PathBuf> = Vec::new();
    let chosen = choose_proxy(&info.slots, runtime);
    let (target_dir, dll_name, source_name) = match (chosen, info.engine) {
        (Some(proxy), _) => (info.proxy_dir.clone(), proxy.to_string(), proxy),
        (None, Engine::ReEngine) => {
            log("no proxy name is free; RE Engine fallback through REFramework plugins".into());
            written.extend(ensure_reframework(&info.root, log)?);
            (reframework_plugins(&info.root), "dlssg_sm86.dll".to_string(), "version.dll")
        }
        (None, _) => return Err(no_proxy_message(&info.slots, runtime)),
    };
    let src = fg_source(source_name, runtime).ok_or_else(|| format!("no source for {source_name}"))?;
    let frames = clamp_frames(runtime, max_frames);
    log(format!(
        "GPU: {} ({router}), proxy: {dll_name}, build {} from {} at {}, up to {}X",
        gpu.name,
        runtime.label(),
        crate::sources::SDLI_REPO,
        &SDLI_COMMIT[..12],
        frames + 1
    ));

    let cached = fetch(src.url, src.sha256, log)?;

    let result = (|| -> Result<(), String> {
        let dll_dst = target_dir.join(&dll_name);
        fs::copy(&cached, &dll_dst).map_err(|e| format!("cannot write {}: {e}", dll_dst.display()))?;
        written.push(dll_dst.clone());
        log(format!("wrote: {}", dll_dst.display()));
        let ini_dst = target_dir.join(INI_NAME);
        fs::write(&ini_dst, make_ini(runtime, frames)).map_err(|e| format!("cannot write ini: {e}"))?;
        written.push(ini_dst.clone());
        log(format!("wrote: {}", ini_dst.display()));
        let m = Manifest {
            tool_version: env!("CARGO_PKG_VERSION").into(),
            source_version: runtime.source_version().into(),
            source_commit: SDLI_COMMIT.into(),
            proxy: dll_name.clone(),
            router: router.into(),
            legacy: runtime == Runtime::Legacy010,
            runtime: runtime.tag().into(),
            max_frames: frames,
            files: written.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
            date_unix: now_unix(),
        };
        m.save(&info.proxy_dir)?;
        Ok(())
    })();
    if let Err(e) = result {
        for p in &written {
            let _ = fs::remove_file(p);
        }
        let _ = fs::remove_file(info.proxy_dir.join(MANIFEST_NAME));
        return Err(e);
    }
    log("FG unlock installed. Launch the game; DLSS Frame Generation should appear in its settings.".into());
    if target_dir != info.proxy_dir {
        log("RE Engine: the proxy lives in reframework\\plugins and REFramework loads it at startup.".into());
    }
    Ok(())
}

fn remove_logs(dir: &Path, log: &dyn Fn(String)) {
    let logs = dir.join(LOG_DIR).join("logs");
    if logs.is_dir() {
        match fs::remove_dir_all(&logs) {
            Ok(()) => log(format!("deleted: {}", logs.display())),
            Err(e) => log(format!("could not delete {}: {e}", logs.display())),
        }
    }
    let _ = fs::remove_dir(dir.join(LOG_DIR));
}

pub fn remove(info: &GameInfo, log: &dyn Fn(String)) -> Result<(), String> {
    let Some(m) = Manifest::load(&info.proxy_dir) else {
        return Err("no manifest: this FG unlock was not installed by rtx-unlock, remove it by hand".into());
    };
    let root = info.root.canonicalize().unwrap_or(info.root.clone());
    let mut dirs: Vec<PathBuf> = Vec::new();
    for f in &m.files {
        let p = PathBuf::from(f);
        let inside = p
            .canonicalize()
            .map(|c| c.starts_with(&root))
            .unwrap_or(false);
        if !inside {
            log(format!("skipped (outside the game folder): {f}"));
            continue;
        }
        if let Some(d) = p.parent() {
            if !dirs.iter().any(|x| x == d) {
                dirs.push(d.to_path_buf());
            }
        }
        match fs::remove_file(&p) {
            Ok(()) => log(format!("deleted: {f}")),
            Err(e) => log(format!("could not delete {f}: {e}")),
        }
    }
    for d in &dirs {
        remove_logs(d, log);
    }
    fs::remove_file(info.proxy_dir.join(MANIFEST_NAME)).map_err(|e| format!("cannot delete manifest: {e}"))?;
    log("FG unlock removed.".into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(name: &'static str, load: Load, owner: Option<&str>, known: bool) -> ProxySlot {
        ProxySlot { name, load, known_dll: known, owner: owner.map(str::to_string) }
    }

    #[test]
    fn proxy_choice_prefers_certain_loads_then_family_names() {
        let slots = vec![
            slot("version.dll", Load::Import, None, false),
            slot("winmm.dll", Load::Import, None, false),
            slot("dbghelp.dll", Load::DelayLoad, None, false),
            slot("dinput8.dll", Load::Never, None, false),
            slot("dxgi.dll", Load::Import, None, false),
            slot("d3d12.dll", Load::DelayLoad, None, false),
        ];
        assert_eq!(choose_proxy(&slots, Runtime::Dlssg3109), Some("version.dll"));
        let mut taken = slots.clone();
        taken[0].owner = Some("ReShade".into());
        assert_eq!(choose_proxy(&taken, Runtime::Dlssg3109), Some("winmm.dll"));
        taken[1].owner = Some("OptiScaler".into());
        assert_eq!(choose_proxy(&taken, Runtime::Dlssg3109), Some("dxgi.dll"));
        taken[4].owner = Some("ReShade".into());
        assert_eq!(choose_proxy(&taken, Runtime::Dlssg3109), Some("dbghelp.dll"));
        taken[2].known_dll = true;
        assert_eq!(choose_proxy(&taken, Runtime::Dlssg3109), Some("d3d12.dll"));
        taken[5].load = Load::Reference;
        assert_eq!(choose_proxy(&taken, Runtime::Dlssg3109), None);
        assert_eq!(choose_proxy(&taken, Runtime::Legacy010), None);
        assert_eq!(choose_proxy(&slots, Runtime::Legacy010), Some("version.dll"));
    }

    #[test]
    fn ini_layouts() {
        let i = make_ini(Runtime::Dlssg3109, 5);
        assert!(i.contains("Optimized=1"));
        assert!(i.contains("MaxGeneratedFrames=5"));
        assert!(i.contains("Mode=Bundled"));
        let i = make_ini(Runtime::Dlssg3101, 5);
        assert!(i.contains("MaxGeneratedFrames=3"));
        let i = make_ini(Runtime::Legacy010, 9);
        assert!(i.contains("MaxGeneratedFrames=3"));
        assert!(i.contains("KernelImage=Auto"));
        assert!(!i.contains("Optimized="));
    }

    #[test]
    fn no_proxy_message_lists_facts() {
        let slots = vec![
            slot("version.dll", Load::Import, Some("ReShade"), false),
            slot("winmm.dll", Load::Never, None, false),
        ];
        let m = no_proxy_message(&slots, Runtime::Dlssg3109);
        assert!(m.contains("version.dll (ReShade)"));
        assert!(m.contains("[version.dll]"));
        assert!(m.contains("ASI Loader"));
        assert!(no_proxy_message(&slots, Runtime::Legacy010).contains("only as version.dll"));
    }

    #[test]
    #[ignore]
    fn e2e_install_remove_install() {
        let Ok(dir) = std::env::var("RTXU_E2E_GAME") else {
            eprintln!("RTXU_E2E_GAME not set, skipped");
            return;
        };
        let log = |s: String| eprintln!("  {s}");
        let root = Path::new(&dir);
        let gpu = crate::gpu::detect().unwrap();
        let mut info = crate::game::analyze(root).unwrap();
        if let FgStatus::Manual(ini) = status(&info) {
            let dll = ini.with_file_name("version.dll");
            let bytes = fs::read(&dll).expect("hand-made version.dll missing");
            let known: Vec<&str> = Runtime::ALL
                .iter()
                .filter_map(|r| crate::sources::fg_source("version.dll", *r))
                .map(|s| s.sha256)
                .collect();
            assert!(known.contains(&crate::download::sha256_hex(&bytes).as_str()), "unknown version.dll, left untouched");
            fs::remove_file(&dll).unwrap();
            fs::remove_file(&ini).unwrap();
            info = crate::game::analyze(root).unwrap();
        }
        if let FgStatus::Installed(_) = status(&info) {
            remove(&info, &log).unwrap();
            info = crate::game::analyze(root).unwrap();
        }
        assert!(matches!(status(&info), FgStatus::NotInstalled));
        install(&info, &gpu, Runtime::Dlssg3109, 3, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        let FgStatus::Installed(m) = status(&info) else { panic!("not reported as installed") };
        assert!(m.files.len() >= 2);
        assert_eq!(m.runtime, "310.9");
        for f in &m.files {
            assert!(Path::new(f).is_file(), "missing: {f}");
        }
        remove(&info, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        assert!(matches!(status(&info), FgStatus::NotInstalled));
        for f in &m.files {
            assert!(!Path::new(f).exists(), "not deleted: {f}");
        }
        install(&info, &gpu, Runtime::Dlssg3109, 3, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        assert!(matches!(status(&info), FgStatus::Installed(_)));
        if std::env::var("RTXU_E2E_CLEAN").as_deref() == Ok("1") {
            remove(&info, &log).unwrap();
            let info = crate::game::analyze(root).unwrap();
            assert!(matches!(status(&info), FgStatus::NotInstalled));
        }
    }

    #[test]
    fn manifest_roundtrip_and_old_layout() {
        let dir = std::env::temp_dir().join(format!("rtxu-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let m = Manifest {
            tool_version: "0.5.0".into(),
            source_version: "0.3.5".into(),
            source_commit: "abc".into(),
            proxy: "version.dll".into(),
            router: "SM86".into(),
            legacy: false,
            runtime: "310.9".into(),
            max_frames: 3,
            files: vec![dir.join("a").to_string_lossy().into()],
            date_unix: 1,
        };
        m.save(&dir).unwrap();
        let back = Manifest::load(&dir).unwrap();
        assert_eq!(back.proxy, "version.dll");
        assert_eq!(back.files.len(), 1);
        assert_eq!(back.build_label(), "0.3.5 (310.9)");
        let old = r#"{"tool_version":"0.4.0","source_version":"0.2.4","source_commit":"x","proxy":"winmm.dll","router":"SM86","legacy":true,"files":[],"date_unix":1}"#;
        fs::write(dir.join(MANIFEST_NAME), old).unwrap();
        let back = Manifest::load(&dir).unwrap();
        assert_eq!(back.build_label(), "0.1.0");
        assert!(FgStatus::Installed(back).label().contains("winmm.dll"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
