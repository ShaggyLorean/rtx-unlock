pub const SDLI_REPO: &str = "sdli1995/dlssg_for_sm86";
pub const SDLI_COMMIT: &str = "9621db573e07ed54f50c15bbb585ed9a7bdfac28";
pub const SDLI_VERSION: &str = "0.3.5";
pub const SDLI_LEGACY_VERSION: &str = "0.1.0";

pub const AUTOPILOT_REPO: &str = "Kizzuwatnaa/DLSS5-Autopilot";

pub const REFRAMEWORK_REPO: &str = "praydog/REFramework-nightly";
pub const REFRAMEWORK_ASSET: &str = "REFramework.zip";

pub const SM_REPO: &str = "ShaggyLorean/smooth-motion-rtx30-winmm";
pub const SM_VERSION: &str = "0.2.0";
pub const SM_TESTED_DRIVER: &str = "616.92";

pub const SM_PROXIES: [&str; 11] = [
    "winmm.dll",
    "version.dll",
    "dinput8.dll",
    "dsound.dll",
    "hid.dll",
    "winhttp.dll",
    "wininet.dll",
    "dwmapi.dll",
    "xinput1_4.dll",
    "xinput1_3.dll",
    "dbghelp.dll",
];

pub const TABLE_PROXIES: [&str; 13] = [
    "version.dll",
    "winmm.dll",
    "dbghelp.dll",
    "dinput8.dll",
    "dxgi.dll",
    "d3d12.dll",
    "dsound.dll",
    "hid.dll",
    "winhttp.dll",
    "wininet.dll",
    "dwmapi.dll",
    "xinput1_4.dll",
    "xinput1_3.dll",
];

pub const PROXY_FAMILY: [&str; 4] = ["version.dll", "winmm.dll", "dbghelp.dll", "dinput8.dll"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runtime {
    Dlssg3109,
    Dlssg3101,
    Legacy010,
}

impl Runtime {
    pub const ALL: [Runtime; 3] = [Runtime::Dlssg3109, Runtime::Dlssg3101, Runtime::Legacy010];

    pub fn label(self) -> &'static str {
        match self {
            Runtime::Dlssg3109 => "0.3.5, DLSS-G 310.9 (up to 6X)",
            Runtime::Dlssg3101 => "0.3.5, DLSS-G 310.1 (up to 4X)",
            Runtime::Legacy010 => "0.1.0 legacy",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Runtime::Dlssg3109 => "310.9",
            Runtime::Dlssg3101 => "310.1",
            Runtime::Legacy010 => "0.1.0",
        }
    }

    pub fn source_version(self) -> &'static str {
        match self {
            Runtime::Legacy010 => SDLI_LEGACY_VERSION,
            _ => SDLI_VERSION,
        }
    }

    pub fn max_generated_frames(self) -> u32 {
        match self {
            Runtime::Dlssg3109 => 5,
            _ => 3,
        }
    }

    pub fn proxies(self) -> &'static [&'static str] {
        match self {
            Runtime::Legacy010 => &PROXY_FAMILY[..1],
            _ => &FG_PROXIES,
        }
    }
}

pub const FG_PROXIES: [&str; 6] = ["version.dll", "winmm.dll", "dbghelp.dll", "dinput8.dll", "dxgi.dll", "d3d12.dll"];

#[derive(Debug, Clone, Copy)]
pub struct Source {
    pub url: &'static str,
    pub sha256: &'static str,
}

macro_rules! raw {
    ($path:literal) => {
        concat!(
            "https://raw.githubusercontent.com/sdli1995/dlssg_for_sm86/9621db573e07ed54f50c15bbb585ed9a7bdfac28/",
            $path
        )
    };
}

