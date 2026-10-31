use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::core::battery::BatteryCollector;
use crate::core::cpu::CpuCollector;
use crate::core::diagnostics::{DiagSeverity, DiagnosticsEngine};
use crate::core::disk::DiskCollector;
use crate::core::gaming::GamingAnalyzer;
use crate::core::gpu::GpuCollector;
use crate::core::memory::MemoryCollector;
use crate::core::os_info::OsCollector;
use crate::core::peripherals::{NetworkCollector, PeripheralsCollector};
use crate::core::sensors::SensorsCollector;

pub struct ReportExporter;

impl ReportExporter {
    fn home_dir() -> PathBuf {
        env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
    }

    fn current_timestamp() -> String {
        if let Ok(out) = Command::new("date")
            .arg("+%d/%m/%Y às %H:%M:%S")
            .output()
        {
            if let Ok(s) = String::from_utf8(out.stdout) {
                let t = s.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
        "Data atual".to_string()
    }

    fn escape_html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    fn escape_json(input: &str) -> String {
        input
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "")
            .replace('\t', " ")
    }

    /// Gera um Laudo Técnico Completo em HTML estilizado (Dark Mode + pronto para impressão/PDF).
    pub fn export_html() -> Result<PathBuf, String> {
        let os = OsCollector::read();
        let uptime = OsCollector::format_uptime();
        let mut cpu_col = CpuCollector::new();
        let cpu = cpu_col.refresh();
        let mut mem_col = MemoryCollector::new();
        let mem = mem_col.refresh();
        let ram_slots = MemoryCollector::read_ram_modules();
        let disks = DiskCollector::list_disks();
        let gpus = GpuCollector::list_gpus();
        let mut sens_col = SensorsCollector::new();
        let sensors = sens_col.refresh();
        let battery = BatteryCollector::read();
        let mut net_col = NetworkCollector::new();
        let net_ifaces = net_col.refresh();
        let displays = PeripheralsCollector::read_displays();
        let usb_devs = PeripheralsCollector::read_usb_devices();
        let serial_ports = PeripheralsCollector::read_serial_ports();
        let diag = DiagnosticsEngine::run_full_diagnostics();
        let bn = GamingAnalyzer::analyze_bottleneck(&cpu, &gpus, &mem);
        let timestamp = Self::current_timestamp();

        let safe_host = os
            .host_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();
        let filename = format!(
            "AnalisePC_Laudo_{}.html",
            if safe_host.is_empty() {
                "Sistema"
            } else {
                &safe_host
            }
        );
        let out_path = Self::home_dir().join(filename);

        let mut html = String::with_capacity(32768);
        html.push_str(
            r#"<!DOCTYPE html>
<html lang="pt-BR">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Laudo Técnico de Hardware — Analise PC</title>
<style>
  :root {
    --bg: #16181d;
    --card: #20232a;
    --card-border: rgba(255,255,255,0.08);
    --text: #f3f4f6;
    --muted: #9ca3af;
    --blue: #3584e4;
    --green: #2ec27e;
    --amber: #f5c211;
    --red: #e01b24;
    --purple: #c061cb;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: 'Inter', 'Cantarell', system-ui, -apple-system, sans-serif;
    background: var(--bg);
    color: var(--text);
    line-height: 1.55;
    padding: 32px 20px;
  }
  .container { max-width: 1080px; margin: 0 auto; }
  header {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    background: linear-gradient(135deg, #1e2536, #20232a);
    border: 1px solid var(--card-border);
    border-radius: 16px;
    padding: 24px 28px;
    margin-bottom: 24px;
  }
  header h1 { font-size: 1.6rem; font-weight: 800; letter-spacing: -0.02em; }
  header p { color: var(--muted); font-size: 0.92rem; margin-top: 4px; }
  .score-box {
    text-align: right;
    background: rgba(46, 194, 126, 0.14);
    border: 1px solid rgba(46, 194, 126, 0.35);
    padding: 12px 20px;
    border-radius: 14px;
  }
  .score-box .num { font-size: 1.8rem; font-weight: 800; color: var(--green); }
  .score-box .lbl { font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); }
  .grid-4 {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 14px;
    margin-bottom: 24px;
  }
  .kpi {
    background: var(--card);
    border: 1px solid var(--card-border);
    border-radius: 14px;
    padding: 18px;
  }
  .kpi .k-title { font-size: 0.75rem; font-weight: 700; text-transform: uppercase; color: var(--muted); letter-spacing: 0.05em; }
  .kpi .k-val { font-size: 1.45rem; font-weight: 800; margin: 6px 0 2px; }
  .kpi .k-sub { font-size: 0.84rem; color: var(--muted); }
  section {
    background: var(--card);
    border: 1px solid var(--card-border);
    border-radius: 16px;
    padding: 22px 24px;
    margin-bottom: 20px;
  }
  section h2 {
    font-size: 1.12rem;
    font-weight: 700;
    margin-bottom: 14px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--card-border);
    color: #fff;
  }
  table { width: 100%; border-collapse: collapse; font-size: 0.92rem; }
  th, td { text-align: left; padding: 10px 8px; border-bottom: 1px solid rgba(255,255,255,0.05); }
  th { color: var(--muted); font-weight: 600; font-size: 0.8rem; text-transform: uppercase; }
  tr:last-child td { border-bottom: none; }
  .badge {
    display: inline-block;
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 0.78rem;
    font-weight: 700;
  }
  .b-green { background: rgba(46,194,126,0.18); color: var(--green); }
  .b-amber { background: rgba(245,194,17,0.18); color: var(--amber); }
  .b-red { background: rgba(224,27,36,0.18); color: var(--red); }
  .b-blue { background: rgba(53,132,228,0.18); color: #62a0ea; }
  footer { text-align: center; color: var(--muted); font-size: 0.82rem; margin-top: 28px; }
  @media print {
    body { background: #fff; color: #111; padding: 0; }
    header, .kpi, section { background: #fff; border-color: #ddd; color: #111; }
    section h2 { color: #111; border-color: #ddd; }
    th, td { border-color: #eee; }
  }
</style>
</head>
<body>
<div class="container">
"#,
        );

        // Header
        html.push_str(&format!(
            r#"<header>
  <div>
    <h1>Analise PC — Laudo Técnico de Hardware</h1>
    <p>{} ({}) • {} • Gerado em {}</p>
  </div>
  <div class="score-box">
    <div class="num">{} / 100</div>
    <div class="lbl">Índice de Integridade</div>
  </div>
</header>"#,
            Self::escape_html(&os.machine_model),
            Self::escape_html(&os.host_name),
            Self::escape_html(&os.distro_name),
            Self::escape_html(&timestamp),
            diag.overall_score
        ));

        // 4 KPI Cards
        let max_freq = cpu
            .cores
            .iter()
            .map(|c| c.frequency_mhz)
            .max()
            .unwrap_or(0);
        let ch_mode = if ram_slots.len() >= 2 {
            "Dual Channel"
        } else {
            "Sistema"
        };
        html.push_str(&format!(
            r#"<div class="grid-4">
  <div class="kpi">
    <div class="k-title">Processador</div>
    <div class="k-val">{:.1}%</div>
    <div class="k-sub">{}C / {}T • {} MHz</div>
  </div>
  <div class="kpi">
    <div class="k-title">Memória RAM ({})</div>
    <div class="k-val">{:.1} / {:.1} GiB</div>
    <div class="k-sub">Ocupação: {:.1}%</div>
  </div>
  <div class="kpi">
    <div class="k-title">Equilíbrio Gamer (CPU/GPU)</div>
    <div class="k-val">{}</div>
    <div class="k-sub">{}</div>
  </div>
  <div class="kpi">
    <div class="k-title">Tempo de Atividade</div>
    <div class="k-val">{}</div>
    <div class="k-sub">Sessão {} • Kernel {}</div>
  </div>
</div>"#,
            cpu.global_usage_percent,
            cpu.physical_cores,
            cpu.logical_cores,
            max_freq,
            ch_mode,
            mem.used_gib(),
            mem.total_gib(),
            mem.used_percent(),
            Self::escape_html(&bn.gpu_summary),
            Self::escape_html(&bn.vram_summary),
            Self::escape_html(&uptime),
            Self::escape_html(&os.session_type),
            Self::escape_html(&os.kernel_version),
        ));

        // Section 1: Diagnóstico de Saúde
        html.push_str("<section><h2>1. Auditoria de Integridade do Hardware</h2><table><thead><tr><th>Subsistema</th><th>Verificação</th><th>Detalhes Técnicos</th><th>Status</th></tr></thead><tbody>");
        for chk in &diag.checks {
            let cls = match chk.severity {
                DiagSeverity::Healthy => "b-green",
                DiagSeverity::Warning => "b-amber",
                DiagSeverity::Critical => "b-red",
            };
            html.push_str(&format!(
                "<tr><td><strong>{}</strong></td><td>{}</td><td>{}</td><td><span class=\"badge {}\">{}</span></td></tr>",
                Self::escape_html(&chk.component),
                Self::escape_html(&chk.title),
                Self::escape_html(&chk.details),
                cls,
                Self::escape_html(&chk.badge_text)
            ));
        }
        html.push_str("</tbody></table></section>");

        // Section 2: Plataforma, CPU e Memória RAM
        html.push_str("<section><h2>2. Plataforma, Processador e Memória RAM</h2><table><tbody>");
        html.push_str(&format!(
            "<tr><th>Modelo / Placa-Mãe</th><td>{} • {} {} (Rev {})</td></tr>",
            Self::escape_html(&os.machine_model),
            Self::escape_html(&os.board_vendor),
            Self::escape_html(&os.board_name),
            Self::escape_html(&os.board_version)
        ));
        html.push_str(&format!(
            "<tr><th>BIOS / UEFI</th><td>{} v{} ({})</td></tr>",
            Self::escape_html(&os.bios_vendor),
            Self::escape_html(&os.bios_version),
            Self::escape_html(&os.bios_date)
        ));
        html.push_str(&format!(
            "<tr><th>Processador</th><td>{} ({} núcleos físicos / {} threads)</td></tr>",
            Self::escape_html(&cpu.brand),
            cpu.physical_cores,
            cpu.logical_cores
        ));
        for m in &ram_slots {
            html.push_str(&format!(
                "<tr><th>Slot {} ({})</th><td>{} GiB {} @ {} MT/s • {} • Serial: {}</td></tr>",
                Self::escape_html(&m.slot_locator),
                Self::escape_html(&m.bank_locator),
                m.size_gib,
                Self::escape_html(&m.mem_type),
                m.configured_speed_mts,
                Self::escape_html(&m.manufacturer),
                Self::escape_html(&m.serial_number)
            ));
        }
        html.push_str("</tbody></table></section>");

        // Section 3: Armazenamento e S.M.A.R.T.
        html.push_str("<section><h2>3. Armazenamento Físico e Saúde S.M.A.R.T.</h2><table><thead><tr><th>Unidade</th><th>Serial / Firmware</th><th>Telemetria S.M.A.R.T.</th><th>Saúde</th></tr></thead><tbody>");
        for d in &disks {
            if let Some(ref s) = d.smart {
                html.push_str(&format!(
                    "<tr><td><strong>{}</strong></td><td>{} (FW {})</td><td>TBW: {:.2} TB gravados • {:.2} TB lidos • {} h ligado • {} erros</td><td><span class=\"badge b-green\">{}% ({})</span></td></tr>",
                    Self::escape_html(&d.model),
                    Self::escape_html(&d.serial),
                    Self::escape_html(&d.firmware),
                    s.total_written_tb,
                    s.total_read_tb,
                    s.power_on_hours,
                    s.media_errors,
                    s.health_remaining_pct,
                    Self::escape_html(&s.verdict)
                ));
            } else {
                html.push_str(&format!(
                    "<tr><td><strong>{}</strong></td><td>{} (FW {})</td><td>Estado: {}</td><td><span class=\"badge b-blue\">Ativo</span></td></tr>",
                    Self::escape_html(&d.model),
                    Self::escape_html(&d.serial),
                    Self::escape_html(&d.firmware),
                    Self::escape_html(&d.state)
                ));
            }
        }
        html.push_str("</tbody></table></section>");

        // Section 4: Placas de Vídeo (GPUs)
        html.push_str("<section><h2>4. Placas de Vídeo (GPUs)</h2><table><thead><tr><th>Modelo</th><th>Tipo</th><th>VRAM / Clocks / PCIe</th><th>Driver / VBIOS</th></tr></thead><tbody>");
        for g in &gpus {
            let vram = g.vram.as_deref().unwrap_or("Compartilhada");
            let clocks = g.clocks_info.as_deref().unwrap_or("Dinâmico");
            let pcie = g.pcie_link.as_deref().unwrap_or("Barramento Interno");
            let vbios = g.vbios_version.as_deref().unwrap_or("Integrado");
            html.push_str(&format!(
                "<tr><td><strong>{}</strong></td><td>{}</td><td>{} • {} • {}</td><td>{} ({})</td></tr>",
                Self::escape_html(&g.model),
                if g.is_dedicated { "Dedicada" } else { "Integrada" },
                Self::escape_html(vram),
                Self::escape_html(clocks),
                Self::escape_html(pcie),
                Self::escape_html(&g.driver),
                Self::escape_html(vbios)
            ));
        }
        html.push_str("</tbody></table></section>");

        // Section 5: Sensores, Bateria e Periféricos
        html.push_str("<section><h2>5. Sensores Térmicos, Bateria e Interfaces</h2><table><tbody>");
        for t in &sensors.temperatures {
            html.push_str(&format!(
                "<tr><th>Sensor Térmico: {}</th><td>{:.1} °C</td></tr>",
                Self::escape_html(&t.label),
                t.temperature_c
            ));
        }
        for f in &sensors.fans {
            html.push_str(&format!(
                "<tr><th>Ventoinha: {}</th><td>{} RPM</td></tr>",
                Self::escape_html(&f.label),
                f.rpm
            ));
        }
        if let Some(ref b) = battery {
            html.push_str(&format!(
                "<tr><th>Bateria</th><td>{}% ({}) • {:.2} V • {:.1} W • Saúde: {} ({:.1}% desgaste)</td></tr>",
                b.capacity_percent,
                Self::escape_html(&b.status),
                b.voltage_v,
                b.power_w,
                Self::escape_html(&b.health),
                b.wear_percent
            ));
        }
        for n in &net_ifaces {
            html.push_str(&format!(
                "<tr><th>Rede: {} ({})</th><td>{} • IPv4: {} • MAC: {} • Link: {}</td></tr>",
                Self::escape_html(&n.name),
                Self::escape_html(&n.kind),
                Self::escape_html(&n.hw_model),
                Self::escape_html(&n.ipv4),
                Self::escape_html(&n.mac_address),
                Self::escape_html(&n.link_speed)
            ));
        }
        for d in &displays {
            html.push_str(&format!(
                "<tr><th>Monitor DRM: {}</th><td>{} • Resolução: {} ({})</td></tr>",
                Self::escape_html(&d.connector),
                Self::escape_html(&d.kind),
                Self::escape_html(&d.resolution),
                Self::escape_html(&d.gpu_card)
            ));
        }
        for sp in &serial_ports {
            html.push_str(&format!(
                "<tr><th>Porta Serial: {}</th><td>{} (Driver: {})</td></tr>",
                Self::escape_html(&sp.port_path),
                Self::escape_html(&sp.description),
                Self::escape_html(&sp.driver)
            ));
        }
        html.push_str(&format!(
            "<tr><th>Periféricos USB Conectados</th><td>{} dispositivos identificados no barramento USB</td></tr>",
            usb_devs.len()
        ));
        html.push_str("</tbody></table></section>");

        html.push_str(&format!(
            "<footer>Gerado nativamente por <strong>Analise PC v0.1.0</strong> (Rust + GTK 4 + Libadwaita) • {}</footer></div></body></html>",
            Self::escape_html(&timestamp)
        ));

        fs::write(&out_path, html)
            .map_err(|e| format!("Erro ao gravar laudo HTML em {:?}: {}", out_path, e))?;

        Ok(out_path)
    }

    /// Gera um Laudo Técnico Completo estruturado em JSON (`~/AnalisePC_Laudo_<hostname>.json`).
    pub fn export_json() -> Result<PathBuf, String> {
        let os = OsCollector::read();
        let mut cpu_col = CpuCollector::new();
        let cpu = cpu_col.refresh();
        let mut mem_col = MemoryCollector::new();
        let mem = mem_col.refresh();
        let ram_slots = MemoryCollector::read_ram_modules();
        let disks = DiskCollector::list_disks();
        let gpus = GpuCollector::list_gpus();
        let diag = DiagnosticsEngine::run_full_diagnostics();
        let bn = GamingAnalyzer::analyze_bottleneck(&cpu, &gpus, &mem);
        let timestamp = Self::current_timestamp();

        let safe_host = os
            .host_name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();
        let filename = format!(
            "AnalisePC_Laudo_{}.json",
            if safe_host.is_empty() {
                "Sistema"
            } else {
                &safe_host
            }
        );
        let out_path = Self::home_dir().join(filename);

        let mut json = String::with_capacity(16384);
        json.push_str("{\n");
        json.push_str(&format!(
            "  \"generated_at\": \"{}\",\n  \"health_score\": {},\n  \"health_verdict\": \"{}\",\n",
            Self::escape_json(&timestamp),
            diag.overall_score,
            Self::escape_json(&diag.overall_verdict)
        ));

        json.push_str(&format!(
            "  \"system\": {{\n    \"distro\": \"{}\",\n    \"kernel\": \"{}\",\n    \"session\": \"{}\",\n    \"machine_model\": \"{}\",\n    \"board\": \"{} {} (Rev {})\",\n    \"bios\": \"{} v{} ({})\"\n  }},\n",
            Self::escape_json(&os.distro_name),
            Self::escape_json(&os.kernel_version),
            Self::escape_json(&os.session_type),
            Self::escape_json(&os.machine_model),
            Self::escape_json(&os.board_vendor),
            Self::escape_json(&os.board_name),
            Self::escape_json(&os.board_version),
            Self::escape_json(&os.bios_vendor),
            Self::escape_json(&os.bios_version),
            Self::escape_json(&os.bios_date)
        ));

        json.push_str(&format!(
            "  \"cpu\": {{\n    \"brand\": \"{}\",\n    \"physical_cores\": {},\n    \"logical_cores\": {},\n    \"usage_percent\": {:.1}\n  }},\n",
            Self::escape_json(&cpu.brand),
            cpu.physical_cores,
            cpu.logical_cores,
            cpu.global_usage_percent
        ));

        json.push_str(&format!(
            "  \"memory\": {{\n    \"total_gib\": {:.2},\n    \"used_gib\": {:.2},\n    \"used_percent\": {:.1},\n    \"slots\": [\n",
            mem.total_gib(),
            mem.used_gib(),
            mem.used_percent()
        ));
        for (i, m) in ram_slots.iter().enumerate() {
            let comma = if i + 1 < ram_slots.len() { "," } else { "" };
            json.push_str(&format!(
                "      {{ \"slot\": \"{}\", \"size_gib\": {}, \"type\": \"{}\", \"speed_mts\": {}, \"manufacturer\": \"{}\", \"serial\": \"{}\" }}{}\n",
                Self::escape_json(&m.slot_locator),
                m.size_gib,
                Self::escape_json(&m.mem_type),
                m.configured_speed_mts,
                Self::escape_json(&m.manufacturer),
                Self::escape_json(&m.serial_number),
                comma
            ));
        }
        json.push_str("    ]\n  },\n");

        json.push_str("  \"disks\": [\n");
        for (i, d) in disks.iter().enumerate() {
            let comma = if i + 1 < disks.len() { "," } else { "" };
            let (health_pct, tbw_tb, poh) = d
                .smart
                .as_ref()
                .map(|s| (s.health_remaining_pct, s.total_written_tb, s.power_on_hours))
                .unwrap_or((100, 0.0, 0));
            json.push_str(&format!(
                "    {{ \"model\": \"{}\", \"serial\": \"{}\", \"firmware\": \"{}\", \"smart_health_pct\": {}, \"tbw_tb\": {:.2}, \"power_on_hours\": {} }}{}\n",
                Self::escape_json(&d.model),
                Self::escape_json(&d.serial),
                Self::escape_json(&d.firmware),
                health_pct,
                tbw_tb,
                poh,
                comma
            ));
        }
        json.push_str("  ],\n");

        json.push_str("  \"gpus\": [\n");
        for (i, g) in gpus.iter().enumerate() {
            let comma = if i + 1 < gpus.len() { "," } else { "" };
            json.push_str(&format!(
                "    {{ \"model\": \"{}\", \"dedicated\": {}, \"driver\": \"{}\", \"vram\": \"{}\" }}{}\n",
                Self::escape_json(&g.model),
                g.is_dedicated,
                Self::escape_json(&g.driver),
                Self::escape_json(g.vram.as_deref().unwrap_or("Shared")),
                comma
            ));
        }
        json.push_str("  ],\n");

        json.push_str(&format!(
            "  \"gaming_bottleneck\": {{\n    \"cpu_summary\": \"{}\",\n    \"gpu_summary\": \"{}\",\n    \"vram_summary\": \"{}\",\n    \"verdict\": \"{}\"\n  }}\n",
            Self::escape_json(&bn.cpu_summary),
            Self::escape_json(&bn.gpu_summary),
            Self::escape_json(&bn.vram_summary),
            Self::escape_json(&bn.overall_balance_verdict)
        ));

        json.push_str("}\n");

        fs::write(&out_path, json)
            .map_err(|e| format!("Erro ao gravar laudo JSON em {:?}: {}", out_path, e))?;

        Ok(out_path)
    }
}
