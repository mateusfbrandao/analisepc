use std::fs;
use std::path::Path;
use std::process::Command;
use sysinfo::Disks;

#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_gib: f64,
    pub used_gib: f64,
    pub available_gib: f64,
    pub used_percent: f32,
}

#[derive(Debug, Clone)]
pub struct SmartHealthInfo {
    pub health_remaining_pct: u32,
    pub wear_used_pct: u32,
    pub avail_spare_pct: u32,
    pub power_on_hours: u64,
    pub power_cycles: u64,
    pub unsafe_shutdowns: u64,
    pub total_written_tb: f64,
    pub total_read_tb: f64,
    pub media_errors: u64,
    pub nvme_version: String,
    pub verdict: String,
}

#[derive(Debug, Clone)]
pub struct PhysicalDiskInfo {
    pub name: String,
    pub model: String,
    pub serial: String,
    pub firmware: String,
    pub state: String,
    pub temperature_c: Option<f32>,
    pub smart: Option<SmartHealthInfo>,
    pub partitions: Vec<PartitionInfo>,
}

pub struct DiskCollector;

impl DiskCollector {
    pub fn list_disks() -> Vec<PhysicalDiskInfo> {
        let disks = Disks::new_with_refreshed_list();
        let mut partitions = Vec::new();

        for d in &disks {
            let total = d.total_space() as f64 / (1024.0 * 1024.0 * 1024.0);
            let available = d.available_space() as f64 / (1024.0 * 1024.0 * 1024.0);
            let used = (total - available).max(0.0);
            let used_percent = if total > 0.0 {
                (used / total * 100.0) as f32
            } else {
                0.0
            };

            let mount = d.mount_point().to_string_lossy().to_string();
            if !mount.starts_with("/snap")
                && !mount.starts_with("/var/lib/docker")
                && !mount.starts_with("/run")
            {
                partitions.push(PartitionInfo {
                    name: d.name().to_string_lossy().to_string(),
                    mount_point: mount,
                    file_system: d.file_system().to_string_lossy().to_string(),
                    total_gib: total,
                    used_gib: used,
                    available_gib: available,
                    used_percent,
                });
            }
        }

        let mut physical_disks = Vec::new();

        let nvme_dir = Path::new("/sys/class/nvme");
        if let Ok(entries) = fs::read_dir(nvme_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                let ctrl_name = entry.file_name().to_string_lossy().to_string();
                let model = fs::read_to_string(p.join("model"))
                    .unwrap_or_else(|_| "NVMe SSD".into())
                    .trim()
                    .to_string();
                let serial = fs::read_to_string(p.join("serial"))
                    .unwrap_or_else(|_| "Desconhecido".into())
                    .trim()
                    .to_string();
                let firmware = fs::read_to_string(p.join("firmware_rev"))
                    .unwrap_or_else(|_| "Desconhecido".into())
                    .trim()
                    .to_string();
                let state = fs::read_to_string(p.join("state"))
                    .unwrap_or_else(|_| "live".into())
                    .trim()
                    .to_string();

                let mut temp_c = None;
                if let Ok(hwmon_entries) = fs::read_dir(&p) {
                    for h in hwmon_entries.flatten() {
                        if h.file_name().to_string_lossy().starts_with("hwmon") {
                            let temp_file = h.path().join("temp1_input");
                            if let Ok(content) = fs::read_to_string(temp_file) {
                                if let Ok(milli) = content.trim().parse::<f32>() {
                                    temp_c = Some(milli / 1000.0);
                                }
                            }
                        }
                    }
                }

                let block_dev = format!("/dev/{}n1", ctrl_name);
                let smart = Self::read_udisks_nvme_smart(&block_dev);

                physical_disks.push(PhysicalDiskInfo {
                    name: ctrl_name,
                    model,
                    serial,
                    firmware,
                    state: if state == "live" {
                        "Saudável (Live)".into()
                    } else {
                        state
                    },
                    temperature_c: temp_c,
                    smart,
                    partitions: partitions.clone(),
                });
            }
        }

        if physical_disks.is_empty() {
            physical_disks.push(PhysicalDiskInfo {
                name: "Disco Principal".into(),
                model: "Armazenamento do Sistema".into(),
                serial: "-".into(),
                firmware: "-".into(),
                state: "Operacional".into(),
                temperature_c: None,
                smart: None,
                partitions,
            });
        }

