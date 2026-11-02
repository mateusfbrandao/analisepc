mod core;
mod ui;

use std::thread;
use std::time::Duration;
use libadwaita::prelude::*;
use libadwaita::{glib, Application};

const APP_ID: &str = "io.github.analisepc.AnalisePC";

fn main() -> glib::ExitCode {
    if std::env::args().any(|arg| arg == "--cli") {
        run_cli_mode();
        return glib::ExitCode::SUCCESS;
    }

    glib::set_application_name("Analise PC");

    let app = Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(ui::window::build_ui);
    app.run()
}

fn run_cli_mode() {
    println!("==================================================");
    println!("                ANALISE PC (Linux)                ");
    println!("         Diagnóstico & Benchmark de Hardware      ");
    println!("==================================================\n");

    let os = core::os_info::OsCollector::read();
    println!("💻 [Sistema & Plataforma]");
    println!("   Distro: {}", os.distro_name);
    println!("   Kernel: {}", os.kernel_version);
    println!("   Interface: {} ({})", os.gnome_version, os.session_type);
    println!("   Dispositivo: {} ({})", os.machine_model, os.host_name);
    println!("   Placa-Mãe: {} {} (Rev {})", os.board_vendor, os.board_name, os.board_version);
    println!("   BIOS: {} v{} ({})", os.bios_vendor, os.bios_version, os.bios_date);
    println!("   Uptime: {}\n", core::os_info::OsCollector::format_uptime());

    println!("🔍 [Processador]");
    let mut cpu_collector = core::cpu::CpuCollector::new();
    thread::sleep(Duration::from_millis(200));
    let cpu_info = cpu_collector.refresh();
    println!("   Modelo: {}", cpu_info.brand);
    println!("   Núcleos Físicos: {}", cpu_info.physical_cores);
    println!("   Threads Lógicas: {}", cpu_info.logical_cores);
    println!("   Uso da CPU: {:.1}%\n", cpu_info.global_usage_percent);

    println!("🧠 [Memória RAM & Slots]");
    let mut mem_collector = core::memory::MemoryCollector::new();
    let mem_info = mem_collector.refresh();
    println!("   Total: {:.2} GiB", mem_info.total_gib());
    println!("   Em Uso: {:.2} GiB ({:.1}%)", mem_info.used_gib(), mem_info.used_percent());
    for m in core::memory::MemoryCollector::read_ram_modules() {
        println!(
            "   • Slot {} ({}): {} GiB {} {} MT/s ({}) | Fabricante: {} | Serial: {}",
            m.slot_locator, m.bank_locator, m.size_gib, m.mem_type, m.configured_speed_mts,
            m.form_factor, m.manufacturer, m.serial_number
        );
    }
    println!();

    if let Some(bat) = core::battery::BatteryCollector::read() {
        println!("🔋 [Bateria & Energia]");
        println!("   Nível: {}%", bat.capacity_percent);
        println!("   Status: {}", bat.status);
        println!("   Saúde: {}", bat.health);
        println!("   Capacidade Atual: {:.0} mAh / Projeto: {:.0} mAh", bat.charge_full_mah, bat.charge_design_mah);
        println!("   Desgaste da Bateria: {:.1}%", bat.wear_percent);
        println!("   Tensão: {:.2} V\n", bat.voltage_v);
    }

    println!("💾 [Armazenamento / Discos & S.M.A.R.T.]");
    let disks = core::disk::DiskCollector::list_disks();
    for d in disks {
        let temp_str = d.temperature_c.map(|t| format!(" ({:.1} °C)", t)).unwrap_or_default();
        println!("   • Modelo: {}{}", d.model, temp_str);
        println!("     Estado: {} | Serial: {} | Firmware: {}", d.state, d.serial, d.firmware);
        if let Some(ref s) = d.smart {
            println!("     Saúde S.M.A.R.T.: {} ({}% restante / {}% desgaste | Reserva: {}%)",
                s.verdict, s.health_remaining_pct, s.wear_used_pct, s.avail_spare_pct);
            println!("     TBW (Gravado): {:.2} TB | Total Lido: {:.2} TB | NVMe v{}",
                s.total_written_tb, s.total_read_tb, s.nvme_version);
            println!("     Horas Ligado: {} h | Ciclos: {} | Desligamentos Abruptos: {} | Erros de Mídia: {}",
                s.power_on_hours, s.power_cycles, s.unsafe_shutdowns, s.media_errors);
        }
        for p in d.partitions {
            println!("     └─ Partição: {} ({:.1} GiB livres de {:.1} GiB)", p.mount_point, p.available_gib, p.total_gib);
        }
    }
    println!();

    println!("🎮 [Placas de Vídeo (Estilo GPU-Z)]");
    let gpus = core::gpu::GpuCollector::list_gpus();
    for (i, gpu) in gpus.iter().enumerate() {
        let vram_str = gpu.vram.as_deref().unwrap_or("Compartilhada");
        let type_str = if gpu.is_dedicated { "Dedicada" } else { "Integrada" };
        println!("   [{}] Modelo: {} ({})", i + 1, gpu.model, type_str);
        println!("       Fabricante: {} | Driver: {}", gpu.vendor, gpu.driver);
        println!("       VRAM: {} | Slot: {}", vram_str, gpu.pci_id);
        if let Some(ref pcie) = gpu.pcie_link {
            println!("       Barramento: {}", pcie);
        }
        if let Some(ref clks) = gpu.clocks_info {
            println!("       Clocks: {}", clks);
        }
        if let Some(ref pwr) = gpu.power_info {
            println!("       Energia: {}", pwr);
        }
        if let Some(ref vbios) = gpu.vbios_version {
            println!("       VBIOS: {}", vbios);
        }
        println!("       APIs: {}", gpu.apis_supported);
    }
    println!();

    println!("🌐 [Rede, Monitores, Áudio e Periféricos USB]");
    let mut net_col = core::peripherals::NetworkCollector::new();
    for iface in net_col.refresh() {
        let rx_gib = iface.rx_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let tx_gib = iface.tx_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let tipo_label = if iface.kind.contains("Wi-Fi") { "Wi-Fi" } else { "Cabeada" };
        println!("   • {}: [{}]", tipo_label, iface.state);
        println!("     IP: {} | MAC: {}", iface.ipv4, iface.mac_address);
        println!("     Dispositivo: {} (Link: {} / Driver: {})", iface.name, iface.link_speed, iface.driver);
        println!("     Fabricante: {}", iface.vendor);
        println!("     Modelo: {} ({})", iface.model, iface.hw_model);
        println!("     Tráfego Acumulado: {:.2} GiB recebidos / {:.2} GiB enviados", rx_gib, tx_gib);
    }
    for disp in core::peripherals::PeripheralsCollector::read_displays() {
        println!("   • Monitor: {} ({}) — Resolução: {} ({})",
            disp.connector, disp.kind, disp.resolution, disp.gpu_card);
    }
    let audio = core::peripherals::PeripheralsCollector::read_audio();
    println!("   • Servidor de Áudio: {}", audio.server_name);
    for sink in audio.sinks.iter().filter(|s| s.is_default) {
        println!("     └─ Saída Ativa: {} (Volume: {}%)", sink.name, sink.volume_pct);
    }
    for src in audio.sources.iter().filter(|s| s.is_default) {
        println!("     └─ Microfone Ativo: {} (Volume: {}%)", src.name, src.volume_pct);
    }
    for usb in core::peripherals::PeripheralsCollector::read_usb_devices() {
        println!("   • USB [{}]: {} ({})", usb.vendor_product_id, usb.name, usb.category);
    }
    println!();

    println!("🩺 [Diagnóstico Automático do Hardware & Logs do Kernel]");
    let diag = core::diagnostics::DiagnosticsEngine::run_full_diagnostics();
    println!("   Índice Global de Saúde: {}/100 — {}", diag.overall_score, diag.overall_verdict);
    for chk in &diag.checks {
        println!("   • [{}] {}: {} ({})", chk.badge_text, chk.component, chk.title, chk.details);
    }
    println!("   Eventos de Hardware no Kernel (journalctl -k): {} analisados", diag.kernel_events.len());
    for ev in diag.kernel_events.iter().take(5) {
        println!("     └─ [{}] {}: {}", ev.timestamp, ev.subsystem, ev.message);
    }
    println!();

    println!("🕹️  [Jogos, Detector de Gargalo & Biblioteca Steam]");
    let bn = core::gaming::GamingAnalyzer::analyze_bottleneck(&cpu_info, &gpus, &mem_info);
    println!("   Veredito: {}", bn.overall_balance_verdict);
    for sc in &bn.scenarios {
        println!("   • [{}] {}: {}", sc.status_badge, sc.resolution_preset, sc.limiting_component);
    }
    let steam_items = core::gaming::GamingAnalyzer::scan_steam_library();
    println!("   Itens Steam Detectados ({}):", steam_items.len());
    for st in &steam_items {
        println!("     • {} (AppID: {} | {:.2} GiB)", st.name, st.app_id, st.size_gib);
    }
    let boost = core::gaming_boost::GamingBoost::read();
    println!("   Desempenho para jogos:");
    if boost.in_flatpak {
        println!("     • Indisponível no Flatpak. Use a instalação .deb.");
    } else {
        println!(
            "     • APT: {} | i386: {}",
            if boost.apt_available { "disponível" } else { "ausente" },
            if boost.i386_enabled { "ativa" } else { "ausente" }
        );
        for pkg in &boost.packages {
            println!(
                "     • {}: {}",
                pkg.name,
                if pkg.installed { "instalado" } else { "ausente" }
            );
        }
        println!("     • Cache Mesa: {}", boost.shader_cache_label());
        println!("     • Split lock: {}", boost.split_lock_label());
        println!(
            "     • LACT: {}",
            if boost.lact_installed { "instalado" } else { "ausente" }
        );
    }
    println!();

    println!("⚙️  [Otimizações & Sistema]");
    println!("   Perfil de Energia Atual: {}", core::optimizer::Optimizer::get_power_profile());
    println!("   Governador de CPU: {}", core::optimizer::Optimizer::get_cpu_governor());
    let hybrid = core::optimizer::Optimizer::read_hybrid_gpu_thermal();
    println!(
        "   Gráficos Híbridos PRIME: {} | Estado PCI dGPU: {}",
        hybrid.prime_mode.to_uppercase(),
        hybrid.dgpu_runtime_status
    );
    println!(
        "   Perfil Térmico ACPI: {} (Modos: {}) | Intel Turbo Boost: {}",
        hybrid.acpi_platform_profile,
        hybrid.acpi_choices,
        if hybrid.cpu_turbo_enabled { "Ativado" } else { "Desativado (Clock Base)" }
    );
    println!("   Agendamento TRIM SSD: {}", core::optimizer::Optimizer::get_fstrim_info());
    let cleanup = core::optimizer::Optimizer::scan_disk_cleanup();
    println!(
        "   Caches de Disco: Logs Systemd ({}) | APT ({}) | Shaders ({}) | Miniaturas ({}) | Lixeira ({})",
        cleanup.journal_usage,
        cleanup.apt_cache_size,
        cleanup.shader_cache_size,
        cleanup.thumbnails_size,
        cleanup.trash_size
    );
    let hogs = core::optimizer::Optimizer::scan_resource_hogs();
    println!("   Vilões de CPU (Top 3):");
    for p in hogs.top_cpu.iter().take(3) {
        println!("     • {} (PID {}): {:.1}% CPU | {:.0} MiB RAM", p.name, p.pid, p.cpu_pct, p.ram_mib);
    }
    println!("   Vilões de RAM (Top 3):");
    for p in hogs.top_ram.iter().take(3) {
        println!("     • {} (PID {}): {:.0} MiB RAM | {:.1}% CPU", p.name, p.pid, p.ram_mib, p.cpu_pct);
    }
    println!("   Processos na GPU Dedicada NVIDIA ({}):", hogs.gpu_processes.len());
    for gp in &hogs.gpu_processes {
        println!("     • {} (PID {}): {} | VRAM: {}", gp.name, gp.pid, gp.engine_type, gp.vram_usage);
    }
    let autostart_apps = core::optimizer::Optimizer::list_autostart_apps();
    println!("   Apps na Inicialização: {} detectados", autostart_apps.len());
    for app in &autostart_apps {
        let status_str = if app.is_enabled { "Ativo" } else { "Desativado" };
        println!("     • {} [{}] ({})", app.name, status_str, app.exec);
    }
    println!();

    println!("⚡ [Analise PC Benchmark (CPU / RAM / NVMe)]");
    let bench = core::stress::StressEngine::run_cpu_benchmark(1000);
    println!("   CPU Single-Core: {} pts", bench.single_core_score);
    println!("   CPU Multi-Core:  {} pts", bench.multi_core_score);
    let ram_bench = core::stress::StressEngine::run_ram_benchmark();
    println!(
        "   RAM DDR5: Leitura {:.1} GB/s | Escrita {:.1} GB/s | Cópia {:.1} GB/s | Latência {:.1} ns",
        ram_bench.read_gbps, ram_bench.write_gbps, ram_bench.copy_gbps, ram_bench.latency_ns
    );
    if let Ok(disk_bench) = core::stress::StressEngine::run_disk_benchmark(128) {
        println!(
            "   SSD NVMe (128 MiB): Leitura {:.0} MB/s | Escrita (fsync) {:.0} MB/s\n",
            disk_bench.seq_read_mbps, disk_bench.seq_write_mbps
        );
    }
}
