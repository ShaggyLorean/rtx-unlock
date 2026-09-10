use std::path::{Path, PathBuf};
use std::process::Command;

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn no_window(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("rtx-unlock")
}

pub fn original_filename(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        GetFileVersionInfoW(PCWSTR(wide.as_ptr()), None, size, buf.as_mut_ptr() as *mut _).ok()?;

        let mut ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut len: u32 = 0;
        let q: Vec<u16> = "\\VarFileInfo\\Translation"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        if !VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(q.as_ptr()), &mut ptr, &mut len).as_bool()
            || len < 4
        {
            return None;
        }
        let lang = *(ptr as *const u16);
        let cp = *(ptr as *const u16).add(1);
        let sub = format!("\\StringFileInfo\\{:04x}{:04x}\\OriginalFilename", lang, cp);
        let q2: Vec<u16> = sub.encode_utf16().chain(std::iter::once(0)).collect();
        let mut p2: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut l2: u32 = 0;
        if !VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(q2.as_ptr()), &mut p2, &mut l2).as_bool()
            || l2 == 0
        {
            return None;
        }
        let s = std::slice::from_raw_parts(p2 as *const u16, l2 as usize);
        let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
        let s = String::from_utf16_lossy(&s[..end]).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}
