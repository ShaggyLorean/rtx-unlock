pub const SDLI_REPO: &str = "sdli1995/dlssg_for_sm86";
pub const SDLI_COMMIT: &str = "5f62ff44a9c08f9841fa605e7b7160f79ccd2c40";
pub const SDLI_VERSION: &str = "0.2.4";
pub const SDLI_LEGACY_VERSION: &str = "0.1.0";

pub const AUTOPILOT_REPO: &str = "Kizzuwatnaa/DLSS5-Autopilot";

pub const REFRAMEWORK_REPO: &str = "praydog/REFramework-nightly";
pub const REFRAMEWORK_ASSET: &str = "REFramework.zip";

#[derive(Debug, Clone, Copy)]
pub struct Source {
    pub url: &'static str,
    pub sha256: &'static str,
}

macro_rules! raw {
    ($path:literal) => {
        concat!(
            "https://raw.githubusercontent.com/sdli1995/dlssg_for_sm86/5f62ff44a9c08f9841fa605e7b7160f79ccd2c40/",
            $path
        )
    };
}

pub fn fg_source(proxy: &str, legacy: bool) -> Option<Source> {
    let s = match (proxy, legacy) {
        ("version.dll", true) => Source {
            url: raw!("archive/version.dll"),
            sha256: "03d445237d519ac48cd9226278a0f07aecd7ac597697697eb64404e1d51b3c5a",
        },
        (_, true) => return None,
        ("version.dll", false) => Source {
            url: raw!("version.dll"),
            sha256: "c844646d835a7b88ed1382eea80403d38b433f8ac09cf92581c73698c44ae7c2",
        },
        ("winmm.dll", false) => Source {
            url: raw!("altnative/winmm.dll"),
            sha256: "1004dd4ee0edbe4e1af4c8c7b30d4786bea0f5e7c0412566996b4c2543ae7e36",
        },
        ("dinput8.dll", false) => Source {
            url: raw!("altnative/dinput8.dll"),
            sha256: "ef3c3d49c5b5c8a17289c24da9b22885570793d72f3db628fa500f9efdb20489",
        },
        ("winhttp.dll", false) => Source {
            url: raw!("altnative/winhttp.dll"),
            sha256: "1619839e4d1b6145ce9a587ba807f42e64f2b0984af9e81700d42ccf46ff7253",
        },
        ("dxgi.dll", false) => Source {
            url: raw!("altnative/dxgi.dll"),
            sha256: "8d29eddbd7f1c3e272d07f94ab8812a80ef5b7aeb73923320bf9a432ddcf74c0",
        },
        _ => return None,
    };
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_complete() {
        for p in ["version.dll", "winmm.dll", "dinput8.dll", "winhttp.dll", "dxgi.dll"] {
            let s = fg_source(p, false).unwrap();
            assert!(s.url.contains(SDLI_COMMIT));
            assert_eq!(s.sha256.len(), 64);
        }
        assert!(fg_source("version.dll", true).is_some());
        assert!(fg_source("winmm.dll", true).is_none());
        assert!(fg_source("dwmapi.dll", false).is_none());
    }
}
