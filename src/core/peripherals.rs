use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct NetworkInterfaceInfo {
    pub name: String,
    pub vendor: String,
    pub model: String,
    pub hw_model: String,
    pub driver: String,
    pub kind: String,
    pub state: String,
    pub mac_address: String,
    pub ipv4: String,
    pub link_speed: String,
    pub rx_total_bytes: u64,
    pub tx_total_bytes: u64,
    pub rx_speed_kbps: f64,
    pub tx_speed_kbps: f64,
}

#[derive(Debug, Clone)]
pub struct AudioEndpoint {
    pub name: String,
    pub volume_pct: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct AudioReport {
    pub server_name: String,
    pub sinks: Vec<AudioEndpoint>,
    pub sources: Vec<AudioEndpoint>,
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub connector: String,
    pub kind: String,
    pub resolution: String,
    pub gpu_card: String,
}

#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    pub bus_dev: String,
    pub vendor_product_id: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct SerialPortInfo {
    pub port_path: String,
    pub driver: String,
    pub description: String,
    pub baud_hint: String,
}

pub struct NetworkCollector {
    prev_stats: HashMap<String, (u64, u64, Instant)>,
    pci_models: HashMap<String, (String, String, String)>,
    ipv4_cache: HashMap<String, String>,
    tick_count: u32,
}

impl NetworkCollector {
    pub fn new() -> Self {
        let pci_models = Self::read_pci_network_models();
        let ipv4_cache = Self::read_ipv4_map();
        let mut collector = Self {
            prev_stats: HashMap::new(),
            pci_models,
            ipv4_cache,
            tick_count: 0,
        };
        let _ = collector.refresh();
        collector
    }

    fn split_vendor_and_model(raw: &str) -> (String, String) {
        let known_vendors = [
            "Realtek Semiconductor Co., Ltd.",
            "Intel Corporation",
            "Qualcomm Atheros",
            "Qualcomm Technologies, Inc",
            "Broadcom Inc. and subsidiaries",
            "Broadcom Limited",
            "MediaTek Inc.",
            "Advanced Micro Devices, Inc. [AMD]",
            "Marvell Technology Group Ltd.",
            "Aquantia Corp.",
            "ASIX Electronics Corp.",
            "Ralink corp.",
        ];
        for v in known_vendors {
            if let Some(rest) = raw.strip_prefix(v) {
                let model = rest.trim();
                if !model.is_empty() {
                    return (v.to_string(), model.to_string());
                }
            }
        }
        if let Some((v, m)) = raw.split_once(" Co., Ltd. ") {
            return (format!("{} Co., Ltd.", v.trim()), m.trim().to_string());
        }
        if let Some((v, m)) = raw.split_once(" Corporation ") {
            return (format!("{} Corporation", v.trim()), m.trim().to_string());
        }
        if let Some((v, m)) = raw.split_once(" Inc. ") {
            return (format!("{} Inc.", v.trim()), m.trim().to_string());
        }
        ("Genérico / Kernel".to_string(), raw.trim().to_string())
    }

    fn read_pci_network_models() -> HashMap<String, (String, String, String)> {
        let mut map = HashMap::new();

        // 1. Tenta ler Vendor e Device estruturados via `lspci -vmm -D`
        if let Ok(out) = Command::new("lspci").args(["-vmm", "-D"]).output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                for block in text.split("\n\n") {
                    let mut slot = String::new();
                    let mut vendor = String::new();
                    let mut device = String::new();
                    let mut rev = String::new();

                    for line in block.lines() {
                        if let Some((k, v)) = line.split_once(':') {
                            let val = v.trim();
                            match k.trim() {
                                "Slot" => slot = val.to_string(),
                                "Vendor" => vendor = val.to_string(),
                                "Device" => device = val.to_string(),
                                "Rev" => rev = val.to_string(),
                                _ => {}
                            }
                        }
                    }

                    if !slot.is_empty() && (!vendor.is_empty() || !device.is_empty()) {
                        let model_with_rev = if !rev.is_empty() {
                            format!("{} (rev {})", device, rev)
                        } else {
                            device.clone()
                        };
                        let full = format!("{} {}", vendor, model_with_rev).trim().to_string();
                        map.insert(slot, (vendor, model_with_rev, full));
                    }
                }
            }
        }

