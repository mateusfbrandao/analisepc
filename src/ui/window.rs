use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use libadwaita::gio;
use libadwaita::glib;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{
    self, Align, Box as GtkBox, Button, CheckButton, CssProvider, FlowBox, Image, Label, ListBox,
    ListBoxRow, MenuButton, Orientation, PolicyType, ProgressBar, ScrolledWindow, SelectionMode,
    Separator, Spinner, ToggleButton,
};
use libadwaita::prelude::*;
use libadwaita::{
    AboutDialog, ActionRow, AlertDialog, Application, ApplicationWindow, Banner, Breakpoint,
    BreakpointCondition, BreakpointConditionLengthType, EntryRow, ExpanderRow, HeaderBar,
    LengthUnit, NavigationPage, NavigationSplitView, PreferencesGroup, PreferencesPage,
    ResponseAppearance, SpinRow, StatusPage, SwitchRow, Toast, ToastOverlay, ToolbarView,
    ViewStack, WindowTitle,
};

use crate::core::battery::BatteryCollector;
use crate::core::cpu::CpuCollector;
use crate::core::diagnostics::{DiagSeverity, DiagnosticsEngine};
use crate::core::disk::DiskCollector;
use crate::core::exporter::ReportExporter;
use crate::core::gaming::GamingAnalyzer;
use crate::core::gaming_boost::{
    GamingBoost, GamingBoostStatus, SHADER_CONF_PATH, SPLIT_LOCK_CONF_PATH,
};
use crate::core::gpu::GpuCollector;
use crate::core::memory::MemoryCollector;
use crate::core::optimizer::Optimizer;
use crate::core::os_info::OsCollector;
use crate::core::peripherals::{NetworkCollector, PeripheralsCollector};
use crate::core::sensors::SensorsCollector;
use crate::core::stress::StressEngine;
use crate::ui::charts::{
    CoreBarsChart, RealtimeChart, GNOME_BLUE_3, GNOME_GREEN_4,
};

const CUSTOM_CSS: &str = "
/* Cards de resumo (KPIs) seguindo GNOME HIG Boxing & Cards */
.kpi-card {
    padding: 14px 16px;
    border-radius: 12px;
}

.bench-card {
    padding: 18px 20px;
    border-radius: 12px;
}

/* Remove destaque de foco/seleção em FlowBox estático de cards */
flowbox.cards-flowbox child {
    padding: 0;
    background: transparent;
    border: none;
    outline: none;
}

/* Ícones circulares semânticos (adaptativos ao tema claro/escuro) */
.icon-circle {
    min-width: 34px;
    min-height: 34px;
    border-radius: 999px;
}
.icon-circle-blue {
    background-color: alpha(@accent_bg_color, 0.16);
    color: @accent_color;
}
.icon-circle-green {
    background-color: alpha(@success_bg_color, 0.16);
    color: @success_color;
}
.icon-circle-amber {
    background-color: alpha(@warning_bg_color, 0.18);
    color: @warning_color;
}
.icon-circle-purple {
    background-color: alpha(#9141ac, 0.16);
    color: #c061cb;
}
.icon-circle-red {
    background-color: alpha(@error_bg_color, 0.16);
    color: @error_color;
}

/* Barras de progresso semânticas (GNOME HIG Color Palette) */
progressbar.cpu-bar progress {
    background-color: @accent_bg_color;
    border-radius: 99px;
}
progressbar.ram-bar progress {
    background-color: @success_bg_color;
    border-radius: 99px;
}
progressbar.bat-bar progress {
    background-color: @warning_bg_color;
    border-radius: 99px;
}
progressbar.disk-bar progress {
    background-color: @accent_bg_color;
    border-radius: 99px;
}
progressbar.temp-bar progress {
    background-color: @warning_bg_color;
    border-radius: 99px;
}
progressbar.critical-bar progress {
    background-color: @error_bg_color;
    border-radius: 99px;
}
progressbar trough {
    min-height: 6px;
    border-radius: 99px;
    background-color: alpha(currentColor, 0.10);
}

/* Badges semânticos em formato pílula com números tabulares (.numeric) */
.badge-pill {
    padding: 3px 10px;
    border-radius: 999px;
    font-weight: 700;
    font-size: 0.80rem;
    font-variant-numeric: tabular-nums;
}
.badge-green {
    background-color: alpha(@success_bg_color, 0.16);
    color: @success_color;
}
.badge-blue {
    background-color: alpha(@accent_bg_color, 0.16);
    color: @accent_color;
}
.badge-amber {
    background-color: alpha(@warning_bg_color, 0.18);
    color: @warning_color;
}
.badge-red {
    background-color: alpha(@error_bg_color, 0.16);
    color: @error_color;
}
.badge-purple {
    background-color: alpha(#9141ac, 0.18);
    color: #c061cb;
}
.badge-subtle {
    background-color: alpha(currentColor, 0.08);
}
";

pub fn build_ui(app: &Application) {
    let provider = CssProvider::new();
    provider.load_from_data(CUSTOM_CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        let icon_theme = gtk::IconTheme::for_display(&display);
        icon_theme.add_search_path("/usr/share/icons");
        icon_theme.add_search_path("/usr/local/share/icons");
        icon_theme.add_search_path(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons"));
    }
    gtk::Window::set_default_icon_name("io.github.analisepc.AnalisePC");

    // Dimensões baseadas em developer.gnome.org/hig/guidelines/adaptive.html:
    // Desktop baseline >= 1024x600, Mobile minimum = 360x294
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Analise PC")
        .icon_name("io.github.analisepc.AnalisePC")
        .default_width(1040)
        .default_height(680)
        .width_request(360)
        .height_request(294)
        .build();

    let toast_overlay = ToastOverlay::new();
    let view_stack = ViewStack::new();
    view_stack.set_hhomogeneous(false);
    view_stack.set_vhomogeneous(false);

    // Ações da janela para o Primary Menu e atalhos (GNOME HIG Controls & Resources)
    let toast_copy = toast_overlay.clone();
    let action_copy = gio::SimpleAction::new("copy-summary", None);
    action_copy.connect_activate(move |_, _| {
        let os = OsCollector::read();
        let mut cpu = CpuCollector::new();
        let cpu_info = cpu.refresh();
        let mut mem = MemoryCollector::new();
        let mem_info = mem.refresh();
        let summary = format!(
            "Analise PC — Resumo do Sistema\n\
             • Sistema: {} ({}) — Kernel {}\n\
             • Máquina: {} ({} {})\n\
             • CPU: {} ({} núcleos / {} threads)\n\
             • Memória RAM: {:.1} GiB / {:.1} GiB ({:.1}%)",
            os.distro_name,
            os.session_type,
            os.kernel_version,
            os.machine_model,
            os.board_vendor,
            os.board_name,
            cpu_info.brand,
            cpu_info.physical_cores,
            cpu_info.logical_cores,
            mem_info.used_gib(),
            mem_info.total_gib(),
            mem_info.used_percent()
        );
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&summary);
            toast_copy.add_toast(Toast::new(
                "Resumo do hardware copiado para a área de transferência",
            ));
        }
    });
    window.add_action(&action_copy);

    let toast_html = toast_overlay.clone();
    let action_export_html = gio::SimpleAction::new("export-html", None);
    action_export_html.connect_activate(move |_, _| {
        let t_ov = toast_html.clone();
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = ReportExporter::export_html();
            let _ = sender.send_blocking(res);
        });
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                match res {
                    Ok(path) => t_ov.add_toast(Toast::new(&format!(
                        "Laudo HTML salvo em {}",
                        path.display()
                    ))),
                    Err(e) => t_ov.add_toast(Toast::new(&e)),
                }
            }
        });
    });
    window.add_action(&action_export_html);

    let toast_json = toast_overlay.clone();
    let action_export_json = gio::SimpleAction::new("export-json", None);
    action_export_json.connect_activate(move |_, _| {
        let t_ov = toast_json.clone();
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = ReportExporter::export_json();
            let _ = sender.send_blocking(res);
        });
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                match res {
                    Ok(path) => t_ov.add_toast(Toast::new(&format!(
                        "Relatório JSON salvo em {}",
                        path.display()
                    ))),
                    Err(e) => t_ov.add_toast(Toast::new(&e)),
                }
            }
        });
    });
    window.add_action(&action_export_json);

    let win_weak = window.downgrade();
    let action_about = gio::SimpleAction::new("about", None);
    action_about.connect_activate(move |_, _| {
        if let Some(win) = win_weak.upgrade() {
            let about = AboutDialog::builder()
                .application_name("Analise PC")
                .application_icon("io.github.analisepc.AnalisePC")
                .developer_name("Mateus Brandão")
                .version("0.1.0")
                .comments(
                    "Monitoramento de hardware em tempo real, diagnóstico S.M.A.R.T., \
                     telemetria térmica, detector de gargalo e otimização para Linux \
                     projetado conforme a GNOME Human Interface Guidelines (HIG).",
                )
                .website("https://github.com/mateusfbrandao/analisepc")
                .issue_url("https://github.com/mateusfbrandao/analisepc/issues")
                .developers(vec!["Mateus Brandão <https://github.com/mateusfbrandao>"])
                .license_type(gtk::License::Gpl30)
                .copyright("© 2026 Mateus Brandão")
                .build();
            about.present(Some(&win));
        }
    });
    window.add_action(&action_about);

    // Atalhos de teclado padrão GNOME HIG
    app.set_accels_for_action("win.copy-summary", &["<Ctrl><Shift>c"]);
    app.set_accels_for_action("win.export-html", &["<Ctrl>e"]);
    app.set_accels_for_action("win.about", &["F1"]);

    let cpu_collector = Rc::new(RefCell::new(CpuCollector::new()));
    let mem_collector = Rc::new(RefCell::new(MemoryCollector::new()));
    let sensors_collector = Rc::new(RefCell::new(SensorsCollector::new()));
    let net_collector = Rc::new(RefCell::new(NetworkCollector::new()));

    // 1. Visão Geral (com Refrigeração e Ventoinhas integradas)
    let overview_page = {
        let mut sens = sensors_collector.borrow_mut();
        build_overview_page(&mut sens)
    };
    view_stack.add_named(&overview_page.widget, Some("overview"));

    // 2. Armazenamento
    let disk_page = build_disk_page();
    view_stack.add_named(&disk_page, Some("disks"));

    // 3. Placas de Vídeo
    let gpu_page = build_gpu_page(&toast_overlay);
    view_stack.add_named(&gpu_page, Some("gpus"));

    // 4. Rede
    let network_page = {
        let mut net = net_collector.borrow_mut();
        build_network_page(&mut net, &toast_overlay)
    };
    view_stack.add_named(&network_page.widget, Some("network"));

    // 5. Periféricos e Áudio
    let peripherals_page = build_peripherals_page();
    view_stack.add_named(&peripherals_page, Some("peripherals"));

    // 6. Diagnóstico do Hardware
    let diagnostics_page = build_diagnostics_page(&toast_overlay);
    view_stack.add_named(&diagnostics_page, Some("diagnostics"));

    // 7. Jogos e Desempenho
    let gaming_page = build_gaming_page(&toast_overlay);
    view_stack.add_named(&gaming_page, Some("gaming"));

    // 8. Otimizações
    let optimizer_page = build_optimizer_page(&toast_overlay);
    view_stack.add_named(&optimizer_page, Some("optimizer"));

    // 9. Benchmark
    let (bench_page, bench_flow) = build_bench_page(&toast_overlay);
    view_stack.add_named(&bench_page, Some("bench"));

    // ===== SIDEBAR NAVEGAÇÃO (developer.gnome.org/hig/patterns/nav/sidebars.html) =====
    let sidebar_header = HeaderBar::new();

    let primary_menu = gio::Menu::new();
    let section_tools = gio::Menu::new();
    section_tools.append(Some("_Copiar Resumo do Hardware"), Some("win.copy-summary"));
    section_tools.append(
        Some("Exportar Laudo Técnico (_HTML)"),
        Some("win.export-html"),
    );
    section_tools.append(Some("Exportar Relatório (_JSON)"), Some("win.export-json"));
    primary_menu.append_section(None, &section_tools);

    let section_about = gio::Menu::new();
    section_about.append(Some("_Sobre o Analise PC"), Some("win.about"));
    primary_menu.append_section(None, &section_about);

    let menu_btn = MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&primary_menu)
        .tooltip_text("Menu Principal (F10)")
        .build();
    sidebar_header.pack_end(&menu_btn);

    let sidebar_list = ListBox::new();
    sidebar_list.add_css_class("navigation-sidebar");
    sidebar_list.set_selection_mode(SelectionMode::Single);

    // Cabeçalho de seções semânticas na Barra Lateral (HIG Sidebars):
    // Seção 1 (índice 0): "MONITORAMENTO"
    // Seção 2 (índice 6): "DIAGNÓSTICO & FERRAMENTAS"
    sidebar_list.set_header_func(|row, _before| {
        let idx = row.index();
        if idx == 0 {
            let label = Label::new(Some("MONITORAMENTO"));
            label.add_css_class("heading");
            label.add_css_class("dim-label");
            label.set_xalign(0.0);
            label.set_margin_start(12);
            label.set_margin_bottom(6);
            label.set_margin_top(4);
            row.set_header(Some(&label));
        } else if idx == 5 {
            let box_header = GtkBox::new(Orientation::Vertical, 6);
            let sep = Separator::new(Orientation::Horizontal);
            sep.set_margin_top(8);
            sep.set_margin_bottom(8);

            let label = Label::new(Some("FERRAMENTAS"));
            label.add_css_class("heading");
            label.add_css_class("dim-label");
            label.set_xalign(0.0);
            label.set_margin_start(12);
            label.set_margin_bottom(6);

            box_header.append(&sep);
            box_header.append(&label);
            row.set_header(Some(&box_header));
        } else {
            row.set_header(gtk::Widget::NONE);
        }
    });

    let items = [
        ("overview", "Visão Geral", "utilities-system-monitor-symbolic"),
        ("disks", "Armazenamento", "drive-harddisk-symbolic"),
        ("gpus", "Placas de Vídeo", "video-display-symbolic"),
        ("network", "Rede", "network-wireless-symbolic"),
        ("peripherals", "Periféricos", "input-mouse-symbolic"),
        ("diagnostics", "Diagnóstico", "dialog-information-symbolic"),
        ("gaming", "Jogos", "input-gaming-symbolic"),
        ("optimizer", "Otimizações", "preferences-system-symbolic"),
        ("bench", "Benchmark", "speedometer-symbolic"),
    ];

    for (page_id, name, icon) in items {
        let row = ListBoxRow::new();
        row.set_widget_name(page_id);
        let hbox = GtkBox::new(Orientation::Horizontal, 12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        hbox.set_margin_start(8);
        hbox.set_margin_end(8);

        let img = Image::from_icon_name(icon);
        let lbl = Label::new(Some(name));
        lbl.set_xalign(0.0);
        lbl.set_hexpand(true);

        hbox.append(&img);
        hbox.append(&lbl);
        row.set_child(Some(&hbox));
        sidebar_list.append(&row);
    }

    let sidebar_scroll = ScrolledWindow::builder()
        .child(&sidebar_list)
        .hscrollbar_policy(PolicyType::Never)
        .build();

    let sidebar_toolbar = ToolbarView::new();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(&sidebar_scroll));

    let sidebar_page = NavigationPage::builder()
        .title("Analise PC")
        .child(&sidebar_toolbar)
        .build();

    // ===== CONTEÚDO PRINCIPAL (ToolbarView + HeaderBar + ToastOverlay + ViewStack) =====
    let content_header = HeaderBar::new();
    let content_title = WindowTitle::new("Visão Geral", "Resumo em tempo real do sistema");
    content_header.set_title_widget(Some(&content_title));

    let copy_btn = Button::builder()
        .icon_name("edit-copy-symbolic")
        .action_name("win.copy-summary")
        .tooltip_text("Copiar Resumo do Hardware (Ctrl+Shift+C)")
        .build();
    content_header.pack_end(&copy_btn);

    toast_overlay.set_child(Some(&view_stack));

    let content_toolbar = ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&toast_overlay));

    let content_page = NavigationPage::builder()
        .title("Visão Geral")
        .child(&content_toolbar)
        .build();

    // SplitView adaptativo com Breakpoints (developer.gnome.org/hig/guidelines/adaptive.html)
    let split_view = NavigationSplitView::new();
    split_view.set_sidebar(Some(&sidebar_page));
    split_view.set_content(Some(&content_page));
    split_view.set_sidebar_width_fraction(0.24);
    split_view.set_min_sidebar_width(200.0);
    split_view.set_max_sidebar_width(260.0);

    // Breakpoint 1 (<= 820sp): Recolhe a sidebar e organiza os 4 KPIs em grade 2x2
    let bp_medium = Breakpoint::new(BreakpointCondition::new_length(
        BreakpointConditionLengthType::MaxWidth,
        820.0,
        LengthUnit::Sp,
    ));
    bp_medium.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    bp_medium.add_setter(
        &overview_page.kpi_flow,
        "max-children-per-line",
        Some(&2u32.to_value()),
    );
    window.add_breakpoint(bp_medium);

    // Breakpoint 2 (<= 560sp): Formato mobile (360px) — empilha cards 1 por linha
    let bp_narrow = Breakpoint::new(BreakpointCondition::new_length(
        BreakpointConditionLengthType::MaxWidth,
        560.0,
        LengthUnit::Sp,
    ));
    bp_narrow.add_setter(&split_view, "collapsed", Some(&true.to_value()));
    bp_narrow.add_setter(
        &overview_page.kpi_flow,
        "max-children-per-line",
        Some(&1u32.to_value()),
    );
    bp_narrow.add_setter(
        &bench_flow,
        "max-children-per-line",
        Some(&1u32.to_value()),
    );
    window.add_breakpoint(bp_narrow);

    // Evento de seleção na sidebar
    let stack_clone = view_stack.clone();
    let title_clone = content_title.clone();
    let content_page_clone = content_page.clone();
    let split_clone = split_view.clone();
    sidebar_list.connect_row_selected(move |_, row| {
        if let Some(r) = row {
            let page_id = r.widget_name();
            let (title, subtitle) = match page_id.as_str() {
                "overview" => ("Visão Geral", "Resumo em tempo real do sistema"),
                "disks" => ("Armazenamento", "Unidades físicas, S.M.A.R.T. e partições"),
                "gpus" => ("Placas de Vídeo", "Adaptadores gráficos, VRAM e drivers"),
                "network" => ("Rede", "Tráfego em tempo real, interfaces e latência"),
                "peripherals" => ("Periféricos", "Monitores DRM, áudio PipeWire e USB"),
                "diagnostics" => ("Diagnóstico", "Integridade dos componentes e eventos do kernel"),
                "gaming" => ("Jogos e Desempenho", "Gargalo, FPS, Steam, MangoHud e pacote de desempenho"),
                "optimizer" => ("Otimizações", "Perfil de energia, manutenção e inicialização"),
                "bench" => ("Benchmark", "Teste sintético Single-Core e Multi-Core"),
                _ => ("Analise PC", ""),
            };
            title_clone.set_title(title);
            title_clone.set_subtitle(subtitle);
            content_page_clone.set_title(title);
            stack_clone.set_visible_child_name(page_id.as_str());
            split_clone.set_show_content(true);
        }
    });

    if let Some(first_row) = sidebar_list.row_at_index(0) {
        sidebar_list.select_row(Some(&first_row));
    }

    window.set_content(Some(&split_view));

    // Primeira leitura imediata para preencher gráficos e cards na abertura
    {
        let mut cpu = cpu_collector.borrow_mut();
        let cpu_info = cpu.refresh();
        let mut mem = mem_collector.borrow_mut();
        let mem_info = mem.refresh();
        let battery = BatteryCollector::read();
        let mut sens = sensors_collector.borrow_mut();
        let sens_report = sens.refresh();

        overview_page.update(&cpu_info, &mem_info, battery.as_ref(), &sens_report);

        let mut net = net_collector.borrow_mut();
        let net_ifaces = net.refresh();
        network_page.update(&net_ifaces);
    }

    // Atualização em tempo real (1s)
    glib::timeout_add_local(Duration::from_millis(1000), move || {
        let mut cpu = cpu_collector.borrow_mut();
        let cpu_info = cpu.refresh();

        let mut mem = mem_collector.borrow_mut();
        let mem_info = mem.refresh();

        let battery = BatteryCollector::read();
        let mut sens = sensors_collector.borrow_mut();
        let sens_report = sens.refresh();

        overview_page.update(&cpu_info, &mem_info, battery.as_ref(), &sens_report);

        let mut net = net_collector.borrow_mut();
        let net_ifaces = net.refresh();
        network_page.update(&net_ifaces);

        glib::ControlFlow::Continue
    });

    window.present();
}

