use std::process::Command;

use crate::winutil::no_window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuClass {
    Sm75,
    Sm86,
    NativeFg,
    Unsupported,
}

impl GpuClass {
    pub fn router(self) -> Option<&'static str> {
        match self {
            GpuClass::Sm75 => Some("SM75"),
            GpuClass::Sm86 => Some("SM86"),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GpuClass::Sm75 => "Turing (SM75)",
            GpuClass::Sm86 => "Ampere (SM86)",
            GpuClass::NativeFg => "RTX 40/50, FG unlock not needed",
            GpuClass::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Gpu {
    pub name: String,
    pub driver: String,
    pub class: GpuClass,
}

pub fn classify(name: &str) -> GpuClass {
    let up = name.to_uppercase();
    let mut toks = up.split_whitespace().peekable();
    while let Some(t) = toks.next() {
        if t != "RTX" && t != "GTX" {
            continue;
        }
        let Some(n) = toks.peek() else { break };
        let digits: String = n.chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(num) = digits.parse::<u32>() else { continue };
        return match (t, num) {
            ("GTX", 1600..=1699) => GpuClass::Sm75,
            ("RTX", 2050) => GpuClass::Sm86,
            ("RTX", 2000..=2999) => GpuClass::Sm75,
            ("RTX", 3000..=3999) => GpuClass::Sm86,
            ("RTX", 4000..=5999) => GpuClass::NativeFg,
            _ => GpuClass::Unsupported,
        };
    }
    GpuClass::Unsupported
}

pub fn parse_smi(out: &str) -> Option<Gpu> {
    let line = out.lines().map(str::trim).find(|l| !l.is_empty())?;
    let (name, driver) = line.rsplit_once(',')?;
    let name = name.trim().to_string();
    Some(Gpu {
        class: classify(&name),
        name,
        driver: driver.trim().to_string(),
    })
}

pub fn detect() -> Result<Gpu, String> {
    let candidates = ["nvidia-smi", r"C:\Windows\System32\nvidia-smi.exe"];
    for exe in candidates {
        let out = no_window(&mut Command::new(exe))
            .args(["--query-gpu=name,driver_version", "--format=csv,noheader"])
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                let text = String::from_utf8_lossy(&o.stdout);
                if let Some(g) = parse_smi(&text) {
                    return Ok(g);
                }
            }
        }
    }
    Err("NVIDIA driver not found (nvidia-smi did not run)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes() {
        assert_eq!(classify("NVIDIA GeForce RTX 3090"), GpuClass::Sm86);
        assert_eq!(classify("NVIDIA GeForce RTX 3050 Laptop GPU"), GpuClass::Sm86);
        assert_eq!(classify("NVIDIA GeForce RTX 2070 SUPER"), GpuClass::Sm75);
        assert_eq!(classify("NVIDIA GeForce RTX 2050"), GpuClass::Sm86);
        assert_eq!(classify("NVIDIA GeForce GTX 1660 Ti"), GpuClass::Sm75);
        assert_eq!(classify("NVIDIA GeForce RTX 4070"), GpuClass::NativeFg);
        assert_eq!(classify("NVIDIA GeForce RTX 5090"), GpuClass::NativeFg);
        assert_eq!(classify("NVIDIA GeForce GTX 1080"), GpuClass::Unsupported);
        assert_eq!(classify("AMD Radeon RX 7800"), GpuClass::Unsupported);
    }

    #[test]
    fn parse_smi_line() {
        let g = parse_smi("NVIDIA GeForce RTX 3090, 616.86\n").unwrap();
        assert_eq!(g.driver, "616.86");
        assert_eq!(g.name, "NVIDIA GeForce RTX 3090");
        assert_eq!(g.class, GpuClass::Sm86);
    }
}
