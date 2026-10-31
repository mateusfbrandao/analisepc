use std::cell::RefCell;
use std::collections::VecDeque;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

use libadwaita::gtk::cairo;
use libadwaita::gtk::pango::EllipsizeMode;
use libadwaita::gtk::prelude::*;
use libadwaita::gtk::{Align, Box as GtkBox, DrawingArea, Label, Orientation};

const HISTORY_LEN: usize = 60;

/// Paleta de cores oficial da GNOME HIG (developer.gnome.org/hig/resources.html)
pub const GNOME_BLUE_3: (f64, f64, f64) = (0.208, 0.518, 0.894);   // #3584e4
pub const GNOME_GREEN_4: (f64, f64, f64) = (0.180, 0.761, 0.494);  // #2ec27e
pub const GNOME_YELLOW_5: (f64, f64, f64) = (0.898, 0.647, 0.039); // #e5a50a
pub const GNOME_RED_3: (f64, f64, f64) = (0.878, 0.106, 0.141);    // #e01b24

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

#[derive(Clone)]
pub struct RealtimeChart {
    container: GtkBox,
    area: DrawingArea,
    history: Rc<RefCell<VecDeque<f64>>>,
    stats_label: Label,
    current_badge: Label,
    unit: String,
}

impl RealtimeChart {
    pub fn new(title: &str, unit: &str, max_val: f64, color_rgb: (f64, f64, f64)) -> Self {
        let mut init_deque = VecDeque::with_capacity(HISTORY_LEN);
        for _ in 0..HISTORY_LEN {
            init_deque.push_back(0.0);
        }
        let history = Rc::new(RefCell::new(init_deque));

        let container = GtkBox::new(Orientation::Vertical, 8);
        container.add_css_class("chart-card");
        container.set_margin_top(10);
        container.set_margin_bottom(10);
        container.set_margin_start(12);
        container.set_margin_end(12);

        // Cabeçalho adaptativo seguindo GNOME HIG (Typography & Adaptive Guidelines)
        let header_box = GtkBox::new(Orientation::Horizontal, 8);
        header_box.set_valign(Align::Center);

        let title_label = Label::new(Some(title));
        title_label.set_xalign(0.0);
        title_label.set_hexpand(true);
        title_label.set_ellipsize(EllipsizeMode::End);
        title_label.add_css_class("heading");

        let stats_label = Label::new(Some("Méd: 0.0 • Máx: 0.0"));
        stats_label.set_halign(Align::End);
        stats_label.set_valign(Align::Center);
        stats_label.set_ellipsize(EllipsizeMode::End);
        stats_label.add_css_class("caption");
        stats_label.add_css_class("dim-label");
        stats_label.add_css_class("numeric");

        let current_badge = Label::new(Some(&format!("0.0{}", unit)));
        current_badge.set_halign(Align::End);
        current_badge.set_valign(Align::Center);
        current_badge.add_css_class("badge-pill");
        current_badge.add_css_class("numeric");
        current_badge.add_css_class("badge-subtle");

        header_box.append(&title_label);
        header_box.append(&stats_label);
        header_box.append(&current_badge);

        let area = DrawingArea::new();
        area.set_content_height(120);
        area.set_hexpand(true);

        let hist_clone = history.clone();
        let unit_owned = unit.to_string();
        let (r, g, b) = color_rgb;

        area.set_draw_func(move |_area, cr, width, height| {
            let w = width as f64;
            let h = height as f64;
            let pad_left = 4.0;
            let pad_right = 4.0;
            let pad_top = 6.0;
            let pad_bottom = 6.0;
            let plot_w = (w - pad_left - pad_right).max(10.0);
            let plot_h = (h - pad_top - pad_bottom).max(10.0);
            let radius = 8.0;

            // 1. Quadro arredondado com clip suave
            cr.save().ok();
            rounded_rect(cr, pad_left, pad_top, plot_w, plot_h, radius);
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.05);
            let _ = cr.fill_preserve();
            cr.clip();

            // 2. Linhas de grade tracejadas
            cr.set_line_width(1.0);
            cr.set_dash(&[3.0, 4.0], 0.0);
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.16);
            for frac in [0.25, 0.5, 0.75] {
                let y = pad_top + plot_h * frac;
                cr.move_to(pad_left, y);
                cr.line_to(pad_left + plot_w, y);
            }
            for step in [0.25, 0.5, 0.75] {
                let x = pad_left + plot_w * step;
                cr.move_to(x, pad_top);
                cr.line_to(x, pad_top + plot_h);
            }
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);