struct KpiCardWidgets {
    card: GtkBox,
    value_label: Label,
    sub_label: Label,
    progress: ProgressBar,
}

fn create_kpi_card(
    title: &str,
    icon_name: &str,
    icon_circle_class: &str,
    bar_class: &str,
) -> KpiCardWidgets {
    let card = GtkBox::new(Orientation::Vertical, 8);
    card.add_css_class("card");
    card.add_css_class("kpi-card");
    card.set_hexpand(true);

    let top_row = GtkBox::new(Orientation::Horizontal, 10);
    top_row.set_valign(Align::Center);

    let icon_box = GtkBox::new(Orientation::Horizontal, 0);
    icon_box.add_css_class("icon-circle");
    icon_box.add_css_class(icon_circle_class);
    icon_box.set_halign(Align::Center);
    icon_box.set_valign(Align::Center);

    let icon = Image::from_icon_name(icon_name);
    icon.set_halign(Align::Center);
    icon.set_valign(Align::Center);
    icon.set_hexpand(true);
    icon_box.append(&icon);

    let title_lbl = Label::new(Some(title));
    title_lbl.add_css_class("caption-heading");
    title_lbl.add_css_class("dim-label");
    title_lbl.set_xalign(0.0);
    title_lbl.set_hexpand(true);
    title_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);

    top_row.append(&icon_box);
    top_row.append(&title_lbl);

    let value_label = Label::new(Some("0.0%"));
    value_label.add_css_class("title-2");
    value_label.add_css_class("numeric");
    value_label.set_xalign(0.0);
    value_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let sub_label = Label::new(Some("Detectando..."));
    sub_label.add_css_class("caption");
    sub_label.add_css_class("dim-label");
    sub_label.add_css_class("numeric");
    sub_label.set_xalign(0.0);
    sub_label.set_ellipsize(gtk::pango::EllipsizeMode::End);

    let progress = ProgressBar::new();
    progress.add_css_class(bar_class);

    card.append(&top_row);
    card.append(&value_label);
    card.append(&sub_label);
    card.append(&progress);

    KpiCardWidgets {
        card,
        value_label,
        sub_label,
        progress,
    }
}

struct OverviewWidgets {
    widget: PreferencesPage,
    kpi_flow: FlowBox,
    kpi_cpu: KpiCardWidgets,
    kpi_ram: KpiCardWidgets,
    kpi_temp: KpiCardWidgets,
    kpi_uptime: KpiCardWidgets,
    uptime_row: ActionRow,
    cpu_model_row: ActionRow,
    cpu_bar: ProgressBar,
    cpu_badge: Label,
    cpu_chart: RealtimeChart,
    cpu_cores_chart: CoreBarsChart,
    mem_model_row: ActionRow,
    mem_bar: ProgressBar,
    mem_badge: Label,
    mem_chart: RealtimeChart,
    swap_usage_row: ActionRow,
    swap_badge: Label,
    fan_rows: Vec<(String, Label)>,
    cooling_chart: RealtimeChart,
    cooling_summary_row: ActionRow,
    cooling_summary_badge: Label,
    battery_group: PreferencesGroup,
    battery_level_row: ActionRow,
    battery_bar: ProgressBar,
    battery_badge: Label,
    battery_health_row: ActionRow,
    battery_wear_badge: Label,
}

impl OverviewWidgets {
    fn update(
        &self,
        cpu: &crate::core::cpu::CpuInfo,
        mem: &crate::core::memory::MemoryInfo,
        bat: Option<&crate::core::battery::BatteryReport>,
        sens: &crate::core::sensors::SensorsReport,
    ) {
        let uptime_txt = OsCollector::format_uptime();
        self.uptime_row.set_subtitle(&uptime_txt);

        let max_freq = cpu
            .cores
            .iter()
            .map(|c| c.frequency_mhz)
            .max()
            .unwrap_or(0);

        // Atualiza KPI CPU
        self.kpi_cpu
            .value_label
            .set_text(&format!("{:.1}%", cpu.global_usage_percent));
        self.kpi_cpu.sub_label.set_text(&format!(
            "{} núcleos ({} threads) • {} MHz",
            cpu.physical_cores, cpu.logical_cores, max_freq
        ));
        self.kpi_cpu
            .progress
            .set_fraction((cpu.global_usage_percent / 100.0).clamp(0.0, 1.0) as f64);

        // Atualiza KPI RAM
        self.kpi_ram
            .value_label
            .set_text(&format!("{:.1}%", mem.used_percent()));
        self.kpi_ram.sub_label.set_text(&format!(
            "{:.1} GiB / {:.1} GiB em uso",
            mem.used_gib(),
            mem.total_gib()
        ));
        self.kpi_ram
            .progress
            .set_fraction((mem.used_percent() / 100.0).clamp(0.0, 1.0) as f64);

        // Atualiza KPI Temperatura
        let pkg_temp = sens
            .temperatures
            .iter()
            .find(|t| {
                let l = t.label.to_lowercase();
                l.contains("package") || l.contains("coretemp") || l.contains("cpu")
            })
            .or_else(|| sens.temperatures.first())
            .map(|t| t.temperature_c);

        if let Some(t) = pkg_temp {
            self.kpi_temp.value_label.set_text(&format!("{:.1} °C", t));
            let state_txt = if t < 60.0 {
                "Operação fria e silenciosa"
            } else if t < 80.0 {
                "Carga térmica moderada"
            } else {
                "Temperatura elevada"
            };
            self.kpi_temp.sub_label.set_text(state_txt);
            self.kpi_temp
                .progress
                .set_fraction((t as f64 / 100.0).clamp(0.0, 1.0));
        } else {
            self.kpi_temp.value_label.set_text("N/D");
            self.kpi_temp.sub_label.set_text("Sem sensor térmico");
            self.kpi_temp.progress.set_fraction(0.0);
        }

        // Atualiza KPI Energia / Uptime
        if let Some(b) = bat {
            self.kpi_uptime
                .value_label
                .set_text(&format!("{}%", b.capacity_percent));
            self.kpi_uptime.sub_label.set_text(&format!(
                "{} • {:.1} W • Ativo {}",
                b.status, b.power_w, uptime_txt
            ));
            self.kpi_uptime
                .progress
                .set_fraction((b.capacity_percent as f64 / 100.0).clamp(0.0, 1.0));
        } else {
            self.kpi_uptime.value_label.set_text(&uptime_txt);
            self.kpi_uptime.sub_label.set_text("Energia AC Contínua");
            self.kpi_uptime.progress.set_fraction(1.0);
        }

        // Atualiza Grupo Processador
        if !cpu.brand.is_empty() {
            self.cpu_model_row.set_title(&cpu.brand);
        }
        self.cpu_model_row.set_subtitle(&format!(
            "{} núcleos físicos • {} threads • {} MHz",
            cpu.physical_cores, cpu.logical_cores, max_freq
        ));
        self.cpu_bar
            .set_fraction((cpu.global_usage_percent / 100.0) as f64);
        self.cpu_badge
            .set_text(&format!("{:.1}%", cpu.global_usage_percent));
        self.cpu_chart.push_value(cpu.global_usage_percent as f64);
        self.cpu_cores_chart.update_cores(&cpu.cores);

        // Atualiza Grupo Memória
        self.mem_model_row.set_subtitle(&format!(
            "{:.2} GiB em uso de {:.2} GiB disponíveis",
            mem.used_gib(),
            mem.total_gib()
        ));
        self.mem_bar
            .set_fraction((mem.used_percent() / 100.0) as f64);
        self.mem_badge
            .set_text(&format!("{:.1}%", mem.used_percent()));
        self.mem_chart.push_value(mem.used_percent() as f64);

        let swap_total_gib = mem.swap_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let swap_used_gib = mem.swap_used_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let swap_pct = if swap_total_gib > 0.0 {
            (swap_used_gib / swap_total_gib) * 100.0
        } else {
            0.0
        };
        self.swap_usage_row.set_subtitle(&format!(
            "{:.2} GiB utilizados de {:.2} GiB totais",
            swap_used_gib, swap_total_gib
        ));
        self.swap_badge.set_text(&format!("{:.1}%", swap_pct));

        // Atualiza Grupo Refrigeração e Ventoinhas
        let mut max_fan_rpm: Option<u32> = None;
        for fan in &sens.fans {
            if max_fan_rpm.is_none_or(|cur| fan.rpm > cur) {
                max_fan_rpm = Some(fan.rpm);
            }
            if let Some((_, badge)) = self.fan_rows.iter().find(|(l, _)| l == &fan.label) {
                badge.set_text(&format!("{} RPM", fan.rpm));
            }
        }
        if let Some(rpm) = max_fan_rpm {
            self.cooling_chart.push_value(rpm as f64);
            self.cooling_summary_badge.set_text(&format!("{} RPM", rpm));
            self.cooling_summary_row.set_subtitle(&format!(
                "Tacômetro ativo • Pico atual: {} RPM • {} ventoinha(s) monitorada(s)",
                rpm, sens.fans.len()
            ));
        } else {
            self.cooling_summary_badge.set_text("Passivo");
            self.cooling_summary_row.set_subtitle(
                "Sem sensor de ventoinha ou modo passivo de dissipação"
            );
        }

        if let Some(b) = bat {
            self.battery_group.set_visible(true);
            self.battery_level_row.set_subtitle(&format!(
                "{} • {:.2} V • {:.1} W",
                b.status, b.voltage_v, b.power_w
            ));
            self.battery_bar
                .set_fraction((b.capacity_percent as f64) / 100.0);
            self.battery_badge
                .set_text(&format!("{}%", b.capacity_percent));

            self.battery_health_row.set_subtitle(&format!(
                "Saúde: {} • Carga máxima: {:.0} mAh (Projeto: {:.0} mAh)",
                b.health, b.charge_full_mah, b.charge_design_mah
            ));
            self.battery_wear_badge
                .set_text(&format!("{:.1}% desgaste", b.wear_percent));
        } else {
            self.battery_group.set_visible(false);
        }
    }
}

fn build_overview_page(sensors_collector: &mut SensorsCollector) -> OverviewWidgets {
    let page = PreferencesPage::new();
    let os = OsCollector::read();
    let initial_sens = sensors_collector.refresh();

    // 0. Painel Adaptativo de KPIs com FlowBox (developer.gnome.org/hig/guidelines/adaptive.html)
    let kpi_group = PreferencesGroup::new();
    let kpi_flow = FlowBox::new();
    kpi_flow.add_css_class("cards-flowbox");
    kpi_flow.set_selection_mode(SelectionMode::None);
    kpi_flow.set_homogeneous(true);
    kpi_flow.set_column_spacing(12);
    kpi_flow.set_row_spacing(12);
    kpi_flow.set_min_children_per_line(1);
    kpi_flow.set_max_children_per_line(4);

    let kpi_cpu = create_kpi_card(
        "PROCESSADOR",
        "analisepc-cpu-symbolic",
        "icon-circle-blue",
        "cpu-bar",
    );
    let kpi_ram = create_kpi_card(
        "MEMÓRIA RAM",
        "analisepc-ram-symbolic",
        "icon-circle-green",
        "ram-bar",
    );
    let kpi_temp = create_kpi_card(
        "TEMPERATURA",
        "analisepc-temp-symbolic",
        "icon-circle-amber",
        "temp-bar",
    );
    let kpi_uptime = create_kpi_card(
        "ENERGIA / ATIVIDADE",
        "preferences-system-time-symbolic",
        "icon-circle-purple",
        "bat-bar",
    );

    kpi_flow.insert(&kpi_cpu.card, -1);
    kpi_flow.insert(&kpi_ram.card, -1);
    kpi_flow.insert(&kpi_temp.card, -1);
    kpi_flow.insert(&kpi_uptime.card, -1);
    kpi_group.add(&kpi_flow);
    page.add(&kpi_group);

    // 1. Sistema Operacional e Plataforma (No início da visão geral)
    let sys_group = PreferencesGroup::new();
    sys_group.set_title("Sistema e Plataforma");

    let distro_row = ActionRow::new();
    distro_row.add_css_class("property");
    distro_row.set_title("Distribuição e Kernel");
    distro_row.set_subtitle(&format!(
        "{} • Kernel {} ({})",
        os.distro_name, os.kernel_version, os.architecture
    ));
    let distro_icon = Image::from_icon_name("emblem-system-symbolic");
    distro_row.add_prefix(&distro_icon);
    sys_group.add(&distro_row);

    let desktop_row = ActionRow::new();
    desktop_row.add_css_class("property");
    desktop_row.set_title("Ambiente Gráfico");
    desktop_row.set_subtitle(&os.gnome_version);
    let desktop_icon = Image::from_icon_name("preferences-desktop-display-symbolic");
    desktop_row.add_prefix(&desktop_icon);

    let session_badge = Label::new(Some(&os.session_type));
    session_badge.add_css_class("badge-pill");
    session_badge.add_css_class("badge-green");
    session_badge.set_valign(Align::Center);
    desktop_row.add_suffix(&session_badge);
    sys_group.add(&desktop_row);

    let machine_row = ActionRow::new();
    machine_row.add_css_class("property");
    machine_row.set_title("Modelo do Dispositivo");
    machine_row.set_subtitle(&format!("{} ({})", os.machine_model, os.host_name));
    let pc_icon = Image::from_icon_name("computer-symbolic");
    machine_row.add_prefix(&pc_icon);
    sys_group.add(&machine_row);

    let board_row = ActionRow::new();
    board_row.add_css_class("property");
    board_row.set_title("Placa-Mãe e Firmware UEFI/BIOS");
    board_row.set_subtitle(&format!(
        "{} {} (Rev {}) • BIOS {} v{} ({})",
        os.board_vendor,
        os.board_name,
        os.board_version,
        os.bios_vendor,
        os.bios_version,
        os.bios_date
    ));
    let board_icon = Image::from_icon_name("application-x-firmware-symbolic");
    board_row.add_prefix(&board_icon);
    sys_group.add(&board_row);

    let uptime_row = ActionRow::new();
    uptime_row.add_css_class("property");
    uptime_row.set_title("Tempo de Atividade");
    uptime_row.set_subtitle(&OsCollector::format_uptime());
    let clock_icon = Image::from_icon_name("preferences-system-time-symbolic");
    uptime_row.add_prefix(&clock_icon);
    sys_group.add(&uptime_row);

    page.add(&sys_group);

    // 2. Processador
    let cpu_group = PreferencesGroup::new();
    cpu_group.set_title("Processador");

    let cpu_model_row = ActionRow::new();
    cpu_model_row.set_title("Processador do Sistema");
    cpu_model_row.set_subtitle("Detectando núcleos e frequências...");
    let cpu_icon = Image::from_icon_name("analisepc-cpu-symbolic");
    cpu_model_row.add_prefix(&cpu_icon);

    let cpu_box = GtkBox::new(Orientation::Horizontal, 8);
    cpu_box.set_valign(Align::Center);

    let cpu_bar = ProgressBar::new();
    cpu_bar.add_css_class("cpu-bar");
    cpu_bar.set_size_request(68, -1);
    cpu_bar.set_valign(Align::Center);
    cpu_box.append(&cpu_bar);

    let cpu_badge = Label::new(Some("0.0%"));
    cpu_badge.add_css_class("badge-pill");
    cpu_badge.add_css_class("badge-blue");
    cpu_badge.add_css_class("numeric");
    cpu_box.append(&cpu_badge);

    cpu_model_row.add_suffix(&cpu_box);
    cpu_group.add(&cpu_model_row);

    let cpu_chart = RealtimeChart::new("Uso Global (60s)", "%", 100.0, GNOME_BLUE_3);
    let cpu_chart_row = ListBoxRow::new();
    cpu_chart_row.set_activatable(false);
    cpu_chart_row.set_selectable(false);
    cpu_chart_row.set_child(Some(cpu_chart.widget()));
    cpu_group.add(&cpu_chart_row);

    let cpu_cores_chart = CoreBarsChart::new(16);
    let cpu_cores_row = ListBoxRow::new();
    cpu_cores_row.set_activatable(false);
    cpu_cores_row.set_selectable(false);
    cpu_cores_row.set_child(Some(cpu_cores_chart.widget()));
    cpu_group.add(&cpu_cores_row);

    page.add(&cpu_group);

    // 2. Memória RAM e Slots Físicos (com AdwExpanderRow — HIG Boxed Lists Pattern)
    let mem_group = PreferencesGroup::new();
    let ram_modules = MemoryCollector::read_ram_modules();
    if !ram_modules.is_empty() {
        let ch_str = if ram_modules.len() >= 2 {
            "Dual Channel"
        } else {
            "Single Channel"
        };
        let first = &ram_modules[0];
        mem_group.set_title(&format!(
            "Memória do Sistema ({} • {}-{} {})",
            ch_str, first.mem_type, first.configured_speed_mts, first.form_factor
        ));
    } else {
        mem_group.set_title("Memória do Sistema");
    }

    let mem_model_row = ActionRow::new();
    mem_model_row.set_title("Memória Física (RAM)");
    mem_model_row.set_subtitle("Calculando consumo...");
    let mem_icon = Image::from_icon_name("analisepc-ram-symbolic");
    mem_model_row.add_prefix(&mem_icon);

    let mem_box = GtkBox::new(Orientation::Horizontal, 8);
    mem_box.set_valign(Align::Center);

    let mem_bar = ProgressBar::new();
    mem_bar.add_css_class("ram-bar");
    mem_bar.set_size_request(68, -1);
    mem_bar.set_valign(Align::Center);
    mem_box.append(&mem_bar);

    let mem_badge = Label::new(Some("0.0%"));
    mem_badge.add_css_class("badge-pill");
    mem_badge.add_css_class("badge-green");
    mem_badge.add_css_class("numeric");
    mem_box.append(&mem_badge);

    mem_model_row.add_suffix(&mem_box);
    mem_group.add(&mem_model_row);

    let mem_chart = RealtimeChart::new("Histórico de Ocupação (60s)", "%", 100.0, GNOME_GREEN_4);
    let mem_chart_row = ListBoxRow::new();
    mem_chart_row.set_activatable(false);
    mem_chart_row.set_selectable(false);
    mem_chart_row.set_child(Some(mem_chart.widget()));
    mem_group.add(&mem_chart_row);

    for m in &ram_modules {
        let exp_row = ExpanderRow::new();
        exp_row.set_title(&format!(
            "Slot {} — {} GiB {} ({} MT/s)",
            m.slot_locator, m.size_gib, m.mem_type, m.configured_speed_mts
        ));
        exp_row.set_subtitle(&format!("{} • {}", m.manufacturer, m.form_factor));

        let slot_badge = Label::new(Some(&format!("{} GiB", m.size_gib)));
        slot_badge.add_css_class("badge-pill");
        slot_badge.add_css_class("badge-blue");
        slot_badge.add_css_class("numeric");
        slot_badge.set_valign(Align::Center);
        exp_row.add_suffix(&slot_badge);

        let bank_row = ActionRow::new();
        bank_row.add_css_class("property");
        bank_row.set_title("Banco e Organização");
        bank_row.set_subtitle(&format!(
            "{} • Formato {} ({} Rank)",
            m.bank_locator, m.form_factor, m.rank
        ));
        exp_row.add_row(&bank_row);

        let serial_row = ActionRow::new();
        serial_row.add_css_class("property");
        serial_row.set_title("Fabricante e Número de Série");
        serial_row.set_subtitle(&format!("{} • Serial: {}", m.manufacturer, m.serial_number));
        exp_row.add_row(&serial_row);

        mem_group.add(&exp_row);
    }

    let swap_usage_row = ActionRow::new();
    swap_usage_row.set_title("Swap (Paginação em Disco)");
    swap_usage_row.set_subtitle("0 GiB");
    let swap_badge = Label::new(Some("0.0%"));
    swap_badge.add_css_class("badge-pill");
    swap_badge.add_css_class("badge-subtle");
    swap_badge.add_css_class("numeric");
    swap_badge.set_valign(Align::Center);
    swap_usage_row.add_suffix(&swap_badge);
    mem_group.add(&swap_usage_row);

    page.add(&mem_group);

    // 3. Bateria
    let battery_group = PreferencesGroup::new();
    battery_group.set_title("Bateria e Energia");

    let battery_level_row = ActionRow::new();
    battery_level_row.set_title("Carga da Bateria");
    battery_level_row.set_subtitle("Detectando...");
    let bat_icon = Image::from_icon_name("battery-symbolic");
    battery_level_row.add_prefix(&bat_icon);

    let bat_box = GtkBox::new(Orientation::Horizontal, 8);
    bat_box.set_valign(Align::Center);

    let battery_bar = ProgressBar::new();
    battery_bar.add_css_class("bat-bar");
    battery_bar.set_size_request(68, -1);
    battery_bar.set_valign(Align::Center);
    bat_box.append(&battery_bar);

    let battery_badge = Label::new(Some("80%"));
    battery_badge.add_css_class("badge-pill");
    battery_badge.add_css_class("badge-amber");
    battery_badge.add_css_class("numeric");
    bat_box.append(&battery_badge);

    battery_level_row.add_suffix(&bat_box);
    battery_group.add(&battery_level_row);

    let battery_health_row = ActionRow::new();
    battery_health_row.set_title("Integridade da Célula");
    battery_health_row.set_subtitle("Calculando...");

    let battery_wear_badge = Label::new(Some("0.0% desgaste"));
    battery_wear_badge.add_css_class("badge-pill");
    battery_wear_badge.add_css_class("badge-green");
    battery_wear_badge.add_css_class("numeric");
    battery_wear_badge.set_valign(Align::Center);
    battery_health_row.add_suffix(&battery_wear_badge);
    battery_group.add(&battery_health_row);

    page.add(&battery_group);

    // 4. Refrigeração e Ventoinhas (Integrado dos Sensores)
    let cooling_group = PreferencesGroup::new();
    cooling_group.set_title("Refrigeração do Chassi e Ventoinhas");
    cooling_group.set_description(Some(
        "Monitoramento em tempo real da velocidade das ventoinhas e da curva de resfriamento",
    ));

    let cooling_summary_row = ActionRow::new();
    cooling_summary_row.set_title("Velocidade de Pico das Ventoinhas");
    cooling_summary_row.set_subtitle("Calculando rotação...");
    let fan_main_icon = Image::from_icon_name("weather-windy-symbolic");
    cooling_summary_row.add_prefix(&fan_main_icon);

    let cooling_summary_badge = Label::new(Some("0 RPM"));
    cooling_summary_badge.add_css_class("badge-pill");
    cooling_summary_badge.add_css_class("badge-blue");
    cooling_summary_badge.add_css_class("numeric");
    cooling_summary_badge.set_valign(Align::Center);
    cooling_summary_row.add_suffix(&cooling_summary_badge);
    cooling_group.add(&cooling_summary_row);

    let cooling_chart = RealtimeChart::new("Velocidade das Ventoinhas (60s)", " RPM", 5500.0, GNOME_BLUE_3);
    let cooling_chart_row = ListBoxRow::new();
    cooling_chart_row.set_activatable(false);
    cooling_chart_row.set_selectable(false);
    cooling_chart_row.set_child(Some(cooling_chart.widget()));
    cooling_group.add(&cooling_chart_row);

    let mut fan_rows = Vec::new();
    for fan in &initial_sens.fans {
        let row = ActionRow::new();
        row.add_css_class("property");
        row.set_title(&fan.label);
        row.set_subtitle("Tacômetro ativo");

        let fan_icon = Image::from_icon_name("weather-windy-symbolic");
        row.add_prefix(&fan_icon);

        let badge = Label::new(Some(&format!("{} RPM", fan.rpm)));
        badge.add_css_class("badge-pill");
        badge.add_css_class("badge-blue");
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        cooling_group.add(&row);
        fan_rows.push((fan.label.clone(), badge));
    }

    page.add(&cooling_group);

    OverviewWidgets {
        widget: page,
        kpi_flow,
        kpi_cpu,
        kpi_ram,
        kpi_temp,
        kpi_uptime,
        uptime_row,
        cpu_model_row,
        cpu_bar,
        cpu_badge,
        cpu_chart,
        cpu_cores_chart,
        mem_model_row,
        mem_bar,
        mem_badge,
        mem_chart,
        swap_usage_row,
        swap_badge,
        fan_rows,
        cooling_chart,
        cooling_summary_row,
        cooling_summary_badge,
        battery_group,
        battery_level_row,
        battery_bar,
        battery_badge,
        battery_health_row,
        battery_wear_badge,
    }
}

