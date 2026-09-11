use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::winutil::data_dir;

pub const FILE_NAME: &str = "games.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomGame {
    pub name: String,
    pub exe: PathBuf,
}

fn store_path() -> PathBuf {
    data_dir().join(FILE_NAME)
}

pub fn load() -> Vec<CustomGame> {
    load_from(&store_path())
}

pub fn load_from(path: &Path) -> Vec<CustomGame> {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(list: &[CustomGame]) -> Result<(), String> {
    save_to(&store_path(), list)
}

pub fn save_to(path: &Path, list: &[CustomGame]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

pub fn name_for(exe: &Path) -> String {
    exe.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_end_matches("-Win64-Shipping").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| exe.display().to_string())
}

pub fn add(list: &mut Vec<CustomGame>, exe: PathBuf) -> usize {
    if let Some(i) = list.iter().position(|g| g.exe.eq_ignore_ascii_case_path(&exe)) {
        return i;
    }
    list.push(CustomGame { name: name_for(&exe), exe });
    list.sort_by_key(|g| g.name.to_lowercase());
    list.len() - 1
}

trait PathEqIgnoreCase {
    fn eq_ignore_ascii_case_path(&self, other: &Path) -> bool;
}

impl PathEqIgnoreCase for PathBuf {
    fn eq_ignore_ascii_case_path(&self, other: &Path) -> bool {
        self.to_string_lossy().eq_ignore_ascii_case(&other.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_exe() {
        assert_eq!(name_for(Path::new(r"D:\G\Ravage\Binaries\Win64\Halloween.exe")), "Halloween");
        assert_eq!(name_for(Path::new(r"D:\G\Bodycam-Win64-Shipping.exe")), "Bodycam");
    }

    #[test]
    fn add_dedups_and_roundtrips() {
        let mut list = Vec::new();
        add(&mut list, PathBuf::from(r"D:\G\b.exe"));
        add(&mut list, PathBuf::from(r"D:\G\A.exe"));
        let i = add(&mut list, PathBuf::from(r"d:\g\B.EXE"));
        assert_eq!(list.len(), 2);
        assert_eq!(list[i].name, "b");
        let p = std::env::temp_dir().join(format!("rtxu-custom-{}.json", std::process::id()));
        save_to(&p, &list).unwrap();
        assert_eq!(load_from(&p), list);
        fs::remove_file(&p).unwrap();
    }
}
