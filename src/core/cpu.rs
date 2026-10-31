use sysinfo::{CpuRefreshKind, RefreshKind, System};

#[derive(Debug, Clone)]
pub struct CpuCoreInfo {
    pub name: String,
    pub frequency_mhz: u64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone)]
pub struct CpuInfo {
    pub brand: String,
    pub vendor_id: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub global_usage_percent: f32,
    pub cores: Vec<CpuCoreInfo>,
}

pub struct CpuCollector {
    sys: System,
}

impl CpuCollector {
    pub fn new() -> Self {
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );
        sys.refresh_cpu_all();
        Self { sys }
    }

    pub fn refresh(&mut self) -> CpuInfo {
        // sysinfo precisa de duas leituras ou um pequeno intervalo para calcular o delta de uso de CPU
        self.sys.refresh_cpu_all();

        let cpus = self.sys.cpus();
        let brand = cpus.first().map(|c| c.brand().to_string()).unwrap_or_else(|| "Desconhecido".into());
        let vendor_id = cpus.first().map(|c| c.vendor_id().to_string()).unwrap_or_else(|| "Desconhecido".into());
        let physical_cores = System::physical_core_count().unwrap_or(cpus.len());
        let logical_cores = cpus.len();
        let global_usage_percent = self.sys.global_cpu_usage();

        let cores = cpus
            .iter()
            .map(|c| CpuCoreInfo {
                name: c.name().to_string(),
                frequency_mhz: c.frequency(),
                usage_percent: c.cpu_usage(),
            })
            .collect();

        CpuInfo {
            brand,
            vendor_id,
            physical_cores,
            logical_cores,
            global_usage_percent,
            cores,
        }
    }
}
