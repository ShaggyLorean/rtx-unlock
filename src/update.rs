use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::dlss5::parse_sha256sums;
use crate::download::{fetch, fetch_text};

pub const REPO: &str = "ShaggyLorean/rtx-unlock";
pub const ASSET: &str = "rtx-unlock.exe";
pub const SUMS: &str = "SHA256SUMS.txt";

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub exe_url: String,
    pub sums_url: String,
}

pub fn current_version() -> String {
    std::env::var("RTXU_VERSION_OVERRIDE").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
}

fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split('.').map(|p| p.parse::<u32>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

pub fn check() -> Result<Option<Release>, String> {
    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let json = fetch_text(&api)?;
    let v: serde_json::Value = serde_json::from_str(&json).map_err(|e| format!("bad release JSON: {e}"))?;
    let tag = v["tag_name"].as_str().unwrap_or("").to_string();
    if !is_newer(&tag, &current_version()) {
        return Ok(None);
    }
    let mut exe_url = None;
    let mut sums_url = None;
    for a in v["assets"].as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        let name = a["name"].as_str().unwrap_or("");
        let url = a["browser_download_url"].as_str().unwrap_or("").to_string();
        if name == ASSET {
            exe_url = Some(url);
        } else if name == SUMS {
            sums_url = Some(url);
        }
    }
    match (exe_url, sums_url) {
        (Some(exe_url), Some(sums_url)) => Ok(Some(Release {
            version: tag.trim_start_matches('v').to_string(),
            exe_url,
            sums_url,
        })),
        _ => Err(format!("release {tag} has no {ASSET} or {SUMS} asset")),
    }
}

pub fn old_exe_path() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    Some(me.with_extension("old"))
}

pub fn cleanup_old() {
    if let Some(old) = old_exe_path() {
        let _ = fs::remove_file(old);
    }
}

pub fn apply(rel: &Release, log: &dyn Fn(String)) -> Result<(), String> {
    let sums = fetch_text(&rel.sums_url)?;
    let sha = parse_sha256sums(&sums, ASSET).ok_or_else(|| format!("{ASSET} is not listed in {SUMS}"))?;
    let cached = fetch(&rel.exe_url, &sha, log)?;
    let me = std::env::current_exe().map_err(|e| format!("cannot locate the running executable: {e}"))?;
    let old = me.with_extension("old");
    let _ = fs::remove_file(&old);
    fs::rename(&me, &old).map_err(|e| format!("cannot move the running executable aside: {e}"))?;
    if let Err(e) = fs::copy(&cached, &me) {
        let _ = fs::rename(&old, &me);
        return Err(format!("cannot place the new executable: {e}"));
    }
    log(format!("updated to {}, restarting", rel.version));
    let args: Vec<String> = std::env::args().skip(1).collect();
    Command::new(&me)
        .args(args)
        .spawn()
        .map_err(|e| format!("new executable did not start: {e}"))?;
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.3.0", "0.2.1"));
        assert!(is_newer("0.2.10", "0.2.9"));
        assert!(!is_newer("v0.2.1", "0.2.1"));
        assert!(!is_newer("v0.2.0", "0.2.1"));
        assert!(!is_newer("garbage", "0.2.1"));
    }

    #[test]
    #[ignore]
    fn check_env() {
        if std::env::var("RTXU_UPDATE_CHECK").is_err() {
            return;
        }
        eprintln!("{:?}", check());
    }
}
