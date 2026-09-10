use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::winutil::data_dir;

const UA: &str = "rtx-unlock";
const LIMIT: u64 = 1 << 30;

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut resp = ureq::get(url)
        .header("User-Agent", UA)
        .call()
        .map_err(|e| format!("download failed ({url}): {e}"))?;
    resp.body_mut()
        .with_config()
        .limit(LIMIT)
        .read_to_vec()
        .map_err(|e| format!("cannot read response body ({url}): {e}"))
}

pub fn fetch_text(url: &str) -> Result<String, String> {
    let b = get_bytes(url)?;
    Ok(String::from_utf8_lossy(&b).into_owned())
}

/// Download, verify SHA-256, keep under `cache/<sha>`. A cached copy with a matching hash is reused.
pub fn fetch(url: &str, sha256: &str, log: &dyn Fn(String)) -> Result<PathBuf, String> {
    let sha256 = sha256.to_ascii_lowercase();
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create cache folder: {e}"))?;
    let dst = dir.join(&sha256);
    if let Ok(existing) = fs::read(&dst) {
        if sha256_hex(&existing) == sha256 {
            return Ok(dst);
        }
        let _ = fs::remove_file(&dst);
    }
    let name = url.rsplit('/').next().unwrap_or(url);
    log(format!("downloading: {name}"));
    let bytes = get_bytes(url)?;
    let got = sha256_hex(&bytes);
    if got != sha256 {
        return Err(format!("SHA-256 mismatch: {name} (expected {sha256}, got {got})"));
    }
    fs::write(&dst, &bytes).map_err(|e| format!("cannot write cache: {e}"))?;
    log(format!("downloaded: {name} ({:.1} MB)", bytes.len() as f64 / 1e6));
    Ok(dst)
}

/// A source whose hash is not known ahead of time (nightly builds). Downloads, logs the hash,
/// stores under `cache/<sha>`.
pub fn fetch_unpinned(url: &str, log: &dyn Fn(String)) -> Result<PathBuf, String> {
    let dir = cache_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("cannot create cache folder: {e}"))?;
    let name = url.rsplit('/').next().unwrap_or(url);
    log(format!("downloading: {name}"));
    let bytes = get_bytes(url)?;
    let sha = sha256_hex(&bytes);
    let dst = dir.join(&sha);
    if !dst.exists() {
        fs::write(&dst, &bytes).map_err(|e| format!("cannot write cache: {e}"))?;
    }
    log(format!("downloaded: {name} ({:.1} MB) sha256 {sha}", bytes.len() as f64 / 1e6));
    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha_known() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