fn build_disk_page() -> PreferencesPage {
    let page = PreferencesPage::new();
    let physical_disks = DiskCollector::list_disks();

    for disk in physical_disks {
        let group = PreferencesGroup::new();

        group.set_title(&disk.model);
        group.set_description(Some(&format!(
            "Serial: {} • Firmware: {} • Estado: {}",
            disk.serial, disk.firmware, disk.state
        )));

        // Linha dedicada de Temperatura Operacional do Disco
        if let Some(temp) = disk.temperature_c {
            let temp_row = ActionRow::new();
            temp_row.add_css_class("property");
            temp_row.set_title("Temperatura Operacional");
            let temp_state = if temp < 50.0 {
                "Excelente (Temperatura segura de operação)"
            } else if temp < 70.0 {
                "Moderada (Carga de trabalho ativa)"
            } else {
                "Elevada (Atenção térmica / Throttle de gravação)"
            };
            temp_row.set_subtitle(temp_state);

            let temp_icon = Image::from_icon_name("analisepc-temp-symbolic");
            temp_row.add_prefix(&temp_icon);

            let t_box = GtkBox::new(Orientation::Horizontal, 8);
            t_box.set_valign(Align::Center);

            let t_bar = ProgressBar::new();
            t_bar.add_css_class("temp-bar");
            t_bar.set_size_request(68, -1);
            t_bar.set_valign(Align::Center);
            t_bar.set_fraction((temp as f64 / 85.0).clamp(0.0, 1.0));
            t_box.append(&t_bar);

            let t_badge = Label::new(Some(&format!("{:.1} °C", temp)));
            t_badge.add_css_class("badge-pill");
            t_badge.add_css_class(if temp < 55.0 {
                "badge-green"
            } else if temp < 70.0 {
                "badge-amber"
            } else {
                "badge-red"
            });
            t_badge.add_css_class("numeric");
            t_box.append(&t_badge);

            temp_row.add_suffix(&t_box);
            group.add(&temp_row);
        }

        if let Some(ref smart) = disk.smart {
            let health_row = ActionRow::new();
            health_row.set_title("Saúde S.M.A.R.T. e Vida Útil");
            health_row.set_subtitle(&format!(
                "{}% restante ({}% desgaste) • Reserva: {}%",
                smart.health_remaining_pct, smart.wear_used_pct, smart.avail_spare_pct
            ));
            let smart_icon = Image::from_icon_name("emblem-ok-symbolic");
            health_row.add_prefix(&smart_icon);

            let h_box = GtkBox::new(Orientation::Horizontal, 8);
            h_box.set_valign(Align::Center);

            let h_bar = ProgressBar::new();
            h_bar.add_css_class("ram-bar");
            h_bar.set_size_request(68, -1);
            h_bar.set_valign(Align::Center);
            h_bar.set_fraction(smart.health_remaining_pct as f64 / 100.0);
            h_box.append(&h_bar);

            let v_badge = Label::new(Some(&smart.verdict));
            v_badge.add_css_class("badge-pill");
            v_badge.add_css_class(if smart.health_remaining_pct >= 80 {
                "badge-green"
            } else {
                "badge-amber"
            });
            h_box.append(&v_badge);

            health_row.add_suffix(&h_box);
            group.add(&health_row);

            let tbw_row = ActionRow::new();
            tbw_row.add_css_class("property");
            tbw_row.set_title("Volume Total de Dados (TBW / Leitura)");
            tbw_row.set_subtitle(&format!(
                "{:.2} TB gravados • {:.2} TB lidos • NVMe {}",
                smart.total_written_tb, smart.total_read_tb, smart.nvme_version
            ));
            group.add(&tbw_row);

            let hours_row = ActionRow::new();
            hours_row.add_css_class("property");
            hours_row.set_title("Histórico de Operação");
            let days = smart.power_on_hours / 24;
            hours_row.set_subtitle(&format!(
                "{} h ligado (~{} dias) • {} ciclos • {} desligamentos abruptos",
                smart.power_on_hours, days, smart.power_cycles, smart.unsafe_shutdowns
            ));

            let err_badge = Label::new(Some(&format!("{} erros", smart.media_errors)));
            err_badge.add_css_class("badge-pill");
            err_badge.add_css_class(if smart.media_errors == 0 {
                "badge-green"
            } else {
                "badge-red"
            });
            err_badge.set_valign(Align::Center);
            hours_row.add_suffix(&err_badge);
            group.add(&hours_row);
        }

        for part in disk.partitions {
            let row = ActionRow::new();
            row.set_title(&format!(
                "Partição {} ({})",
                part.mount_point, part.file_system
            ));
            row.set_subtitle(&format!(
                "{:.1} GiB usados de {:.1} GiB • {:.1} GiB livres",
                part.used_gib, part.total_gib, part.available_gib
            ));
            let part_icon = Image::from_icon_name("drive-harddisk-symbolic");
            row.add_prefix(&part_icon);

            let bar_box = GtkBox::new(Orientation::Horizontal, 8);
            bar_box.set_valign(Align::Center);

            let bar = ProgressBar::new();
            bar.add_css_class(if part.used_percent >= 90.0 {
                "critical-bar"
            } else {
                "disk-bar"
            });
            bar.set_size_request(68, -1);
            bar.set_valign(Align::Center);
            bar.set_fraction((part.used_percent / 100.0) as f64);
            bar_box.append(&bar);

            let pct_label = Label::new(Some(&format!("{:.0}%", part.used_percent)));
            pct_label.add_css_class("badge-pill");
            pct_label.add_css_class(if part.used_percent >= 90.0 {
                "badge-red"
            } else {
                "badge-blue"
            });
            pct_label.add_css_class("numeric");
            bar_box.append(&pct_label);

            row.add_suffix(&bar_box);
            group.add(&row);
        }

        page.add(&group);
    }

    page
}

fn build_gpu_page(toast_overlay: &ToastOverlay) -> gtk::Widget {
    let gpus = GpuCollector::list_gpus();

    if gpus.is_empty() {
        let status = StatusPage::builder()
            .icon_name("video-display-symbolic")
            .title("Nenhuma GPU Detectada")
            .description("Nenhum adaptador de vídeo DRM foi encontrado no sistema")
            .build();
        return status.upcast();
    }

    let page = PreferencesPage::new();

    // Se houver gráficos híbridos (iGPU + dGPU), exibe o Controle NVIDIA Optimus / PRIME no topo
    if gpus.iter().any(|g| g.is_dedicated) {
        let hybrid = Optimizer::read_hybrid_gpu_thermal();
        let prime_group = PreferencesGroup::new();
        prime_group.set_title("Controle de Gráficos Híbridos (NVIDIA Optimus / PRIME)");
        prime_group.set_description(Some(
            "Gerencie a alternância entre a placa integrada (economia de bateria) e a placa dedicada NVIDIA",
        ));

        let status_row = ActionRow::new();
        status_row.set_title("Modo Gráfico Ativo");
        status_row.set_subtitle(&format!(
            "Modo: {} • dGPU PCI: {} (requer novo login para aplicar)",
            hybrid.prime_mode.to_uppercase(),
            hybrid.dgpu_runtime_status
        ));
        let prime_icon = Image::from_icon_name("video-display-symbolic");
        status_row.add_prefix(&prime_icon);

        let prime_badge = Label::new(Some(&hybrid.prime_mode.to_uppercase()));
        prime_badge.add_css_class("badge-pill");
        prime_badge.add_css_class("badge-green");
        prime_badge.set_valign(Align::Center);
        status_row.add_suffix(&prime_badge);
        prime_group.add(&status_row);

        // Seletor de modo em linha própria com botões compactos
        let mode_row = ActionRow::new();
        mode_row.set_title("Alternar Modo PRIME");
        mode_row.set_subtitle("Escolha o perfil e confirme com senha de administrador");

        let mode_box = GtkBox::new(Orientation::Horizontal, 6);
        mode_box.set_valign(Align::Center);

        let btn_ondemand = Button::with_label("Híbrido (On-Demand)");
        btn_ondemand.add_css_class("row-action-button");
        btn_ondemand.set_tooltip_text(Some(
            "Modo Recomendado: A placa integrada Intel gerencia a tela e economiza bateria. A GPU NVIDIA dedicada é ligada sob demanda apenas ao abrir jogos ou aplicativos pesados."
        ));

        let btn_nvidia = Button::with_label("NVIDIA");
        btn_nvidia.add_css_class("row-action-button");
        btn_nvidia.set_tooltip_text(Some(
            "Desempenho Máximo: Mantém a GPU NVIDIA ligada 100% do tempo para renderizar todo o desktop e programas. Oferece a maior taxa de quadros e menor latência, mas consome mais bateria."
        ));

        let btn_intel = Button::with_label("Intel");
        btn_intel.add_css_class("row-action-button");
        btn_intel.set_tooltip_text(Some(
            "Economia Extrema: Desliga completamente a GPU NVIDIA dedicada do barramento PCI. Ideal para viagens ou uso em bateria, reduzindo o calor e o consumo de energia ao mínimo."
        ));

        let bind_prime_btn = |btn: &Button, target_mode: &'static str| {
            let row_c = status_row.clone();
            let badge_c = prime_badge.clone();
            let toast_c = toast_overlay.clone();
            let b_od = btn_ondemand.clone();
            let b_nv = btn_nvidia.clone();
            let b_in = btn_intel.clone();
            btn.connect_clicked(move |_| {
                b_od.set_sensitive(false);
                b_nv.set_sensitive(false);
                b_in.set_sensitive(false);
                row_c.set_subtitle(&format!("Aplicando modo PRIME '{}' via pkexec...", target_mode));

                let (sender, receiver) = async_channel::bounded(1);
                std::thread::spawn(move || {
                    let res = Optimizer::set_prime_mode(target_mode);
                    let _ = sender.send_blocking(res);
                });

                let r_dest = row_c.clone();
                let b_dest = badge_c.clone();
                let t_dest = toast_c.clone();
                let od_d = b_od.clone();
                let nv_d = b_nv.clone();
                let in_d = b_in.clone();
                glib::spawn_future_local(async move {
                    if let Ok(res) = receiver.recv().await {
                        od_d.set_sensitive(true);
                        nv_d.set_sensitive(true);
                        in_d.set_sensitive(true);
                        match res {
                            Ok(msg) => {
                                b_dest.set_text(&target_mode.to_uppercase());
                                r_dest.set_subtitle(&format!("{} (Requer novo login)", msg));
                                t_dest.add_toast(Toast::new(&msg));
                            }
                            Err(e) => {
                                r_dest.set_subtitle(&e);
                                t_dest.add_toast(Toast::new(&e));
                            }
                        }
                    }
                });
            });
        };

        bind_prime_btn(&btn_ondemand, "on-demand");
        bind_prime_btn(&btn_nvidia, "nvidia");
        bind_prime_btn(&btn_intel, "intel");

        mode_box.append(&btn_ondemand);
        mode_box.append(&btn_nvidia);
        mode_box.append(&btn_intel);
        mode_row.add_suffix(&mode_box);
        prime_group.add(&mode_row);

        let offload_row = ActionRow::new();
        offload_row.add_css_class("property");
        offload_row.set_title("Parâmetro de Inicialização Sob Demanda (Steam / Lutris)");
        offload_row.set_subtitle(&hybrid.prime_offload_cmd);

        let copy_offload_btn = Button::with_label("Copiar");
        copy_offload_btn.set_valign(Align::Center);
        copy_offload_btn.add_css_class("row-action-button");
        let cmd_str = hybrid.prime_offload_cmd.clone();
        let toast_off = toast_overlay.clone();
        copy_offload_btn.connect_clicked(move |_| {
            if let Some(display) = gtk::gdk::Display::default() {
                display.clipboard().set_text(&cmd_str);
                toast_off.add_toast(Toast::new(
                    "Comando PRIME Render Offload copiado!",
                ));
            }
        });
        offload_row.add_suffix(&copy_offload_btn);
        prime_group.add(&offload_row);

        page.add(&prime_group);
    }

    for (index, gpu) in gpus.iter().enumerate() {
        let group = PreferencesGroup::new();
        let (type_label, badge_class) = if gpu.is_dedicated {
            ("Dedicada", "badge-green")
        } else {
            ("Integrada", "badge-blue")
        };

        group.set_title(&gpu.model);
        group.set_description(Some(&format!(
            "GPU #{} • Fabricante: {} • Slot PCI: {}",
            index + 1,
            gpu.vendor,
            gpu.pci_id
        )));

        let model_row = ActionRow::new();
        model_row.add_css_class("property");
        model_row.set_title("Processador Gráfico");
        model_row.set_subtitle(&gpu.model);
        let gpu_icon = Image::from_icon_name("video-display-symbolic");
        model_row.add_prefix(&gpu_icon);

        let type_badge = Label::new(Some(type_label));
        type_badge.add_css_class("badge-pill");
        type_badge.add_css_class(badge_class);
        type_badge.set_valign(Align::Center);
        model_row.add_suffix(&type_badge);
        group.add(&model_row);

        if let Some(ref vram) = gpu.vram {
            let vram_row = ActionRow::new();
            vram_row.add_css_class("property");
            vram_row.set_title("Memória de Vídeo (VRAM)");
            vram_row.set_subtitle(vram);
            group.add(&vram_row);
        }

        if let Some(ref clocks) = gpu.clocks_info {
            let clk_row = ActionRow::new();
            clk_row.add_css_class("property");
            clk_row.set_title("Frequências de Operação");
            clk_row.set_subtitle(clocks);
            group.add(&clk_row);
        }

        if let Some(ref pcie) = gpu.pcie_link {
            let pcie_row = ActionRow::new();
            pcie_row.add_css_class("property");
            pcie_row.set_title("Interface de Barramento PCIe");
            pcie_row.set_subtitle(pcie);
            group.add(&pcie_row);
        }

        if gpu.temperature_c.is_some() || gpu.power_info.is_some() {
            let live_row = ActionRow::new();
            live_row.add_css_class("property");
            live_row.set_title("Telemetria Térmica e Energia");
            let temp_str = gpu
                .temperature_c
                .map(|t| format!("Temperatura: {} °C", t))
                .unwrap_or_default();
            let pwr_str = gpu
                .power_info
                .as_ref()
                .map(|p| format!(" • Consumo: {}", p))
                .unwrap_or_default();
            let util_str = gpu
                .gpu_util_pct
                .map(|u| format!(" • Carga GPU: {}%", u))
                .unwrap_or_default();
            live_row.set_subtitle(&format!("{}{}{}", temp_str, pwr_str, util_str));
            group.add(&live_row);
        }

        let driver_row = ActionRow::new();
        driver_row.add_css_class("property");
        driver_row.set_title("Driver do Kernel e Firmware VBIOS");
        let vbios_str = gpu
            .vbios_version
            .as_ref()
            .map(|v| format!(" • VBIOS: {}", v))
            .unwrap_or_default();
        driver_row.set_subtitle(&format!("Driver: {}{}", gpu.driver, vbios_str));
        group.add(&driver_row);

        let api_row = ActionRow::new();
        api_row.add_css_class("property");
        api_row.set_title("APIs Gráficas Suportadas");
        api_row.set_subtitle(&gpu.apis_supported);
        group.add(&api_row);

        if gpu.is_dedicated {
            let hogs = Optimizer::scan_resource_hogs();
            let procs_row = ActionRow::new();
            procs_row.add_css_class("property");
            procs_row.set_title(&format!(
                "Processos Usando a GPU Dedicada ({})",
                hogs.gpu_processes.len()
            ));
            if hogs.gpu_processes.is_empty() {
                procs_row.set_subtitle(
                    "Nenhum processo mantendo a dGPU ativa no momento (Economia de bateria máxima)",
                );
            } else {
                let list_str = hogs
                    .gpu_processes
                    .iter()
                    .map(|p| format!("{} (PID {} • {})", p.name.replace('&', "e"), p.pid, p.vram_usage))
                    .collect::<Vec<_>>()
                    .join("  •  ");
                procs_row.set_subtitle(&list_str);
            }
            group.add(&procs_row);
        }

        page.add(&group);
    }

    page.upcast()
}

