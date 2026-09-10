use std::fs;
use std::path::Path;

pub const SECTION: &str = "RenoDX.DLSS5";
pub const RESHADE_INI: &str = "ReShade.ini";
pub const RESHADE_LOG: &str = "ReShade.log";

pub const RE_ENGINE_PAPER_WHITE: f32 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub struct NrSettings {
    pub enabled: bool,
    pub preset: i32,
    pub style: i32,
    pub intensity: f32,
    pub local_tone: f32,
    pub local_structure: f32,
    pub skin_structure: f32,
    pub auto_mask: bool,
    pub ui_correction: bool,
    pub paper_white: f32,
}

impl Default for NrSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            preset: 0,
            style: 1,
            intensity: 1.0,
            local_tone: 1.0,
            local_structure: 1.0,
            skin_structure: 0.5,
            auto_mask: false,
            ui_correction: false,
            paper_white: 1.0,
        }
    }
}

pub const STYLE_NAMES: [&str; 3] = ["Default", "Natural", "Cinematic"];
pub const PRESET_NAMES: [&str; 4] = ["Default", "Preset #1", "Preset #2", "Preset #3"];

fn fmt(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".into() } else { s.to_string() }
}

impl NrSettings {
    pub fn to_pairs(&self) -> Vec<(String, String)> {
        vec![
            ("NeuralUplift".into(), (self.enabled as i32).to_string()),
            ("NRPreset".into(), self.preset.to_string()),
            ("NRStyle".into(), self.style.to_string()),
            ("NRIntensity".into(), fmt(self.intensity)),
            ("NRLocalTone".into(), fmt(self.local_tone)),
            ("NRLocalStructure".into(), fmt(self.local_structure)),
            ("NRSkinStructure".into(), fmt(self.skin_structure)),
            ("NRAutoMask".into(), (self.auto_mask as i32).to_string()),
            ("NRUICorrection".into(), (self.ui_correction as i32).to_string()),
            ("NRPaperWhiteScale".into(), fmt(self.paper_white)),
        ]
    }

    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        let mut s = Self::default();
        let get = |k: &str| pairs.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
        let f = |k: &str, d: f32| get(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(d);
        let i = |k: &str, d: i32| get(k).and_then(|v| v.parse::<f32>().ok()).map(|v| v as i32).unwrap_or(d);
        s.enabled = i("NeuralUplift", 1) != 0;
        s.preset = i("NRPreset", 0);
        s.style = i("NRStyle", 1);
        s.intensity = f("NRIntensity", 1.0);
        s.local_tone = f("NRLocalTone", 1.0);
        s.local_structure = f("NRLocalStructure", 1.0);
        s.skin_structure = f("NRSkinStructure", 0.5);
        s.auto_mask = i("NRAutoMask", 0) != 0;
        s.ui_correction = i("NRUICorrection", 0) != 0;
        s.paper_white = f("NRPaperWhiteScale", 1.0);
        s
    }
}