            // 3. Curva suavizada com gradiente vertical
            let data = hist_clone.borrow();
            let n = data.len();
            if n >= 2 {
                let effective_max = data.iter().copied().fold(max_val, f64::max).max(1.0);
                let coords: Vec<(f64, f64)> = data
                    .iter()
                    .enumerate()
                    .map(|(idx, &val)| {
                        let x = pad_left + (idx as f64 / (n - 1) as f64) * plot_w;
                        let norm = (val / effective_max).clamp(0.0, 1.0);
                        let y = pad_top + plot_h * (1.0 - norm);
                        (x, y)
                    })
                    .collect();

                let trace_smooth_path = |cr: &cairo::Context, pts: &[(f64, f64)]| {
                    cr.move_to(pts[0].0, pts[0].1);
                    for i in 0..pts.len() - 1 {
                        let (x0, y0) = pts[i];
                        let (x1, y1) = pts[i + 1];
                        let cx = (x0 + x1) * 0.5;
                        cr.curve_to(cx, y0, cx, y1, x1, y1);
                    }
                };

                // Gradiente linear vertical abaixo da curva
                trace_smooth_path(cr, &coords);
                if let Some(&(last_x, _)) = coords.last() {
                    cr.line_to(last_x, pad_top + plot_h);
                    cr.line_to(coords[0].0, pad_top + plot_h);
                    cr.close_path();
                }

                let grad = cairo::LinearGradient::new(0.0, pad_top, 0.0, pad_top + plot_h);
                grad.add_color_stop_rgba(0.0, r, g, b, 0.32);
                grad.add_color_stop_rgba(0.85, r, g, b, 0.06);
                grad.add_color_stop_rgba(1.0, r, g, b, 0.01);
                let _ = cr.set_source(&grad);
                let _ = cr.fill();

                // Linha principal anti-aliased
                cr.set_line_width(2.2);
                cr.set_line_cap(cairo::LineCap::Round);
                cr.set_line_join(cairo::LineJoin::Round);
                cr.set_source_rgb(r, g, b);
                trace_smooth_path(cr, &coords);
                let _ = cr.stroke();

                // Halo + ponto indicador na leitura atual
                if let Some(&(last_x, last_y)) = coords.last() {
                    cr.arc(last_x, last_y, 6.5, 0.0, TAU);
                    cr.set_source_rgba(r, g, b, 0.22);
                    let _ = cr.fill();

                    cr.arc(last_x, last_y, 3.2, 0.0, TAU);
                    cr.set_source_rgb(r, g, b);
                    let _ = cr.fill();
                }

                // Rótulo sutil de escala no topo esquerdo
                cr.select_font_face(
                    "Adwaita Sans",
                    cairo::FontSlant::Normal,
                    cairo::FontWeight::Normal,
                );
                cr.set_font_size(10.0);
                cr.set_source_rgba(0.5, 0.5, 0.5, 0.65);
                cr.move_to(pad_left + 8.0, pad_top + 14.0);
                let _ = cr.show_text(&format!("Escala: {:.0}{}", effective_max, unit_owned));
            }

            cr.restore().ok();

            // Contorno arredondado sutil do gráfico
            rounded_rect(cr, pad_left, pad_top, plot_w, plot_h, radius);
            cr.set_line_width(1.0);
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.16);
            let _ = cr.stroke();
        });

        container.append(&header_box);
        container.append(&area);

        Self {
            container,
            area,
            history,
            stats_label,
            current_badge,
            unit: unit.to_string(),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn push_value(&self, value: f64) {
        let clamped = value.max(0.0);
        let mut hist = self.history.borrow_mut();

        if hist.iter().all(|&v| v == 0.0) && clamped > 0.0 {
            for v in hist.iter_mut() {
                *v = clamped;
            }
        } else {
            if hist.len() >= HISTORY_LEN {
                hist.pop_front();
            }
            hist.push_back(clamped);
        }

        let mut min_v = f64::MAX;
        let mut max_v = f64::MIN;
        let mut sum_v = 0.0;
        for &v in hist.iter() {
            if v < min_v {
                min_v = v;
            }
            if v > max_v {
                max_v = v;
            }
            sum_v += v;
        }
        let avg_v = sum_v / (hist.len() as f64);
        drop(hist);

        self.stats_label.set_text(&format!(
            "Mín: {:.1}{} • Méd: {:.1}{} • Máx: {:.1}{}",
            min_v, self.unit, avg_v, self.unit, max_v, self.unit
        ));
        self.current_badge
            .set_text(&format!("{:.1}{}", clamped, self.unit));

        self.area.queue_draw();
    }
}