fn build_bench_page(toast_overlay: &ToastOverlay) -> (PreferencesPage, FlowBox) {
    let page = PreferencesPage::new();
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let cards_group = PreferencesGroup::new();
    cards_group.set_title("Desempenho Computacional da CPU");
    cards_group.set_description(Some(
        "Avalia a capacidade de processamento matemático de ponto flutuante (FPU) em thread única e em paralelismo máximo",
    ));

    let cards_flow = FlowBox::new();
    cards_flow.add_css_class("cards-flowbox");
    cards_flow.set_selection_mode(SelectionMode::None);
    cards_flow.set_homogeneous(true);
    cards_flow.set_column_spacing(14);
    cards_flow.set_row_spacing(14);
    cards_flow.set_min_children_per_line(1);
    cards_flow.set_max_children_per_line(2);
    cards_flow.set_margin_bottom(12);

    // Card Single-Core
    let single_card = GtkBox::new(Orientation::Vertical, 10);
    single_card.add_css_class("card");
    single_card.add_css_class("bench-card");

    let sc_top = GtkBox::new(Orientation::Horizontal, 8);
    let sc_title = Label::new(Some("SINGLE-CORE"));
    sc_title.add_css_class("caption-heading");
    sc_title.add_css_class("dim-label");
    sc_title.set_xalign(0.0);
    sc_title.set_hexpand(true);

    let sc_badge = Label::new(Some("1 Thread"));
    sc_badge.add_css_class("badge-pill");
    sc_badge.add_css_class("badge-blue");
    sc_top.append(&sc_title);
    sc_top.append(&sc_badge);

    let single_score_lbl = Label::new(Some("—"));
    single_score_lbl.add_css_class("title-1");
    single_score_lbl.add_css_class("numeric");
    single_score_lbl.set_xalign(0.0);

    let single_sub_lbl = Label::new(Some("Aguardando execução do benchmark"));
    single_sub_lbl.add_css_class("caption");
    single_sub_lbl.add_css_class("dim-label");
    single_sub_lbl.set_xalign(0.0);
    single_sub_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);

    single_card.append(&sc_top);
    single_card.append(&single_score_lbl);
    single_card.append(&single_sub_lbl);

    // Card Multi-Core
    let multi_card = GtkBox::new(Orientation::Vertical, 10);
    multi_card.add_css_class("card");
    multi_card.add_css_class("bench-card");

    let mc_top = GtkBox::new(Orientation::Horizontal, 8);
    let mc_title = Label::new(Some("MULTI-CORE"));
    mc_title.add_css_class("caption-heading");
    mc_title.add_css_class("dim-label");
    mc_title.set_xalign(0.0);
    mc_title.set_hexpand(true);

    let mc_badge = Label::new(Some(&format!("{} Threads", num_threads)));
    mc_badge.add_css_class("badge-pill");
    mc_badge.add_css_class("badge-purple");
    mc_top.append(&mc_title);
    mc_top.append(&mc_badge);

    let multi_score_lbl = Label::new(Some("—"));
    multi_score_lbl.add_css_class("title-1");
    multi_score_lbl.add_css_class("numeric");
    multi_score_lbl.set_xalign(0.0);

    let multi_sub_lbl = Label::new(Some("Escalonamento paralelo em todos os núcleos"));
    multi_sub_lbl.add_css_class("caption");
    multi_sub_lbl.add_css_class("dim-label");
    multi_sub_lbl.set_xalign(0.0);
    multi_sub_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);

    multi_card.append(&mc_top);
    multi_card.append(&multi_score_lbl);
    multi_card.append(&multi_sub_lbl);

    cards_flow.insert(&single_card, -1);
    cards_flow.insert(&multi_card, -1);
    cards_group.add(&cards_flow);

    // Controle SpinRow para configurar o tempo do teste (HIG Spin Buttons Pattern)
    let duration_spin = SpinRow::with_range(500.0, 5000.0, 500.0);
    duration_spin.set_title("_Duração por Etapa (ms)");
    duration_spin.set_use_underline(true);
    duration_spin.set_subtitle("Tempo de estresse dedicado a cada modo (Single-Core e Multi-Core)");
    duration_spin.set_value(1000.0);
    cards_group.add(&duration_spin);

    // Linha de controle com único botão .suggested-action na vista (HIG Buttons Pattern)
    let action_row = ActionRow::new();
    action_row.set_title("Executar Bateria de Testes");
    action_row.set_subtitle("Inicia a medição matemática em ponto flutuante");

    let spinner = Spinner::new();
    spinner.set_visible(false);
    action_row.add_prefix(&spinner);

    let start_btn = Button::with_mnemonic("_Iniciar Benchmark");
    start_btn.set_valign(Align::Center);
    start_btn.add_css_class("suggested-action");
    start_btn.add_css_class("pill");
    action_row.add_suffix(&start_btn);

    cards_group.add(&action_row);
    page.add(&cards_group);

    let sc_val = single_score_lbl.clone();
    let sc_sub = single_sub_lbl.clone();
    let mc_val = multi_score_lbl.clone();
    let mc_sub = multi_sub_lbl.clone();
    let spinner_clone = spinner.clone();
    let btn_clone = start_btn.clone();
    let spin_row_clone = duration_spin.clone();
    let toast_ov = toast_overlay.clone();

    start_btn.connect_clicked(move |_| {
        let duration_ms = spin_row_clone.value() as u64;
        btn_clone.set_sensitive(false);
        spin_row_clone.set_sensitive(false);
        spinner_clone.set_visible(true);
        spinner_clone.start();
        sc_val.set_text("...");
        mc_val.set_text("...");
        sc_sub.set_text("Executando estresse Single-Core...");
        mc_sub.set_text("Executando estresse Multi-Core...");

        let (sender, receiver) = async_channel::bounded(1);

        std::thread::spawn(move || {
            let result = StressEngine::run_cpu_benchmark(duration_ms);
            let _ = sender.send_blocking(result);
        });

        let sc_v = sc_val.clone();
        let sc_s = sc_sub.clone();
        let mc_v = mc_val.clone();
        let mc_s = mc_sub.clone();
        let spin_dest = spinner_clone.clone();
        let btn_dest = btn_clone.clone();
        let dur_dest = spin_row_clone.clone();
        let toast_dest = toast_ov.clone();

        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                spin_dest.stop();
                spin_dest.set_visible(false);
                btn_dest.set_sensitive(true);
                dur_dest.set_sensitive(true);

                let ratio = if res.single_core_score > 0 {
                    res.multi_core_score as f64 / res.single_core_score as f64
                } else {
                    1.0
                };

                sc_v.set_text(&format!("{} pts", res.single_core_score));
                sc_s.set_text("Desempenho por núcleo (IPC / Clock Boost)");
                mc_v.set_text(&format!("{} pts", res.multi_core_score));
                mc_s.set_text(&format!("Ganho paralelo: {:.2}x sobre Single-Core", ratio));

                let toast = Toast::new(&format!(
                    "Benchmark concluído: {} pts (Single) / {} pts (Multi)",
                    res.single_core_score, res.multi_core_score
                ));
                toast_dest.add_toast(toast);
            }
        });
    });

    // 2. Benchmark de Memória RAM (Largura de Banda DDR4/DDR5 em GB/s + Latência ns)
    let ram_group = PreferencesGroup::new();
    ram_group.set_title("Benchmark de Memória RAM (Largura de Banda e Latência)");
    ram_group.set_description(Some(
        "Mede a taxa real de transferência em paralelo (256 MiB fora do cache L3) para validar o ganho Dual Channel",
    ));

    let ram_res_row = ActionRow::new();
    ram_res_row.set_title("Largura de Banda (Leitura / Escrita / Cópia) e Latência");
    ram_res_row.set_subtitle("Clique em Testar RAM para medir GB/s e tempo de acesso em nanossegundos");
    let ram_bench_icon = Image::from_icon_name("analisepc-ram-symbolic");
    ram_res_row.add_prefix(&ram_bench_icon);

    let ram_spinner = Spinner::new();
    ram_spinner.set_visible(false);
    ram_res_row.add_prefix(&ram_spinner);

    let ram_badge = Label::new(Some("Aguardando"));
    ram_badge.add_css_class("badge-pill");
    ram_badge.add_css_class("badge-subtle");
    ram_badge.add_css_class("numeric");
    ram_badge.set_valign(Align::Center);
    ram_res_row.add_suffix(&ram_badge);

    let ram_btn = Button::with_mnemonic("Testar _RAM");
    ram_btn.set_valign(Align::Center);
    ram_btn.add_css_class("row-action-button");
    ram_res_row.add_suffix(&ram_btn);

    let r_row_c = ram_res_row.clone();
    let r_spin_c = ram_spinner.clone();
    let r_badge_c = ram_badge.clone();
    let r_btn_c = ram_btn.clone();
    let toast_ram_b = toast_overlay.clone();

    ram_btn.connect_clicked(move |_| {
        r_btn_c.set_sensitive(false);
        r_spin_c.set_visible(true);
        r_spin_c.start();
        r_row_c.set_subtitle("Executando varredura paralela de leitura, escrita, cópia e latência...");

        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = StressEngine::run_ram_benchmark();
            let _ = sender.send_blocking(res);
        });

        let row_d = r_row_c.clone();
        let spin_d = r_spin_c.clone();
        let badge_d = r_badge_c.clone();
        let btn_d = r_btn_c.clone();
        let t_ov = toast_ram_b.clone();

        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                spin_d.stop();
                spin_d.set_visible(false);
                btn_d.set_sensitive(true);

                row_d.set_subtitle(&format!(
                    "Leitura: {:.1} GB/s  •  Escrita: {:.1} GB/s  •  Cópia: {:.1} GB/s  •  Latência: {:.1} ns",
                    res.read_gbps, res.write_gbps, res.copy_gbps, res.latency_ns
                ));
                badge_d.set_text(&format!("{:.1} GB/s", res.copy_gbps));
                badge_d.remove_css_class("badge-subtle");
                badge_d.add_css_class("badge-green");

                t_ov.add_toast(Toast::new(&format!(
                    "RAM Benchmark: {:.1} GB/s Cópia • {:.1} ns Latência",
                    res.copy_gbps, res.latency_ns
                )));
            }
        });
    });

    ram_group.add(&ram_res_row);
    page.add(&ram_group);

    // 3. Benchmark de Armazenamento (SSD NVMe Sequencial em MB/s)
    let disk_group = PreferencesGroup::new();
    disk_group.set_title("Benchmark de Armazenamento (SSD NVMe / Disco)");
    disk_group.set_description(Some(
        "Mede a velocidade real de gravação física (com fsync) e leitura sequencial em blocos de 4 MiB",
    ));

    let disk_size_spin = SpinRow::with_range(128.0, 1024.0, 128.0);
    disk_size_spin.set_title("_Volume do Arquivo Temporário (MiB)");
    disk_size_spin.set_use_underline(true);
    disk_size_spin.set_subtitle("O arquivo temporário é removido automaticamente logo após a medição");
    disk_size_spin.set_value(256.0);
    disk_group.add(&disk_size_spin);

    let disk_res_row = ActionRow::new();
    disk_res_row.set_title("Taxa de Transferência Sequencial (Leitura e Escrita)");
    disk_res_row.set_subtitle("Clique em Testar Disco para aferir o desempenho real em MB/s");
    let disk_bench_icon = Image::from_icon_name("drive-harddisk-solidstate-symbolic");
    disk_res_row.add_prefix(&disk_bench_icon);

    let disk_spinner = Spinner::new();
    disk_spinner.set_visible(false);
    disk_res_row.add_prefix(&disk_spinner);

    let disk_badge = Label::new(Some("Aguardando"));
    disk_badge.add_css_class("badge-pill");
    disk_badge.add_css_class("badge-subtle");
    disk_badge.add_css_class("numeric");
    disk_badge.set_valign(Align::Center);
    disk_res_row.add_suffix(&disk_badge);

    let disk_btn = Button::with_mnemonic("Testar _Disco");
    disk_btn.set_valign(Align::Center);
    disk_btn.add_css_class("row-action-button");
    disk_res_row.add_suffix(&disk_btn);

    let d_row_c = disk_res_row.clone();
    let d_spin_c = disk_spinner.clone();
    let d_badge_c = disk_badge.clone();
    let d_btn_c = disk_btn.clone();
    let d_size_c = disk_size_spin.clone();
    let toast_disk_b = toast_overlay.clone();

    disk_btn.connect_clicked(move |_| {
        let size_mib = d_size_c.value() as u64;
        d_btn_c.set_sensitive(false);
        d_size_c.set_sensitive(false);
        d_spin_c.set_visible(true);
        d_spin_c.start();
        d_row_c.set_subtitle(&format!(
            "Gravando e lendo {} MiB com sincronização física no disco...",
            size_mib
        ));

        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = StressEngine::run_disk_benchmark(size_mib);
            let _ = sender.send_blocking(res);
        });

        let row_d = d_row_c.clone();
        let spin_d = d_spin_c.clone();
        let badge_d = d_badge_c.clone();
        let btn_d = d_btn_c.clone();
        let size_d = d_size_c.clone();
        let t_ov = toast_disk_b.clone();

        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                spin_d.stop();
                spin_d.set_visible(false);
                btn_d.set_sensitive(true);
                size_d.set_sensitive(true);

                match res {
                    Ok(r) => {
                        row_d.set_subtitle(&format!(
                            "Leitura Sequencial: {:.0} MB/s  •  Escrita Física (fsync): {:.0} MB/s  •  Bloco: {} MiB",
                            r.seq_read_mbps, r.seq_write_mbps, r.test_size_mib
                        ));
                        badge_d.set_text(&format!("{:.0} MB/s", r.seq_read_mbps));
                        badge_d.remove_css_class("badge-subtle");
                        badge_d.add_css_class("badge-blue");

                        t_ov.add_toast(Toast::new(&format!(
                            "Disco: {:.0} MB/s Leitura • {:.0} MB/s Escrita",
                            r.seq_read_mbps, r.seq_write_mbps
                        )));
                    }
                    Err(e) => {
                        row_d.set_subtitle(&e);
                        t_ov.add_toast(Toast::new(&e));
                    }
                }
            }
        });
    });

    disk_group.add(&disk_res_row);
    page.add(&disk_group);

    // 4. Stress Test Contínuo de Estabilidade com Trava Térmica Automática
    let stress_group = PreferencesGroup::new();
    stress_group.set_title("Teste de Estabilidade Contínuo com Trava Térmica");
    stress_group.set_description(Some(
        "Estressa todos os núcleos e controladora de memória continuamente, abortando automaticamente caso a temperatura atinja o limite de segurança",
    ));

    let temp_limit_spin = SpinRow::with_range(70.0, 100.0, 1.0);
    temp_limit_spin.set_title("Limite de _Corte Térmico Automático (°C)");
    temp_limit_spin.set_use_underline(true);
    temp_limit_spin.set_subtitle("Interrompe imediatamente o Stress Test se a CPU atingir esta temperatura");
    temp_limit_spin.set_value(90.0);
    stress_group.add(&temp_limit_spin);

    let stress_row = ActionRow::new();
    stress_row.set_title("Monitor de Estresse e Proteção Térmica");
    stress_row.set_subtitle("Pronto — Clique em Iniciar Stress para testar a refrigeração do sistema");
    let stress_icon = Image::from_icon_name("analisepc-temp-symbolic");
    stress_row.add_prefix(&stress_icon);

    let stress_spinner = Spinner::new();
    stress_spinner.set_visible(false);
    stress_row.add_prefix(&stress_spinner);

    let stress_badge = Label::new(Some("Inativo"));
    stress_badge.add_css_class("badge-pill");
    stress_badge.add_css_class("badge-subtle");
    stress_badge.add_css_class("numeric");
    stress_badge.set_valign(Align::Center);
    stress_row.add_suffix(&stress_badge);

    let stress_btn = Button::with_mnemonic("Iniciar _Stress");
    stress_btn.set_valign(Align::Center);
    stress_btn.add_css_class("row-action-button");
    stress_row.add_suffix(&stress_btn);

    let stop_flag = Rc::new(RefCell::new(None::<std::sync::Arc<std::sync::atomic::AtomicBool>>));
    let s_row_c = stress_row.clone();
    let s_spin_c = stress_spinner.clone();
    let s_badge_c = stress_badge.clone();
    let s_btn_c = stress_btn.clone();
    let s_limit_c = temp_limit_spin.clone();
    let toast_stress = toast_overlay.clone();

    stress_btn.connect_clicked(move |_| {
        // Se já estiver rodando, parar imediatamente
        if let Some(flag) = stop_flag.borrow_mut().take() {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            s_spin_c.stop();
            s_spin_c.set_visible(false);
            s_btn_c.set_label("Iniciar _Stress");
            s_btn_c.remove_css_class("destructive-action");
            s_badge_c.set_text("Parado");
            s_badge_c.remove_css_class("badge-amber");
            s_badge_c.add_css_class("badge-subtle");
            s_row_c.set_subtitle("Teste de estabilidade encerrado manualmente pelo usuário");
            return;
        }

        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        *stop_flag.borrow_mut() = Some(flag.clone());

        let flag_thread = flag.clone();
        std::thread::spawn(move || {
            StressEngine::run_continuous_stress(flag_thread);
        });

        s_spin_c.set_visible(true);
        s_spin_c.start();
        s_btn_c.set_label("_Parar Stress");
        s_btn_c.add_css_class("destructive-action");
        s_badge_c.set_text("Em Carga 100%");
        s_badge_c.remove_css_class("badge-subtle");
        s_badge_c.remove_css_class("badge-red");
        s_badge_c.add_css_class("badge-amber");

        let start_inst = std::time::Instant::now();
        let mut peak_temp: f32 = 0.0;

        let flag_watch = flag.clone();
        let stop_holder = stop_flag.clone();
        let row_w = s_row_c.clone();
        let spin_w = s_spin_c.clone();
        let badge_w = s_badge_c.clone();
        let btn_w = s_btn_c.clone();
        let limit_w = s_limit_c.clone();
        let t_ov = toast_stress.clone();

        glib::timeout_add_local(Duration::from_millis(500), move || {
            if flag_watch.load(std::sync::atomic::Ordering::Relaxed) {
                return glib::ControlFlow::Break;
            }

            let cur_temp = StressEngine::read_max_cpu_temp_c();
            if cur_temp > peak_temp {
                peak_temp = cur_temp;
            }
            let limit_c = limit_w.value() as f32;
            let elapsed_s = start_inst.elapsed().as_secs();

            if cur_temp >= limit_c {
                // Aciona Trava Térmica!
                flag_watch.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = stop_holder.borrow_mut().take();
                spin_w.stop();
                spin_w.set_visible(false);
                btn_w.set_label("Iniciar _Stress");
                btn_w.remove_css_class("destructive-action");
                badge_w.set_text(&format!("Corte {:.1} °C", cur_temp));
                badge_w.remove_css_class("badge-amber");
                badge_w.add_css_class("badge-red");

                let msg = format!(
                    "Trava Térmica Acionada aos {}s! CPU atingiu {:.1} °C (Limite configurado: {:.0} °C)",
                    elapsed_s, cur_temp, limit_c
                );
                row_w.set_subtitle(&msg);
                t_ov.add_toast(Toast::new(&msg));
                return glib::ControlFlow::Break;
            }

            row_w.set_subtitle(&format!(
                "Tempo: {}s  •  Temp. Atual da CPU: {:.1} °C  •  Pico: {:.1} °C  •  Trava: {:.0} °C",
                elapsed_s, cur_temp, peak_temp, limit_c
            ));
            badge_w.set_text(&format!("{:.1} °C ({}s)", cur_temp, elapsed_s));

            glib::ControlFlow::Continue
        });
    });

    stress_group.add(&stress_row);
    page.add(&stress_group);

    (page, cards_flow)
}

