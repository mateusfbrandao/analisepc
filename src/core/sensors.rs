use sysinfo::Components;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TemperatureSensor {
    pub label: String,
    pub temperature_c: f32,
    pub max_c: Option<f32>,
    pub critical_c: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct FanSensor {
    pub label: String,
    pub rpm: u32,
}

#[derive(Debug, Clone)]
pub struct SensorsReport {
    pub temperatures: Vec<TemperatureSensor>,
    pub fans: Vec<FanSensor>,
}

pub struct SensorsCollector {
    components: Components,
}

impl SensorsCollector {
    pub fn new() -> Self {
        let components = Components::new_with_refreshed_list();
        Self { components }
    }

    pub fn refresh(&mut self) -> SensorsReport {
        self.components.refresh(true);

        let mut temperatures: Vec<TemperatureSensor> = self
            .components
            .iter()
            .map(|c| TemperatureSensor {
                label: c.label().to_string(),
                temperature_c: c.temperature().unwrap_or(0.0),
                max_c: c.max(),
                critical_c: c.critical(),
            })
            .collect();

        // Se sysinfo não encontrar ou para enriquecer com ventoinhas, lemos /sys/class/hwmon
        let fans = Self::read_hwmon_fans();

        if temperatures.is_empty() {
            temperatures = Self::read_hwmon_temps();
        }

        SensorsReport { temperatures, fans }
    }

    fn read_hwmon_fans() -> Vec<FanSensor> {
        let mut fans = Vec::new();
        let hwmon_dir = Path::new("/sys/class/hwmon");
        if let Ok(entries) = fs::read_dir(hwmon_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = fs::read_to_string(path.join("name"))
                    .unwrap_or_else(|_| "hwmon".into())
                    .trim()
                    .to_string();

                // Procura arquivos fan*_input
                for i in 1..=8 {
                    let fan_file = path.join(format!("fan{}_input", i));
                    if let Ok(content) = fs::read_to_string(&fan_file) {
                        if let Ok(rpm) = content.trim().parse::<u32>() {
                            let label_file = path.join(format!("fan{}_label", i));
                            let fan_label = fs::read_to_string(label_file)
                                .unwrap_or_else(|_| format!("{}: Fan {}", name, i))
                                .trim()
                                .to_string();

                            fans.push(FanSensor {
                                label: fan_label,
                                rpm,
                            });
                        }
                    }
                }
            }
        }
        fans
    }

    fn read_hwmon_temps() -> Vec<TemperatureSensor> {
        let mut temps = Vec::new();
        let hwmon_dir = Path::new("/sys/class/hwmon");
        if let Ok(entries) = fs::read_dir(hwmon_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = fs::read_to_string(path.join("name"))
                    .unwrap_or_else(|_| "Sensor".into())
                    .trim()
                    .to_string();

                for i in 1..=16 {
                    let temp_file = path.join(format!("temp{}_input", i));
                    if let Ok(content) = fs::read_to_string(&temp_file) {
                        if let Ok(milli_c) = content.trim().parse::<f32>() {
                            let temp_c = milli_c / 1000.0;
                            let label_file = path.join(format!("temp{}_label", i));
                            let label = fs::read_to_string(label_file)
                                .unwrap_or_else(|_| format!("{}: Temp {}", name, i))
                                .trim()
                                .to_string();

                            temps.push(TemperatureSensor {
                                label,
                                temperature_c: temp_c,
                                max_c: None,
                                critical_c: None,
                            });
                        }
                    }
                }
            }
        }
        temps
    }
}
