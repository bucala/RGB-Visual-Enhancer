//! Host hardware identification (motherboard, BIOS, CPU, OS).

use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub board_vendor: Option<String>,
    pub board_name: Option<String>,
    pub system_product: Option<String>,
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub cpu: Option<String>,
}

impl SystemInfo {
    pub fn collect() -> Self {
        let mut info = SystemInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            ..Default::default()
        };
        platform::fill(&mut info);
        info
    }
}

fn clean(s: String) -> Option<String> {
    let s = s.trim().trim_matches('\0').trim().to_string();
    let placeholder = [
        "",
        "To be filled by O.E.M.",
        "Default string",
        "System Product Name",
    ];
    (!placeholder.contains(&s.as_str())).then_some(s)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::fs;

    fn dmi(field: &str) -> Option<String> {
        fs::read_to_string(format!("/sys/class/dmi/id/{field}"))
            .ok()
            .and_then(clean)
    }

    pub fn fill(info: &mut SystemInfo) {
        info.board_vendor = dmi("board_vendor");
        info.board_name = dmi("board_name");
        info.system_product = dmi("product_name");
        info.bios_vendor = dmi("bios_vendor");
        info.bios_version = dmi("bios_version");
        info.cpu = fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .and_then(|(_, v)| clean(v.to_string()))
        });
        if let Ok(release) = fs::read_to_string("/etc/os-release") {
            if let Some(name) = release.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")) {
                info.os = name.trim_matches('"').to_string();
            }
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    pub fn fill(info: &mut SystemInfo) {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(bios) = hklm.open_subkey(r"HARDWARE\DESCRIPTION\System\BIOS") {
            let get = |name: &str| bios.get_value::<String, _>(name).ok().and_then(clean);
            info.board_vendor = get("BaseBoardManufacturer");
            info.board_name = get("BaseBoardProduct");
            info.system_product = get("SystemProductName");
            info.bios_vendor = get("BIOSVendor");
            info.bios_version = get("BIOSVersion");
        }
        if let Ok(cpu) = hklm.open_subkey(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") {
            info.cpu = cpu
                .get_value::<String, _>("ProcessorNameString")
                .ok()
                .and_then(clean);
        }
        if let Ok(cv) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
            if let Ok(name) = cv.get_value::<String, _>("ProductName") {
                let build = cv
                    .get_value::<String, _>("CurrentBuild")
                    .unwrap_or_default();
                info.os = format!("{name} (build {build})");
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    pub fn fill(_info: &mut super::SystemInfo) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholders_are_dropped() {
        assert_eq!(super::clean("To be filled by O.E.M.".into()), None);
        assert_eq!(
            super::clean(" ASUSTeK COMPUTER INC.\n".into()).as_deref(),
            Some("ASUSTeK COMPUTER INC.")
        );
    }

    #[test]
    fn collect_does_not_panic() {
        let info = super::SystemInfo::collect();
        assert!(!info.os.is_empty());
    }
}