fn build_optimizer_page(toast_overlay: &ToastOverlay) -> PreferencesPage {
    let page = PreferencesPage::new();

    // 1. Perfil de Energia usando Radio Buttons em Boxed List
    // (developer.gnome.org/hig/patterns/controls/radio-buttons.html — recomendado para < 4 opções)
    let power_group = PreferencesGroup::new();
    power_group.set_title("Modo de Energia");
    power_group.set_description(Some(
        "Afeta o comportamento térmico, consumo de bateria e frequência do processador",
    ));

    let current = Optimizer::get_power_profile();

    let perf_row = ActionRow::new();
    perf_row.set_title("_Alto Desempenho");
    perf_row.set_use_underline(true);
    perf_row.set_subtitle("Prioriza frequência máxima de clock e resposta imediata");
    let perf_icon = Image::from_icon_name("power-profile-performance-symbolic");
    perf_row.add_prefix(&perf_icon);
    let radio_perf = CheckButton::new();
    radio_perf.set_valign(Align::Center);
    perf_row.add_prefix(&radio_perf);
    perf_row.set_activatable_widget(Some(&radio_perf));

    let bal_row = ActionRow::new();
    bal_row.set_title("_Equilibrado");
    bal_row.set_use_underline(true);
    bal_row.set_subtitle("Desempenho, acústica e consumo de energia padrão do sistema");
    let bal_icon = Image::from_icon_name("power-profile-balanced-symbolic");
    bal_row.add_prefix(&bal_icon);
    let radio_bal = CheckButton::new();
    radio_bal.set_group(Some(&radio_perf));
    radio_bal.set_valign(Align::Center);
    bal_row.add_prefix(&radio_bal);
    bal_row.set_activatable_widget(Some(&radio_bal));

    let saver_row = ActionRow::new();
    saver_row.set_title("E_conomia de Energia");
    saver_row.set_use_underline(true);
    saver_row.set_subtitle("Reduz o consumo energético e a emissão térmica");
    let saver_icon = Image::from_icon_name("power-profile-power-saver-symbolic");
    saver_row.add_prefix(&saver_icon);
    let radio_saver = CheckButton::new();
    radio_saver.set_group(Some(&radio_perf));
    radio_saver.set_valign(Align::Center);
    saver_row.add_prefix(&radio_saver);
    saver_row.set_activatable_widget(Some(&radio_saver));

    match current.as_str() {
        "performance" => radio_perf.set_active(true),
        "power-saver" => radio_saver.set_active(true),
        _ => radio_bal.set_active(true),
    }

    let toast_p1 = toast_overlay.clone();
    radio_perf.connect_toggled(move |btn| {
        if btn.is_active() {
            let _ = Optimizer::set_power_profile("performance");
            toast_p1.add_toast(Toast::new("Modo de energia alterado para Alto Desempenho"));
        }
    });

    let toast_p2 = toast_overlay.clone();
    radio_bal.connect_toggled(move |btn| {
        if btn.is_active() {
            let _ = Optimizer::set_power_profile("balanced");
            toast_p2.add_toast(Toast::new("Modo de energia alterado para Equilibrado"));
        }
    });

    let toast_p3 = toast_overlay.clone();
    radio_saver.connect_toggled(move |btn| {
        if btn.is_active() {
            let _ = Optimizer::set_power_profile("power-saver");
            toast_p3.add_toast(Toast::new(
                "Modo de energia alterado para Economia de Energia",
            ));
        }
    });

    power_group.add(&perf_row);
    power_group.add(&bal_row);
    power_group.add(&saver_row);

    // Perfil Térmico ACPI e Controle de Intel Turbo Boost
    let hybrid_thermal = Optimizer::read_hybrid_gpu_thermal();
    let acpi_row = ActionRow::new();
    acpi_row.add_css_class("property");
    acpi_row.set_title("Perfil Térmico de Plataforma ACPI e Estado dGPU");
    acpi_row.set_subtitle(&format!(
        "Perfil Ativo: {}  •  Modos Suportados: {}  •  NVIDIA dGPU: {}",
        hybrid_thermal.acpi_platform_profile,
        hybrid_thermal.acpi_choices,
        hybrid_thermal.dgpu_runtime_status
    ));
    let acpi_icon = Image::from_icon_name("analisepc-temp-symbolic");
    acpi_row.add_prefix(&acpi_icon);
    power_group.add(&acpi_row);

    if hybrid_thermal.turbo_control_available {
        let turbo_row = ActionRow::new();
        turbo_row.set_title("Intel Turbo Boost (Frequência de Pico vs Operação Fria)");
        turbo_row.set_subtitle(if hybrid_thermal.cpu_turbo_enabled {
            "Ativado — Permite clock máximo de Boost (desempenho total em jogos e compilação)"
        } else {
            "Desativado — Trava no Clock Base para reduzir a temperatura em até 15–20 °C na bateria"
        });
        let turbo_icon = Image::from_icon_name("analisepc-cpu-symbolic");
        turbo_row.add_prefix(&turbo_icon);

        let turbo_badge = Label::new(Some(if hybrid_thermal.cpu_turbo_enabled {
            "Turbo Ativo"
        } else {
            "Clock Base"
        }));
        turbo_badge.add_css_class("badge-pill");
        turbo_badge.add_css_class(if hybrid_thermal.cpu_turbo_enabled {
            "badge-green"
        } else {
            "badge-blue"
        });
        turbo_badge.set_valign(Align::Center);
        turbo_row.add_suffix(&turbo_badge);

        let turbo_btn = Button::with_label(if hybrid_thermal.cpu_turbo_enabled {
            "Desativar Turbo"
        } else {
            "Ativar Turbo"
        });
        turbo_btn.set_valign(Align::Center);
        turbo_btn.add_css_class("row-action-button");

        let is_turbo_rc = Rc::new(RefCell::new(hybrid_thermal.cpu_turbo_enabled));
        let t_row_c = turbo_row.clone();
        let t_badge_c = turbo_badge.clone();
        let t_btn_c = turbo_btn.clone();
        let toast_tb = toast_overlay.clone();

        turbo_btn.connect_clicked(move |_| {
            let next_state = !*is_turbo_rc.borrow();
            t_btn_c.set_sensitive(false);

            let (sender, receiver) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let res = Optimizer::set_intel_turbo_enabled(next_state);
                let _ = sender.send_blocking((next_state, res));
            });

            let state_rc = is_turbo_rc.clone();
            let row_d = t_row_c.clone();
            let badge_d = t_badge_c.clone();
            let btn_d = t_btn_c.clone();
            let t_ov = toast_tb.clone();

            glib::spawn_future_local(async move {
                if let Ok((applied_state, res)) = receiver.recv().await {
                    btn_d.set_sensitive(true);
                    match res {
                        Ok(msg) => {
                            *state_rc.borrow_mut() = applied_state;
                            row_d.set_subtitle(&msg);
                            if applied_state {
                                badge_d.set_text("Turbo Ativo");
                                badge_d.remove_css_class("badge-blue");
                                badge_d.add_css_class("badge-green");
                                btn_d.set_label("Desativar Turbo");
                            } else {
                                badge_d.set_text("Clock Base");
                                badge_d.remove_css_class("badge-green");
                                badge_d.add_css_class("badge-blue");
                                btn_d.set_label("Ativar Turbo");
                            }
                            t_ov.add_toast(Toast::new(&msg));
                        }
                        Err(e) => {
                            row_d.set_subtitle(&e);
                            t_ov.add_toast(Toast::new(&e));
                        }
                    }
                }
            });
        });

        turbo_row.add_suffix(&turbo_btn);
        power_group.add(&turbo_row);
    }

    page.add(&power_group);

    // 2. Manutenção e Armazenamento (Botões consistentes sem múltiplos .suggested-action)
    let maint_group = PreferencesGroup::new();
    maint_group.set_title("Manutenção do Sistema");

    let ram_clean_row = ActionRow::new();
    ram_clean_row.set_title("Buffers e Cache de Memória");
    ram_clean_row
        .set_subtitle("Sincroniza escritas pendentes no disco e libera páginas de cache inativas");
    let ram_icon = Image::from_icon_name("analisepc-ram-symbolic");
    ram_clean_row.add_prefix(&ram_icon);

    let ram_clean_btn = Button::with_mnemonic("_Limpar Cache");
    ram_clean_btn.set_valign(Align::Center);
    ram_clean_btn.add_css_class("row-action-button");

    let ram_row_clone = ram_clean_row.clone();
    let ram_btn_clone = ram_clean_btn.clone();
    let toast_ram = toast_overlay.clone();
    ram_clean_btn.connect_clicked(move |_| {
        ram_btn_clone.set_sensitive(false);
        ram_row_clone.set_subtitle("Sincronizando sistemas de arquivos e limpando cache...");

        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = Optimizer::clean_memory_cache();
            let _ = sender.send_blocking(res);
        });

        let row = ram_row_clone.clone();
        let btn = ram_btn_clone.clone();
        let t_ov = toast_ram.clone();
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                btn.set_sensitive(true);
                match res {
                    Ok(msg) => {
                        row.set_subtitle(&msg);
                        t_ov.add_toast(Toast::new(&msg));
                    }
                    Err(err) => {
                        row.set_subtitle(&err);
                        t_ov.add_toast(Toast::new(&err));
                    }
                }
            }
        });
    });
    ram_clean_row.add_suffix(&ram_clean_btn);
    maint_group.add(&ram_clean_row);

    // TRIM do SSD
    let trim_row = ActionRow::new();
    trim_row.set_title("Descarte de Blocos SSD (TRIM)");
    trim_row.set_subtitle(&format!("Status: {}", Optimizer::get_fstrim_info()));
    let ssd_icon = Image::from_icon_name("drive-harddisk-solidstate-symbolic");
    trim_row.add_prefix(&ssd_icon);

    let trim_btn = Button::with_mnemonic("_Executar TRIM");
    trim_btn.set_valign(Align::Center);
    trim_btn.add_css_class("row-action-button");

    let trim_row_clone = trim_row.clone();
    let trim_btn_clone = trim_btn.clone();
    let toast_trim = toast_overlay.clone();
    trim_btn.connect_clicked(move |_| {
        trim_btn_clone.set_sensitive(false);
        trim_row_clone.set_subtitle("Executando descarte de blocos livres (fstrim)...");

        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = Optimizer::run_fstrim_now();
            let _ = sender.send_blocking(res);
        });

        let row = trim_row_clone.clone();
        let btn = trim_btn_clone.clone();
        let t_ov = toast_trim.clone();
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                btn.set_sensitive(true);
                match res {
                    Ok(msg) => {
                        row.set_subtitle(&msg);
                        t_ov.add_toast(Toast::new(&msg));
                    }
                    Err(err) => {
                        row.set_subtitle(&err);
                        t_ov.add_toast(Toast::new(&err));
                    }
                }
            }
        });
    });
    trim_row.add_suffix(&trim_btn);
    maint_group.add(&trim_row);

    page.add(&maint_group);

    // 3. Limpeza Inteligente de Espaço em Disco (Ubuntu / Linux)
    let cleanup_info = Optimizer::scan_disk_cleanup();
    let cleanup_group = PreferencesGroup::new();
    cleanup_group.set_title("Limpeza de Espaço em Disco e Caches");
    cleanup_group.set_description(Some(
        "Libere armazenamento removendo logs antigos do systemd, pacotes .deb em cache, miniaturas e shaders compilados",
    ));

    // 3.1 Logs do Systemd (journalctl)
    let journal_row = ActionRow::new();
    journal_row.set_title("Logs do Sistema (Systemd Journal)");
    journal_row.set_subtitle(&format!(
        "Ocupação atual em /var/log/journal: {} • Remove registros com mais de 7 dias",
        cleanup_info.journal_usage
    ));
    let journal_icon = Image::from_icon_name("emblem-system-symbolic");
    journal_row.add_prefix(&journal_icon);

    let journal_badge = Label::new(Some(&cleanup_info.journal_usage));
    journal_badge.add_css_class("badge-pill");
    journal_badge.add_css_class("badge-blue");
    journal_badge.add_css_class("numeric");
    journal_badge.set_valign(Align::Center);
    journal_row.add_suffix(&journal_badge);

    let journal_btn = Button::with_mnemonic("Limpar _Logs");
    journal_btn.set_valign(Align::Center);
    journal_btn.add_css_class("row-action-button");

    let j_row_c = journal_row.clone();
    let j_badge_c = journal_badge.clone();
    let j_btn_c = journal_btn.clone();
    let toast_j = toast_overlay.clone();
    journal_btn.connect_clicked(move |_| {
        j_btn_c.set_sensitive(false);
        j_row_c.set_subtitle("Executando journalctl --vacuum-time=7d...");
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = Optimizer::clean_journal_logs();
            let _ = sender.send_blocking(res);
        });
        let row_d = j_row_c.clone();
        let badge_d = j_badge_c.clone();
        let btn_d = j_btn_c.clone();
        let t_ov = toast_j.clone();
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                btn_d.set_sensitive(true);
                match res {
                    Ok(msg) => {
                        let updated = Optimizer::scan_disk_cleanup();
                        badge_d.set_text(&updated.journal_usage);
                        row_d.set_subtitle(&msg);
                        t_ov.add_toast(Toast::new(&msg));
                    }
                    Err(e) => {
                        row_d.set_subtitle(&e);
                        t_ov.add_toast(Toast::new(&e));
                    }
                }
            }
        });
    });
    journal_row.add_suffix(&journal_btn);
    cleanup_group.add(&journal_row);

    // 3.2 Cache de Pacotes APT (/var/cache/apt/archives)
    let apt_row = ActionRow::new();
    apt_row.set_title("Cache de Instaladores APT (.deb)");
    apt_row.set_subtitle(&format!(
        "Arquivos retidos em /var/cache/apt/archives: {}",
        cleanup_info.apt_cache_size
    ));
    let apt_icon = Image::from_icon_name("drive-harddisk-symbolic");
    apt_row.add_prefix(&apt_icon);

    let apt_btn = Button::with_mnemonic("Limpar _APT");
    apt_btn.set_valign(Align::Center);
    apt_btn.add_css_class("row-action-button");

    let a_row_c = apt_row.clone();
    let a_btn_c = apt_btn.clone();
    let toast_a = toast_overlay.clone();
    apt_btn.connect_clicked(move |_| {
        a_btn_c.set_sensitive(false);
        a_row_c.set_subtitle("Limpando pacotes .deb em cache (apt-get clean)...");
        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = Optimizer::clean_apt_cache();
            let _ = sender.send_blocking(res);
        });
        let row_d = a_row_c.clone();
        let btn_d = a_btn_c.clone();
        let t_ov = toast_a.clone();
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                btn_d.set_sensitive(true);
                match res {
                    Ok(msg) => {
                        row_d.set_subtitle(&msg);
                        t_ov.add_toast(Toast::new(&msg));
                    }
                    Err(e) => {
                        row_d.set_subtitle(&e);
                        t_ov.add_toast(Toast::new(&e));
                    }
                }
            }
        });
    });
    apt_row.add_suffix(&apt_btn);
    cleanup_group.add(&apt_row);

    // 3.3 Cache de Shaders (NVIDIA GLCache / Optix / Mesa / Vulkan)
    let shader_row = ActionRow::new();
    shader_row.set_title("Cache de Shaders Gráficos (NVIDIA / Mesa / Vulkan)");
    shader_row.set_subtitle(&format!(
        "Ocupação atual em ~/.cache: {} • Útil para corrigir micro-travamentos após atualizar driver de vídeo",
        cleanup_info.shader_cache_size
    ));
    let shader_icon = Image::from_icon_name("video-display-symbolic");
    shader_row.add_prefix(&shader_icon);

    let shader_badge = Label::new(Some(&cleanup_info.shader_cache_size));
    shader_badge.add_css_class("badge-pill");
    shader_badge.add_css_class("badge-subtle");
    shader_badge.add_css_class("numeric");
    shader_badge.set_valign(Align::Center);
    shader_row.add_suffix(&shader_badge);

    let shader_btn = Button::with_mnemonic("Limpar _Shaders");
    shader_btn.set_valign(Align::Center);
    shader_btn.add_css_class("row-action-button");

    let sh_row_c = shader_row.clone();
    let sh_badge_c = shader_badge.clone();
    let sh_btn_c = shader_btn.clone();
    let toast_sh = toast_overlay.clone();
    shader_btn.connect_clicked(move |_| {
        sh_btn_c.set_sensitive(false);
        match Optimizer::clean_shader_cache() {
            Ok(msg) => {
                sh_badge_c.set_text("0 MiB (Limpo)");
                sh_row_c.set_subtitle(&msg);
                toast_sh.add_toast(Toast::new(&msg));
            }
            Err(e) => {
                sh_row_c.set_subtitle(&e);
                toast_sh.add_toast(Toast::new(&e));
            }
        }
        sh_btn_c.set_sensitive(true);
    });
    shader_row.add_suffix(&shader_btn);
    cleanup_group.add(&shader_row);

    // 3.4 Cache de Miniaturas do GNOME (~/.cache/thumbnails)
    let thumb_row = ActionRow::new();
    thumb_row.set_title("Cache de Miniaturas de Arquivos (~/.cache/thumbnails)");
    thumb_row.set_subtitle(&format!(
        "Ocupação atual: {} • Dispensa senha de administrador",
        cleanup_info.thumbnails_size
    ));
    let thumb_icon = Image::from_icon_name("preferences-desktop-display-symbolic");
    thumb_row.add_prefix(&thumb_icon);

    let thumb_badge = Label::new(Some(&cleanup_info.thumbnails_size));
    thumb_badge.add_css_class("badge-pill");
    thumb_badge.add_css_class("badge-subtle");
    thumb_badge.add_css_class("numeric");
    thumb_badge.set_valign(Align::Center);
    thumb_row.add_suffix(&thumb_badge);

    let thumb_btn = Button::with_mnemonic("Limpar _Miniaturas");
    thumb_btn.set_valign(Align::Center);
    thumb_btn.add_css_class("row-action-button");

    let th_row_c = thumb_row.clone();
    let th_badge_c = thumb_badge.clone();
    let th_btn_c = thumb_btn.clone();
    let toast_th = toast_overlay.clone();
    thumb_btn.connect_clicked(move |_| {
        th_btn_c.set_sensitive(false);
        match Optimizer::clean_thumbnails_cache() {
            Ok(msg) => {
                th_badge_c.set_text("0 MiB (Limpo)");
                th_row_c.set_subtitle(&msg);
                toast_th.add_toast(Toast::new(&msg));
            }
            Err(e) => {
                th_row_c.set_subtitle(&e);
                toast_th.add_toast(Toast::new(&e));
            }
        }
        th_btn_c.set_sensitive(true);
    });
    thumb_row.add_suffix(&thumb_btn);
    cleanup_group.add(&thumb_row);

    // 3.5 Lixeira do Usuário (~/.local/share/Trash)
    let trash_row = ActionRow::new();
    trash_row.set_title("Lixeira da Sessão do Usuário");
    trash_row.set_subtitle(&format!(
        "Ocupação atual em ~/.local/share/Trash: {}",
        cleanup_info.trash_size
    ));
    let trash_icon = Image::from_icon_name("user-trash-symbolic");
    trash_row.add_prefix(&trash_icon);

    let trash_badge = Label::new(Some(&cleanup_info.trash_size));
    trash_badge.add_css_class("badge-pill");
    trash_badge.add_css_class("badge-subtle");
    trash_badge.add_css_class("numeric");
    trash_badge.set_valign(Align::Center);
    trash_row.add_suffix(&trash_badge);

    let trash_btn = Button::with_mnemonic("Esvaziar _Lixeira");
    trash_btn.set_valign(Align::Center);
    trash_btn.add_css_class("row-action-button");

    let tr_row_c = trash_row.clone();
    let tr_badge_c = trash_badge.clone();
    let tr_btn_c = trash_btn.clone();
    let toast_tr = toast_overlay.clone();
    trash_btn.connect_clicked(move |_| {
        tr_btn_c.set_sensitive(false);
        match Optimizer::empty_user_trash() {
            Ok(msg) => {
                tr_badge_c.set_text("0 MiB (Limpo)");
                tr_row_c.set_subtitle(&msg);
                toast_tr.add_toast(Toast::new(&msg));
            }
            Err(e) => {
                tr_row_c.set_subtitle(&e);
                toast_tr.add_toast(Toast::new(&e));
            }
        }
        tr_btn_c.set_sensitive(true);
    });
    trash_row.add_suffix(&trash_btn);
    cleanup_group.add(&trash_row);

    page.add(&cleanup_group);

    // 4. Detector de Vilões de Recursos (Top Processos de CPU, RAM e VRAM da GPU NVIDIA)
    let hogs = Optimizer::scan_resource_hogs();
    let hogs_group = PreferencesGroup::new();
    hogs_group.set_title("Detector de Vilões de Recursos (CPU, RAM e VRAM)");
    hogs_group.set_description(Some(
        "Identifica os processos que mais consomem processador, memória física e mantêm a GPU dedicada NVIDIA acordada",
    ));

    // 4.1 Top 5 Processos por Uso de CPU (%)
    let cpu_hogs_exp = ExpanderRow::new();
    cpu_hogs_exp.set_title("Maiores Consumidores de Processador (Top 5 CPU)");
    cpu_hogs_exp.set_subtitle("Processos ordenados pelo percentual instantâneo de uso de núcleos");
    cpu_hogs_exp.set_expanded(true);
    let cpu_hog_icon = Image::from_icon_name("analisepc-cpu-symbolic");
    cpu_hogs_exp.add_prefix(&cpu_hog_icon);

    for proc_info in hogs.top_cpu {
        let row = ActionRow::new();
        let safe_name = proc_info.name.replace('&', "e");
        row.set_title(&format!("{} (PID {})", safe_name, proc_info.pid));
        row.set_subtitle(&format!(
            "Uso de CPU: {:.1}%  •  Memória Residente (RSS): {:.0} MiB",
            proc_info.cpu_pct, proc_info.ram_mib
        ));

        let badge = Label::new(Some(&format!("{:.1}% CPU", proc_info.cpu_pct)));
        badge.add_css_class("badge-pill");
        badge.add_css_class(if proc_info.cpu_pct >= 25.0 {
            "badge-amber"
        } else {
            "badge-blue"
        });
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        if proc_info.is_system_critical {
            let sys_badge = Label::new(Some("Sistema"));
            sys_badge.add_css_class("badge-pill");
            sys_badge.add_css_class("badge-subtle");
            sys_badge.set_valign(Align::Center);
            row.add_suffix(&sys_badge);
        } else {
            let kill_btn = Button::with_label("Encerrar");
            kill_btn.set_valign(Align::Center);
            kill_btn.add_css_class("row-action-button");
            let pid = proc_info.pid;
            let pname = safe_name.clone();
            let row_c = row.clone();
            let btn_c = kill_btn.clone();
            let toast_k = toast_overlay.clone();
            kill_btn.connect_clicked(move |_| {
                btn_c.set_sensitive(false);
                match Optimizer::terminate_process(pid, &pname) {
                    Ok(msg) => {
                        row_c.set_subtitle(&msg);
                        toast_k.add_toast(Toast::new(&msg));
                    }
                    Err(e) => {
                        btn_c.set_sensitive(true);
                        toast_k.add_toast(Toast::new(&e));
                    }
                }
            });
            row.add_suffix(&kill_btn);
        }

        cpu_hogs_exp.add_row(&row);
    }
    hogs_group.add(&cpu_hogs_exp);

    // 4.2 Top 5 Processos por Uso de Memória RAM (MiB)
    let ram_hogs_exp = ExpanderRow::new();
    ram_hogs_exp.set_title("Maiores Consumidores de Memória RAM (Top 5 RSS)");
    ram_hogs_exp.set_subtitle("Processos ordenados pelo volume de memória física alocada");
    let ram_hog_icon = Image::from_icon_name("analisepc-ram-symbolic");
    ram_hogs_exp.add_prefix(&ram_hog_icon);

    for proc_info in hogs.top_ram {
        let row = ActionRow::new();
        let safe_name = proc_info.name.replace('&', "e");
        row.set_title(&format!("{} (PID {})", safe_name, proc_info.pid));
        row.set_subtitle(&format!(
            "Memória Física (RSS): {:.0} MiB ({:.2} GiB)  •  CPU: {:.1}%",
            proc_info.ram_mib,
            proc_info.ram_mib / 1024.0,
            proc_info.cpu_pct
        ));

        let badge = Label::new(Some(&format!("{:.0} MiB", proc_info.ram_mib)));
        badge.add_css_class("badge-pill");
        badge.add_css_class(if proc_info.ram_mib >= 1536.0 {
            "badge-amber"
        } else {
            "badge-green"
        });
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        if proc_info.is_system_critical {
            let sys_badge = Label::new(Some("Sistema"));
            sys_badge.add_css_class("badge-pill");
            sys_badge.add_css_class("badge-subtle");
            sys_badge.set_valign(Align::Center);
            row.add_suffix(&sys_badge);
        } else {
            let kill_btn = Button::with_label("Encerrar");
            kill_btn.set_valign(Align::Center);
            kill_btn.add_css_class("row-action-button");
            let pid = proc_info.pid;
            let pname = safe_name.clone();
            let row_c = row.clone();
            let btn_c = kill_btn.clone();
            let toast_k = toast_overlay.clone();
            kill_btn.connect_clicked(move |_| {
                btn_c.set_sensitive(false);
                match Optimizer::terminate_process(pid, &pname) {
                    Ok(msg) => {
                        row_c.set_subtitle(&msg);
                        toast_k.add_toast(Toast::new(&msg));
                    }
                    Err(e) => {
                        btn_c.set_sensitive(true);
                        toast_k.add_toast(Toast::new(&e));
                    }
                }
            });
            row.add_suffix(&kill_btn);
        }

        ram_hogs_exp.add_row(&row);
    }
    hogs_group.add(&ram_hogs_exp);

    // 4.3 Processos Ativos na GPU Dedicada NVIDIA (VRAM e Consumo dGPU)
    let gpu_hogs_exp = ExpanderRow::new();
    gpu_hogs_exp.set_title(&format!(
        "Processos na GPU Dedicada NVIDIA ({} ativos)",
        hogs.gpu_processes.len()
    ));
    gpu_hogs_exp.set_subtitle(
        "Aplicativos que estão alocando VRAM e mantendo a placa dedicada acordada (impacta autonomia na bateria)",
    );
    let gpu_hog_icon = Image::from_icon_name("video-display-symbolic");
    gpu_hogs_exp.add_prefix(&gpu_hog_icon);

    if hogs.gpu_processes.is_empty() {
        let empty_row = ActionRow::new();
        empty_row.set_title("Nenhum processo utilizando a GPU dedicada");
        empty_row.set_subtitle("A placa dedicada está livre para entrar em suspensão profunda (D3cold)");
        gpu_hogs_exp.add_row(&empty_row);
    } else {
        for gp in hogs.gpu_processes {
            let row = ActionRow::new();
            let safe_name = gp.name.replace('&', "e");
            row.set_title(&format!("{} (PID {})", safe_name, gp.pid));
            row.set_subtitle(&format!(
                "Contexto: {}  •  VRAM Dedicada Alocada: {}",
                gp.engine_type, gp.vram_usage
            ));

            let badge = Label::new(Some(&gp.vram_usage));
            badge.add_css_class("badge-pill");
            badge.add_css_class("badge-purple");
            badge.add_css_class("numeric");
            badge.set_valign(Align::Center);
            row.add_suffix(&badge);

            if gp.is_system_critical {
                let sys_badge = Label::new(Some("Sessão Gráfica"));
                sys_badge.add_css_class("badge-pill");
                sys_badge.add_css_class("badge-subtle");
                sys_badge.set_valign(Align::Center);
                row.add_suffix(&sys_badge);
            } else {
                let kill_btn = Button::with_label("Encerrar");
                kill_btn.set_valign(Align::Center);
                kill_btn.add_css_class("row-action-button");
                let pid = gp.pid;
                let pname = safe_name.clone();
                let row_c = row.clone();
                let btn_c = kill_btn.clone();
                let toast_k = toast_overlay.clone();
                kill_btn.connect_clicked(move |_| {
                    btn_c.set_sensitive(false);
                    match Optimizer::terminate_process(pid, &pname) {
                        Ok(msg) => {
                            row_c.set_subtitle(&msg);
                            toast_k.add_toast(Toast::new(&msg));
                        }
                        Err(e) => {
                            btn_c.set_sensitive(true);
                            toast_k.add_toast(Toast::new(&e));
                        }
                    }
                });
                row.add_suffix(&kill_btn);
            }

            gpu_hogs_exp.add_row(&row);
        }
    }
    hogs_group.add(&gpu_hogs_exp);
    page.add(&hogs_group);

    // 5. Inicialização com o Sistema (Switches com rótulos de substantivos — HIG Switches Pattern)
    let auto_group = PreferencesGroup::new();
    auto_group.set_title("Aplicativos na Inicialização");
    auto_group.set_description(Some(
        "Controle quais aplicativos iniciam automaticamente junto com a sessão gráfica",
    ));

    let apps = Optimizer::list_autostart_apps();
    if apps.is_empty() {
        let empty_row = ActionRow::new();
        empty_row.set_title("Nenhum aplicativo configurado");
        empty_row.set_subtitle("Não foram encontrados arquivos .desktop em ~/.config/autostart");
        auto_group.add(&empty_row);
    } else {
        for app in apps {
            let switch_row = SwitchRow::new();
            switch_row.set_title(&app.name);
            switch_row.set_subtitle(&app.exec);
            switch_row.set_active(app.is_enabled);

            let path_clone = app.file_path.clone();
            switch_row.connect_active_notify(move |sw| {
                let is_active = sw.is_active();
                let _ = Optimizer::set_autostart_enabled(&path_clone, is_active);
            });

            auto_group.add(&switch_row);
        }
    }

    page.add(&auto_group);
    page
}

