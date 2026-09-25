use std::fs;
use std::path::{Path, PathBuf};

use crate::sources::{is_sm_hash, FG_PROXIES, PROXY_FAMILY, SM_PROXIES, TABLE_PROXIES};
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
            Engine::ReEngine => "RE Engine",
            Engine::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Import,
    DelayLoad,
    Reference,
    Never,
}

impl Load {
    pub fn label(self) -> &'static str {
        match self {
            Load::Import => "imported at startup",
            Load::DelayLoad => "delay-loaded on first use",
            Load::Reference => "only named in the executable",
            Load::Never => "not referenced",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProxySlot {
    pub name: &'static str,
    pub load: Load,
    pub known_dll: bool,
    pub owner: Option<String>,
}

impl ProxySlot {
    pub fn free(&self) -> bool {
        self.owner.is_none() && !self.known_dll
    }

    pub fn loads(&self) -> bool {
        matches!(self.load, Load::Import | Load::DelayLoad)
    }

    pub fn usable(&self) -> bool {
        self.free() && self.loads()
    }

    pub fn is_family(&self) -> bool {
        PROXY_FAMILY.contains(&self.name)
    }

    pub fn render_path(&self) -> bool {
        matches!(self.name, "dxgi.dll" | "d3d12.dll")
    }

    pub fn fits_fg(&self) -> bool {
        FG_PROXIES.contains(&self.name)
    }

    pub fn fits_sm(&self) -> bool {
        SM_PROXIES.contains(&self.name)
    }

    pub fn verdict(&self) -> String {
        if self.known_dll {
            return "KnownDLL, system copy wins".into();
        }
        if let Some(o) = &self.owner {
            return format!("taken by {o}");
        }
        match self.load {
            Load::Import if !self.render_path() => "usable".into(),
            Load::Import => "usable, render path".into(),
            Load::DelayLoad => "usable, may load too late".into(),
            Load::Reference => "unclear, never picked".into(),
            Load::Never => "never loaded".into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExeLoads {
    pub imports: Vec<String>,
    pub delay: Vec<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GameInfo {
    pub root: PathBuf,
    pub exe: PathBuf,
    pub proxy_dir: PathBuf,
    pub engine: Engine,
    pub slots: Vec<ProxySlot>,
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

pub const PROXY_NAMES: [&str; 14] = [
    "version.dll",
    "winmm.dll",
    "dbghelp.dll",
    "dinput8.dll",
    "dxgi.dll",
    "d3d12.dll",
    "d3d11.dll",
    "dsound.dll",
    "hid.dll",
    "winhttp.dll",
    "wininet.dll",
    "dwmapi.dll",
    "xinput1_4.dll",
    "xinput1_3.dll",
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

fn rva_to_offset(sections: &[goblin::pe::section_table::SectionTable], rva: u32) -> Option<usize> {
    sections.iter().find_map(|s| {
        let span = s.virtual_size.max(s.size_of_raw_data);
        (rva >= s.virtual_address && rva < s.virtual_address.saturating_add(span))
            .then(|| (rva - s.virtual_address + s.pointer_to_raw_data) as usize)
    })
}

fn c_string_at(data: &[u8], offset: usize) -> Option<String> {
    let rest = data.get(offset..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&rest[..end]).to_ascii_lowercase())
}

fn delay_loaded(data: &[u8], pe: &goblin::pe::PE) -> Vec<String> {
    const DELAY_IMPORT_DIRECTORY: usize = 13;
    const ENTRY_SIZE: usize = 32;
    let mut out = Vec::new();
    let Some(opt) = pe.header.optional_header.as_ref() else {
        return out;
    };
    let Some(Some((_, dir))) = opt.data_directories.data_directories.get(DELAY_IMPORT_DIRECTORY) else {
        return out;
    };
    let Some(mut off) = rva_to_offset(&pe.sections, dir.virtual_address) else {
        return out;
    };
    while let Some(entry) = data.get(off..off + ENTRY_SIZE) {
        let name_rva = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
        if name_rva == 0 {
            break;
        }
        if let Some(name) = rva_to_offset(&pe.sections, name_rva).and_then(|o| c_string_at(data, o)) {
            out.push(name);
        }
        off += ENTRY_SIZE;
    }
    out.sort();
    out.dedup();
    out
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

fn referenced_names(data: &[u8], names: &[&str]) -> Vec<String> {
    let lower = data.to_ascii_lowercase();
    names
        .iter()
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            memchr::memmem::find(&lower, n.as_bytes()).is_some() || memchr::memmem::find(&lower, &utf16(&n)).is_some()
        })
        .map(|n| n.to_string())
        .collect()
}

pub fn exe_loads(exe: &Path) -> Result<ExeLoads, String> {
    let data = fs::read(exe).map_err(|e| format!("cannot read executable: {e}"))?;
    let mut opts = goblin::pe::options::ParseOptions::default();
    opts.parse_attribute_certificates = false;
    opts.parse_tls_data = false;
    opts.parse_resources = false;
    opts.parse_mode = goblin::options::ParseMode::Permissive;
    let pe = goblin::pe::PE::parse_with_opts(&data, &opts)
        .map_err(|e| format!("cannot parse PE: {e}"))?;
    let mut imports: Vec<String> = pe.libraries.iter().map(|s| s.to_ascii_lowercase()).collect();
    imports.sort();
    imports.dedup();
    let delay = delay_loaded(&data, &pe);
    let references = referenced_names(&data, &TABLE_PROXIES);
    Ok(ExeLoads { imports, delay, references })
}

pub fn known_dlls() -> Vec<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager\KnownDLLs")
    else {
        return Vec::new();
    };
    key.enum_values()
        .flatten()
        .filter_map(|(_, v)| {
            let s = v.to_string().trim_matches('"').to_ascii_lowercase();
            s.ends_with(".dll").then_some(s)
        })
        .collect()
}

pub fn proxy_slots(loads: &ExeLoads, proxy_dir: &Path, known: &[String]) -> Vec<ProxySlot> {
    let has_ini = proxy_dir.join("dlssg_sm86.ini").exists();
    TABLE_PROXIES
        .iter()
        .map(|&name| {
            let has = |v: &[String]| v.iter().any(|x| x.eq_ignore_ascii_case(name));
            let load = if has(&loads.imports) {
                Load::Import
            } else if has(&loads.delay) {
                Load::DelayLoad
            } else if has(&loads.references) {
                Load::Reference
            } else {
                Load::Never
            };
            let path = proxy_dir.join(name);
            let owner = path.is_file().then(|| identify(&path, has_ini));
            ProxySlot { name, load, known_dll: has(known), owner }
        })
        .collect()
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

pub const SM_LABEL: &str = "Smooth Motion";

pub fn identify(path: &Path, has_sm86_ini: bool) -> String {
    const SMALL: u64 = 8 << 20;
    let orig = original_filename(path);
    if orig.is_none() && fs::metadata(path).map(|m| m.len() < SMALL).unwrap_or(false) {
        if let Ok(bytes) = fs::read(path) {
            if is_sm_hash(&crate::download::sha256_hex(&bytes)) {
                return SM_LABEL.into();
            }
        }
    }
    label_for(orig.as_deref(), has_sm86_ini)
}

pub fn present_proxies(proxy_dir: &Path) -> Vec<(String, String)> {
    let has_ini = proxy_dir.join("dlssg_sm86.ini").exists();
    PROXY_NAMES
        .iter()
        .filter_map(|n| {
            let p = proxy_dir.join(n);
            p.is_file().then(|| (n.to_string(), identify(&p, has_ini)))
        })
        .collect()
}

pub fn root_for_exe(exe: &Path) -> PathBuf {
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let lower = dir.to_string_lossy().to_ascii_lowercase();
    if lower.ends_with("\\binaries\\win64") {
        if let Some(root) = dir.parent().and_then(Path::parent).and_then(Path::parent) {
            return root.to_path_buf();
        }
    }
    dir
}

pub fn analyze(root: &Path) -> Result<GameInfo, String> {
    if !root.is_dir() {
        return Err(format!("folder does not exist: {}", root.display()));
    }
    let exe = find_exe(root)?;
    analyze_with(root, exe)
}

pub fn analyze_exe(exe: &Path) -> Result<GameInfo, String> {
    if !exe.is_file() {
        return Err(format!("executable does not exist: {}", exe.display()));
    }
    let root = root_for_exe(exe);
    analyze_with(&root, exe.to_path_buf())
}

pub fn reanalyze(info: &GameInfo) -> Result<GameInfo, String> {
    analyze_with(&info.root, info.exe.clone())
}

fn analyze_with(root: &Path, exe: PathBuf) -> Result<GameInfo, String> {
    let proxy_dir = exe
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "executable has no parent folder".to_string())?;
    let loads = exe_loads(&exe)?;
    let engine = detect_engine(root, &exe);
    let present = present_proxies(&proxy_dir);
    let slots = proxy_slots(&loads, &proxy_dir, &known_dlls());
    let dlssg = dlssg_present(root);
    let anticheat = detect_anticheat(root);
    Ok(GameInfo {
        root: root.to_path_buf(),
        exe,
        proxy_dir,
        engine,
        slots,
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
    fn root_from_exe_layouts() {
        assert_eq!(
            root_for_exe(Path::new(r"D:\G\Halloween\Ravage\Binaries\Win64\Halloween.exe")),
            PathBuf::from(r"D:\G\Halloween")
        );
        assert_eq!(root_for_exe(Path::new(r"D:\G\Flat\game.exe")), PathBuf::from(r"D:\G\Flat"));
    }

    #[test]
    fn helper_exes_are_skipped() {
        assert!(is_helper_exe("crashreport.exe"));
        assert!(is_helper_exe("installermessage.exe"));
        assert!(!is_helper_exe("re9demo.exe"));
    }

    #[test]
    fn name_scan_is_case_insensitive_and_utf16() {
        let mut data = b"...VERSION.DLL...".to_vec();
        data.extend(utf16("d3d12.dll"));
        let r = referenced_names(&data, &TABLE_PROXIES);
        assert_eq!(r, vec!["version.dll".to_string(), "d3d12.dll".to_string()]);
        assert!(referenced_names(b"sh", &TABLE_PROXIES).is_empty());
    }

    #[test]
    fn slots_rank_evidence_and_owners() {
        let tmp = std::env::temp_dir().join(format!("rtxu-slots-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("winmm.dll"), b"x").unwrap();
        let loads = ExeLoads {
            imports: vec!["version.dll".into(), "winmm.dll".into(), "dxgi.dll".into()],
            delay: vec!["dbghelp.dll".into()],
            references: vec!["dinput8.dll".into()],
        };
        let known = vec!["dinput8.dll".to_string()];
        let slots = proxy_slots(&loads, &tmp, &known);
        let get = |n: &str| slots.iter().find(|s| s.name == n).unwrap();
        assert!(get("version.dll").usable());
        assert_eq!(get("winmm.dll").owner.as_deref(), Some("unknown"));
        assert!(!get("winmm.dll").usable());
        assert_eq!(get("dbghelp.dll").load, Load::DelayLoad);
        assert!(get("dbghelp.dll").usable());
        assert!(get("dinput8.dll").known_dll);
        assert!(!get("dinput8.dll").usable());
        assert_eq!(get("dxgi.dll").verdict(), "usable, render path");
        assert_eq!(get("d3d12.dll").load, Load::Never);
        assert!(get("winmm.dll").fits_fg() && get("winmm.dll").fits_sm());
        assert!(get("dxgi.dll").fits_fg() && !get("dxgi.dll").fits_sm());
        assert!(!get("dsound.dll").fits_fg() && get("dsound.dll").fits_sm());
        assert_eq!(slots.len(), TABLE_PROXIES.len());
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    #[ignore]
    fn analyze_env_dir() {
        let Ok(dir) = std::env::var("RTXU_ANALYZE") else { return };
        let p = Path::new(&dir);
        let i = if p.is_file() { analyze_exe(p).unwrap() } else { analyze(p).unwrap() };
        eprintln!("root: {}", i.root.display());
        eprintln!("exe: {}", i.exe.display());
        eprintln!("engine: {:?}", i.engine);
        eprintln!("dlssg: {}", i.dlssg);
        eprintln!("anticheat: {:?}", i.anticheat);
        for s in &i.slots {
            eprintln!("{:12} {:28} {:<24} {}", s.name, s.load.label(), s.owner.clone().unwrap_or_default(), s.verdict());
        }
        eprintln!("present: {:?}", i.present);
    }

    #[test]
    #[ignore]
    fn bodycam_analysis() {
        let i = analyze(Path::new(r"D:\SteamLibrary\steamapps\common\Bodycam")).unwrap();
        assert!(i.exe.ends_with("Bodycam-Win64-Shipping.exe"));
        assert_eq!(i.engine, Engine::Unreal);
        let version = i.slots.iter().find(|s| s.name == "version.dll").unwrap();
        assert_eq!(version.load, Load::Import);
        let dbghelp = i.slots.iter().find(|s| s.name == "dbghelp.dll").unwrap();
        assert_eq!(dbghelp.load, Load::DelayLoad);
        assert!(i.dlssg);
        assert!(i.present.iter().any(|(n, o)| n == "dwmapi.dll" && o == "UE4SS"));
    }
}
