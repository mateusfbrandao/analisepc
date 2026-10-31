use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BatteryReport {
    pub is_present: bool,
    pub capacity_percent: u32,
    pub status: String,
    pub health: String,
    pub charge_now_mah: f32,
    pub charge_full_mah: f32,
    pub charge_design_mah: f32,
    pub wear_percent: f32,
    pub voltage_v: f32,
    pub power_w: f32,
    pub manufacturer: String,
    pub model: String,
    pub technology: String,
    pub temperature_c: Option<f32>,
}

pub struct BatteryCollector;

impl BatteryCollector {
    pub fn read() -> Option<BatteryReport> {
        let bat_path = Path::new("/sys/class/power_supply/BAT0");
        if !bat_path.exists() {
            return None;
        }

        let read_val = |file: &str| -> String {
            fs::read_to_string(bat_path.join(file))
                .unwrap_or_default()
                .trim()
                .to_string()
        };

        let read_u64 = |file: &str| -> u64 {
            read_val(file).parse::<u64>().unwrap_or(0)
        };

        let capacity_percent = read_u64("capacity") as u32;
        let status_raw = read_val("status");
        let status = match status_raw.as_str() {
            "Charging" => "Carregando",
            "Discharging" => "Descarregando",
            "Full" => "Carregada (100%)",
            "Not charging" => "Conectada (Não carregando)",
            other => other,
        }.to_string();

        let health_raw = read_val("health");
        let health = match health_raw.as_str() {
            "Good" => "Boa",
            "Overheat" => "Superaquecimento",
            "Dead" => "Danificada",
            other => other,
        }.to_string();

        let charge_now = read_u64("charge_now");
        let charge_full = read_u64("charge_full");
        let charge_design = read_u64("charge_full_design");

        let charge_now_mah = charge_now as f32 / 1000.0;
        let charge_full_mah = charge_full as f32 / 1000.0;
        let charge_design_mah = charge_design as f32 / 1000.0;

        let wear_percent = if charge_design > 0 && charge_full > 0 {
            let ratio = charge_full as f32 / charge_design as f32;
            (1.0 - ratio).max(0.0) * 100.0
        } else {
            0.0
        };

        let voltage_v = read_u64("voltage_now") as f32 / 1_000_000.0;
        let current_a = read_u64("current_now") as f32 / 1_000_000.0;
        let power_w = voltage_v * current_a;

        let manufacturer = read_val("manufacturer");
        let model = read_val("model_name");
        let technology = read_val("technology");

        let temp_raw = read_u64("temp");
        let temperature_c = if temp_raw > 0 {
            Some(temp_raw as f32 / 10.0)
        } else {
            None
        };

        Some(BatteryReport {
            is_present: true,
            capacity_percent,
            status,
            health,
            charge_now_mah,
            charge_full_mah,
            charge_design_mah,
            wear_percent,
            voltage_v,
            power_w,
            manufacturer,
            model,
            technology,
            temperature_c,
        })
    }
}