pub fn read_section(text: &str, section: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            inside = t[1..t.len() - 1].eq_ignore_ascii_case(section);
            continue;
        }
        if inside {
            if let Some((k, v)) = t.split_once('=') {
                out.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

pub fn write_section(text: &str, section: &str, pairs: &[(String, String)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header_idx = lines
        .iter()
        .position(|l| {
            let t = l.trim();
            t.starts_with('[') && t.ends_with(']') && t[1..t.len() - 1].eq_ignore_ascii_case(section)
        });
    let (start, end) = match header_idx {
        Some(h) => {
            let mut e = h + 1;
            while e < lines.len() && !lines[e].trim().starts_with('[') {
                e += 1;
            }
            (h + 1, e)
        }
        None => {
            if !lines.is_empty() && !lines.last().map_or(true, |l| l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{section}]"));
            let n = lines.len();
            (n, n)
        }
    };
    let mut body: Vec<String> = lines[start..end].to_vec();
    for (k, v) in pairs {
        let hit = body.iter().position(|l| {
            l.split_once('=').map_or(false, |(a, _)| a.trim().eq_ignore_ascii_case(k))
        });
        match hit {
            Some(i) => body[i] = format!("{k}={v}"),
            None => {
                let mut ins = body.len();
                while ins > 0 && body[ins - 1].trim().is_empty() {
                    ins -= 1;
                }
                body.insert(ins, format!("{k}={v}"));
            }
        }
    }
    lines.splice(start..end, body);
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn load(proxy_dir: &Path) -> Option<NrSettings> {
    let text = fs::read_to_string(proxy_dir.join(RESHADE_INI)).ok()?;
    let pairs = read_section(&text, SECTION);
    if pairs.is_empty() {
        return None;
    }
    Some(NrSettings::from_pairs(&pairs))
}

pub fn save(proxy_dir: &Path, s: &NrSettings) -> Result<(), String> {
    let p = proxy_dir.join(RESHADE_INI);
    let text = fs::read_to_string(&p).map_err(|e| format!("cannot read ReShade.ini: {e}"))?;
    let out = write_section(&text, SECTION, &s.to_pairs());
    fs::write(&p, out).map_err(|e| format!("cannot write ReShade.ini: {e}"))
}

pub fn set_if_absent(proxy_dir: &Path, key: &str, value: &str) -> Result<bool, String> {
    let p = proxy_dir.join(RESHADE_INI);
    let text = fs::read_to_string(&p).map_err(|e| format!("cannot read ReShade.ini: {e}"))?;
    let has = read_section(&text, SECTION)
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case(key));
    if has {
        return Ok(false);
    }
    let out = write_section(&text, SECTION, &[(key.to_string(), value.to_string())]);
    fs::write(&p, out).map_err(|e| format!("cannot write ReShade.ini: {e}"))?;
    Ok(true)
}

#[derive(Debug, Default, Clone)]
pub struct LogFacts {
    pub hdr_codec: bool,
    pub nr_evaluated: bool,
    pub active_settings: Option<String>,
    pub device_lost: Option<String>,
    pub driver: Option<String>,
}

pub fn read_log(proxy_dir: &Path) -> Option<LogFacts> {
    let text = fs::read_to_string(proxy_dir.join(RESHADE_LOG)).ok()?;
    Some(parse_log(&text))
}

pub fn parse_log(text: &str) -> LogFacts {
    let mut f = LogFacts::default();
    for line in text.lines() {
        if line.contains("Control-equivalent") {
            f.hdr_codec = true;
        }
        if line.contains("evaluation succeeded") {
            f.nr_evaluated = true;
        }
        if let Some(i) = line.find("DLSS5 active settings:") {
            f.active_settings = Some(line[i + "DLSS5 active settings:".len()..].trim().to_string());
        }
        if let Some(i) = line.find("Device removal reason is ") {
            f.device_lost = Some(line[i + "Device removal reason is ".len()..].trim_end_matches('.').to_string());
        }
        if let Some(i) = line.find("Driver ") {
            if line.contains("Running on") {
                f.driver = Some(line[i + 7..].trim_end_matches('.').to_string());
            }
        }
    }
    f
}

pub fn driver_faults_new_addon(driver: &str) -> bool {
    let mut it = driver.split('.');
    let major: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor) >= (616, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INI: &str = "[ADDON]\nAddonPath=.\\\n\n[RenoDX.DLSS5]\nNeuralUplift=1\nNRStyle=1\nNRPaperWhiteScale=16\n\n[GENERAL]\nNoDebugInfo=1\n";

    #[test]
    fn read_and_parse() {
        let s = NrSettings::from_pairs(&read_section(INI, SECTION));
        assert!(s.enabled);
        assert_eq!(s.style, 1);
        assert_eq!(s.paper_white, 16.0);
        assert_eq!(s.intensity, 1.0);
    }

    #[test]
    fn write_updates_and_inserts_without_touching_others() {
        let out = write_section(
            INI,
            SECTION,
            &[("NRPaperWhiteScale".into(), "8".into()), ("NRIntensity".into(), "0.7".into())],
        );
        assert!(out.contains("NRPaperWhiteScale=8\n"));
        assert!(!out.contains("NRPaperWhiteScale=16"));
        assert!(out.contains("NRIntensity=0.7\n"));
        assert!(out.contains("[GENERAL]\nNoDebugInfo=1"));
        assert!(out.contains("[ADDON]\nAddonPath=.\\"));
        let sec = read_section(&out, SECTION);
        assert_eq!(sec.len(), 4);
    }

    #[test]
    fn write_creates_missing_section() {
        let out = write_section("[ADDON]\nAddonPath=.\\\n", SECTION, &[("NRPaperWhiteScale".into(), "16".into())]);
        assert!(out.ends_with("[RenoDX.DLSS5]\nNRPaperWhiteScale=16\n"));
    }

    #[test]
    fn roundtrip_defaults() {
        let s = NrSettings { paper_white: 16.0, style: 2, ..Default::default() };
        let out = write_section("", SECTION, &s.to_pairs());
        let back = NrSettings::from_pairs(&read_section(&out, SECTION));
        assert_eq!(back, s);
    }

    #[test]
    fn log_parse() {
        let log = "x | INFO | Running on NVIDIA GeForce RTX 3090 Driver 616.86.\n\
                   x | INFO | [DLSS 5 Neural Rendering] DLSS5 Generic: created Control-equivalent soft-clip/sRGB/UpgradeToneMap codec\n\
                   x | INFO | [DLSS 5 Neural Rendering] DLSS5 Generic: inline feature 18 evaluation succeeded (count=1)\n\
                   x | INFO | [DLSS 5 Neural Rendering] DLSS5 Generic: DLSS5 active settings: upscaling=OFF paper_white=1.000000\n\
                   x | ERROR | > Device removal reason is DXGI_ERROR_DEVICE_HUNG.\n";
        let f = parse_log(log);
        assert!(f.hdr_codec);
        assert!(f.nr_evaluated);
        assert_eq!(f.driver.as_deref(), Some("616.86"));
        assert_eq!(f.device_lost.as_deref(), Some("DXGI_ERROR_DEVICE_HUNG"));
        assert!(f.active_settings.unwrap().contains("paper_white"));
    }

    #[test]
    #[ignore]
    fn nr_env_dir() {
        let Ok(dir) = std::env::var("RTXU_NR_DIR") else { return };
        let p = Path::new(&dir);
        eprintln!("settings: {:?}", load(p));
        eprintln!("facts: {:?}", read_log(p));
    }

    #[test]
    fn driver_threshold() {
        assert!(driver_faults_new_addon("616.86"));
        assert!(driver_faults_new_addon("616.64"));
        assert!(!driver_faults_new_addon("616.56"));
        assert!(driver_faults_new_addon("617.10"));
    }
}
