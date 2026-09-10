use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::download::{fetch, fetch_text};
use crate::sources::AUTOPILOT_REPO;
use crate::winutil::{data_dir, no_window};

pub const EXE_NAME: &str = "dlss5-autopilot.exe";
pub const STATE_NAME: &str = "dlss5-autopilot.json";

pub fn autopilot_dir() -> PathBuf {
    data_dir().join("autopilot")
}

pub fn find_autopilot() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(me) = std::env::current_exe() {
        if let Some(d) = me.parent() {
            candidates.push(d.join(EXE_NAME));
        }
    }
    candidates.push(autopilot_dir().join(EXE_NAME));
    candidates.into_iter().find(|p| p.is_file())
}

pub fn parse_sha256sums(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let hash = it.next()?;
        let file = it.next()?.trim_start_matches('*');
        if file == name && hash.len() == 64 {
            Some(hash.to_ascii_lowercase())
        } else {
            None
        }
    })
}

pub fn ensure_autopilot(log: &dyn Fn(String)) -> Result<PathBuf, String> {
    if let Some(p) = find_autopilot() {
        return Ok(p);
    }
    log("DLSS 5 Autopilot not found, fetching the latest release from GitHub".into());
    let api = format!("https://api.github.com/repos/{AUTOPILOT_REPO}/releases/latest");
    let json = fetch_text(&api)?;
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("bad release JSON: {e}"))?;
    let tag = v["tag_name"].as_str().unwrap_or("?").to_string();
    let assets = v["assets"].as_array().ok_or("release has no assets")?;
    let mut zip_url = None;
    let mut zip_name = None;
    let mut sums_url = None;
    for a in assets {
        let name = a["name"].as_str().unwrap_or("");
        let url = a["browser_download_url"].as_str().unwrap_or("");
        if name.ends_with("-win64.zip") {
            zip_url = Some(url.to_string());
            zip_name = Some(name.to_string());
        } else if name == "SHA256SUMS.txt" {
            sums_url = Some(url.to_string());
        }
    }
    let (zip_url, zip_name, sums_url) = match (zip_url, zip_name, sums_url) {
        (Some(a), Some(b), Some(c)) => (a, b, c),
        _ => return Err("release has no win64 zip or SHA256SUMS.txt".into()),
    };
    let sums = fetch_text(&sums_url)?;
    let hash = parse_sha256sums(&sums, &zip_name)
        .ok_or_else(|| format!("{zip_name} is not listed in SHA256SUMS.txt"))?;
    let zip_path = fetch(&zip_url, &hash, log)?;

    let file = fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("cannot open zip: {e}"))?;
    let dir = autopilot_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().rsplit('/').next().unwrap_or("").to_string();
        if name.eq_ignore_ascii_case(EXE_NAME) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            fs::write(dir.join(EXE_NAME), bytes).map_err(|e| e.to_string())?;
            found = true;
            break;
        }
    }
    if !found {
        return Err(format!("zip contains no {EXE_NAME}"));
    }
    log(format!("DLSS 5 Autopilot {tag} ready: {}", dir.join(EXE_NAME).display()));
    Ok(dir.join(EXE_NAME))
}

fn pump(mut reader: impl Read, log: &dyn Fn(String)) {
    let mut buf = [0u8; 4096];
    let mut line: Vec<u8> = Vec::new();
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buf[..n] {
            if b == b'\n' || b == b'\r' {
                if !line.is_empty() {
                    log(String::from_utf8_lossy(&line).trim_end().to_string());
                    line.clear();
                }
            } else {
                line.push(b);
            }
        }
    }
    if !line.is_empty() {
        log(String::from_utf8_lossy(&line).trim_end().to_string());
    }
}

pub fn run(exe: &Path, args: &[String], log: &dyn Fn(String)) -> Result<i32, String> {
    log(format!("> {} {}", exe.display(), args.join(" ")));
    let mut child = no_window(&mut Command::new(exe))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start Autopilot: {e}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut stderr = child.stderr.take().ok_or("no stderr")?;
    let err_thread = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = stderr.read_to_end(&mut v);
        v
    });
    pump(stdout, log);
    if let Ok(v) = err_thread.join() {
        pump(v.as_slice(), log);
    }
    let st = child.wait().map_err(|e| e.to_string())?;
    Ok(st.code().unwrap_or(-1))
}

fn run_ok(exe: &Path, args: &[String], log: &dyn Fn(String)) -> Result<(), String> {
    let code = run(exe, args, log)?;
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Autopilot exited with code {code}"))
    }
}

pub fn install(exe: &Path, root: &Path, route: &str, log: &dyn Fn(String)) -> Result<(), String> {
    let args = vec![
        root.to_string_lossy().into_owned(),
        "--route".into(),
        route.into(),
    ];
    run_ok(exe, &args, log)?;
    log("DLSS 5 installed. In game, press Home and enable neural rendering on the DLSS 5 tab.".into());
    Ok(())
}

pub fn remove(exe: &Path, root: &Path, log: &dyn Fn(String)) -> Result<(), String> {
    let args = vec![root.to_string_lossy().into_owned(), "--remove".into()];
    run_ok(exe, &args, log)?;
    log("DLSS 5 removed.".into());
    Ok(())
}

pub fn check(exe: &Path, root: &Path, log: &dyn Fn(String)) -> Result<(), String> {
    let args = vec![root.to_string_lossy().into_owned(), "--check".into()];
    run(exe, &args, log).map(|_| ())
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dlss5State {
    #[serde(default)]
    pub complete: bool,
    pub path: Option<String>,
    pub components: Option<serde_json::Value>,
}

impl Dlss5State {
    pub fn label(&self) -> String {
        let comps = self
            .components
            .as_ref()
            .and_then(|c| c.as_object())
            .map(|o| {
                o.iter()
                    .map(|(k, v)| format!("{k} {}", v.as_str().unwrap_or("?")))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        format!(
            "{} (route {}{}{})",
            if self.complete { "installed" } else { "incomplete" },
            self.path.as_deref().unwrap_or("?"),
            if comps.is_empty() { "" } else { "; " },
            comps
        )
    }
}

pub fn status(proxy_dir: &Path) -> Option<Dlss5State> {
    let text = fs::read_to_string(proxy_dir.join(STATE_NAME)).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums() {
        let t = "aaaa  DLSS5-Autopilot-v1.8.0-win64.zip\nbbbb *other.zip\n";
        let long = "a".repeat(64);
        let t2 = format!("{long}  x.zip\n");
        assert_eq!(parse_sha256sums(&t2, "x.zip").as_deref(), Some(long.as_str()));
        assert!(parse_sha256sums(t, "DLSS5-Autopilot-v1.8.0-win64.zip").is_none());
        assert!(parse_sha256sums(t, "nope").is_none());
    }

    #[test]
    fn state_parses() {
        let s: Dlss5State = serde_json::from_str(
            r#"{"complete":true,"path":"native","components":{"renodx":"4.55"}}"#,
        )
        .unwrap();
        assert!(s.complete);
        assert!(s.label().contains("renodx 4.55"));
    }
}