struct NetRowItem {
    name: String,
    ip_row: ActionRow,
    dev_row: ActionRow,
    rate_row: ActionRow,
    state_badge: Label,
}

struct NetworkWidgets {
    widget: PreferencesPage,
    rx_chart: RealtimeChart,
    tx_chart: RealtimeChart,
    net_rows: Vec<NetRowItem>,
}

impl NetworkWidgets {
    fn update(&self, ifaces: &[crate::core::peripherals::NetworkInterfaceInfo]) {
        let mut total_rx_kbps = 0.0;
        let mut total_tx_kbps = 0.0;

        for iface in ifaces {
            total_rx_kbps += iface.rx_speed_kbps;
            total_tx_kbps += iface.tx_speed_kbps;

            if let Some(item) = self.net_rows.iter().find(|i| i.name == iface.name) {
                let rx_gib = iface.rx_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
                let tx_gib = iface.tx_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;

                item.ip_row.set_subtitle(&iface.ipv4);
                item.dev_row.set_subtitle(&format!(
                    "{} • Link: {} (Driver: {})",
                    iface.name, iface.link_speed, iface.driver
                ));
                item.rate_row.set_subtitle(&format!(
                    "↓ {:.1} KB/s • ↑ {:.1} KB/s (Acumulado: {:.2} GiB ↓ / {:.2} GiB ↑)",
                    iface.rx_speed_kbps, iface.tx_speed_kbps, rx_gib, tx_gib
                ));
                item.state_badge.set_text(&iface.state);
                item.state_badge.remove_css_class("badge-green");
                item.state_badge.remove_css_class("badge-amber");
                if iface.state.contains("Conectada") {
                    item.state_badge.add_css_class("badge-green");
                } else {
                    item.state_badge.add_css_class("badge-amber");
                }
            }
        }

        self.rx_chart.push_value(total_rx_kbps);
        self.tx_chart.push_value(total_tx_kbps);
    }
}

fn build_network_page(
    net_collector: &mut NetworkCollector,
    toast_overlay: &ToastOverlay,
) -> NetworkWidgets {
    let page = PreferencesPage::new();

    // 1. Gráficos de Rede em Tempo Real (60s)
    let chart_group = PreferencesGroup::new();
    chart_group.set_title("Tráfego em Tempo Real (60s)");

    let rx_chart = RealtimeChart::new("Download Global (RX)", " KB/s", 128.0, GNOME_GREEN_4);
    let rx_row = ListBoxRow::new();
    rx_row.set_activatable(false);
    rx_row.set_selectable(false);
    rx_row.set_child(Some(rx_chart.widget()));
    chart_group.add(&rx_row);

    let tx_chart = RealtimeChart::new("Upload Global (TX)", " KB/s", 64.0, GNOME_BLUE_3);
    let tx_row = ListBoxRow::new();
    tx_row.set_activatable(false);
    tx_row.set_selectable(false);
    tx_row.set_child(Some(tx_chart.widget()));
    chart_group.add(&tx_row);

    page.add(&chart_group);

    // 2. Interfaces de Rede separadas por tipo (Cabeada vs Wi-Fi)
    let initial_ifaces = net_collector.refresh();
    let wired_count = initial_ifaces
        .iter()
        .filter(|i| !i.kind.contains("Wi-Fi"))
        .count();
    let wifi_count = initial_ifaces
        .iter()
        .filter(|i| i.kind.contains("Wi-Fi"))
        .count();

    let mut net_rows = Vec::new();

    for iface in initial_ifaces {
        let is_wifi = iface.kind.contains("Wi-Fi");
        let group = PreferencesGroup::new();

        let group_title = if is_wifi {
            if wifi_count > 1 {
                format!("Wi-Fi ({})", iface.name)
            } else {
                "Wi-Fi".to_string()
            }
        } else if wired_count > 1 {
            format!("Cabeada ({})", iface.name)
        } else {
            "Cabeada".to_string()
        };
        group.set_title(&group_title);

        let icon_name = if is_wifi {
            "network-wireless-symbolic"
        } else {
            "network-wired-symbolic"
        };

        // 1. IP (+ badge de estado da conexão)
        let ip_row = ActionRow::new();
        ip_row.add_css_class("property");
        ip_row.set_title("IP");
        ip_row.set_subtitle(&iface.ipv4);
        let icon = Image::from_icon_name(icon_name);
        ip_row.add_prefix(&icon);

        let state_badge = Label::new(Some(&iface.state));
        state_badge.add_css_class("badge-pill");
        if iface.state.contains("Conectada") {
            state_badge.add_css_class("badge-green");
        } else {
            state_badge.add_css_class("badge-amber");
        }
        state_badge.set_valign(Align::Center);
        ip_row.add_suffix(&state_badge);
        group.add(&ip_row);

        // 2. MAC
        let mac_row = ActionRow::new();
        mac_row.add_css_class("property");
        mac_row.set_title("MAC");
        mac_row.set_subtitle(&iface.mac_address);
        group.add(&mac_row);

        // 3. Dispositivo
        let dev_row = ActionRow::new();
        dev_row.add_css_class("property");
        dev_row.set_title("Dispositivo");
        dev_row.set_subtitle(&format!(
            "{} • Link: {} (Driver: {})",
            iface.name, iface.link_speed, iface.driver
        ));
        group.add(&dev_row);

        // 4. Fabricante
        let vendor_row = ActionRow::new();
        vendor_row.add_css_class("property");
        vendor_row.set_title("Fabricante");
        vendor_row.set_subtitle(&iface.vendor.replace('&', "e"));
        group.add(&vendor_row);

        // 5. Modelo
        let model_row = ActionRow::new();
        model_row.add_css_class("property");
        model_row.set_title("Modelo");
        model_row.set_subtitle(&iface.model.replace('&', "e"));
        group.add(&model_row);

        // 6. Taxa de Transferência
        let rx_gib = iface.rx_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let tx_gib = iface.tx_total_bytes as f64 / 1024.0 / 1024.0 / 1024.0;
        let rate_row = ActionRow::new();
        rate_row.add_css_class("property");
        rate_row.set_title("Taxa de Transferência");
        rate_row.set_subtitle(&format!(
            "↓ {:.1} KB/s • ↑ {:.1} KB/s (Acumulado: {:.2} GiB ↓ / {:.2} GiB ↑)",
            iface.rx_speed_kbps, iface.tx_speed_kbps, rx_gib, tx_gib
        ));
        group.add(&rate_row);

        page.add(&group);
        net_rows.push(NetRowItem {
            name: iface.name,
            ip_row,
            dev_row,
            rate_row,
            state_badge,
        });
    }

    // 3. Teste de Estabilidade e Latência com AdwEntryRow (HIG Text Fields Pattern)
    let net_test_group = PreferencesGroup::new();
    net_test_group.set_title("Diagnóstico de Latência ICMP e Jitter");
    net_test_group.set_description(Some(
        "Digite um endereço IPv4 ou domínio e pressione Enter ou clique em Testar",
    ));

    let host_entry = EntryRow::new();
    host_entry.set_title("_Servidor Alvo (Host ou IPv4)");
    host_entry.set_use_underline(true);
    host_entry.set_text("1.1.1.1");

    let ping_btn = Button::with_mnemonic("_Testar");
    ping_btn.set_valign(Align::Center);
    ping_btn.add_css_class("suggested-action");
    host_entry.add_suffix(&ping_btn);

    let ping_result_row = ActionRow::new();
    ping_result_row.add_css_class("property");
    ping_result_row.set_title("Resultado da Medição");
    ping_result_row.set_subtitle("Aguardando execução do teste ICMP...");
    let net_icon = Image::from_icon_name("network-transmit-receive-symbolic");
    ping_result_row.add_prefix(&net_icon);

    let trigger_ping = {
        let entry_c = host_entry.clone();
        let res_row_c = ping_result_row.clone();
        let btn_c = ping_btn.clone();
        let toast_c = toast_overlay.clone();

        Rc::new(move || {
            let target = entry_c.text().trim().to_string();
            let host = if target.is_empty() {
                "1.1.1.1".to_string()
            } else {
                target
            };

            btn_c.set_sensitive(false);
            entry_c.set_sensitive(false);
            res_row_c.set_subtitle(&format!(
                "Medindo Ping, Jitter e perda de pacotes para {}...",
                host
            ));

            let (sender, receiver) = async_channel::bounded(1);
            let host_thread = host.clone();
            std::thread::spawn(move || {
                let res = DiagnosticsEngine::run_network_stability_test(&host_thread);
                let _ = sender.send_blocking(res);
            });

            let row_dest = res_row_c.clone();
            let btn_dest = btn_c.clone();
            let entry_dest = entry_c.clone();
            let t_ov = toast_c.clone();
            glib::spawn_future_local(async move {
                if let Ok(res) = receiver.recv().await {
                    btn_dest.set_sensitive(true);
                    entry_dest.set_sensitive(true);
                    match res {
                        Ok(r) => {
                            row_dest.set_subtitle(&format!(
                                "Ping Médio: {:.1} ms (Mín: {:.1} / Máx: {:.1} ms) • Jitter: {:.2} ms • Perda: {:.0}% • {}",
                                r.ping_avg_ms, r.ping_min_ms, r.ping_max_ms, r.jitter_ms, r.packet_loss_pct, r.verdict
                            ));
                            t_ov.add_toast(Toast::new(&format!(
                                "Ping ({}): {:.1} ms • Jitter: {:.2} ms",
                                host, r.ping_avg_ms, r.jitter_ms
                            )));
                        }
                        Err(e) => {
                            row_dest.set_subtitle(&e);
                            t_ov.add_toast(Toast::new(&e));
                        }
                    }
                }
            });
        })
    };

    let trig_click = trigger_ping.clone();
    ping_btn.connect_clicked(move |_| trig_click());

    let trig_enter = trigger_ping.clone();
    host_entry.connect_entry_activated(move |_| trig_enter());

    net_test_group.add(&host_entry);
    net_test_group.add(&ping_result_row);
    page.add(&net_test_group);

    NetworkWidgets {
        widget: page,
        rx_chart,
        tx_chart,
        net_rows,
    }
}

