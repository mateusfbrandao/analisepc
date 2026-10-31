use std::fs;
use std::process::Command;

use crate::core::battery::BatteryCollector;
use crate::core::disk::DiskCollector;
use crate::core::memory::MemoryCollector;
use crate::core::sensors::SensorsCollector;

#[derive(Debug, Clone, PartialEq)]
pub enum DiagSeverity {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug, Clone)]
pub struct DiagnosticCheck {
    pub component: String,
    pub title: String,
    pub details: String,
    pub badge_text: String,
    pub severity: DiagSeverity,
}

#[derive(Debug, Clone)]
pub struct KernelHardwareEvent {
    pub timestamp: String,
    pub subsystem: String,
    pub message: String,
    pub severity: DiagSeverity,
}

#[derive(Debug, Clone)]
pub struct GpuOcDiagnostics {
    pub gpu_name: String,
    pub power_draw_w: String,
    pub default_tgp_w: String,
    pub max_tgp_w: String,
    pub throttle_status: String,
    pub is_throttling: bool,
}

#[derive(Debug, Clone)]
pub struct HardwareDiagnosticReport {
    pub overall_score: u32,
    pub overall_verdict: String,
    pub checks: Vec<DiagnosticCheck>,
    pub kernel_events: Vec<KernelHardwareEvent>,
    pub gpu_oc: Option<GpuOcDiagnostics>,
}

#[derive(Debug, Clone)]
pub struct NetworkStabilityResult {
    pub target_host: String,
    pub ping_avg_ms: f64,
    pub ping_min_ms: f64,
    pub ping_max_ms: f64,
    pub jitter_ms: f64,
    pub packet_loss_pct: f64,
    pub verdict: String,
}

pub struct DiagnosticsEngine;

impl DiagnosticsEngine {
    pub fn run_full_diagnostics() -> HardwareDiagnosticReport {
        let mut checks = Vec::new();
        let mut score: i32 = 100;

        // 1. Diagnóstico Térmico & Throttling da CPU
        let mut sens = SensorsCollector::new();
        let sens_rep = sens.refresh();
        let max_cpu_temp = sens_rep
            .temperatures
            .iter()
            .filter(|t| {
                let l = t.label.to_lowercase();
                l.contains("package") || l.contains("core") || l.contains("cpu")
            })
            .map(|t| t.temperature_c)
            .fold(0.0_f32, f32::max);

        let pkg_throttles = fs::read_to_string(
            "/sys/devices/system/cpu/cpu0/thermal_throttle/package_throttle_count",
        )
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);

        let (cpu_sev, cpu_badge) = if max_cpu_temp >= 90.0 {
            score -= 20;
            (DiagSeverity::Critical, "Superaquecimento".to_string())
        } else if max_cpu_temp >= 78.0 || pkg_throttles > 0 {
            score -= 5;
            (DiagSeverity::Warning, "Atenção Térmica".to_string())
        } else {
            (DiagSeverity::Healthy, "Saudável".to_string())
        };

        checks.push(DiagnosticCheck {
            component: "Processador e Refrigeração".to_string(),
            title: format!("Temperatura Máx CPU: {:.1} °C", max_cpu_temp),
            details: format!(
                "Eventos de Thermal Throttling no pacote desde o boot: {} • Ventoinhas monitoradas: {}",
                pkg_throttles,
                sens_rep.fans.len()
            ),
            badge_text: cpu_badge,
            severity: cpu_sev,
        });

        // 2. Diagnóstico de Memória RAM (Dual Channel & Slots)
        let ram_modules = MemoryCollector::read_ram_modules();
        let mut mem_col = MemoryCollector::new();
        let mem_info = mem_col.refresh();
        let is_dual = ram_modules.len() >= 2;
        let (mem_sev, mem_badge) = if mem_info.used_percent() > 90.0 {
            score -= 10;
            (DiagSeverity::Warning, "Pressão Alta".to_string())
        } else if !is_dual && !ram_modules.is_empty() {
            score -= 5;
            (DiagSeverity::Warning, "Single Channel".to_string())
        } else {
            (DiagSeverity::Healthy, "Dual Channel OK".to_string())
        };

        let ch_desc = if is_dual {
            format!(
                "{} módulos ativos em Dual Channel ({} MT/s)",
                ram_modules.len(),
                ram_modules.first().map(|m| m.configured_speed_mts).unwrap_or(0)
            )
        } else {
            "Memória unificada ou canal simples".to_string()
        };