pub fn fg_source(proxy: &str, runtime: Runtime) -> Option<Source> {
    let s = match (runtime, proxy) {
        (Runtime::Legacy010, "version.dll") => Source {
            url: raw!("archive/0.1.0/version.dll"),
            sha256: "03d445237d519ac48cd9226278a0f07aecd7ac597697697eb64404e1d51b3c5a",
        },
        (Runtime::Legacy010, _) => return None,
        (Runtime::Dlssg3109, "version.dll") => Source {
            url: raw!("version.dll"),
            sha256: "c3934a09399f022504227c72df0bf8c0de55f9a08880dddde898c5262cefa838",
        },
        (Runtime::Dlssg3109, "winmm.dll") => Source {
            url: raw!("alternatives/winmm.dll"),
            sha256: "bc3cef25c1ccbbdccdd93db9fae845104bb6c71fd433240e26641881a7e46658",
        },
        (Runtime::Dlssg3109, "dbghelp.dll") => Source {
            url: raw!("alternatives/dbghelp.dll"),
            sha256: "dfe7f44baa68d237dfe040d4b4cfc9bd44b01e661f3051996db1144e6bb388cb",
        },
        (Runtime::Dlssg3109, "dinput8.dll") => Source {
            url: raw!("alternatives/dinput8.dll"),
            sha256: "4fbad08bb9360a1cd1309a82586353484fe64b7220199344ab83fae308176c6b",
        },
        (Runtime::Dlssg3109, "dxgi.dll") => Source {
            url: raw!("alternatives/dxgi.dll"),
            sha256: "52e67020a725c74d5dee62aee4dc166a2f4e5c3adabf04ecab3005ab968fc24d",
        },
        (Runtime::Dlssg3109, "d3d12.dll") => Source {
            url: raw!("alternatives/d3d12.dll"),
            sha256: "64b7ecc3f6d70011feed8dfaa20ce500522e8b04170142e2a2000146b4d7ac71",
        },
        (Runtime::Dlssg3101, "version.dll") => Source {
            url: raw!("310.1/version.dll"),
            sha256: "4ba7fecc68e2229193757839f326176a4f0f4325baab3c324007e0ea844e4d17",
        },
        (Runtime::Dlssg3101, "winmm.dll") => Source {
            url: raw!("310.1/alternatives/winmm.dll"),
            sha256: "a62e669d2ceae7645cbafe98d1df69892ede07bf78166acf2185a8beb9843a85",
        },
        (Runtime::Dlssg3101, "dbghelp.dll") => Source {
            url: raw!("310.1/alternatives/dbghelp.dll"),
            sha256: "fc58ee19b315732821ea3216d4d360972c38ce076ebc7d730b3f601c0e5c4a2c",
        },
        (Runtime::Dlssg3101, "dinput8.dll") => Source {
            url: raw!("310.1/alternatives/dinput8.dll"),
            sha256: "ed56447cefa8b5836ec39116db620c289cac783fba938a9740c7a1b845a6ffaa",
        },
        (Runtime::Dlssg3101, "dxgi.dll") => Source {
            url: raw!("310.1/alternatives/dxgi.dll"),
            sha256: "682e9a1318c5e2cb4ccf490c6cabfa2f119bf2f4793ced9c68ea56f3a21dc288",
        },
        (Runtime::Dlssg3101, "d3d12.dll") => Source {
            url: raw!("310.1/alternatives/d3d12.dll"),
            sha256: "1b6c39e4e2aa34e216d70d991ecb3d83eb6bdc01fa5e977a2e837f4433f77f2b",
        },
        _ => return None,
    };
    Some(s)
}

macro_rules! sm_asset {
    ($name:literal) => {
        concat!(
            "https://github.com/ShaggyLorean/smooth-motion-rtx30-winmm/releases/download/v0.2.0/",
            $name
        )
    };
}