fn build_peripherals_page() -> PreferencesPage {
    let page = PreferencesPage::new();

    // 1. Monitores e Telas Conectadas (DRM)
    let displays = PeripheralsCollector::read_displays();
    if !displays.is_empty() {
        let disp_group = PreferencesGroup::new();
        disp_group.set_title("Monitores e Saídas de Vídeo (DRM)");

        for d in displays {
            let row = ActionRow::new();
            row.set_title(&format!("{} — {}", d.connector, d.kind));
            row.set_subtitle(&format!("Controladora DRM: {}", d.gpu_card));

            let icon = Image::from_icon_name("video-display-symbolic");
            row.add_prefix(&icon);

            let badge = Label::new(Some(&d.resolution));
            badge.add_css_class("badge-pill");
            badge.add_css_class("badge-blue");
            badge.add_css_class("numeric");
            badge.set_valign(Align::Center);
            row.add_suffix(&badge);

            disp_group.add(&row);
        }

        page.add(&disp_group);
    }

    // 2. Áudio (PipeWire / WirePlumber / ALSA)
    let audio = PeripheralsCollector::read_audio();
    let audio_group = PreferencesGroup::new();
    let safe_server = audio.server_name.replace('&', "e");
    audio_group.set_title("Subsistema de Áudio");
    audio_group.set_description(Some(&format!("Servidor de som ativo: {}", safe_server)));

    for sink in audio.sinks {
        let row = ActionRow::new();
        let safe_name = sink.name.replace('&', "e");
        row.set_title(&format!("Saída: {}", safe_name));
        row.set_subtitle(if sink.is_default {
            "Dispositivo de saída padrão ativo"
        } else {
            "Dispositivo disponível"
        });

        let icon = Image::from_icon_name("audio-speakers-symbolic");
        row.add_prefix(&icon);

        let badge_txt = if sink.is_default {
            format!("Padrão • {}%", sink.volume_pct)
        } else {
            format!("{}%", sink.volume_pct)
        };
        let badge = Label::new(Some(&badge_txt));
        badge.add_css_class("badge-pill");
        badge.add_css_class(if sink.is_default {
            "badge-green"
        } else {
            "badge-blue"
        });
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        audio_group.add(&row);
    }

    for src in audio.sources {
        let row = ActionRow::new();
        let safe_name = src.name.replace('&', "e");
        row.set_title(&format!("Entrada: {}", safe_name));
        row.set_subtitle(if src.is_default {
            "Microfone padrão ativo"
        } else {
            "Entrada disponível"
        });

        let icon = Image::from_icon_name("audio-input-microphone-symbolic");
        row.add_prefix(&icon);

        let badge_txt = if src.is_default {
            format!("Padrão • {}%", src.volume_pct)
        } else {
            format!("{}%", src.volume_pct)
        };
        let badge = Label::new(Some(&badge_txt));
        badge.add_css_class("badge-pill");
        badge.add_css_class(if src.is_default {
            "badge-green"
        } else {
            "badge-purple"
        });
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        audio_group.add(&row);
    }

    page.add(&audio_group);

    // 3. Periféricos USB Conectados (com filtro de busca — HIG Controls / Text Fields)
    let usb_devices = PeripheralsCollector::read_usb_devices();
    let usb_group = PreferencesGroup::new();
    usb_group.set_title(&format!(
        "Dispositivos USB e Controladores ({})",
        usb_devices.len()
    ));

    let search_row = EntryRow::new();
    search_row.set_title("_Filtrar por nome, categoria ou ID USB...");
    search_row.set_use_underline(true);
    let search_icon = Image::from_icon_name("system-search-symbolic");
    search_row.add_prefix(&search_icon);
    usb_group.add(&search_row);

    let mut usb_rows: Vec<(ActionRow, String)> = Vec::new();

    for dev in usb_devices {
        let row = ActionRow::new();
        let safe_name = dev.name.replace('&', "e");
        row.set_title(&safe_name);
        row.set_subtitle(&format!(
            "ID: {} • Barramento: {}",
            dev.vendor_product_id, dev.bus_dev
        ));

        let icon_name = if dev.category.contains("Mouse") {
            "input-mouse-symbolic"
        } else if dev.category.contains("Teclado") {
            "input-keyboard-symbolic"
        } else if dev.category.contains("Áudio") {
            "audio-input-microphone-symbolic"
        } else if dev.category.contains("Webcam") {
            "camera-web-symbolic"
        } else if dev.category.contains("Bluetooth") {
            "bluetooth-active-symbolic"
        } else {
            "drive-removable-media-symbolic"
        };
        let icon = Image::from_icon_name(icon_name);
        row.add_prefix(&icon);

        let badge = Label::new(Some(&dev.category));
        badge.add_css_class("badge-pill");
        badge.add_css_class("badge-subtle");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        let searchable = format!(
            "{} {} {} {}",
            safe_name.to_lowercase(),
            dev.category.to_lowercase(),
            dev.vendor_product_id.to_lowercase(),
            dev.bus_dev.to_lowercase()
        );
        usb_group.add(&row);
        usb_rows.push((row, searchable));
    }

    search_row.connect_changed(move |entry| {
        let query = entry.text().trim().to_lowercase();
        for (row, text) in &usb_rows {
            row.set_visible(query.is_empty() || text.contains(&query));
        }
    });

    page.add(&usb_group);

    // 4. Portas Seriais e Microcontroladores (ttyUSB / ttyACM — Arduino, ESP32, CH340, CP210x)
    let serial_ports = PeripheralsCollector::read_serial_ports();
    let serial_group = PreferencesGroup::new();
    serial_group.set_title(&format!(
        "Portas Seriais e Microcontroladores ({})",
        serial_ports.len()
    ));
    serial_group.set_description(Some(
        "Interfaces UART USB-Serial (/dev/ttyUSB* e /dev/ttyACM*) para Arduino, ESP32 e telas auxiliares",
    ));

    if serial_ports.is_empty() {
        let empty_row = ActionRow::new();
        empty_row.set_title("Nenhuma porta serial USB ativa no momento");
        empty_row.set_subtitle(
            "Conecte uma placa Arduino, ESP32 ou conversor USB-Serial (CH340 / CP210x / FTDI) para inspecionar",
        );
        let icon = Image::from_icon_name("drive-removable-media-symbolic");
        empty_row.add_prefix(&icon);
        serial_group.add(&empty_row);
    } else {
        for sp in serial_ports {
            let row = ActionRow::new();
            row.set_title(&format!("{} — {}", sp.port_path, sp.description.replace('&', "e")));
            row.set_subtitle(&format!(
                "Driver do Kernel: {} • Parâmetro Padrão: {}",
                sp.driver, sp.baud_hint
            ));
            let icon = Image::from_icon_name("drive-removable-media-symbolic");
            row.add_prefix(&icon);

            let badge = Label::new(Some("UART Ativa"));
            badge.add_css_class("badge-pill");
            badge.add_css_class("badge-green");
            badge.set_valign(Align::Center);
            row.add_suffix(&badge);
            serial_group.add(&row);
        }
    }

    page.add(&serial_group);
    page
}

fn build_diagnostics_page(toast_overlay: &ToastOverlay) -> GtkBox {
    let outer_box = GtkBox::new(Orientation::Vertical, 0);
    let report = DiagnosticsEngine::run_full_diagnostics();

    // Banner superior caso haja alertas (GNOME HIG Banner Feedback Pattern)
    let warnings_count = report
        .checks
        .iter()
        .filter(|c| !matches!(c.severity, DiagSeverity::Healthy))
        .count();
    if warnings_count > 0 {
        let banner = Banner::new(&format!(
            "Foram encontrados {} pontos de atenção na verificação de integridade do hardware",
            warnings_count
        ));
        banner.set_revealed(true);
        outer_box.append(&banner);
    }

    let page = PreferencesPage::new();
    page.set_vexpand(true);

    // 1. Laudo Automático de Saúde do Hardware
    let health_group = PreferencesGroup::new();
    health_group.set_title("Laudo de Integridade do Hardware");
    health_group.set_description(Some(
        "Avaliação combinada de sensores térmicos, S.M.A.R.T., Dual Channel, bateria, limites de energia e eventos do kernel",
    ));

    let score_row = ActionRow::new();
    score_row.set_title(&format!(
        "Índice Global de Integridade: {} / 100",
        report.overall_score
    ));
    score_row.set_subtitle(&report.overall_verdict);

    let score_icon = Image::from_icon_name("emblem-ok-symbolic");
    score_row.add_prefix(&score_icon);

    let score_box = GtkBox::new(Orientation::Horizontal, 8);
    score_box.set_valign(Align::Center);

    let score_bar = ProgressBar::new();
    score_bar.add_css_class("ram-bar");
    score_bar.set_size_request(68, -1);
    score_bar.set_valign(Align::Center);
    score_bar.set_fraction(report.overall_score as f64 / 100.0);
    score_box.append(&score_bar);

    let score_badge = Label::new(Some(&format!("{}%", report.overall_score)));
    score_badge.add_css_class("badge-pill");
    score_badge.add_css_class("numeric");
    score_badge.add_css_class(if report.overall_score >= 85 {
        "badge-green"
    } else {
        "badge-amber"
    });
    score_box.append(&score_badge);

    score_row.add_suffix(&score_box);
    health_group.add(&score_row);

    let export_row = ActionRow::new();
    export_row.set_title("Exportar Laudo Técnico Completo");
    export_row.set_subtitle(
        "Salva um relatório detalhado da máquina na sua pasta pessoal (HTML estilizado ou JSON)",
    );
    let export_icon = Image::from_icon_name("document-save-symbolic");
    export_row.add_prefix(&export_icon);

    let export_btns = GtkBox::new(Orientation::Horizontal, 8);
    export_btns.set_valign(Align::Center);

    let btn_html = Button::with_mnemonic("Exportar _HTML");
    btn_html.set_action_name(Some("win.export-html"));
    btn_html.add_css_class("row-action-button");

    let btn_json = Button::with_mnemonic("Exportar _JSON");
    btn_json.set_action_name(Some("win.export-json"));
    btn_json.add_css_class("row-action-button");

    export_btns.append(&btn_html);
    export_btns.append(&btn_json);
    export_row.add_suffix(&export_btns);
    health_group.add(&export_row);

    for chk in &report.checks {
        let row = ActionRow::new();
        let safe_comp = chk.component.replace('&', "e");
        let safe_title = chk.title.replace('&', "e");
        let safe_details = chk.details.replace('&', "e");
        row.set_title(&format!("{} — {}", safe_comp, safe_title));
        row.set_subtitle(&safe_details);

        let badge = Label::new(Some(&chk.badge_text));
        badge.add_css_class("badge-pill");
        match chk.severity {
            DiagSeverity::Healthy => badge.add_css_class("badge-green"),
            DiagSeverity::Warning => badge.add_css_class("badge-amber"),
            DiagSeverity::Critical => badge.add_css_class("badge-red"),
        }
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        health_group.add(&row);
    }

    page.add(&health_group);

    // 2. Teste de Estabilidade de Rede
    let net_test_group = PreferencesGroup::new();
    net_test_group.set_title("Estabilidade de Rede");
    net_test_group.set_description(Some(
        "Mede a latência real e a variação (Jitter) contra 1.1.1.1 para diagnosticar oscilações em tempo real",
    ));

    let ping_row = ActionRow::new();
    ping_row.set_title("Teste de Latência ICMP e Jitter (Cloudflare 1.1.1.1)");
    ping_row.set_subtitle("Clique em Testar Rede para enviar 8 pacotes ICMP de diagnóstico");
    let net_icon = Image::from_icon_name("network-transmit-receive-symbolic");
    ping_row.add_prefix(&net_icon);

    let ping_btn = Button::with_mnemonic("_Testar Rede");
    ping_btn.set_valign(Align::Center);
    ping_btn.add_css_class("suggested-action");

    let ping_row_clone = ping_row.clone();
    let ping_btn_clone = ping_btn.clone();
    let toast_diag = toast_overlay.clone();
    ping_btn.connect_clicked(move |_| {
        ping_btn_clone.set_sensitive(false);
        ping_row_clone.set_subtitle("Medindo latência, variação (Jitter) e perda de pacotes...");

        let (sender, receiver) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let res = DiagnosticsEngine::run_network_stability_test("1.1.1.1");
            let _ = sender.send_blocking(res);
        });

        let row_c = ping_row_clone.clone();
        let btn_c = ping_btn_clone.clone();
        let t_ov = toast_diag.clone();
        glib::spawn_future_local(async move {
            if let Ok(res) = receiver.recv().await {
                btn_c.set_sensitive(true);
                match res {
                    Ok(r) => {
                        row_c.set_subtitle(&format!(
                            "Ping Médio: {:.1} ms (Mín: {:.1} / Máx: {:.1} ms) • Jitter: {:.2} ms • Perda: {:.0}% • {}",
                            r.ping_avg_ms, r.ping_min_ms, r.ping_max_ms, r.jitter_ms, r.packet_loss_pct, r.verdict
                        ));
                        t_ov.add_toast(Toast::new(&format!(
                            "Teste concluído: {:.1} ms ({})",
                            r.ping_avg_ms, r.verdict
                        )));
                    }
                    Err(e) => {
                        row_c.set_subtitle(&e);
                        t_ov.add_toast(Toast::new(&e));
                    }
                }
            }
        });
    });

    ping_row.add_suffix(&ping_btn);
    net_test_group.add(&ping_row);
    page.add(&net_test_group);

    // 3. Monitor de Eventos do Kernel com AdwExpanderRow e Linked Toggle Buttons (HIG Controls Pattern)
    let kernel_group = PreferencesGroup::new();
    kernel_group.set_title(&format!(
        "Eventos do Kernel ({})",
        report.kernel_events.len()
    ));
    kernel_group.set_description(Some(
        "Inspeção direta do ring-buffer do kernel Linux (journalctl -k) em busca de eventos de MCE, PCIe AER, NVRM, NVMe e EDAC",
    ));

    if report.kernel_events.is_empty() {
        let ok_row = ActionRow::new();
        ok_row.set_title("Nenhum erro ou aviso de hardware registrado no kernel");
        ok_row.set_subtitle("O subsistema de logs do kernel está limpo");
        let ok_icon = Image::from_icon_name("emblem-ok-symbolic");
        ok_row.add_prefix(&ok_icon);
        kernel_group.add(&ok_row);
    } else {
        let filter_box = GtkBox::new(Orientation::Horizontal, 0);
        filter_box.add_css_class("linked");
        filter_box.set_valign(Align::Center);

        let btn_all = ToggleButton::with_label("Todos");
        btn_all.set_active(true);
        let btn_warn = ToggleButton::with_label("Avisos");
        btn_warn.set_group(Some(&btn_all));
        let btn_crit = ToggleButton::with_label("Críticos");
        btn_crit.set_group(Some(&btn_all));

        filter_box.append(&btn_all);
        filter_box.append(&btn_warn);
        filter_box.append(&btn_crit);
        kernel_group.set_header_suffix(Some(&filter_box));

        let mut event_rows: Vec<(ExpanderRow, DiagSeverity)> = Vec::new();

        for ev in report.kernel_events.iter().take(15) {
            let exp_row = ExpanderRow::new();
            let clean_msg = ev.message.replace('&', "e");
            exp_row.set_title(&format!("{} — {}", ev.subsystem, ev.timestamp));
            exp_row.set_subtitle(&clean_msg);

            let (badge_txt, badge_cls) = match ev.severity {
                DiagSeverity::Critical => ("Crítico", "badge-red"),
                DiagSeverity::Warning => ("Aviso", "badge-amber"),
                DiagSeverity::Healthy => ("Info", "badge-blue"),
            };
            let badge = Label::new(Some(badge_txt));
            badge.add_css_class("badge-pill");
            badge.add_css_class(badge_cls);
            badge.set_valign(Align::Center);
            exp_row.add_suffix(&badge);

            let detail_row = ActionRow::new();
            detail_row.add_css_class("property");
            detail_row.set_title("Mensagem Completa do Kernel (dmesg / journalctl)");
            detail_row.set_subtitle(&clean_msg);
            exp_row.add_row(&detail_row);

            kernel_group.add(&exp_row);
            event_rows.push((exp_row, ev.severity.clone()));
        }

        let rows_rc = Rc::new(event_rows);

        let rows_all = rows_rc.clone();
        btn_all.connect_toggled(move |b| {
            if b.is_active() {
                for (row, _) in rows_all.iter() {
                    row.set_visible(true);
                }
            }
        });

        let rows_warn = rows_rc.clone();
        btn_warn.connect_toggled(move |b| {
            if b.is_active() {
                for (row, sev) in rows_warn.iter() {
                    row.set_visible(matches!(sev, DiagSeverity::Warning | DiagSeverity::Critical));
                }
            }
        });

        let rows_crit = rows_rc.clone();
        btn_crit.connect_toggled(move |b| {
            if b.is_active() {
                for (row, sev) in rows_crit.iter() {
                    row.set_visible(matches!(sev, DiagSeverity::Critical));
                }
            }
        });
    }

    page.add(&kernel_group);
    outer_box.append(&page);
    outer_box
}

fn set_status_badge(label: &Label, text: &str, css_class: &str) {
    label.set_text(text);
    label.set_css_classes(&["badge-pill", css_class, "numeric"]);
    label.set_valign(Align::Center);
}

fn package_subtitle(status: &GamingBoostStatus) -> String {
    if !status.apt_available {
        return "Disponível em Debian, Ubuntu e derivadas, na instalação .deb.".into();
    }
    let arch = if status.i386_enabled {
        "i386 ativa"
    } else {
        "i386 ausente"
    };
    let missing = status.missing_package_names();
    if missing.is_empty() {
        format!("Arquitetura {arch}. GameMode, MangoHud, Gamescope, Mesa e Vulkan instalados.")
    } else {
        format!("Arquitetura {arch}. Ausentes: {}.", missing.join(", "))
    }
}

fn split_lock_subtitle(status: &GamingBoostStatus) -> String {
    let base = "Desligar a penalidade melhora jogos antigos e o Wine/Proton. Um processo com split lock pode atrasar os outros núcleos. Não altera o GRUB e não exige reiniciar.";
    match (status.split_lock_mitigate, status.split_lock_persisted) {
        (Some(runtime), Some(saved)) if runtime == saved => {
            format!("{base} O valor atual está gravado em {SPLIT_LOCK_CONF_PATH}.")
        }
        (Some(_), Some(_)) => {
            format!("{base} O arquivo {SPLIT_LOCK_CONF_PATH} difere do valor atual.")
        }
        _ => base.to_string(),
    }
}

fn spawn_boost_action(
    toast: ToastOverlay,
    buttons: Vec<Button>,
    spinner: Spinner,
    work: impl FnOnce() -> Result<String, String> + Send + 'static,
    done: impl FnOnce() + 'static,
) {
    for button in &buttons {
        button.set_sensitive(false);
    }
    spinner.set_visible(true);
    spinner.start();

    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = sender.send_blocking(work());
    });

    glib::spawn_future_local(async move {
        if let Ok(result) = receiver.recv().await {
            spinner.stop();
            spinner.set_visible(false);
            match result {
                Ok(msg) => toast.add_toast(Toast::new(&msg)),
                Err(err) => toast.add_toast(Toast::new(&err)),
            }
            done();
        }
    });
}

