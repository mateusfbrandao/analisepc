use std::collections::HashMap;
use std::process::Command;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

#[derive(Debug, Clone)]
pub struct RamModuleInfo {
    pub slot_locator: String,
    pub bank_locator: String,
    pub size_gib: u64,
    pub mem_type: String,
    pub form_factor: String,
    pub speed_mts: u32,
    pub configured_speed_mts: u32,
    pub manufacturer: String,
    pub serial_number: String,
    pub part_number: String,
    pub rank: u32,
}

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub free_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

impl MemoryInfo {
    pub fn used_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used_bytes as f64 / self.total_bytes as f64 * 100.0) as f32
        }
    }

    pub fn total_gib(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn used_gib(&self) -> f64 {
        self.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

pub struct MemoryCollector {
    sys: System,
}

impl MemoryCollector {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_memory();
        Self { sys }
    }

    pub fn refresh(&mut self) -> MemoryInfo {
        self.sys.refresh_memory();

        MemoryInfo {
            total_bytes: self.sys.total_memory(),
            used_bytes: self.sys.used_memory(),
            available_bytes: self.sys.available_memory(),
            free_bytes: self.sys.free_memory(),
            swap_total_bytes: self.sys.total_swap(),
            swap_used_bytes: self.sys.used_swap(),
        }
    }

    /// Lê os módulos físicos de RAM (slots DIMM/SODIMM) via base DMI do udevadm sem exigir root
    pub fn read_ram_modules() -> Vec<RamModuleInfo> {
        let mut modules = Vec::new();

        let output = match Command::new("udevadm")
            .args(["info", "--path=/sys/devices/virtual/dmi/id"])
            .output()
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            _ => return modules,
        };

        let mut props: HashMap<String, String> = HashMap::new();
        for line in output.lines() {
            if let Some(rest) = line.strip_prefix("E: ") {
                if let Some((k, v)) = rest.split_once('=') {
                    props.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }

        let num_devices = props
            .get("MEMORY_ARRAY_NUM_DEVICES")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(8);

        for idx in 0..num_devices {
            let prefix = format!("MEMORY_DEVICE_{}_", idx);
            let size_bytes = props
                .get(&format!("{}SIZE", prefix))
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);

            if size_bytes == 0 {
                continue;
            }

            let size_gib = size_bytes / (1024 * 1024 * 1024);
            let slot_locator = props
                .get(&format!("{}LOCATOR", prefix))
                .cloned()
                .unwrap_or_else(|| format!("Slot {}", idx));
            let bank_locator = props
                .get(&format!("{}BANK_LOCATOR", prefix))
                .cloned()
                .unwrap_or_else(|| "-".into());
            let mem_type = props
                .get(&format!("{}TYPE", prefix))
                .cloned()
                .unwrap_or_else(|| "DDR".into());
            let form_factor = props
                .get(&format!("{}FORM_FACTOR", prefix))
                .cloned()
                .unwrap_or_else(|| "DIMM".into());
            let speed_mts = props
                .get(&format!("{}SPEED_MTS", prefix))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let configured_speed_mts = props
                .get(&format!("{}CONFIGURED_SPEED_MTS", prefix))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(speed_mts);
            let raw_mfg = props
                .get(&format!("{}MANUFACTURER", prefix))
                .cloned()
                .unwrap_or_else(|| "Desconhecido".into());
            let manufacturer = Self::decode_manufacturer(&raw_mfg);
            let serial_number = props
                .get(&format!("{}SERIAL_NUMBER", prefix))
                .cloned()
                .unwrap_or_else(|| "-".into());
            let part_number = props
                .get(&format!("{}PART_NUMBER", prefix))
                .cloned()
                .unwrap_or_else(|| "-".into());
            let rank = props
                .get(&format!("{}RANK", prefix))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1);

            modules.push(RamModuleInfo {
                slot_locator,
                bank_locator,
                size_gib,
                mem_type,
                form_factor,
                speed_mts,
                configured_speed_mts,
                manufacturer,
                serial_number,
                part_number,
                rank,
            });
        }

        modules
    }

    fn decode_manufacturer(raw: &str) -> String {
        let up = raw.to_uppercase();
        if up.contains("859B") {
            "Crucial / Micron (859B)".into()
        } else if up.contains("802C") {
            "Micron Technology (802C)".into()
        } else if up.contains("80AD") {
            "SK Hynix (80AD)".into()
        } else if up.contains("80CE") {
            "Samsung (80CE)".into()
        } else if up.contains("0198") {
            "Kingston (0198)".into()
        } else if up.contains("029E") {
            "Corsair (029E)".into()
        } else if up.contains("04CD") {
            "G.Skill (04CD)".into()
        } else if up.contains("04CB") {
            "ADATA (04CB)".into()
        } else {
            raw.to_string()
        }
    }
}
