use std::fs;
use std::path::{Path, PathBuf};

use crate::winutil::original_filename;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Unreal,
    ReEngine,
    Unknown,
}

impl Engine {
    pub fn label(self) -> &'static str {
        match self {
            Engine::Unreal => "Unreal Engine",
            Engine::ReEngine => "RE Engine (needs REFramework)",
            Engine::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GameInfo {
    pub root: PathBuf,
    pub exe: PathBuf,
    pub proxy_dir: PathBuf,
    pub engine: Engine,
    pub imports: Vec<String>,
    pub present: Vec<(String, String)>,
    pub dlssg: bool,
    pub anticheat: Option<&'static str>,
}

pub fn detect_anticheat(root: &Path) -> Option<&'static str> {
    if root.join("EasyAntiCheat").is_dir() || root.join("start_protected_game.exe").exists() {
        return Some("Easy Anti-Cheat");
    }
    if root.join("BattlEye").is_dir() {
        return Some("BattlEye");
    }
    None
}

pub const PROXY_NAMES: [&str; 8] = [
    "version.dll",
    "winmm.dll",
    "dinput8.dll",
    "winhttp.dll",
    "dxgi.dll",
    "dwmapi.dll",
    "wininet.dll",
    "d3d12.dll",
];

fn walk(dir: &Path, depth: usize, f: &mut dyn FnMut(&Path) -> bool) -> bool {
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    let mut subdirs = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if f(&p) {
            return true;
        }
    }
    if depth == 0 {
        return false;
    }
    for d in subdirs {
        if walk(&d, depth - 1, f) {
            return true;
        }
    }
    false
}

