pub const SDLI_REPO: &str = "sdli1995/dlssg_for_sm86";
pub const SDLI_COMMIT: &str = "9621db573e07ed54f50c15bbb585ed9a7bdfac28";
pub const SDLI_VERSION: &str = "0.3.5";
pub const SDLI_LEGACY_VERSION: &str = "0.1.0";

pub const AUTOPILOT_REPO: &str = "Kizzuwatnaa/DLSS5-Autopilot";

pub const REFRAMEWORK_REPO: &str = "praydog/REFramework-nightly";
pub const REFRAMEWORK_ASSET: &str = "REFramework.zip";

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
            _ => &ALL_PROXIES,
        }
    }
}

pub const ALL_PROXIES: [&str; 6] = ["version.dll", "winmm.dll", "dbghelp.dll", "dinput8.dll", "dxgi.dll", "d3d12.dll"];

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_complete() {
        for r in [Runtime::Dlssg3109, Runtime::Dlssg3101] {
            for p in ALL_PROXIES {
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
    fn runtime_facts() {
        assert_eq!(Runtime::ALL.iter().map(|r| r.tag()).collect::<Vec<_>>(), ["310.9", "310.1", "0.1.0"]);
        assert_eq!(Runtime::Dlssg3109.max_generated_frames(), 5);
        assert_eq!(Runtime::Dlssg3101.max_generated_frames(), 3);
        assert_eq!(Runtime::Legacy010.proxies(), &["version.dll"]);
    }
}