pub fn sm_source(proxy: &str) -> Option<Source> {
    let s = match proxy {
        "winmm.dll" => Source {
            url: sm_asset!("winmm.dll"),
            sha256: "6842b0311120cfca0eca90407dab88bc6922a36e6a2b17233b1594adc8f233f3",
        },
        "version.dll" => Source {
            url: sm_asset!("version.dll"),
            sha256: "94f142953bcc5b34e2564c44be76e9efed30b3124444e6fbddfa06f604b2e781",
        },
        "dinput8.dll" => Source {
            url: sm_asset!("dinput8.dll"),
            sha256: "342d5d900df8e95c8d0fa7bc96320976378ef6ea1b7e663dc9454f4dfca1cadb",
        },
        "dsound.dll" => Source {
            url: sm_asset!("dsound.dll"),
            sha256: "e2206c98b5d52860b7a554838a33f19b7641f2eea491d94944c114fdeebd945e",
        },
        "hid.dll" => Source {
            url: sm_asset!("hid.dll"),
            sha256: "5cafa51e66fbd1da7392c595e12434acc6ad1a3fc2f662556a07fb037c23400b",
        },
        "winhttp.dll" => Source {
            url: sm_asset!("winhttp.dll"),
            sha256: "c2312e3dd2d6c00df41ef3d7f0ae1e71fd3ce6620e9adb158c2841f162e746ac",
        },
        "wininet.dll" => Source {
            url: sm_asset!("wininet.dll"),
            sha256: "9c77e33644e62ea87df12bc551e924b72052b179386ca1a65f3e838e41436541",
        },
        "dwmapi.dll" => Source {
            url: sm_asset!("dwmapi.dll"),
            sha256: "ef467f9c7ee501ac360d2e2a0b1414758c5d76fcb0409e56bf8db87a0f6b4b5e",
        },
        "xinput1_4.dll" => Source {
            url: sm_asset!("xinput1_4.dll"),
            sha256: "49573dbf9634e5772deb2d0fe9440a34524e702c0677bba3599c4db387953e03",
        },
        "xinput1_3.dll" => Source {
            url: sm_asset!("xinput1_3.dll"),
            sha256: "ebe288708649dd99046020d30ec7b324c1ab0d0d7c9818319afd5dbec7cbf481",
        },
        "dbghelp.dll" => Source {
            url: sm_asset!("dbghelp.dll"),
            sha256: "869a06e6355e8d5cf391b5c48fc09ef3f9759fa1afa03418d0337fa49c8e0f67",
        },
        _ => return None,
    };
    Some(s)
}

pub const SM_OLD_HASHES: [&str; 1] = ["1ea71b82635fc72d851e8fa3b03bb4eb0a0de9adcb273f5e760b8cfce17b602a"];

pub fn is_sm_hash(sha256: &str) -> bool {
    SM_OLD_HASHES.contains(&sha256) || SM_PROXIES.iter().any(|p| sm_source(p).is_some_and(|s| s.sha256 == sha256))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_complete() {
        for r in [Runtime::Dlssg3109, Runtime::Dlssg3101] {
            for p in FG_PROXIES {
                let s = fg_source(p, r).unwrap_or_else(|| panic!("no source for {p} {r:?}"));
                assert!(s.url.contains(SDLI_COMMIT));
                assert_eq!(s.sha256.len(), 64);
            }
        }
        assert!(fg_source("version.dll", Runtime::Legacy010).is_some());
        assert!(fg_source("winmm.dll", Runtime::Legacy010).is_none());
        assert!(fg_source("dwmapi.dll", Runtime::Dlssg3109).is_none());
    }

    #[test]
    fn sm_sources_complete() {
        for p in SM_PROXIES {
            let s = sm_source(p).unwrap_or_else(|| panic!("no Smooth Motion source for {p}"));
            assert!(s.url.ends_with(p));
            assert!(s.url.contains(SM_VERSION));
            assert_eq!(s.sha256.len(), 64);
            assert!(is_sm_hash(s.sha256));
            assert!(TABLE_PROXIES.contains(&p));
        }
        for p in FG_PROXIES {
            assert!(TABLE_PROXIES.contains(&p));
        }
        assert!(is_sm_hash(SM_OLD_HASHES[0]));
        assert!(!is_sm_hash("00"));
        assert!(sm_source("dxgi.dll").is_none());
    }

    #[test]
    fn runtime_facts() {
        assert_eq!(Runtime::ALL.iter().map(|r| r.tag()).collect::<Vec<_>>(), ["310.9", "310.1", "0.1.0"]);
        assert_eq!(Runtime::Dlssg3109.max_generated_frames(), 5);
        assert_eq!(Runtime::Dlssg3101.max_generated_frames(), 3);
        assert_eq!(Runtime::Legacy010.proxies(), &["version.dll"]);
    }
}