        physical_disks
    }

    fn read_udisks_nvme_smart(block_dev: &str) -> Option<SmartHealthInfo> {
        // 1. Descobrir o object-path do Drive no UDisks2
        let info_out = Command::new("udisksctl")
            .args(["info", "-b", block_dev])
            .output()
            .ok()?;
        let info_text = String::from_utf8_lossy(&info_out.stdout);

        let mut drive_obj_path = String::new();
        for line in info_text.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("Drive:") {
                drive_obj_path = rest.trim().trim_matches('\'').to_string();
                break;
            }
        }

        if drive_obj_path.is_empty() || drive_obj_path == "/" {
            return None;
        }

        // 2. Ler propriedades gerais do Controller (SmartPowerOnHours, NVMeRevision)
        let short_path = drive_obj_path
            .strip_prefix("/org/freedesktop/UDisks2/")
            .unwrap_or(&drive_obj_path);
        let drv_out = Command::new("udisksctl")
            .args(["info", "-p", short_path])
            .output()
            .ok()?;
        let drv_text = String::from_utf8_lossy(&drv_out.stdout);

        let mut power_on_hours = 0u64;
        let mut nvme_version = "1.4".to_string();
        for line in drv_text.lines() {
            let t = line.trim();
            if let Some(val) = t.strip_prefix("SmartPowerOnHours:") {
                power_on_hours = val.trim().parse::<u64>().unwrap_or(0);
            } else if let Some(val) = t.strip_prefix("NVMeRevision:") {
                nvme_version = val.trim().to_string();
            }
        }

        // 3. Chamar SmartGetAttributes via gdbus
        let gdbus_out = Command::new("gdbus")
            .args([
                "call",
                "--system",
                "--dest",
                "org.freedesktop.UDisks2",
                "--object-path",
                &drive_obj_path,
                "--method",
                "org.freedesktop.UDisks2.NVMe.Controller.SmartGetAttributes",
                "{}",
            ])
            .output()
            .ok()?;

        if !gdbus_out.status.success() {
            return None;
        }

        let raw = String::from_utf8_lossy(&gdbus_out.stdout);
        let wear_used_pct = Self::extract_byte_hex(&raw, "'percent_used':").unwrap_or(0);
        let avail_spare_pct = Self::extract_byte_hex(&raw, "'avail_spare':").unwrap_or(100);
        let total_read_bytes = Self::extract_u64(&raw, "'total_data_read':").unwrap_or(0);
        let total_written_bytes = Self::extract_u64(&raw, "'total_data_written':").unwrap_or(0);
        let power_cycles = Self::extract_u64(&raw, "'power_cycles':").unwrap_or(0);
        let unsafe_shutdowns = Self::extract_u64(&raw, "'unsafe_shutdowns':").unwrap_or(0);
        let media_errors = Self::extract_u64(&raw, "'media_errors':").unwrap_or(0);

        let health_remaining_pct = 100u32.saturating_sub(wear_used_pct);
        let total_written_tb = total_written_bytes as f64 / 1_000_000_000_000.0;
        let total_read_tb = total_read_bytes as f64 / 1_000_000_000_000.0;

        let verdict = if media_errors > 0 || health_remaining_pct < 20 {
            "Crítico / Atenção".to_string()
        } else if health_remaining_pct < 50 {
            "Moderado".to_string()
        } else {
            "Excelente (Bom)".to_string()
        };

        Some(SmartHealthInfo {
            health_remaining_pct,
            wear_used_pct,
            avail_spare_pct,
            power_on_hours,
            power_cycles,
            unsafe_shutdowns,
            total_written_tb,
            total_read_tb,
            media_errors,
            nvme_version,
            verdict,
        })
    }

    fn extract_byte_hex(text: &str, key: &str) -> Option<u32> {
        let pos = text.find(key)?;
        let after = &text[pos + key.len()..];
        let hex_pos = after.find("0x")?;
        let hex_sub = &after[hex_pos + 2..];
        let end = hex_sub
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(hex_sub.len());
        u32::from_str_radix(&hex_sub[..end], 16).ok()
    }

    fn extract_u64(text: &str, key: &str) -> Option<u64> {
        let pos = text.find(key)?;
        let after = &text[pos + key.len()..];
        let u64_pos = after.find("uint64 ")?;
        let num_sub = &after[u64_pos + 7..];
        let end = num_sub
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(num_sub.len());
        num_sub[..end].parse::<u64>().ok()
    }
}
