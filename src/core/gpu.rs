use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub model: String,
    pub vendor: String,
    pub driver: String,
    pub vram: Option<String>,
    pub pci_id: String,
    pub is_dedicated: bool,
    pub vbios_version: Option<String>,
    pub pcie_link: Option<String>,
    pub clocks_info: Option<String>,
    pub power_info: Option<String>,
    pub temperature_c: Option<u32>,
    pub gpu_util_pct: Option<u32>,
    pub vram_used_mib: Option<u32>,
    pub vram_total_mib: Option<u32>,
    pub apis_supported: String,
}

pub struct GpuCollector;

impl GpuCollector {
    pub fn list_gpus() -> Vec<GpuInfo> {
        let mut gpus = Vec::new();

        if let Ok(output) = Command::new("lspci").args(["-vmm", "-D"]).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let parsed = Self::parse_lspci(&text);
                if !parsed.is_empty() {
                    gpus = parsed;
                }
            }
        }

        if gpus.is_empty() {
            gpus = Self::read_sysfs_drm();
        }

        Self::enrich_gpu_details(&mut gpus);

        gpus
    }

    fn parse_lspci(text: &str) -> Vec<GpuInfo> {
        let mut list = Vec::new();
        let mut current_slot = String::new();
        let mut current_class = String::new();
        let mut current_vendor = String::new();
        let mut current_device = String::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                let is_gpu = current_class.contains("VGA")
                    || current_class.contains("3D")
                    || current_class.contains("Display");

                if is_gpu && !current_vendor.is_empty() {
                    let is_dedicated = current_vendor.contains("NVIDIA")
                        || (current_vendor.contains("AMD")
                            && !current_device.contains("Radeon Graphics"));

                    let driver = Self::find_driver_for_pci(&current_slot);
                    let pcie_link = Self::read_pcie_link_sysfs(&current_slot);

                    let apis_supported = if current_vendor.contains("NVIDIA") {
                        "Vulkan 1.3 • OpenGL 4.6 • CUDA • NVENC/NVDEC".to_string()
                    } else {
                        "Vulkan 1.3 • OpenGL 4.6 • OpenCL • VA-API".to_string()
                    };

                    list.push(GpuInfo {
                        model: current_device.clone(),
                        vendor: current_vendor.clone(),
                        driver,
                        vram: None,
                        pci_id: current_slot.clone(),
                        is_dedicated,
                        vbios_version: None,
                        pcie_link,
                        clocks_info: None,
                        power_info: None,
                        temperature_c: None,
                        gpu_util_pct: None,
                        vram_used_mib: None,
                        vram_total_mib: None,
                        apis_supported,
                    });
                }

                current_slot.clear();
                current_class.clear();
                current_vendor.clear();
                current_device.clear();
                continue;
            }

            if let Some((key, val)) = line.split_once(':') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "Slot" => current_slot = val.to_string(),
                    "Class" => current_class = val.to_string(),
                    "Vendor" => current_vendor = val.to_string(),
                    "Device" => current_device = val.to_string(),
                    _ => {}
                }
            }
        }

        list
    }

    fn find_driver_for_pci(slot: &str) -> String {
        let pci_path = Path::new("/sys/bus/pci/devices").join(slot).join("driver");
        if let Ok(link) = fs::read_link(pci_path) {
            link.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            "Desconhecido".into()
        }
    }

    fn read_pcie_link_sysfs(slot: &str) -> Option<String> {
        let base = Path::new("/sys/bus/pci/devices").join(slot);
        let cur_w = fs::read_to_string(base.join("current_link_width"))
            .ok()?
            .trim()
            .to_string();
        let max_w = fs::read_to_string(base.join("max_link_width"))
            .ok()?
            .trim()
            .to_string();
        let cur_s = fs::read_to_string(base.join("current_link_speed"))
            .ok()?
            .trim()
            .to_string();
        let max_s = fs::read_to_string(base.join("max_link_speed"))
            .ok()?
            .trim()
            .to_string();

        if max_w == "0" || max_w == "255" || max_s.is_empty() || max_s.contains("Unknown") {
            return None;
        }
        Some(format!(
            "Máx: x{} ({}) • Atual: x{} ({})",
            max_w, max_s, cur_w, cur_s
        ))
    }

    fn read_sysfs_drm() -> Vec<GpuInfo> {
        let mut gpus = Vec::new();
        let drm_dir = Path::new("/sys/class/drm");

        if let Ok(entries) = fs::read_dir(drm_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("card") && !file_name.contains('-') {
                    let dev_path = entry.path().join("device");
                    if dev_path.exists() {
                        let vendor_raw = fs::read_to_string(dev_path.join("vendor"))
                            .unwrap_or_default()
                            .trim()
                            .to_string();

                        let device_raw = fs::read_to_string(dev_path.join("device"))
                            .unwrap_or_default()
                            .trim()
                            .to_string();

                        let (vendor_name, is_dedicated) = match vendor_raw.as_str() {
                            "0x10de" => ("NVIDIA Corporation", true),
                            "0x1002" => ("Advanced Micro Devices, Inc. (AMD)", true),
                            "0x8086" => ("Intel Corporation", false),
                            other => (other, false),
                        };

                        let driver_name = dev_path
                            .join("driver")
                            .read_link()
                            .map(|p| {
                                p.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string()
                            })
                            .unwrap_or_else(|_| "Desconhecido".into());

                        let pci_slot = dev_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();

                        gpus.push(GpuInfo {
                            model: format!("Adaptador Gráfico ({})", device_raw),
                            vendor: vendor_name.to_string(),
                            driver: driver_name,
                            vram: None,
                            pci_id: pci_slot,
                            is_dedicated,
                            vbios_version: None,
                            pcie_link: None,
                            clocks_info: None,
                            power_info: None,
                            temperature_c: None,
                            gpu_util_pct: None,
                            vram_used_mib: None,
                            vram_total_mib: None,
                            apis_supported: "Vulkan • OpenGL".into(),
                        });
                    }
                }
            }
        }
        gpus
    }

    fn enrich_gpu_details(gpus: &mut [GpuInfo]) {
        // 1. Dados completos da NVIDIA via nvidia-smi (Estilo GPU-Z)
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,driver_version,vbios_version,pcie.link.gen.current,pcie.link.gen.max,pcie.link.width.current,pcie.link.width.max,clocks.current.graphics,clocks.max.graphics,clocks.current.memory,clocks.max.memory,power.draw,temperature.gpu,utilization.gpu,memory.used,memory.total,pstate",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let p: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if p.len() >= 17 {
                        let model_name = p[0];
                        let driver_ver = p[1];
                        let vbios = p[2];
                        let pcie_gen_cur = p[3];
                        let pcie_gen_max = p[4];
                        let pcie_w_cur = p[5];
                        let pcie_w_max = p[6];
                        let clk_gpu_cur = p[7];
                        let clk_gpu_max = p[8];
                        let clk_mem_cur = p[9];
                        let clk_mem_max = p[10];
                        let power_w = p[11];
                        let temp_c = p[12].parse::<u32>().ok();
                        let util_gpu = p[13].parse::<u32>().ok();
                        let mem_used = p[14].parse::<u32>().ok();
                        let mem_total = p[15].parse::<u32>().ok();
                        let pstate = p[16];

                        for gpu in gpus.iter_mut() {
                            if gpu.vendor.contains("NVIDIA") {
                                gpu.model = model_name.to_string();
                                gpu.driver = format!("nvidia v{}", driver_ver);
                                gpu.vbios_version = Some(vbios.to_string());
                                gpu.pcie_link = Some(format!(
                                    "PCIe Gen {} x{} (Em uso: Gen {} x{})",
                                    pcie_gen_max, pcie_w_max, pcie_gen_cur, pcie_w_cur
                                ));
                                gpu.clocks_info = Some(format!(
                                    "Núcleo: {} MHz (Boost: {} MHz) • Memória: {} MHz (Máx: {} MHz)",
                                    clk_gpu_cur, clk_gpu_max, clk_mem_cur, clk_mem_max
                                ));
                                gpu.power_info = Some(format!("{} W (Estado de Energia: {})", power_w, pstate));
                                gpu.temperature_c = temp_c;
                                gpu.gpu_util_pct = util_gpu;
                                gpu.vram_used_mib = mem_used;
                                gpu.vram_total_mib = mem_total;
                                if let (Some(used), Some(total)) = (mem_used, mem_total) {
                                    gpu.vram = Some(format!(
                                        "{} MiB usados de {} MiB ({:.1} GiB GDDR6)",
                                        used,
                                        total,
                                        total as f64 / 1024.0
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Dados de frequências DRM para GPU Integrada Intel
        for gpu in gpus.iter_mut() {
            if gpu.vendor.contains("Intel") {
                if gpu.vram.is_none() {
                    gpu.vram = Some("Memória Dinâmica Compartilhada (RAM DDR5)".into());
                }
                if gpu.pcie_link.is_none() {
                    gpu.pcie_link = Some("Barramento Interno Integrado ao SoC (Ring Bus)".into());
                }
                // Ler frequência atual e máxima em /sys/class/drm/card*
                if let Some((cur, max, min)) = Self::read_intel_drm_clocks() {
                    gpu.clocks_info = Some(format!(
                        "Núcleo Atual: {} MHz (Boost Máx: {} MHz • Mín: {} MHz)",
                        cur, max, min
                    ));
                }
            }
        }
    }

    fn read_intel_drm_clocks() -> Option<(u32, u32, u32)> {
        let drm_dir = Path::new("/sys/class/drm");
        if let Ok(entries) = fs::read_dir(drm_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if let Ok(cur_str) = fs::read_to_string(p.join("gt_cur_freq_mhz")) {
                    let cur = cur_str.trim().parse::<u32>().unwrap_or(0);
                    let max = fs::read_to_string(p.join("gt_max_freq_mhz"))
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .unwrap_or(0);
                    let min = fs::read_to_string(p.join("gt_min_freq_mhz"))
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .unwrap_or(0);
                    return Some((cur, max, min));
                }
            }
        }
        None
    }
}