#[derive(Clone)]
pub struct CoreBarsChart {
    container: GtkBox,
    area: DrawingArea,
    usages: Rc<RefCell<Vec<(f64, u64)>>>,
    summary_label: Label,
    peak_badge: Label,
}

impl CoreBarsChart {
    pub fn new(num_threads: usize) -> Self {
        let usages = Rc::new(RefCell::new(vec![(0.0_f64, 0_u64); num_threads.max(1)]));

        let container = GtkBox::new(Orientation::Vertical, 8);
        container.add_css_class("chart-card");
        container.set_margin_top(10);
        container.set_margin_bottom(10);
        container.set_margin_start(12);
        container.set_margin_end(12);

        let header_box = GtkBox::new(Orientation::Horizontal, 8);
        header_box.set_valign(Align::Center);

        let title_label = Label::new(Some("Carga por Thread"));
        title_label.set_xalign(0.0);
        title_label.set_hexpand(true);
        title_label.set_ellipsize(EllipsizeMode::End);
        title_label.add_css_class("heading");

        let summary_label = Label::new(Some("0 MHz"));
        summary_label.set_halign(Align::End);
        summary_label.set_ellipsize(EllipsizeMode::End);
        summary_label.add_css_class("caption");
        summary_label.add_css_class("dim-label");
        summary_label.add_css_class("numeric");

        let peak_badge = Label::new(Some("Pico: 0.0%"));
        peak_badge.set_halign(Align::End);
        peak_badge.add_css_class("badge-pill");
        peak_badge.add_css_class("badge-subtle");
        peak_badge.add_css_class("numeric");

        header_box.append(&title_label);
        header_box.append(&summary_label);
        header_box.append(&peak_badge);

        let area = DrawingArea::new();
        area.set_content_height(84);
        area.set_hexpand(true);

        let usages_clone = usages.clone();
        area.set_draw_func(move |_area, cr, width, height| {
            let data = usages_clone.borrow();
            let n = data.len();
            if n == 0 {
                return;
            }

            let w = width as f64;
            let h = height as f64;
            let pad_x = 4.0;
            let pad_top = 6.0;
            let pad_bottom = 6.0;
            let plot_w = (w - pad_x * 2.0).max(10.0);
            let plot_h = (h - pad_top - pad_bottom).max(10.0);

            let gap = if plot_w < 360.0 { 3.0 } else { 5.0 };
            let bar_w = ((plot_w - gap * ((n - 1) as f64)) / (n as f64)).max(3.0);
            let bar_radius = 4.0;

            for (i, &(usage, _freq)) in data.iter().enumerate() {
                let x = pad_x + (i as f64) * (bar_w + gap);

                // Trilha de fundo com cantos arredondados
                rounded_rect(cr, x, pad_top, bar_w, plot_h, bar_radius);
                cr.set_source_rgba(0.5, 0.5, 0.5, 0.10);
                let _ = cr.fill();

                // Barra proporcional com cores semânticas da paleta GNOME HIG
                let frac = (usage / 100.0).clamp(0.04, 1.0);
                let bar_h = (plot_h * frac).max(4.0);
                let y = pad_top + (plot_h - bar_h);

                let (r, g, b) = if usage >= 85.0 {
                    GNOME_RED_3
                } else if usage >= 60.0 {
                    GNOME_YELLOW_5
                } else {
                    GNOME_BLUE_3
                };

                rounded_rect(cr, x, y, bar_w, bar_h, bar_radius);
                let grad = cairo::LinearGradient::new(x, y, x, y + bar_h);
                grad.add_color_stop_rgba(0.0, r, g, b, 0.95);
                grad.add_color_stop_rgba(1.0, r, g, b, 0.68);
                let _ = cr.set_source(&grad);
                let _ = cr.fill();
            }
        });

        container.append(&header_box);
        container.append(&area);

        Self {
            container,
            area,
            usages,
            summary_label,
            peak_badge,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn update_cores(&self, cores: &[crate::core::cpu::CpuCoreInfo]) {
        if cores.is_empty() {
            return;
        }
        let mut data = self.usages.borrow_mut();
        data.clear();
        let mut max_usage = 0.0_f32;
        let mut max_freq = 0_u64;
        for c in cores {
            if c.usage_percent > max_usage {
                max_usage = c.usage_percent;
            }
            if c.frequency_mhz > max_freq {
                max_freq = c.frequency_mhz;
            }
            data.push((c.usage_percent as f64, c.frequency_mhz));
        }
        drop(data);

        self.summary_label.set_text(&format!(
            "{} Threads • {} MHz",
            cores.len(),
            max_freq
        ));
        self.peak_badge
            .set_text(&format!("Pico: {:.1}%", max_usage));
        self.area.queue_draw();
    }
}