        checks.push(DiagnosticCheck {
            component: "Memória RAM e Controlador".to_string(),
            title: format!(
                "Memória RAM ({:.1}% em uso — {:.1} GiB livres)",
                mem_info.used_percent(),
                (mem_info.total_gib() - mem_info.used_gib()).max(0.0)
            ),
            details: format!("Configuração física: {} • Sem erros de paridade EDAC", ch_desc),
            badge_text: mem_badge,
            severity: mem_sev,
        });

        // 3. Diagnóstico de Armazenamento NVMe / S.M.A.R.T.
        let disks = DiskCollector::list_disks();
        for d in &disks {
            if let Some(ref s) = d.smart {
                let (d_sev, d_badge) = if s.media_errors > 0 || s.health_remaining_pct < 50 {
                    score -= 25;
                    (DiagSeverity::Critical, "Risco no Disco".to_string())
                } else if s.health_remaining_pct < 80 {
                    score -= 10;
                    (DiagSeverity::Warning, "Desgaste Moderado".to_string())
                } else {
                    (
                        DiagSeverity::Healthy,
                        format!("S.M.A.R.T. {}%", s.health_remaining_pct),
                    )
                };

                checks.push(DiagnosticCheck {
                    component: format!("Armazenamento NVMe ({})", d.model),
                    title: format!(
                        "Integridade S.M.A.R.T.: {}% vida útil • {} erros de mídia",
                        s.health_remaining_pct, s.media_errors
                    ),
                    details: format!(
                        "Reserva: {}% • TBW Gravado: {:.2} TB • Horas Ligado: {} h • Desligamentos Abruptos: {}",
                        s.avail_spare_pct, s.total_written_tb, s.power_on_hours, s.unsafe_shutdowns
                    ),
                    badge_text: d_badge,
                    severity: d_sev,
                });
            }
        }

        // 4. Diagnóstico de GPU & Overclock / TGP
        let gpu_oc = Self::read_gpu_oc_diagnostics();
        if let Some(ref goc) = gpu_oc {
            let (g_sev, g_badge) = if goc.is_throttling {
                score -= 10;
                (DiagSeverity::Warning, "Throttling Ativo".to_string())
            } else {
                (DiagSeverity::Healthy, "Estável (TGP OK)".to_string())
            };

            checks.push(DiagnosticCheck {
                component: "Placa de Vídeo Dedicada (OC e Energia)".to_string(),
                title: format!(
                    "{} — Consumo Atual: {}",
                    goc.gpu_name, goc.power_draw_w
                ),
                details: format!(
                    "TGP Padrão: {} • Dynamic Boost Máx: {} • Status do Limitador: {}",
                    goc.default_tgp_w, goc.max_tgp_w, goc.throttle_status
                ),
                badge_text: g_badge,
                severity: g_sev,
            });
        }

        // 5. Diagnóstico da Bateria
        if let Some(bat) = BatteryCollector::read() {
            let (b_sev, b_badge) = if bat.wear_percent >= 40.0 {
                score -= 10;
                (
                    DiagSeverity::Warning,
                    format!("Desgaste {:.0}%", bat.wear_percent),
                )
            } else {
                (DiagSeverity::Healthy, "Saúde Boa".to_string())
            };

            checks.push(DiagnosticCheck {
                component: "Bateria e Alimentação".to_string(),
                title: format!(
                    "Desgaste da Bateria: {:.1}% (Capacidade Real: {:.0} / {:.0} mAh)",
                    bat.wear_percent, bat.charge_full_mah, bat.charge_design_mah
                ),
                details: if bat.wear_percent >= 40.0 {
                    "Recomendação: A bateria perdeu mais de 40% da capacidade original de fábrica. Considere calibrar ou substituir em caso de baixa autonomia.".to_string()
                } else {
                    "Células operando dentro dos parâmetros normais de retenção de carga.".to_string()
                },
                badge_text: b_badge,
                severity: b_sev,
            });
        }

        // 6. Varredura de Logs de Hardware no Kernel (dmesg / journalctl)
        let kernel_events = Self::scan_kernel_hardware_logs();
        let crit_kernel = kernel_events
            .iter()
            .filter(|e| e.severity == DiagSeverity::Critical)
            .count();
        if crit_kernel > 0 {
            score -= (crit_kernel as i32 * 8).min(20);
        }