fn lower_name(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_helper_exe(lower_name: &str) -> bool {
    const HELPERS: [&str; 14] = [
        "crash", "report", "installer", "setup", "launcher", "unins", "redist", "vc_redist",
        "dxsetup", "easyanticheat", "eac", "helper", "prereq", "start_protected_game",
    ];
    HELPERS.iter().any(|h| lower_name.contains(h))
}

pub fn find_exe(root: &Path) -> Result<PathBuf, String> {
    let mut shipping = None;
    let mut in_binaries: Vec<(u64, PathBuf)> = Vec::new();
    walk(root, 4, &mut |p| {
        let n = lower_name(p);
        if !n.ends_with(".exe") || is_helper_exe(&n) {
            return false;
        }
        if n.ends_with("-win64-shipping.exe") {
            shipping = Some(p.to_path_buf());
            return true;
        }
        let dir = p.to_string_lossy().to_ascii_lowercase();
        if dir.contains("\\binaries\\win64\\") {
            let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            in_binaries.push((size, p.to_path_buf()));
        }
        false
    });
    if let Some(p) = shipping {
        return Ok(p);
    }
    if let Some((_, p)) = in_binaries.into_iter().max_by_key(|(s, _)| *s) {
        return Ok(p);
    }
    let mut best: Option<(u64, PathBuf)> = None;
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            let n = lower_name(&p);
            if p.is_file() && n.ends_with(".exe") && !is_helper_exe(&n) {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                if best.as_ref().map_or(true, |(s, _)| size > *s) {
                    best = Some((size, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
        .ok_or_else(|| format!("no executable found in {}", root.display()))
}

pub fn imports(exe: &Path) -> Result<Vec<String>, String> {
    let data = fs::read(exe).map_err(|e| format!("cannot read executable: {e}"))?;
    let mut opts = goblin::pe::options::ParseOptions::default();
    opts.parse_attribute_certificates = false;
    opts.parse_tls_data = false;
    opts.parse_resources = false;
    opts.parse_mode = goblin::options::ParseMode::Permissive;
    let pe = goblin::pe::PE::parse_with_opts(&data, &opts)
        .map_err(|e| format!("cannot parse PE: {e}"))?;
    let mut v: Vec<String> = pe.libraries.iter().map(|s| s.to_ascii_lowercase()).collect();
    v.sort();
    v.dedup();
    Ok(v)
}

pub fn detect_engine(root: &Path, exe: &Path) -> Engine {
    if root.join("re_chunk_000.pak").exists() || root.join("reframework").is_dir() {
        return Engine::ReEngine;
    }
    let s = exe.to_string_lossy().to_ascii_lowercase();
    if s.contains("\\binaries\\win64\\") {
        return Engine::Unreal;
    }
    Engine::Unknown
}

pub fn dlssg_present(root: &Path) -> bool {
    walk(root, 8, &mut |p| {
        let n = lower_name(p);
        n == "sl.dlss_g.dll" || n == "nvngx_dlssg.dll"
    })
}

pub fn label_for(original: Option<&str>, has_sm86_ini: bool) -> String {
    match original {
        Some("proxy.rc") => "UE4SS".into(),
        Some("ReShade64.dll") | Some("ReShade32.dll") => "ReShade".into(),
        Some("OptiScaler.dll") => "OptiScaler".into(),
        Some(o) => o.to_string(),
        None if has_sm86_ini => "dlssg_sm86 (FG unlock)".into(),
        None => "unknown".into(),
    }
}

pub fn present_proxies(proxy_dir: &Path) -> Vec<(String, String)> {
    let has_ini = proxy_dir.join("dlssg_sm86.ini").exists();
    PROXY_NAMES
        .iter()
        .filter_map(|n| {
            let p = proxy_dir.join(n);
            if !p.is_file() {
                return None;
            }
            let orig = original_filename(&p);
            Some((n.to_string(), label_for(orig.as_deref(), has_ini)))
        })
        .collect()
}

pub fn analyze(root: &Path) -> Result<GameInfo, String> {
    if !root.is_dir() {
        return Err(format!("folder does not exist: {}", root.display()));
    }
    let exe = find_exe(root)?;
    let proxy_dir = exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "executable has no parent folder".to_string())?;
    let imports = imports(&exe)?;
    let engine = detect_engine(root, &exe);
    let present = present_proxies(&proxy_dir);
    let dlssg = dlssg_present(root);
    let anticheat = detect_anticheat(root);
    Ok(GameInfo {
        root: root.to_path_buf(),
        exe,
        proxy_dir,
        engine,
        imports,
        present,
        dlssg,
        anticheat,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_labels() {
        assert_eq!(label_for(Some("proxy.rc"), false), "UE4SS");
        assert_eq!(label_for(Some("ReShade64.dll"), false), "ReShade");
        assert_eq!(label_for(Some("OptiScaler.dll"), false), "OptiScaler");
        assert_eq!(label_for(None, true), "dlssg_sm86 (FG unlock)");
        assert_eq!(label_for(None, false), "unknown");
    }

    #[test]
    fn engine_from_paths() {
        let tmp = std::env::temp_dir().join(format!("rtxu-eng-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        let exe = tmp.join("Game").join("Binaries").join("Win64").join("Game-Win64-Shipping.exe");
        assert_eq!(detect_engine(&tmp, &exe), Engine::Unreal);
        fs::write(tmp.join("re_chunk_000.pak"), b"").unwrap();
        assert_eq!(detect_engine(&tmp, &exe), Engine::ReEngine);
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn binaries_exe_beats_root_helpers() {
        let tmp = std::env::temp_dir().join(format!("rtxu-exe-{}", std::process::id()));
        let bin = tmp.join("Ravage").join("Binaries").join("Win64");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(tmp.join("EasyAntiCheat")).unwrap();
        fs::write(tmp.join("PrereqsBundle.exe"), vec![0u8; 4000]).unwrap();
        fs::write(tmp.join("start_protected_game.exe"), vec![0u8; 3000]).unwrap();
        fs::write(bin.join("CrashReportClient.exe"), vec![0u8; 5000]).unwrap();
        fs::write(bin.join("Halloween.exe"), vec![0u8; 2000]).unwrap();
        assert!(find_exe(&tmp).unwrap().ends_with("Halloween.exe"));
        assert_eq!(detect_anticheat(&tmp), Some("Easy Anti-Cheat"));
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn helper_exes_are_skipped() {
        assert!(is_helper_exe("crashreport.exe"));
        assert!(is_helper_exe("installermessage.exe"));
        assert!(!is_helper_exe("re9demo.exe"));
    }

    #[test]
    #[ignore]
    fn analyze_env_dir() {
        let Ok(dir) = std::env::var("RTXU_ANALYZE") else { return };
        let i = analyze(Path::new(&dir)).unwrap();
        eprintln!("exe: {}", i.exe.display());
        eprintln!("engine: {:?}", i.engine);
        eprintln!("dlssg: {}", i.dlssg);
        eprintln!("anticheat: {:?}", i.anticheat);
        eprintln!("imports: {}", i.imports.join(", "));
        eprintln!("present: {:?}", i.present);
    }

    #[test]
    #[ignore]
    fn bodycam_analysis() {
        let i = analyze(Path::new(r"D:\SteamLibrary\steamapps\common\Bodycam")).unwrap();
        assert!(i.exe.ends_with("Bodycam-Win64-Shipping.exe"));
        assert_eq!(i.engine, Engine::Unreal);
        assert!(i.imports.contains(&"version.dll".to_string()));
        assert!(i.dlssg);
        assert!(i.present.iter().any(|(n, o)| n == "dwmapi.dll" && o == "UE4SS"));
    }
}
