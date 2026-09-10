//! DLSS Frame Generation unlock: the sdli1995/dlssg_for_sm86 proxy DLL.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::download::fetch;
use crate::game::{Engine, GameInfo};
use crate::gpu::Gpu;
use crate::sources::{fg_source, SDLI_COMMIT, SDLI_LEGACY_VERSION, SDLI_VERSION};

pub const PROXY_ORDER: [&str; 5] = ["version.dll", "winmm.dll", "dinput8.dll", "winhttp.dll", "dxgi.dll"];
pub const MANIFEST_NAME: &str = "rtx-unlock.json";
pub const INI_NAME: &str = "dlssg_sm86.ini";

/// First proxy name the executable imports that is not already taken in the folder.
pub fn choose_proxy(imports: &[String], present: &[String]) -> Option<&'static str> {
    PROXY_ORDER.iter().copied().find(|p| {
        imports.iter().any(|i| i.eq_ignore_ascii_case(p))
            && !present.iter().any(|e| e.eq_ignore_ascii_case(p))
    })
}

pub fn make_ini(router: &str) -> String {
    format!(
        "; Written by rtx-unlock. Restart the game after changing this file.\n\
         [Compatibility]\n\
         Router={router}\n\
         KernelImage=PTX\n\
         HardwareBilinear=0\n\
         \n\
         [FrameGeneration]\n\
         MaxGeneratedFrames=3\n\
         \n\
         [Logging]\n\
         Level=1\n"
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub tool_version: String,
    pub source_version: String,
    pub source_commit: String,
    pub proxy: String,
    pub router: String,
    pub legacy: bool,
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
pub enum FgStatus {
    NotInstalled,
    Installed(Manifest),
    /// A dlssg_sm86 install this tool did not make (no manifest). Holds the ini path.
    Manual(PathBuf),
}

impl FgStatus {
    pub fn label(&self) -> String {
        match self {
            FgStatus::NotInstalled => "not installed".into(),
            FgStatus::Installed(m) => format!(
                "installed (as {}, version {}, router {})",
                m.proxy, m.source_version, m.router
            ),
            FgStatus::Manual(p) => format!("installed by hand ({}), remove it by hand first", p.display()),
        }
    }
}

fn reframework_plugins(root: &Path) -> PathBuf {
    root.join("reframework").join("plugins")
}

/// Installs the REFramework nightly (monolithic dinput8.dll) when it is missing.
/// Returns the files it wrote.
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
    // No dinput8.dll: REFramework must be installed even if a plugins folder was left behind.
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

pub fn install(info: &GameInfo, gpu: &Gpu, legacy: bool, log: &dyn Fn(String)) -> Result<(), String> {
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

    let mut written: Vec<PathBuf> = Vec::new();
    let (target_dir, dll_name, source_name) = if info.engine == Engine::ReEngine {
        written.extend(ensure_reframework(&info.root, log)?);
        (reframework_plugins(&info.root), "dlssg_sm86.dll".to_string(), "version.dll")
    } else {
        let present: Vec<String> = info.present.iter().map(|(n, _)| n.clone()).collect();
        let Some(proxy) = choose_proxy(&info.imports, &present) else {
            let taken: Vec<String> = info.present.iter().map(|(n, o)| format!("{n} ({o})")).collect();
            let imported: Vec<&str> = PROXY_ORDER
                .iter()
                .copied()
                .filter(|p| info.imports.iter().any(|i| i.eq_ignore_ascii_case(p)))
                .collect();
            return Err(format!(
                "no free proxy name left. Imported by the executable: [{}], already taken: [{}]. \
                 Workaround: install Ultimate ASI Loader and add version.dll as FG.asi by hand.",
                imported.join(", "),
                taken.join(", ")
            ));
        };
        (info.proxy_dir.clone(), proxy.to_string(), proxy)
    };
    if legacy && source_name != "version.dll" {
        return Err(format!(
            "the legacy 0.1.0 build only exists as version.dll; this game needs {dll_name}"
        ));
    }
    let src = fg_source(source_name, legacy).ok_or_else(|| format!("no source for {source_name}"))?;
    let version = if legacy { SDLI_LEGACY_VERSION } else { SDLI_VERSION };
    log(format!(
        "GPU: {} ({router}), proxy: {dll_name}, source {} {version}",
        gpu.name,
        crate::sources::SDLI_REPO
    ));

    let cached = fetch(src.url, src.sha256, log)?;

    let result = (|| -> Result<(), String> {
        let dll_dst = target_dir.join(&dll_name);
        fs::copy(&cached, &dll_dst).map_err(|e| format!("cannot write {}: {e}", dll_dst.display()))?;
        written.push(dll_dst.clone());
        log(format!("wrote: {}", dll_dst.display()));
        let ini_dst = target_dir.join(INI_NAME);
        fs::write(&ini_dst, make_ini(router)).map_err(|e| format!("cannot write ini: {e}"))?;
        written.push(ini_dst.clone());
        log(format!("wrote: {}", ini_dst.display()));
        let m = Manifest {
            tool_version: env!("CARGO_PKG_VERSION").into(),
            source_version: version.into(),
            source_commit: SDLI_COMMIT.into(),
            proxy: dll_name.clone(),
            router: router.into(),
            legacy,
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
    if info.engine == Engine::ReEngine {
        log("RE Engine: the proxy lives in reframework\\plugins and REFramework loads it at startup.".into());
    }
    Ok(())
}

pub fn remove(info: &GameInfo, log: &dyn Fn(String)) -> Result<(), String> {
    let Some(m) = Manifest::load(&info.proxy_dir) else {
        return Err("no manifest: this FG unlock was not installed by rtx-unlock, remove it by hand".into());
    };
    let root = info.root.canonicalize().unwrap_or(info.root.clone());
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
        match fs::remove_file(&p) {
            Ok(()) => log(format!("deleted: {f}")),
            Err(e) => log(format!("could not delete {f}: {e}")),
        }
    }
    fs::remove_file(info.proxy_dir.join(MANIFEST_NAME)).map_err(|e| format!("cannot delete manifest: {e}"))?;
    log("FG unlock removed.".into());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn proxy_choice() {
        assert_eq!(
            choose_proxy(&s(&["kernel32.dll", "version.dll", "dxgi.dll"]), &s(&[])),
            Some("version.dll")
        );
        assert_eq!(
            choose_proxy(&s(&["version.dll", "winmm.dll"]), &s(&["version.dll"])),
            Some("winmm.dll")
        );
        assert_eq!(
            choose_proxy(&s(&["version.dll", "dxgi.dll"]), &s(&["VERSION.DLL", "dxgi.dll"])),
            None
        );
        assert_eq!(choose_proxy(&s(&["xinput1_4.dll"]), &s(&[])), None);
    }

    #[test]
    fn ini_router() {
        let i = make_ini("SM75");
        assert!(i.contains("Router=SM75"));
        assert!(i.contains("KernelImage=PTX"));
        assert!(i.contains("MaxGeneratedFrames=3"));
    }

    /// End-to-end cycle on a real game folder. Runs only with RTXU_E2E_GAME=<folder>.
    /// A hand-made version.dll with a known hash is cleaned up first.
    /// RTXU_E2E_CLEAN=1 removes the install at the end and leaves the folder clean.
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
            let known = crate::sources::fg_source("version.dll", false).unwrap().sha256;
            assert_eq!(crate::download::sha256_hex(&bytes), known, "unknown version.dll, left untouched");
            fs::remove_file(&dll).unwrap();
            fs::remove_file(&ini).unwrap();
            info = crate::game::analyze(root).unwrap();
        }
        if let FgStatus::Installed(_) = status(&info) {
            remove(&info, &log).unwrap();
            info = crate::game::analyze(root).unwrap();
        }
        assert!(matches!(status(&info), FgStatus::NotInstalled));
        install(&info, &gpu, false, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        let FgStatus::Installed(m) = status(&info) else { panic!("not reported as installed") };
        // Unreal and others: dll + ini. RE Engine: plus the REFramework dinput8.dll.
        assert!(m.files.len() >= 2);
        for f in &m.files {
            assert!(Path::new(f).is_file(), "missing: {f}");
        }
        remove(&info, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        assert!(matches!(status(&info), FgStatus::NotInstalled));
        for f in &m.files {
            assert!(!Path::new(f).exists(), "not deleted: {f}");
        }
        install(&info, &gpu, false, &log).unwrap();
        let info = crate::game::analyze(root).unwrap();
        assert!(matches!(status(&info), FgStatus::Installed(_)));
        if std::env::var("RTXU_E2E_CLEAN").as_deref() == Ok("1") {
            remove(&info, &log).unwrap();
            let info = crate::game::analyze(root).unwrap();
            assert!(matches!(status(&info), FgStatus::NotInstalled));
        }
    }

    #[test]
    fn manifest_roundtrip() {
        let dir = std::env::temp_dir().join(format!("rtxu-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let m = Manifest {
            tool_version: "0.1.0".into(),
            source_version: "0.2.4".into(),
            source_commit: "abc".into(),
            proxy: "version.dll".into(),
            router: "SM86".into(),
            legacy: false,
            files: vec![dir.join("a").to_string_lossy().into()],
            date_unix: 1,
        };
        m.save(&dir).unwrap();
        let back = Manifest::load(&dir).unwrap();
        assert_eq!(back.proxy, "version.dll");
        assert_eq!(back.files.len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
