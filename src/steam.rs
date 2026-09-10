use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamGame {
    pub appid: u32,
    pub name: String,
    pub dir: PathBuf,
}

/// Steamworks Common Redistributables is not a game.
const SKIP_APPIDS: [u32; 1] = [228980];

/// Quoted tokens on one line. Handles the `\\` and `\"` escapes Valve's KeyValues use.
fn quoted(line: &str) -> Vec<String> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    let mut inq = false;
    let mut esc = false;
    for c in line.chars() {
        if inq {
            if esc {
                cur.push(c);
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                toks.push(std::mem::take(&mut cur));
                inq = false;
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            inq = true;
        }
    }
    toks
}

/// Every `"key" "value"` line in order; block nesting is ignored.
pub fn pairs(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let t = quoted(l);
            if t.len() == 2 {
                Some((t[0].clone(), t[1].clone()))
            } else {
                None
            }
        })
        .collect()
}

pub fn library_roots(vdf: &str) -> Vec<PathBuf> {
    pairs(vdf)
        .into_iter()
        .filter(|(k, _)| k == "path")
        .map(|(_, v)| PathBuf::from(v))
        .collect()
}

pub fn parse_manifest(acf: &str, library: &Path) -> Option<SteamGame> {
    let mut appid = None;
    let mut name = None;
    let mut installdir = None;
    for (k, v) in pairs(acf) {
        match k.as_str() {
            "appid" if appid.is_none() => appid = v.parse().ok(),
            "name" if name.is_none() => name = Some(v),
            "installdir" if installdir.is_none() => installdir = Some(v),
            _ => {}
        }
    }
    Some(SteamGame {
        appid: appid?,
        name: name?,
        dir: library.join("steamapps").join("common").join(installdir?),
    })
}

pub fn steam_path() -> Result<PathBuf, String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let missing = || "Steam not found (no SteamPath in the registry)".to_string();
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Valve\\Steam")
        .map_err(|_| missing())?;
    let p: String = key.get_value("SteamPath").map_err(|_| missing())?;
    Ok(PathBuf::from(p.replace('/', "\\")))
}

pub fn scan() -> Result<Vec<SteamGame>, String> {
    let steam = steam_path()?;
    let vdf_path = steam.join("steamapps").join("libraryfolders.vdf");
    let vdf = fs::read_to_string(&vdf_path)
        .map_err(|e| format!("cannot read libraryfolders.vdf ({}): {e}", vdf_path.display()))?;
    let mut roots = library_roots(&vdf);
    if roots.is_empty() {
        roots.push(steam.clone());
    }
    let mut games = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(root.join("steamapps")) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Some(n) = p.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
                continue;
            }
            let Ok(t) = fs::read_to_string(&p) else {
                continue;
            };
            if let Some(g) = parse_manifest(&t, &root) {
                if !SKIP_APPIDS.contains(&g.appid) && g.dir.is_dir() {
                    games.push(g);
                }
            }
        }
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    games.dedup_by_key(|g| g.appid);
    Ok(games)
}

#[cfg(test)]
mod tests {
    use super::*;
    const VDF: &str = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "1" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
    const ACF: &str = r#""AppState"
{
	"appid"		"2406770"
	"name"		"Bodycam"
	"installdir"		"Bodycam"
	"InstalledDepots" { "2406771" { "name" "x" } }
}"#;

    #[test]
    fn roots_from_vdf() {
        let r = library_roots(VDF);
        assert_eq!(
            r,
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"D:\SteamLibrary")
            ]
        );
    }

    #[test]
    fn manifest_parses() {
        let g = parse_manifest(ACF, Path::new(r"D:\SteamLibrary")).unwrap();
        assert_eq!(g.appid, 2406770);
        assert_eq!(g.name, "Bodycam");
        assert_eq!(g.dir, PathBuf::from(r"D:\SteamLibrary\steamapps\common\Bodycam"));
    }

    #[test]
    fn manifest_missing_key_is_none() {
        assert!(parse_manifest("\"appid\" \"1\"", Path::new("x")).is_none());
    }

    #[test]
    fn quoted_handles_escapes() {
        assert_eq!(quoted(r#""a" "b\"c""#), vec!["a", "b\"c"]);
    }
}