        // 2. Fallback para `lspci -D` caso algum slot não tenha sido preenchido
        if let Ok(out) = Command::new("lspci").arg("-D").output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                for line in text.lines() {
                    if let Some((slot, rest)) = line.split_once(' ') {
                        let slot_key = slot.trim().to_string();
                        if !map.contains_key(&slot_key) {
                            if let Some((_, full_model)) = rest.split_once(": ") {
                                let full = full_model.trim().to_string();
                                let (vendor, model) = Self::split_vendor_and_model(&full);
                                map.insert(slot_key, (vendor, model, full));
                            }
                        }
                    }
                }
            }
        }
        map
    }

    fn read_ipv4_map() -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Ok(out) = Command::new("ip").args(["-4", "-o", "addr", "show"]).output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                for line in text.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let iface = parts[1];
                        if let Some(inet_pos) = parts.iter().position(|&p| p == "inet") {
                            if let Some(ip_cidr) = parts.get(inet_pos + 1) {
                                map.insert(iface.to_string(), (*ip_cidr).to_string());
                            }
                        }
                    }
                }
            }
        }
        map
    }

    pub fn refresh(&mut self) -> Vec<NetworkInterfaceInfo> {
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.tick_count % 5 == 1 {
            self.ipv4_cache = Self::read_ipv4_map();
        }

        let mut list = Vec::new();
        let net_dir = Path::new("/sys/class/net");
        let now = Instant::now();

        if let Ok(entries) = fs::read_dir(net_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();

                // Filtra apenas interfaces físicas (que possuem symlink 'device' no sysfs)
                let dev_link = path.join("device");
                if !dev_link.exists() {
                    continue;
                }

                let is_wireless = path.join("wireless").exists();
                let kind = if is_wireless {
                    "Wi-Fi (Sem Fio)".to_string()
                } else {
                    "Cabeada (Ethernet)".to_string()
                };

                let operstate = fs::read_to_string(path.join("operstate"))
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                let state = if operstate == "up" {
                    "Conectada (Ativa)".to_string()
                } else {
                    "Desconectada".to_string()
                };

                let mac_address = fs::read_to_string(path.join("address"))
                    .unwrap_or_else(|_| "Desconhecido".into())
                    .trim()
                    .to_uppercase();

                let mut pci_slot = String::new();
                if let Ok(target) = fs::read_link(&dev_link) {
                    if let Some(fname) = target.file_name() {
                        pci_slot = fname.to_string_lossy().to_string();
                    }
                }

                let (vendor, model, hw_model) = self
                    .pci_models
                    .get(&pci_slot)
                    .cloned()
                    .unwrap_or_else(|| {
                        let fallback = format!("Controlador de Rede ({})", name);
                        ("Genérico / Kernel".to_string(), fallback.clone(), fallback)
                    });

                let driver = fs::read_link(dev_link.join("driver"))
                    .ok()
                    .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
                    .unwrap_or_else(|| "kernel".into());

                let link_speed = if !is_wireless {
                    fs::read_to_string(path.join("speed"))
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && s != "-1")
                        .map(|s| format!("{} Mbps", s))
                        .unwrap_or_else(|| "Auto-Negociação".into())
                } else {
                    "Wi-Fi 6 (802.11ax)".into()
                };

                let ipv4 = self
                    .ipv4_cache
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| "Sem IPv4".into());

                let rx_total_bytes = fs::read_to_string(path.join("statistics/rx_bytes"))
                    .unwrap_or_default()
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(0);
                let tx_total_bytes = fs::read_to_string(path.join("statistics/tx_bytes"))
                    .unwrap_or_default()
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(0);

                let (rx_speed_kbps, tx_speed_kbps) =
                    if let Some(&(prev_rx, prev_tx, prev_time)) = self.prev_stats.get(&name) {
                        let dt = now.duration_since(prev_time).as_secs_f64();
                        if dt > 0.05 {
                            let drx = rx_total_bytes.saturating_sub(prev_rx) as f64 / 1024.0 / dt;
                            let dtx = tx_total_bytes.saturating_sub(prev_tx) as f64 / 1024.0 / dt;
                            (drx, dtx)
                        } else {
                            (0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0)
                    };

                self.prev_stats
                    .insert(name.clone(), (rx_total_bytes, tx_total_bytes, now));

                list.push(NetworkInterfaceInfo {
                    name,
                    vendor,
                    model,
                    hw_model,
                    driver,
                    kind,
                    state,
                    mac_address,
                    ipv4,
                    link_speed,
                    rx_total_bytes,
                    tx_total_bytes,
                    rx_speed_kbps,
                    tx_speed_kbps,
                });
            }
        }

        // Ordena primeiro Cabeada (Ethernet), depois Wi-Fi (Sem Fio), e em seguida pelo nome
        list.sort_by(|a, b| {
            let a_wifi = a.kind.contains("Wi-Fi");
            let b_wifi = b.kind.contains("Wi-Fi");
            a_wifi.cmp(&b_wifi).then_with(|| a.name.cmp(&b.name))
        });
        list
    }
}