fn build_gaming_boost_group(toast_overlay: &ToastOverlay, mh_badge: &Label) -> PreferencesGroup {
    let group = PreferencesGroup::new();
    group.set_title("Desempenho para Jogos");

    let pkg_row = ActionRow::new();
    pkg_row.set_title("Pacote gamer");
    let pkg_spinner = Spinner::new();
    pkg_spinner.set_visible(false);
    pkg_row.add_prefix(&pkg_spinner);
    let pkg_badge = Label::new(Some("…"));
    pkg_row.add_suffix(&pkg_badge);
    let pkg_btn = Button::with_label("Instalar");
    pkg_btn.set_valign(Align::Center);
    pkg_btn.add_css_class("suggested-action");
    pkg_btn.set_tooltip_text(Some(
        "Habilita a arquitetura i386, atualiza o apt e instala GameMode, MangoHud, Gamescope, mesa-utils e vulkan-tools.",
    ));
    pkg_row.add_suffix(&pkg_btn);
    group.add(&pkg_row);

    let shader_row = ActionRow::new();
    shader_row.set_title("Cache de shaders da Mesa");
    shader_row.set_subtitle(&format!(
        "Amplia o cache GLSL da Mesa (Intel e AMD) para 12 GB em {SHADER_CONF_PATH}. Não altera Proton, DXVK nem o cache da NVIDIA. Vale depois de um novo login."
    ));
    let shader_spinner = Spinner::new();
    shader_spinner.set_visible(false);
    shader_row.add_prefix(&shader_spinner);
    let shader_badge = Label::new(Some("…"));
    shader_row.add_suffix(&shader_badge);
    let shader_btn = Button::with_label("Aplicar 12 GB");
    shader_btn.set_valign(Align::Center);
    shader_btn.add_css_class("row-action-button");
    shader_row.add_suffix(&shader_btn);
    group.add(&shader_row);

    let split_row = ActionRow::new();
    split_row.set_title("Split lock");
    let split_spinner = Spinner::new();
    split_spinner.set_visible(false);
    split_row.add_prefix(&split_spinner);
    let split_badge = Label::new(Some("…"));
    split_row.add_suffix(&split_badge);
    let split_box = GtkBox::new(Orientation::Horizontal, 6);
    split_box.set_valign(Align::Center);
    let split_toggle_btn = Button::with_label("Desligar");
    split_toggle_btn.add_css_class("row-action-button");
    split_toggle_btn.set_tooltip_text(Some(
        "Altera kernel.split_lock_mitigate agora, sem editar o GRUB.",
    ));
    let split_persist_btn = Button::with_label("Persistir");
    split_persist_btn.add_css_class("row-action-button");
    split_persist_btn.set_tooltip_text(Some(
        "Grava o valor atual para os próximos boots, sem editar o GRUB.",
    ));
    split_box.append(&split_toggle_btn);
    split_box.append(&split_persist_btn);
    split_row.add_suffix(&split_box);
    group.add(&split_row);

    let lact_row = ActionRow::new();
    lact_row.set_title("LACT");
    lact_row.set_subtitle(
        "Controle de ventoinha, limite de energia e clocks da GPU AMD ou NVIDIA. A instalação baixa um .deb do GitHub sem verificação de assinatura e ativa o serviço lactd.",
    );
    let lact_spinner = Spinner::new();
    lact_spinner.set_visible(false);
    lact_row.add_prefix(&lact_spinner);
    let lact_badge = Label::new(Some("…"));
    lact_row.add_suffix(&lact_badge);
    let lact_btn = Button::with_label("Instalar");
    lact_btn.set_valign(Align::Center);
    lact_btn.add_css_class("row-action-button");
    lact_row.add_suffix(&lact_btn);
    group.add(&lact_row);

    let pkg_btn_click = pkg_btn.clone();
    let shader_btn_click = shader_btn.clone();
    let split_toggle_click = split_toggle_btn.clone();
    let split_persist_click = split_persist_btn.clone();
    let lact_btn_click = lact_btn.clone();
    let listed_pkg = pkg_btn.clone();
    let listed_shader = shader_btn.clone();
    let listed_toggle = split_toggle_btn.clone();
    let listed_persist = split_persist_btn.clone();
    let listed_lact = lact_btn.clone();
    let action_buttons: Rc<dyn Fn() -> Vec<Button>> = Rc::new(move || {
        vec![
            listed_pkg.clone(),
            listed_shader.clone(),
            listed_toggle.clone(),
            listed_persist.clone(),
            listed_lact.clone(),
        ]
    });

    let lact_installed = Rc::new(Cell::new(false));
    let lact_state = lact_installed.clone();
    let mh_badge = mh_badge.clone();
    let group_sync = group.clone();
    let sync: Rc<dyn Fn()> = Rc::new(move || {
        let status = GamingBoost::read();
        if status.in_flatpak {
            group_sync.set_description(Some(
                "Pacotes, ambiente do sistema, sysctl e serviços só funcionam na instalação .deb, fora do Flatpak.",
            ));
            group_sync.set_sensitive(false);
        } else {
            group_sync.set_description(Some(
                "Nada é alterado sozinho. Cada botão pede confirmação e, quando precisa, a senha de administrador.",
            ));
            group_sync.set_sensitive(true);
        }

        pkg_row.set_subtitle(&package_subtitle(&status));
        if !status.apt_available {
            set_status_badge(&pkg_badge, "Sem apt", "badge-amber");
        } else if status.packages_complete() {
            set_status_badge(&pkg_badge, "Instalado", "badge-green");
        } else {
            set_status_badge(&pkg_badge, "Incompleto", "badge-amber");
        }
        pkg_btn.set_sensitive(status.apt_available && !status.packages_complete());

        if status.shader_boost_applied {
            set_status_badge(&shader_badge, "12 GB", "badge-green");
        } else if status.glsl_cache_mb.is_some() || status.shader_cache_mb.is_some() {
            set_status_badge(&shader_badge, "Parcial", "badge-amber");
        } else {
            set_status_badge(&shader_badge, "Padrão", "badge-subtle");
        }
        shader_btn.set_sensitive(!status.shader_boost_applied);

        split_row.set_subtitle(&split_lock_subtitle(&status));
        match status.split_lock_mitigate {
            Some(0) => {
                set_status_badge(&split_badge, "Desligada", "badge-amber");
                split_toggle_btn.set_label("Religar");
            }
            Some(1) => {
                set_status_badge(&split_badge, "Ativa", "badge-green");
                split_toggle_btn.set_label("Desligar");
            }
            _ => {
                set_status_badge(&split_badge, "Indisponível", "badge-subtle");
                split_toggle_btn.set_label("Desligar");
            }
        }
        split_toggle_btn.set_sensitive(status.split_lock_mitigate.is_some());
        split_persist_btn.set_sensitive(
            status.split_lock_mitigate.is_some()
                && status.split_lock_mitigate != status.split_lock_persisted,
        );

        lact_installed.set(status.lact_installed);
        if status.lact_installed {
            set_status_badge(&lact_badge, "Instalado", "badge-green");
            lact_btn.set_label("Abrir");
            lact_btn.set_sensitive(true);
        } else {
            set_status_badge(&lact_badge, "Ausente", "badge-subtle");
            lact_btn.set_label("Instalar");
            lact_btn.set_sensitive(status.apt_available);
        }

        let mh_ok = status
            .packages
            .iter()
            .any(|pkg| pkg.name == "mangohud" && pkg.installed)
            || GamingAnalyzer::load_mangohud_config().is_installed;
        set_status_badge(
            &mh_badge,
            if mh_ok { "Instalado" } else { "Ausente" },
            if mh_ok { "badge-green" } else { "badge-subtle" },
        );
    });
    sync();

    let toast_pkg = toast_overlay.clone();
    let sync_pkg = sync.clone();
    let pkg_spinner_c = pkg_spinner.clone();
    let buttons_pkg = action_buttons.clone();
    pkg_btn_click.connect_clicked(move |_| {
        let toast = toast_pkg.clone();
        let sync = sync_pkg.clone();
        let spinner = pkg_spinner_c.clone();
        let buttons = buttons_pkg();
        spawn_boost_action(
            toast,
            buttons,
            spinner,
            GamingBoost::install_gaming_packages,
            move || sync(),
        );
    });

    let toast_shader = toast_overlay.clone();
    let sync_shader = sync.clone();
    let shader_spinner_c = shader_spinner.clone();
    let buttons_shader = action_buttons.clone();
    shader_btn_click.connect_clicked(move |_| {
        let toast = toast_shader.clone();
        let sync = sync_shader.clone();
        let spinner = shader_spinner_c.clone();
        let buttons = buttons_shader();
        spawn_boost_action(
            toast,
            buttons,
            spinner,
            GamingBoost::apply_shader_cache,
            move || sync(),
        );
    });

    let toast_split = toast_overlay.clone();
    let sync_split = sync.clone();
    let split_spinner_toggle = split_spinner.clone();
    let buttons_split = action_buttons.clone();
    split_toggle_click.connect_clicked(move |_| {
        let Some(current) = GamingBoost::read().split_lock_mitigate else {
            return;
        };
        let toast = toast_split.clone();
        let sync = sync_split.clone();
        let spinner = split_spinner_toggle.clone();
        let buttons = buttons_split();
        spawn_boost_action(
            toast,
            buttons,
            spinner,
            move || GamingBoost::set_split_lock_mitigate(current == 0),
            move || sync(),
        );
    });

    let toast_persist = toast_overlay.clone();
    let sync_persist = sync.clone();
    let split_spinner_persist = split_spinner.clone();
    let buttons_persist = action_buttons.clone();
    split_persist_click.connect_clicked(move |_| {
        let Some(current) = GamingBoost::read().split_lock_mitigate else {
            return;
        };
        let toast = toast_persist.clone();
        let sync = sync_persist.clone();
        let spinner = split_spinner_persist.clone();
        let buttons = buttons_persist();
        spawn_boost_action(
            toast,
            buttons,
            spinner,
            move || GamingBoost::persist_split_lock(current == 1),
            move || sync(),
        );
    });

    let toast_lact = toast_overlay.clone();
    let sync_lact = sync.clone();
    let lact_spinner_c = lact_spinner.clone();
    let buttons_lact = action_buttons.clone();
    lact_btn_click.connect_clicked(move |_| {
        if lact_state.get() {
            match GamingBoost::open_lact() {
                Ok(msg) => toast_lact.add_toast(Toast::new(&msg)),
                Err(err) => toast_lact.add_toast(Toast::new(&err)),
            }
            return;
        }

        let dialog = AlertDialog::builder()
            .heading("Instalar o LACT")
            .body("O Analise PC vai baixar o .deb amd64 mais recente do GitHub e instalá-lo com privilégios de administrador. Esse pacote não tem a assinatura verificada. O serviço lactd será ativado e poderá controlar ventoinha, energia e clocks da GPU.")
            .default_response("cancel")
            .close_response("cancel")
            .build();
        dialog.add_responses(&[("cancel", "_Cancelar"), ("install", "_Instalar")]);
        dialog.set_response_appearance("install", ResponseAppearance::Destructive);

        let toast = toast_lact.clone();
        let sync = sync_lact.clone();
        let spinner = lact_spinner_c.clone();
        let parent = toast.clone();
        let buttons_lact = buttons_lact.clone();
        glib::spawn_future_local(async move {
            let response = dialog.choose_future(Some(&parent)).await;
            if response != "install" {
                return;
            }
            spawn_boost_action(
                toast,
                buttons_lact(),
                spinner,
                GamingBoost::install_lact,
                move || sync(),
            );
        });
    });

    group
}

fn build_gaming_page(toast_overlay: &ToastOverlay) -> PreferencesPage {
    let page = PreferencesPage::new();

    let mh_badge = Label::new(Some("Ausente"));
    mh_badge.set_valign(Align::Center);
    page.add(&build_gaming_boost_group(toast_overlay, &mh_badge));

    let mut cpu_col = CpuCollector::new();
    let cpu_info = cpu_col.refresh();
    let mut mem_col = MemoryCollector::new();
    let mem_info = mem_col.refresh();
    let gpus = GpuCollector::list_gpus();

    // 1. Detector de Gargalo (CPU vs GPU vs VRAM)
    let bn = GamingAnalyzer::analyze_bottleneck(&cpu_info, &gpus, &mem_info);
    let bn_group = PreferencesGroup::new();
    bn_group.set_title("Detector de Gargalo (Equilíbrio CPU vs GPU vs VRAM)");
    bn_group.set_description(Some(&bn.overall_balance_verdict.replace('&', "e")));

    let hw_row = ActionRow::new();
    hw_row.set_title(&format!("Conjunto Analisado: {} + {}", bn.cpu_summary, bn.gpu_summary));
    hw_row.set_subtitle(&format!("Buffer de Vídeo: {}", bn.vram_summary));
    let hw_icon = Image::from_icon_name("input-gaming-symbolic");
    hw_row.add_prefix(&hw_icon);
    bn_group.add(&hw_row);

    for sc in &bn.scenarios {
        let exp = ExpanderRow::new();
        exp.set_title(&sc.resolution_preset.replace('&', "e"));
        exp.set_subtitle(&sc.limiting_component.replace('&', "e"));

        let badge = Label::new(Some(&sc.status_badge));
        badge.add_css_class("badge-pill");
        badge.add_css_class(if sc.is_balanced {
            "badge-green"
        } else {
            "badge-amber"
        });
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        exp.add_suffix(&badge);

        let rec_row = ActionRow::new();
        rec_row.add_css_class("property");
        rec_row.set_title(&format!(
            "Desvio / Limitação Estimada: {}% — Recomendação Técnica",
            sc.bottleneck_pct
        ));
        rec_row.set_subtitle(&sc.recommendation.replace('&', "e"));
        exp.add_row(&rec_row);

        bn_group.add(&exp);
    }

    page.add(&bn_group);

    // 2. Estimativa de FPS por Jogo
    let fps_list = GamingAnalyzer::estimate_games_fps(&gpus);
    let fps_group = PreferencesGroup::new();
    fps_group.set_title("Estimativa de Desempenho em Jogos (1080p Full HD)");
    fps_group.set_description(Some(
        "Projeção de quadros por segundo baseada no perfil da sua GPU dedicada e processador",
    ));

    for game in fps_list {
        let row = ActionRow::new();
        row.set_title(&game.title.replace('&', "e"));
        row.set_subtitle(&format!(
            "1080p Competitivo/Médio: {}  •  1080p Alto/DLSS: {}  •  {}",
            game.fps_1080p_med_comp, game.fps_1080p_high_dlss, game.vram_note
        ));

        let icon = Image::from_icon_name("input-gaming-symbolic");
        row.add_prefix(&icon);

        let badge = Label::new(Some(&game.tier_badge));
        badge.add_css_class("badge-pill");
        badge.add_css_class("badge-blue");
        badge.add_css_class("numeric");
        badge.set_valign(Align::Center);
        row.add_suffix(&badge);

        fps_group.add(&row);
    }

    page.add(&fps_group);

    // 3. Biblioteca Steam Local Detectada
    let steam_items = GamingAnalyzer::scan_steam_library();
    let steam_group = PreferencesGroup::new();
    steam_group.set_title(&format!(
        "Biblioteca Steam Local Detectada ({} itens)",
        steam_items.len()
    ));
    steam_group.set_description(Some(
        "Jogos e camadas de compatibilidade Proton/Runtime identificados em ~/.steam/steamapps",
    ));

    if steam_items.is_empty() {
        let empty_row = ActionRow::new();
        empty_row.set_title("Nenhum jogo ou manifesto Steam encontrado");
        empty_row.set_subtitle("Verifique se a Steam possui jogos instalados no diretório padrão");
        steam_group.add(&empty_row);
    } else {
        for item in steam_items {
            let row = ActionRow::new();
            row.set_title(&item.name.replace('&', "e"));
            let kind_str = if item.is_runtime_or_proton {
                "Camada de Compatibilidade / Runtime"
            } else {
                "Jogo Instalado na Biblioteca"
            };
            row.set_subtitle(&format!("{} • Steam AppID: {}", kind_str, item.app_id));

            let icon_name = if item.is_runtime_or_proton {
                "emblem-system-symbolic"
            } else {
                "input-gaming-symbolic"
            };
            let icon = Image::from_icon_name(icon_name);
            row.add_prefix(&icon);

            let badge = Label::new(Some(&format!("{:.2} GiB", item.size_gib)));
            badge.add_css_class("badge-pill");
            badge.add_css_class(if item.is_runtime_or_proton {
                "badge-subtle"
            } else {
                "badge-green"
            });
            badge.add_css_class("numeric");
            badge.set_valign(Align::Center);
            row.add_suffix(&badge);

            steam_group.add(&row);
        }
    }

    page.add(&steam_group);

    // 4. Configurador de Overlay de Jogo (MangoHud)
    let mh_cfg = Rc::new(RefCell::new(GamingAnalyzer::load_mangohud_config()));
    let mh_group = PreferencesGroup::new();
    mh_group.set_title("Overlay de Jogo no Linux (Configurador MangoHud)");
    let status_desc = if mh_cfg.borrow().is_installed {
        "MangoHud instalado no sistema — Configure abaixo os indicadores exibidos nos jogos (Atalho: Right Shift + F12)"
    } else {
        "Gera o arquivo ~/.config/MangoHud/MangoHud.conf (Instale o pacote 'mangohud' para ativar nos jogos Steam/Lutris)"
    };
    mh_group.set_description(Some(status_desc));

    let sw_fps = SwitchRow::new();
    sw_fps.set_title("Exibir Contador de FPS e 1% Low");
    sw_fps.set_subtitle("Mostra a taxa de quadros por segundo e métricas de estabilidade");
    sw_fps.set_active(mh_cfg.borrow().show_fps);
    mh_group.add(&sw_fps);

    let sw_ft = SwitchRow::new();
    sw_ft.set_title("Exibir Gráfico de Frametime em Tempo Real");
    sw_ft.set_subtitle("Desenha a linha de tempo de quadros (ms) para identificar micro-stuttering");
    sw_ft.set_active(mh_cfg.borrow().show_frametime_graph);
    mh_group.add(&sw_ft);

    let sw_gpu = SwitchRow::new();
    sw_gpu.set_title("Telemetria da Placa de Vídeo (GPU %, Temp °C, Clock e Watts)");
    sw_gpu.set_subtitle("Monitora carga, temperatura, frequência do núcleo e consumo da GPU");
    sw_gpu.set_active(mh_cfg.borrow().show_gpu_stats);
    mh_group.add(&sw_gpu);

    let sw_cpu = SwitchRow::new();
    sw_cpu.set_title("Telemetria do Processador (CPU %, Temp °C e MHz)");
    sw_cpu.set_subtitle("Monitora uso do processador, temperatura do pacote e frequência");
    sw_cpu.set_active(mh_cfg.borrow().show_cpu_stats);
    mh_group.add(&sw_cpu);

    let sw_mem = SwitchRow::new();
    sw_mem.set_title("Consumo de Memória VRAM e RAM");
    sw_mem.set_subtitle("Monitora uso do buffer de vídeo dedicado (GiB) e memória do sistema");
    sw_mem.set_active(mh_cfg.borrow().show_vram_ram);
    mh_group.add(&sw_mem);

    let sw_horiz = SwitchRow::new();
    sw_horiz.set_title("Layout Compacto Horizontal no Topo");
    sw_horiz.set_subtitle("Exibe o overlay em barra horizontal discreta na parte superior da tela");
    sw_horiz.set_active(mh_cfg.borrow().horizontal_hud);
    mh_group.add(&sw_horiz);

    let save_row = ActionRow::new();
    save_row.set_title("Aplicar Perfil do MangoHud");
    save_row.set_subtitle(&format!(
        "Arquivo: {} • Na Steam use: mangohud %command%",
        mh_cfg.borrow().config_path.display()
    ));

    let save_btn = Button::with_mnemonic("_Salvar Overlay");
    save_btn.set_valign(Align::Center);
    save_btn.add_css_class("suggested-action");

    let cfg_clone = mh_cfg.clone();
    let toast_mh = toast_overlay.clone();
    save_btn.connect_clicked(move |_| {
        let mut c = cfg_clone.borrow_mut();
        c.show_fps = sw_fps.is_active();
        c.show_frametime_graph = sw_ft.is_active();
        c.show_gpu_stats = sw_gpu.is_active();
        c.show_cpu_stats = sw_cpu.is_active();
        c.show_vram_ram = sw_mem.is_active();
        c.horizontal_hud = sw_horiz.is_active();

        match GamingAnalyzer::save_mangohud_config(&c) {
            Ok(msg) => toast_mh.add_toast(Toast::new(&msg)),
            Err(err) => toast_mh.add_toast(Toast::new(&err)),
        }
    });

    save_row.add_suffix(&mh_badge);
    save_row.add_suffix(&save_btn);
    mh_group.add(&save_row);

    page.add(&mh_group);
    page
}
