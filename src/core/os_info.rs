use std::fs;
use std::process::Command;
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub distro_name: String,
    pub kernel_version: String,
    pub gnome_version: String,
    pub session_type: String,
    pub machine_model: String,
    pub host_name: String,
    pub architecture: String,
    pub board_vendor: String,
    pub board_name: String,
    pub board_version: String,
    pub bios_vendor: String,
    pub bios_version: String,
    pub bios_date: String,
}

pub struct OsCollector;

impl OsCollector {
    pub fn read() -> OsInfo {
        let distro_name = System::long_os_version()
            .or_else(System::name)
            .unwrap_or_else(Self::read_os_release);

        let kernel_version = System::kernel_version()
            .map(|k| format!("Linux {}", k))
            .unwrap_or_else(|| "Linux (Desconhecido)".into());

        let gnome_version = Self::get_gnome_version();

        let session_type = match std::env::var("XDG_SESSION_TYPE")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "wayland" => "Wayland",
            "x11" => "X11",
            other if !other.is_empty() => other,
            _ => "Desconhecido",
        }
        .to_string();

        let machine_model = Self::read_dmi("product_name").unwrap_or_else(|| "Computador Linux".into());
        let board_vendor = Self::read_dmi("board_vendor").unwrap_or_else(|| "Desconhecido".into());
        let board_name = Self::read_dmi("board_name").unwrap_or_else(|| "Desconhecido".into());
        let board_version = Self::read_dmi("board_version").unwrap_or_else(|| "-".into());
        let bios_vendor = Self::read_dmi("bios_vendor").unwrap_or_else(|| "Desconhecido".into());
        let bios_version = Self::read_dmi("bios_version").unwrap_or_else(|| "-".into());
        let bios_date = Self::read_dmi("bios_date").unwrap_or_else(|| "-".into());

        let host_name = System::host_name().unwrap_or_else(|| "localhost".into());
        let architecture = System::cpu_arch();

        OsInfo {
            distro_name,
            kernel_version,
            gnome_version,
            session_type,
            machine_model,
            host_name,
            architecture,
            board_vendor,
            board_name,
            board_version,
            bios_vendor,
            bios_version,
            bios_date,
        }
    }

    fn read_dmi(field: &str) -> Option<String> {
        let p1 = format!("/sys/class/dmi/id/{}", field);
        let p2 = format!("/sys/devices/virtual/dmi/id/{}", field);
        fs::read_to_string(p1)
            .or_else(|_| fs::read_to_string(p2))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn read_os_release() -> String {
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
                    return val.trim_matches('"').to_string();
                }
            }
        }
        "Linux".to_string()
    }

    fn get_gnome_version() -> String {
        if let Ok(output) = Command::new("gnome-shell").arg("--version").output() {
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).trim().to_string();
            }
        }
        "GNOME Shell".to_string()
    }

    pub fn format_uptime() -> String {
        let uptime_secs = System::uptime();
        let days = uptime_secs / 86400;
        let hours = (uptime_secs % 86400) / 3600;
        let minutes = (uptime_secs % 3600) / 60;

        if days > 0 {
            format!("{}d {}h {}min", days, hours, minutes)
        } else if hours > 0 {
            format!("{}h {}min", hours, minutes)
        } else {
            format!("{}min", minutes)
        }
    }
}