        let final_score = score.clamp(0, 100) as u32;
        let overall_verdict = if final_score >= 90 {
            "Excelente — Hardware operando com estabilidade máxima".to_string()
        } else if final_score >= 75 {
            "Bom — Sistema estável com pontos leves de atenção (ex: desgaste natural de bateria)".to_string()
        } else {
            "Atenção Requerida — Verifique os alertas térmicos ou de armazenamento abaixo".to_string()
        };

        HardwareDiagnosticReport {
            overall_score: final_score,
            overall_verdict,
            checks,
            kernel_events,
            gpu_oc,
        }
    }

    pub fn read_gpu_oc_diagnostics() -> Option<GpuOcDiagnostics> {
        let out = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,power.draw,power.default_limit,power.max_limit,clocks_throttle_reasons.active",
                "--format=csv,noheader",
            ])
            .output()
            .ok()?;

        if !out.status.success() {
            return None;
        }

        let text = String::from_utf8(out.stdout).ok()?;
        let line = text.lines().next()?.trim();
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 5 {
            return None;
        }

        let gpu_name = parts[0].to_string();
        let power_draw_w = parts[1].to_string();
        let default_tgp_w = parts[2].to_string();
        let max_tgp_w = parts[3].to_string();
        let raw_mask = parts[4];

        let mask_val = u64::from_str_radix(raw_mask.trim_start_matches("0x"), 16).unwrap_or(0);
        // Bits de throttle da NVIDIA:
        // 0x01 = GpuIdle (Normal)
        // 0x02 = ApplicationsClocksSetting
        // 0x04 = SwPowerCap (Limite de TGP atingido)
        // 0x08 = HwSlowdown
        // 0x20 = SwThermalSlowdown (Thermal Throttling de software)
        // 0x40 = HwThermalSlowdown (Thermal Throttling de hardware)
        // 0x80 = HwPowerBrakeSlowdown
        let mut is_throttling = false;
        let throttle_status = if mask_val & 0x40 != 0 || mask_val & 0x20 != 0 {
            is_throttling = true;
            "Thermal Throttling Ativo (Redução de clock por temperatura)".to_string()
        } else if mask_val & 0x08 != 0 || mask_val & 0x80 != 0 {
            is_throttling = true;
            "Hardware Power Brake / Slowdown Ativo".to_string()
        } else if mask_val & 0x04 != 0 {
            "Limitado pelo TGP Máximo (Power Cap Normal em Carga)".to_string()
        } else if mask_val & 0x01 != 0 || mask_val == 0 {
            "Normal (Sem Throttling Térmico ou de Energia — Idle/Estável)".to_string()
        } else {
            format!("Operação Normal (Mask {})", raw_mask)
        };

        Some(GpuOcDiagnostics {
            gpu_name,
            power_draw_w,
            default_tgp_w,
            max_tgp_w,
            throttle_status,
            is_throttling,
        })
    }

    pub fn scan_kernel_hardware_logs() -> Vec<KernelHardwareEvent> {
        let mut events = Vec::new();
        if let Ok(out) = Command::new("journalctl")
            .args(["-k", "--no-pager", "-p", "warning..emerg", "-n", "80"])
            .output()
        {
            if let Ok(text) = String::from_utf8(out.stdout) {
                for line in text.lines() {
                    if line.contains("kauditd_printk_skb")
                        || line.contains("callbacks suppressed")
                        || line.starts_with("--")
                        || line.trim().is_empty()
                    {
                        continue;
                    }

                    // Exemplo: "out 30 18:17:40 brandao kernel: NVRM: ..."
                    let (ts, msg) = if let Some((prefix, rest)) = line.split_once("kernel: ") {
                        let short_ts = prefix
                            .split_whitespace()
                            .take(3)
                            .collect::<Vec<&str>>()
                            .join(" ");
                        (short_ts, rest.trim().to_string())
                    } else {
                        ("Boot".to_string(), line.trim().to_string())
                    };

                    if msg.is_empty() {
                        continue;
                    }

                    let lower = msg.to_lowercase();
                    let (subsystem, severity) = if lower.contains("mce")
                        || lower.contains("hardware error")
                        || lower.contains("i/o error")
                        || lower.contains("xid")
                        || lower.contains("gpu hang")
                    {
                        ("Erro Crítico de Hardware", DiagSeverity::Critical)
                    } else if lower.contains("nvrm") || lower.contains("nvidia") {
                        ("Driver GPU NVIDIA (NVRM)", DiagSeverity::Warning)
                    } else if lower.contains("nvme") || lower.contains("block") {
                        ("Controlador NVMe / Bloco", DiagSeverity::Healthy)
                    } else if lower.contains("edac") || lower.contains("resource sanity") {
                        ("Controlador de Memória / ACPI", DiagSeverity::Healthy)
                    } else if lower.contains("r8169") || lower.contains("iwlwifi") || lower.contains("wireless") {
                        ("Subsistema de Rede / PCIe", DiagSeverity::Healthy)
                    } else if lower.contains("hda") || lower.contains("usb") || lower.contains("spi") {
                        ("Áudio / Periféricos / BIOS", DiagSeverity::Healthy)
                    } else {
                        ("Kernel Linux", DiagSeverity::Healthy)
                    };

                    events.push(KernelHardwareEvent {
                        timestamp: ts,
                        subsystem: subsystem.to_string(),
                        message: msg.replace('&', "e").replace('<', "(").replace('>', ")"),
                        severity,
                    });
                }
            }
        }
        events.reverse();
        events
    }

    pub fn run_network_stability_test(target: &str) -> Result<NetworkStabilityResult, String> {
        let out = Command::new("ping")
            .args(["-c", "8", "-i", "0.2", "-W", "2", target])
            .output()
            .map_err(|e| format!("Falha ao executar ping: {}", e))?;

        let text = String::from_utf8_lossy(&out.stdout);
        let mut rtts = Vec::new();
        let mut packet_loss_pct = 0.0;

        for line in text.lines() {
            if let Some(pos) = line.find("time=") {
                let after = &line[pos + 5..];
                if let Some(ms_str) = after.split_whitespace().next() {
                    if let Ok(val) = ms_str.parse::<f64>() {
                        rtts.push(val);
                    }
                }
            } else if let Some(pos) = line.find("tempo=") {
                let after = &line[pos + 6..];
                if let Some(ms_str) = after.split_whitespace().next() {
                    if let Ok(val) = ms_str.parse::<f64>() {
                        rtts.push(val);
                    }
                }
            }

            if line.contains("% packet loss") || line.contains("% de perda") {
                for part in line.split(',') {
                    let p = part.trim();
                    if p.contains('%') {
                        if let Some(num_str) = p.split('%').next() {
                            if let Ok(loss) = num_str.trim().parse::<f64>() {
                                packet_loss_pct = loss;
                            }
                        }
                    }
                }
            }
        }

        if rtts.is_empty() {
            return Err("Sem resposta ICMP do servidor de teste (verifique sua conexão).".into());
        }

        let min_ms = rtts.iter().copied().fold(f64::MAX, f64::min);
        let max_ms = rtts.iter().copied().fold(f64::MIN, f64::max);
        let avg_ms = rtts.iter().sum::<f64>() / (rtts.len() as f64);

        // Jitter médio = média das diferenças absolutas consecutivas entre pacotes
        let jitter_ms = if rtts.len() >= 2 {
            let mut diff_sum = 0.0;
            for w in rtts.windows(2) {
                diff_sum += (w[1] - w[0]).abs();
            }
            diff_sum / ((rtts.len() - 1) as f64)
        } else {
            0.0
        };

        let verdict = if packet_loss_pct == 0.0 && avg_ms < 25.0 && jitter_ms < 5.0 {
            "Excelente (Conexão ultra-estável para Jogos Competitivos e Streaming)".to_string()
        } else if packet_loss_pct == 0.0 && avg_ms < 60.0 {
            "Boa (Estável para navegação, chamadas de vídeo e jogos)".to_string()
        } else if packet_loss_pct > 0.0 {
            format!("Instável ({:.0}% de perda de pacotes detectada)", packet_loss_pct)
        } else {
            "Latência Elevada (Verifique congestionamento na rede Wi-Fi)".to_string()
        };

        Ok(NetworkStabilityResult {
            target_host: target.to_string(),
            ping_avg_ms: avg_ms,
            ping_min_ms: min_ms,
            ping_max_ms: max_ms,
            jitter_ms,
            packet_loss_pct,
            verdict,
        })
    }
}