pub struct PeripheralsCollector;

impl PeripheralsCollector {
    pub fn read_audio() -> AudioReport {
        let mut server_name = "PipeWire / ALSA".to_string();
        let mut sinks = Vec::new();
        let mut sources = Vec::new();

        if let Ok(out) = Command::new("wpctl").arg("status").output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                let mut in_audio = false;
                let mut section = "";
                for raw_line in text.lines() {
                    let line = raw_line
                        .replace('│', " ")
                        .replace('├', " ")
                        .replace('└', " ")
                        .replace('─', " ");
                    let trimmed = line.trim();

                    if trimmed.starts_with("PipeWire") {
                        server_name = trimmed.to_string();
                        continue;
                    }
                    if trimmed == "Audio" {
                        in_audio = true;
                        section = "";
                        continue;
                    }
                    if trimmed == "Video" || trimmed == "Settings" {
                        in_audio = false;
                        section = "";
                        continue;
                    }
                    if !in_audio {
                        continue;
                    }
                    if trimmed.starts_with("Sinks:") {
                        section = "sinks";
                        continue;
                    } else if trimmed.starts_with("Sources:") {
                        section = "sources";
                        continue;
                    } else if trimmed.starts_with("Filters:")
                        || trimmed.starts_with("Streams:")
                        || trimmed.starts_with("Devices:")
                    {
                        section = "";
                        continue;
                    }

                    if (section == "sinks" || section == "sources") && !trimmed.is_empty() {
                        let is_default = trimmed.starts_with('*');
                        let clean = trimmed.trim_start_matches('*').trim();

                        // Exemplo: "45. Alder Lake PCH-P High Definition Audio Controller Headphones [vol: 0.47]"
                        if let Some((_id, rest)) = clean.split_once(". ") {
                            let (name_part, vol_pct) = if let Some((n, v_part)) = rest.rsplit_once("[vol:") {
                                let v_str = v_part.trim().trim_end_matches(']').trim();
                                let v_num = v_str
                                    .split_whitespace()
                                    .next()
                                    .and_then(|s| s.parse::<f32>().ok())
                                    .unwrap_or(0.0);
                                (n.trim().to_string(), (v_num * 100.0).round() as u32)
                            } else {
                                (rest.trim().to_string(), 100)
                            };

                            let ep = AudioEndpoint {
                                name: name_part,
                                volume_pct: vol_pct,
                                is_default,
                            };
                            if section == "sinks" {
                                sinks.push(ep);
                            } else {
                                sources.push(ep);
                            }
                        }
                    }
                }
            }
        }

        AudioReport {
            server_name,
            sinks,
            sources,
        }
    }

    pub fn read_displays() -> Vec<DisplayInfo> {
        let mut displays = Vec::new();
        let drm_dir = Path::new("/sys/class/drm");

        if let Ok(entries) = fs::read_dir(drm_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if !fname.starts_with("card") || !fname.contains('-') {
                    continue;
                }
                let path = entry.path();
                let status = fs::read_to_string(path.join("status"))
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if status != "connected" {
                    continue;
                }

                let (gpu_card, connector) = fname
                    .split_once('-')
                    .map(|(c, conn)| (c.to_string(), conn.to_string()))
                    .unwrap_or_else(|| ("card".into(), fname.clone()));

                let resolution = fs::read_to_string(path.join("modes"))
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or("1920x1080")
                    .trim()
                    .to_string();

                let kind = if connector.starts_with("eDP") || connector.starts_with("LVDS") {
                    "Painel Interno do Notebook".to_string()
                } else if connector.starts_with("HDMI") {
                    "Monitor Externo (HDMI)".to_string()
                } else if connector.starts_with("DP") {
                    "Monitor Externo (DisplayPort)".to_string()
                } else {
                    "Monitor Conectado".to_string()
                };

                displays.push(DisplayInfo {
                    connector,
                    kind,
                    resolution,
                    gpu_card,
                });
            }
        }

        displays.sort_by(|a, b| a.connector.cmp(&b.connector));
        displays
    }

    pub fn read_usb_devices() -> Vec<UsbDeviceInfo> {
        let mut devices = Vec::new();
        if let Ok(out) = Command::new("lsusb").output() {
            if let Ok(text) = String::from_utf8(out.stdout) {
                for line in text.lines() {
                    // Ex: Bus 003 Device 011: ID 046d:c08b Logitech, Inc. G502 SE HERO Gaming Mouse
                    if let Some((bus_part, id_rest)) = line.split_once(": ID ") {
                        if let Some((vid_pid, raw_name)) = id_rest.split_once(' ') {
                            let name = raw_name.trim().to_string();
                            let lower = name.to_lowercase();

                            // Ignorar hubs USB internos/root hubs genéricos
                            if lower.contains("root hub") || lower.ends_with(" hub") {
                                continue;
                            }

                            let category = if lower.contains("mouse") {
                                "Mouse / Apontador"
                            } else if lower.contains("keyboard") {
                                "Teclado"
                            } else if lower.contains("microphone") || lower.contains("audio") {
                                "Áudio / Microfone USB"
                            } else if lower.contains("webcam") || lower.contains("powerconf") || lower.contains("camera") {
                                "Webcam / Vídeo USB"
                            } else if lower.contains("bluetooth") {
                                "Adaptador Bluetooth"
                            } else if lower.contains("fingerprint") {
                                "Leitor Biométrico"
                            } else if lower.contains("usbmonitor") || lower.contains("qinheng") {
                                "Tela Auxiliar USB (Turing/TURZX)"
                            } else {
                                "Dispositivo USB"
                            };

                            devices.push(UsbDeviceInfo {
                                bus_dev: bus_part.trim().to_string(),
                                vendor_product_id: vid_pid.trim().to_string(),
                                name,
                                category: category.to_string(),
                            });
                        }
                    }
                }
            }
        }
        devices
    }

    pub fn read_serial_ports() -> Vec<SerialPortInfo> {
        let mut ports = Vec::new();
        let tty_dir = Path::new("/sys/class/tty");

        let mut by_id_map: HashMap<String, String> = HashMap::new();
        if let Ok(entries) = fs::read_dir("/dev/serial/by-id") {
            for entry in entries.flatten() {
                let id_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(target) = fs::read_link(entry.path()) {
                    if let Some(dev_name) = target.file_name() {
                        by_id_map.insert(dev_name.to_string_lossy().to_string(), id_name);
                    }
                }
            }
        }

        if let Ok(entries) = fs::read_dir(tty_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("ttyUSB") && !name.starts_with("ttyACM") {
                    continue;
                }

                let dev_path = entry.path().join("device");
                let driver = fs::read_link(dev_path.join("driver"))
                    .ok()
                    .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                    .unwrap_or_else(|| "usb-serial".to_string());

                let description = if let Some(id_str) = by_id_map.get(&name) {
                    id_str.replace("usb-", "").replace('_', " ")
                } else if name.starts_with("ttyACM") {
                    "Dispositivo CDC-ACM (Arduino / Microcontrolador / Tela Auxiliar)".to_string()
                } else {
                    "Conversor USB-Serial UART (CH340 / CP210x / FTDI)".to_string()
                };

                ports.push(SerialPortInfo {
                    port_path: format!("/dev/{}", name),
                    driver,
                    description,
                    baud_hint: "115200 bps (8N1)".to_string(),
                });
            }
        }

        ports.sort_by(|a, b| a.port_path.cmp(&b.port_path));
        ports
    }
}
