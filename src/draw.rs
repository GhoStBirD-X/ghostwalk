//! Colours, number formatting and the hand-drawn pieces: maps, charts and icons.

use crate::model::{Sport, TrackPoint};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};

pub const ACCENT: Color32 = Color32::from_rgb(255, 92, 38);
pub const GREEN: Color32 = Color32::from_rgb(46, 170, 90);
pub const RED: Color32 = Color32::from_rgb(220, 60, 60);
/// The ghost: an earlier activity raced against.
pub const GHOST: Color32 = Color32::from_rgb(140, 110, 255);

pub fn sport_color(s: Sport) -> Color32 {
    match s {
        Sport::Run => ACCENT,
        Sport::Ride => Color32::from_rgb(60, 120, 230),
        Sport::Walk => Color32::from_rgb(40, 160, 120),
        Sport::Hike => Color32::from_rgb(130, 100, 60),
        Sport::Swim => Color32::from_rgb(30, 170, 210),
        Sport::Workout => Color32::from_rgb(150, 90, 200),
    }
}

fn pick(ui: &Ui, dark: Color32, light: Color32) -> Color32 {
    if ui.visuals().dark_mode { dark } else { light }
}

pub fn bg(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(10, 10, 12), Color32::from_rgb(243, 243, 246))
}

pub fn card_bg(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(22, 22, 26), Color32::WHITE)
}

/// Buttons, chips and fields that sit on a card.
pub fn raised(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(34, 34, 40), Color32::from_rgb(238, 238, 242))
}

pub fn border(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(36, 36, 42), Color32::from_rgb(228, 228, 234))
}

pub fn muted(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(140, 140, 152), Color32::from_rgb(108, 108, 120))
}

pub fn strong(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(246, 246, 248), Color32::from_rgb(16, 16, 20))
}

/// The second accent: streaks, goals reached, GPS ready.
pub fn lime(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(198, 255, 61), Color32::from_rgb(78, 150, 0))
}

pub fn map_bg(ui: &Ui) -> Color32 {
    pick(ui, Color32::from_rgb(26, 27, 30), Color32::from_rgb(232, 236, 230))
}

/// The look of egui's own widgets (fields, sliders, menus) to match the cards.
pub fn theme(style: &mut egui::Style, dark: bool) {
    let c = |d: [u8; 3], l: [u8; 3]| {
        let [r, g, b] = if dark { d } else { l };
        Color32::from_rgb(r, g, b)
    };
    let (bg, card, raised, hover) = (
        c([10, 10, 12], [243, 243, 246]),
        c([22, 22, 26], [255, 255, 255]),
        c([34, 34, 40], [238, 238, 242]),
        c([46, 46, 54], [226, 226, 232]),
    );
    let (line, text) = (c([36, 36, 42], [228, 228, 234]), c([246, 246, 248], [16, 16, 20]));
    let v = &mut style.visuals;
    *v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = bg;
    v.window_fill = card;
    v.extreme_bg_color = raised;
    v.faint_bg_color = raised;
    v.code_bg_color = raised;
    v.window_stroke = Stroke::new(1.0, line);
    v.window_corner_radius = egui::CornerRadius::same(16);
    v.menu_corner_radius = egui::CornerRadius::same(14);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.5, ACCENT);
    v.text_cursor.stroke = Stroke::new(2.0, ACCENT);
    v.slider_trailing_fill = true;
    for (w, fill) in [
        (&mut v.widgets.inactive, raised),
        (&mut v.widgets.hovered, hover),
        (&mut v.widgets.active, hover),
        (&mut v.widgets.open, hover),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = Stroke::NONE;
        w.corner_radius = egui::CornerRadius::same(12);
        w.fg_stroke = Stroke::new(1.5, text);
    }
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text);
    v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(12);
    style.spacing.button_padding = vec2(14.0, 8.0);
    style.spacing.interact_size.y = 34.0;
    style.spacing.item_spacing = vec2(8.0, 8.0);
    use egui::TextStyle;
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Body, FontId::proportional(15.0)),
        (TextStyle::Button, FontId::proportional(15.0)),
        (TextStyle::Heading, FontId::new(22.0, heavy())),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
}

// ---------- type ----------

pub fn semi() -> FontFamily {
    FontFamily::Name("semi".into())
}

pub fn heavy() -> FontFamily {
    FontFamily::Name("heavy".into())
}

/// A storybook serif, for the adventure map style.
pub fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}

/// Barlow for everything, with egui's own fonts behind it for symbols.
pub fn install_fonts(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    let fallback = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (name, bytes) in [
        ("barlow", &include_bytes!("../assets/fonts/Barlow-Medium.ttf")[..]),
        ("barlow-semi", &include_bytes!("../assets/fonts/Barlow-SemiBold.ttf")[..]),
        ("barlow-heavy", &include_bytes!("../assets/fonts/Barlow-ExtraBold.ttf")[..]),
        ("cormorant", &include_bytes!("../assets/fonts/CormorantSC-Bold.ttf")[..]),
    ] {
        f.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    for (family, first) in [(FontFamily::Proportional, "barlow"), (semi(), "barlow-semi"), (heavy(), "barlow-heavy"), (serif(), "cormorant")] {
        let mut list = vec![first.to_string()];
        if first == "cormorant" {
            list.push("barlow-semi".into());
        }
        list.extend(fallback.iter().cloned());
        f.families.insert(family, list);
    }
    ctx.set_fonts(f);
}

/// Big bold number.
pub fn big(text: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(text.into()).family(heavy()).size(size).color(color)
}

/// Small grey label above a section, e.g. "MINGGU INI".
pub fn overline(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).family(semi()).size(11.5).color(muted(ui)));
}

/// A small rounded label; with `dot`, a coloured dot comes first (drawn, since the font has no "●").
pub fn pill(ui: &mut Ui, text: &str, color: Color32, dot: bool) -> egui::Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.14))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            // Painted by hand so the dot stays first and the pill hugs its text in any row direction.
            let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::new(12.0, semi()), color);
            let lead = if dot { 13.0 } else { 0.0 };
            let (r, _) = ui.allocate_exact_size(vec2(lead + galley.size().x, galley.size().y.max(14.0)), Sense::hover());
            if dot {
                ui.painter().circle_filled(pos2(r.left() + 4.0, r.center().y), 4.0, color);
            }
            ui.painter().galley(pos2(r.left() + lead, r.center().y - galley.size().y / 2.0), galley, color);
        })
        .response
}

/// A tappable star, filled when `on` (drawn, since the font has no "★").
pub fn star(ui: &mut Ui, on: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
    let c = rect.center();
    let pts: Vec<Pos2> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { 10.0 } else { 4.2 };
            let a = (-90.0 + 36.0 * i as f32).to_radians();
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    let p = ui.painter();
    if on {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(c, ACCENT);
        for q in &pts {
            mesh.colored_vertex(*q, ACCENT);
        }
        for i in 0..10u32 {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
        }
        p.add(mesh);
    } else {
        let mut closed = pts.clone();
        closed.push(pts[0]);
        p.line(closed, Stroke::new(1.5, muted(ui)));
    }
    resp
}

pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let dark = ui.visuals().dark_mode;
    egui::Frame::new()
        .fill(card_bg(ui))
        .stroke(if dark { Stroke::new(1.0, border(ui)) } else { Stroke::NONE })
        .shadow(if dark {
            egui::Shadow::NONE
        } else {
            egui::Shadow { offset: [0, 4], blur: 18, spread: 0, color: Color32::from_black_alpha(14) }
        })
        .corner_radius(20.0)
        .inner_margin(16.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
}

/// A card that can be tapped; buttons inside it still get their own taps.
pub fn card_click<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    let r = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| card(ui, add).inner);
    let clicked = r.response.clicked();
    (r.inner, clicked)
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(text).family(heavy()).size(20.0).color(strong(ui)));
    ui.add_space(4.0);
}

pub fn accent_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text.trim()).family(heavy()).size(15.5).color(Color32::WHITE))
            .fill(ACCENT)
            .stroke(Stroke::NONE)
            .corner_radius(24.0)
            .min_size(vec2(120.0, 46.0)),
    )
}

pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> egui::Response {
    let (fill, fg) = if selected {
        (ACCENT, Color32::WHITE)
    } else {
        (raised(ui), strong(ui))
    };
    ui.add(
        egui::Button::new(RichText::new(text).family(semi()).size(13.5).color(fg))
            .fill(fill)
            .stroke(Stroke::NONE)
            .corner_radius(18.0)
            .min_size(vec2(0.0, 32.0)),
    )
}

/// A labelled number, as used in the stat rows.
pub fn stat(ui: &mut Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(RichText::new(label).size(12.5).color(muted(ui)));
        ui.add(egui::Label::new(big(value, 22.0, strong(ui))).wrap_mode(egui::TextWrapMode::Extend));
    });
}

pub fn stat_grid(ui: &mut Ui, items: &[(&str, String)], cols: usize) {
    let w = ui.available_width() / cols as f32;
    for row in items.chunks(cols) {
        ui.horizontal(|ui| {
            for (l, v) in row {
                ui.allocate_ui(vec2(w - 6.0, 50.0), |ui| {
                    ui.set_min_width(w - 6.0);
                    stat(ui, l, v);
                });
            }
        });
        ui.add_space(6.0);
    }
}

/// Paints `color` over the corners outside a rounded rectangle, so square drawings look rounded.
pub fn round_corners(p: &egui::Painter, rect: Rect, r: f32, color: Color32) {
    let steps = 8;
    for (corner, centre, start) in [
        (rect.left_top(), rect.left_top() + vec2(r, r), 180.0f32),
        (rect.right_top(), rect.right_top() + vec2(-r, r), 270.0),
        (rect.right_bottom(), rect.right_bottom() + vec2(-r, -r), 0.0),
        (rect.left_bottom(), rect.left_bottom() + vec2(r, -r), 90.0),
    ] {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(corner, color);
        for i in 0..=steps {
            let a = (start + 90.0 * i as f32 / steps as f32).to_radians();
            mesh.colored_vertex(centre + vec2(a.cos(), a.sin()) * r, color);
        }
        for i in 1..=steps as u32 {
            mesh.add_triangle(0, i, i + 1);
        }
        p.add(mesh);
    }
}

// ---------- formatting ----------

fn comma(s: String) -> String {
    s.replace('.', ",")
}

pub fn dist(m: f64, imperial: bool) -> String {
    let m = m.max(0.0) + 0.0;
    if imperial {
        comma(format!("{:.2} mi", m / 1609.344))
    } else {
        comma(format!("{:.2} km", m / 1000.0))
    }
}

pub fn dur(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    let (h, m, sec) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}j {m:02}m")
    } else {
        format!("{m}m {sec:02}d")
    }
}

pub fn clock(s: f64) -> String {
    let s = s.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

pub fn pace(speed_mps: f64, imperial: bool) -> String {
    if speed_mps <= 0.05 {
        return "-".into();
    }
    let unit = if imperial { 1609.344 } else { 1000.0 };
    let spu = unit / speed_mps;
    format!(
        "{}:{:02} /{}",
        (spu / 60.0) as u64,
        (spu % 60.0) as u64,
        if imperial { "mi" } else { "km" }
    )
}

pub fn speed(speed_mps: f64, imperial: bool) -> String {
    if imperial {
        comma(format!("{:.1} mi/j", speed_mps * 3.6 / 1.609_344))
    } else {
        comma(format!("{:.1} km/j", speed_mps * 3.6))
    }
}

pub fn pace_or_speed(sport: Sport, speed_mps: f64, imperial: bool) -> (&'static str, String) {
    if sport.uses_speed() {
        ("Kecepatan", speed(speed_mps, imperial))
    } else {
        ("Tempo", pace(speed_mps, imperial))
    }
}

const MONTHS: [&str; 12] = [
    "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober", "November", "Desember",
];
const DAYS: [&str; 7] = ["Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu"];

pub fn month_year(year: i32, month: u32) -> String {
    format!("{} {year}", MONTHS[(month.clamp(1, 12) - 1) as usize])
}

/// "02 Nov 2026, 06:15"
pub fn date_short<T: chrono::Datelike + chrono::Timelike>(t: &T) -> String {
    let m = &MONTHS[t.month0() as usize][..3];
    format!("{:02} {m} {}, {:02}:{:02}", t.day(), t.year(), t.hour(), t.minute())
}

/// "Senin, 2 November 2026 06:15"
pub fn date_long<T: chrono::Datelike + chrono::Timelike>(t: &T) -> String {
    format!(
        "{}, {} {} {} {:02}:{:02}",
        DAYS[t.weekday().num_days_from_monday() as usize],
        t.day(),
        MONTHS[t.month0() as usize],
        t.year(),
        t.hour(),
        t.minute()
    )
}

// ---------- icons ----------

pub fn sport_badge(ui: &mut Ui, sport: Sport, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let p = ui.painter();
    let c = sport_color(sport);
    p.rect_filled(rect, size * 0.3, c.gamma_multiply(0.18));
    sport_icon(p, sport, rect.center(), size * 0.58, c);
}

fn ellipse(c: Pos2, rx: f32, ry: f32, tilt: f32) -> Vec<Pos2> {
    (0..20)
        .map(|i| {
            let a = i as f32 / 20.0 * std::f32::consts::TAU;
            let (x, y) = (rx * a.cos(), ry * a.sin());
            c + vec2(x * tilt.cos() - y * tilt.sin(), x * tilt.sin() + y * tilt.cos())
        })
        .collect()
}

/// A small drawing for each sport (shoe, bicycle, footprints, mountain, waves, dumbbell), `s` wide.
pub fn sport_icon(p: &egui::Painter, sport: Sport, c: Pos2, s: f32, color: Color32) {
    let q = |x: f32, y: f32| c + vec2(x, y) * (s / 2.0);
    let st = Stroke::new((s * 0.085).max(1.4), color);
    let line = |pts: &[(f32, f32)]| {
        p.line(pts.iter().map(|&(x, y)| q(x, y)).collect(), st);
    };
    match sport {
        Sport::Run => {
            line(&[
                (-0.9, 0.45),
                (0.72, 0.45),
                (0.9, 0.3),
                (0.84, 0.12),
                (0.3, -0.02),
                (0.02, -0.28),
                (-0.28, -0.45),
                (-0.58, -0.45),
                (-0.62, -0.08),
                (-0.9, 0.05),
                (-0.9, 0.45),
            ]);
            line(&[(-0.9, 0.22), (0.86, 0.22)]);
            line(&[(-0.12, -0.2), (0.1, -0.05)]);
            line(&[(0.08, -0.32), (0.26, -0.18)]);
        }
        Sport::Ride => {
            p.circle_stroke(q(-0.52, 0.32), s * 0.19, st);
            p.circle_stroke(q(0.52, 0.32), s * 0.19, st);
            line(&[(-0.52, 0.32), (-0.08, 0.32), (0.3, -0.22), (-0.24, -0.22), (-0.52, 0.32)]);
            line(&[(-0.08, 0.32), (-0.3, -0.42)]);
            line(&[(-0.42, -0.42), (-0.16, -0.42)]);
            line(&[(0.52, 0.32), (0.32, -0.42), (0.52, -0.5)]);
        }
        Sport::Walk => {
            for (x, y, t) in [(-0.32, 0.18, -0.2f32), (0.32, -0.22, 0.2f32)] {
                p.add(egui::Shape::convex_polygon(ellipse(q(x, y), s * 0.15, s * 0.25, t), color, Stroke::NONE));
                p.circle_filled(q(x + t * 0.5, y - 0.66), s * 0.07, color);
            }
        }
        Sport::Hike => {
            line(&[(-0.95, 0.6), (-0.3, -0.42), (0.05, 0.1), (0.35, -0.18), (0.95, 0.6), (-0.95, 0.6)]);
            line(&[(-0.48, -0.14), (-0.3, -0.02), (-0.14, -0.16)]);
            p.circle_filled(q(0.62, -0.58), s * 0.09, color);
        }
        Sport::Swim => {
            for y in [-0.42f32, 0.02, 0.46] {
                let pts: Vec<Pos2> = (0..=24)
                    .map(|i| {
                        let x = -0.92 + i as f32 / 24.0 * 1.84;
                        q(x, y + 0.13 * (x * 6.0).sin())
                    })
                    .collect();
                p.line(pts, st);
            }
        }
        Sport::Workout => {
            line(&[(-0.62, 0.0), (0.62, 0.0)]);
            for (x, w, h) in [(-0.56f32, 0.2f32, 0.8f32), (0.56, 0.2, 0.8), (-0.82, 0.14, 0.5), (0.82, 0.14, 0.5)] {
                p.rect_filled(egui::Rect::from_center_size(q(x, 0.0), vec2(w, h) * (s / 2.0)), s * 0.04, color);
            }
        }
    }
}

/// A track with a knob that must be dragged to the end, so a pocket tap can't trigger it.
pub fn slide_to_confirm(ui: &mut Ui, id_salt: &str, text: &str) -> bool {
    let h = 60.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let id = ui.scope_id().with(id_salt);
    let max = rect.width() - h;
    let mut off: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(0.0);
    let knob = Rect::from_center_size(pos2(rect.left() + h / 2.0 + off, rect.center().y), Vec2::splat(h - 10.0));
    let resp = ui.interact(knob.expand(6.0), id, Sense::drag());
    if resp.dragged() {
        off = (off + resp.drag_delta().x).clamp(0.0, max);
    }
    let done = resp.drag_stopped() && off >= max * 0.85;
    if !resp.dragged() {
        // Spring back unless finished.
        off = if done { 0.0 } else { (off - 30.0).max(0.0) };
        if off > 0.0 {
            ui.ctx().request_repaint();
        }
    }
    ui.data_mut(|d| d.insert_temp(id, off));
    let p = ui.painter();
    p.rect_filled(rect, h / 2.0, raised(ui));
    let filled = Rect::from_min_max(rect.min, pos2(rect.left() + h + off, rect.bottom()));
    p.rect_filled(filled, h / 2.0, RED.gamma_multiply(0.25));
    let fade = 1.0 - (off / max.max(1.0)).clamp(0.0, 1.0);
    p.text(
        rect.center() + vec2(h * 0.3, 0.0),
        Align2::CENTER_CENTER,
        text,
        FontId::new(15.0, semi()),
        muted(ui).gamma_multiply(fade),
    );
    let kc = pos2(rect.left() + h / 2.0 + off, rect.center().y);
    p.circle_filled(kc, h / 2.0 - 5.0, RED);
    for dx in [-6.0, 2.0] {
        p.line(
            vec![kc + vec2(dx, -7.0), kc + vec2(dx + 6.0, 0.0), kc + vec2(dx, 7.0)],
            Stroke::new(2.5, Color32::WHITE),
        );
    }
    done
}

/// Speed along the route as a filled line that fades downwards; faster is higher.
/// `label` turns a speed (m/s) into the text for the top and bottom marks.
pub fn speed_chart(ui: &mut Ui, pts: &[[f64; 2]], color: Color32, height: f32, label: impl Fn(f64) -> String) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if pts.len() < 2 {
        return;
    }
    let p = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(0.0, 16.0), rect.max - vec2(0.0, 16.0));
    let x1 = pts.last().unwrap()[0].max(1.0);
    let lo = pts.iter().map(|q| q[1]).fold(f64::MAX, f64::min);
    let hi = pts.iter().map(|q| q[1]).fold(f64::MIN, f64::max).max(lo + 0.1);
    let at = |q: &[f64; 2]| {
        pos2(
            plot.left() + (q[0] / x1) as f32 * plot.width(),
            plot.bottom() - ((q[1] - lo) / (hi - lo)) as f32 * plot.height(),
        )
    };
    let line: Vec<Pos2> = pts.iter().map(at).collect();
    let mut mesh = egui::Mesh::default();
    let top = color.gamma_multiply(0.45);
    let clear = color.gamma_multiply(0.0);
    for q in &line {
        mesh.colored_vertex(*q, top);
        mesh.colored_vertex(pos2(q.x, plot.bottom()), clear);
    }
    for i in 0..line.len().saturating_sub(1) as u32 {
        let k = i * 2;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    p.add(mesh);
    p.line(line, Stroke::new(2.2, color));
    let font = FontId::new(11.0, semi());
    p.text(rect.left_top(), Align2::LEFT_TOP, label(hi), font.clone(), muted(ui));
    p.text(rect.left_bottom(), Align2::LEFT_BOTTOM, label(lo), font.clone(), muted(ui));
    p.text(rect.right_bottom(), Align2::RIGHT_BOTTOM, format!("{:.1} km", x1 / 1000.0).replace('.', ","), font, muted(ui));
}

/// The little ghost used as the app's mark.
pub fn ghost(p: &egui::Painter, c: Pos2, s: f32, color: Color32, eye: Color32) {
    let w = s * 0.42;
    p.circle_filled(pos2(c.x, c.y - s * 0.08), w, color);
    p.rect_filled(
        Rect::from_min_max(pos2(c.x - w, c.y - s * 0.08), pos2(c.x + w, c.y + s * 0.3)),
        0.0,
        color,
    );
    for i in 0..3 {
        let x = c.x - w + w / 3.0 + i as f32 * (2.0 * w / 3.0);
        p.circle_filled(pos2(x, c.y + s * 0.3), w / 3.0, color);
    }
    p.circle_filled(pos2(c.x - w * 0.38, c.y - s * 0.1), s * 0.07, eye);
    p.circle_filled(pos2(c.x + w * 0.38, c.y - s * 0.1), s * 0.07, eye);
}

pub enum Icon {
    Home,
    Map,
    Record,
    Goals,
    You,
}

pub fn icon(p: &egui::Painter, kind: Icon, c: Pos2, color: Color32) {
    let st = Stroke::new(2.0, color);
    match kind {
        Icon::Home => {
            p.line(vec![pos2(c.x - 9.0, c.y - 1.0), pos2(c.x, c.y - 9.0), pos2(c.x + 9.0, c.y - 1.0)], st);
            p.rect_stroke(
                Rect::from_min_max(pos2(c.x - 6.5, c.y - 2.0), pos2(c.x + 6.5, c.y + 8.0)),
                1.0,
                st,
                egui::StrokeKind::Middle,
            );
        }
        Icon::Map => {
            p.line(
                vec![
                    pos2(c.x - 9.0, c.y - 6.0),
                    pos2(c.x - 3.0, c.y - 9.0),
                    pos2(c.x + 3.0, c.y - 6.0),
                    pos2(c.x + 9.0, c.y - 9.0),
                    pos2(c.x + 9.0, c.y + 6.0),
                    pos2(c.x + 3.0, c.y + 9.0),
                    pos2(c.x - 3.0, c.y + 6.0),
                    pos2(c.x - 9.0, c.y + 9.0),
                    pos2(c.x - 9.0, c.y - 6.0),
                ],
                st,
            );
            p.line_segment([pos2(c.x - 3.0, c.y - 9.0), pos2(c.x - 3.0, c.y + 6.0)], st);
            p.line_segment([pos2(c.x + 3.0, c.y - 6.0), pos2(c.x + 3.0, c.y + 9.0)], st);
        }
        Icon::Record => {
            p.circle_stroke(c, 10.0, st);
            p.circle_filled(c, 5.5, color);
        }
        Icon::Goals => {
            p.circle_stroke(c, 9.5, st);
            p.circle_stroke(c, 5.0, st);
            p.circle_filled(c, 1.8, color);
        }
        Icon::You => {
            p.circle_stroke(pos2(c.x, c.y - 4.0), 4.5, st);
            p.line(
                vec![
                    pos2(c.x - 8.0, c.y + 9.0),
                    pos2(c.x - 6.0, c.y + 3.0),
                    pos2(c.x, c.y + 1.5),
                    pos2(c.x + 6.0, c.y + 3.0),
                    pos2(c.x + 8.0, c.y + 9.0),
                ],
                st,
            );
        }
    }
}

// ---------- maps ----------

pub fn map_backdrop(ui: &Ui, p: &egui::Painter, rect: Rect, radius: f32) {
    let themed = crate::offmap::backdrop(ui.visuals().dark_mode);
    p.rect_filled(rect, radius, themed.map_or(map_bg(ui), |t| t.0));
    let line = themed.map_or(pick(ui, Color32::from_rgb(34, 35, 40), Color32::from_rgb(220, 228, 216)), |t| t.1);
    let road = themed.map_or(pick(ui, Color32::from_rgb(42, 44, 50), Color32::from_rgb(250, 250, 246)), |t| t.2);
    let step = 28.0;
    let mut x = rect.left() + step / 2.0;
    let mut i = 0;
    while x < rect.right() {
        let w = if i % 4 == 1 { 4.0 } else { 1.0 };
        p.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(w, if w > 1.0 { road } else { line }),
        );
        x += step;
        i += 1;
    }
    let mut y = rect.top() + step / 2.0;
    i = 0;
    while y < rect.bottom() {
        let w = if i % 3 == 2 { 4.0 } else { 1.0 };
        p.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(w, if w > 1.0 { road } else { line }),
        );
        y += step;
        i += 1;
    }
    // A soft "park" patch so the map doesn't look empty.
    let park = themed.map_or(pick(ui, Color32::from_rgb(28, 40, 32), Color32::from_rgb(205, 228, 196)), |t| t.3);
    let pr = Rect::from_min_size(
        pos2(rect.left() + rect.width() * 0.62, rect.top() + rect.height() * 0.12),
        vec2(rect.width() * 0.22, rect.height() * 0.3),
    );
    p.rect_filled(pr, 6.0, park);
}

/// Length of the equator in metres. Map positions are Web Mercator scaled to it.
pub const EARTH_M: f64 = 40_075_016.686;

/// Web Mercator position from 0.0 to 1.0 (x east, y south), as map tiles use.
pub fn merc01(lat: f64, lon: f64) -> [f64; 2] {
    let lat = lat.clamp(-85.051_128, 85.051_128).to_radians();
    [
        (lon + 180.0) / 360.0,
        (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0,
    ]
}

/// Map positions for a track, in Web Mercator metres.
pub fn merc(track: &[TrackPoint]) -> Vec<[f64; 2]> {
    track
        .iter()
        .map(|p| {
            let [x, y] = merc01(p.lat, p.lon);
            [x * EARTH_M, y * EARTH_M]
        })
        .collect()
}

/// Latitude and longitude for a position in Web Mercator metres (the reverse of `merc`).
pub fn unmerc(p: [f64; 2]) -> (f64, f64) {
    let lon = p[0] / EARTH_M * 360.0 - 180.0;
    let lat = (std::f64::consts::PI * (1.0 - 2.0 * p[1] / EARTH_M)).sinh().atan().to_degrees();
    (lat, lon)
}

/// Where map positions land on screen: `world * s + offset`, then turned by `rot`
/// (radians, clockwise on screen) around the map's centre `c`.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub s: f64,
    pub ox: f64,
    pub oy: f64,
    pub rot: f64,
    pub c: [f64; 2],
}

impl View {
    pub fn to_screen(&self, p: [f64; 2]) -> Pos2 {
        let (x, y) = (p[0] * self.s + self.ox - self.c[0], p[1] * self.s + self.oy - self.c[1]);
        let (sin, cos) = self.rot.sin_cos();
        pos2((self.c[0] + x * cos - y * sin) as f32, (self.c[1] + x * sin + y * cos) as f32)
    }

    pub fn to_world(&self, p: Pos2) -> [f64; 2] {
        let (x, y) = (p.x as f64 - self.c[0], p.y as f64 - self.c[1]);
        let (sin, cos) = self.rot.sin_cos();
        let (ux, uy) = (self.c[0] + x * cos + y * sin, self.c[1] - x * sin + y * cos);
        [(ux - self.ox) / self.s, (uy - self.oy) / self.s]
    }
}

pub fn fit(paths: &[&[[f64; 2]]], rect: Rect, pad: f32) -> impl Fn([f64; 2]) -> Pos2 + use<> {
    let v = fit_view(paths, rect, pad);
    move |p: [f64; 2]| v.to_screen(p)
}

/// The north-up view that shows every path inside `rect`.
pub fn fit_view(paths: &[&[[f64; 2]]], rect: Rect, pad: f32) -> View {
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for path in paths {
        for p in path.iter() {
            minx = minx.min(p[0]);
            maxx = maxx.max(p[0]);
            miny = miny.min(p[1]);
            maxy = maxy.max(p[1]);
        }
    }
    let inner = rect.shrink(pad);
    let w = (maxx - minx).max(1.0);
    let h = (maxy - miny).max(1.0);
    let s = (inner.width() as f64 / w).min(inner.height() as f64 / h);
    let ox = inner.center().x as f64 - (minx + w / 2.0) * s;
    let oy = inner.center().y as f64 - (miny + h / 2.0) * s;
    View { s, ox, oy, rot: 0.0, c: [rect.center().x as f64, rect.center().y as f64] }
}

/// A north-up view with `center` (Web Mercator metres) in the middle and about `span_m` real metres across.
pub fn centered_view(center: [f64; 2], span_m: f64, rect: Rect) -> View {
    let (lat, _) = unmerc(center);
    // Mercator stretches distances by 1 / cos(latitude).
    let s = rect.width() as f64 / (span_m / lat.to_radians().cos().max(0.01));
    let c = [rect.center().x as f64, rect.center().y as f64];
    View { s, ox: c[0] - center[0] * s, oy: c[1] - center[1] * s, rot: 0.0, c }
}

/// `v` turned by `a` radians (clockwise on screen).
fn turn(v: Vec2, a: f64) -> Vec2 {
    let (sin, cos) = (a as f32).sin_cos();
    vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

/// Where the compass sits on a map that is turned away from north.
fn compass_at(rect: Rect) -> Pos2 {
    pos2(rect.left() + 22.0, rect.top() + 22.0)
}

/// Applies dragging, pinching, turning and the corner buttons to `base` (a north-up view);
/// zoom, pan and turn are remembered per map.
/// Phones turn with two fingers, computers with a right-button drag.
fn pan_zoom(ui: &Ui, resp: &egui::Response, rect: Rect, base: View, double_click_resets: bool) -> View {
    let id = resp.id.with("pan_zoom");
    let (mut zoom, mut pan, mut rot): (f64, Vec2, f64) = ui.data(|d| d.get_temp(id)).unwrap_or((1.0, Vec2::ZERO, 0.0));
    let c = rect.center();
    // Panning and zooming happen before the turn, so screen positions are turned back first.
    let unturn = |at: Pos2, rot: f64| c + turn(at - c, -rot);
    let zoom_at = |zoom: &mut f64, pan: &mut Vec2, factor: f64, at: Pos2| {
        let z2 = (*zoom * factor).clamp(0.25, 64.0);
        *pan = (at - c) - (at - c - *pan) * (z2 / *zoom) as f32;
        *zoom = z2;
    };
    // Turns the map by `d`, keeping the place under `at` where it is.
    let turn_at = |pan: &mut Vec2, rot: &mut f64, d: f64, at: Pos2| {
        let r2 = *rot + d;
        *pan += turn(at - c, -r2) - turn(at - c, -*rot);
        *rot = r2;
    };
    if resp.dragged_by(egui::PointerButton::Secondary) {
        turn_at(&mut pan, &mut rot, resp.drag_delta().x as f64 * 0.01, c);
    } else if resp.dragged() {
        pan += turn(resp.drag_delta(), -rot);
    }
    if resp.hovered() {
        let zd = ui.input(|i| i.zoom_delta()) as f64;
        if zd != 1.0 {
            let at = ui.input(|i| i.pointer.hover_pos()).unwrap_or(c);
            zoom_at(&mut zoom, &mut pan, zd, unturn(at, rot));
        }
    }
    if let Some(mt) = ui.input(|i| i.multi_touch()) {
        if rect.contains(mt.center_pos) && mt.rotation_delta != 0.0 {
            turn_at(&mut pan, &mut rot, mt.rotation_delta as f64, mt.center_pos);
        }
    }
    rot = (rot + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if double_click_resets && resp.double_clicked() {
        (zoom, pan, rot) = (1.0, Vec2::ZERO, 0.0);
    }
    for (i, factor) in [(0, 2.0), (1, 0.5), (2, 0.0)] {
        if i == 2 && zoom == 1.0 && pan == Vec2::ZERO && rot == 0.0 {
            continue;
        }
        let r = Rect::from_center_size(pos2(rect.right() - 22.0, rect.top() + 22.0 + i as f32 * 36.0), Vec2::splat(30.0));
        if ui.interact(r, id.with(i), Sense::click()).clicked() {
            if factor == 0.0 {
                (zoom, pan, rot) = (1.0, Vec2::ZERO, 0.0);
            } else {
                zoom_at(&mut zoom, &mut pan, factor, c);
            }
        }
    }
    // Back to north up, turning around the centre so the middle of the map stays put.
    if rot != 0.0 && ui.interact(Rect::from_center_size(compass_at(rect), Vec2::splat(30.0)), id.with("north"), Sense::click()).clicked() {
        rot = 0.0;
    }
    ui.data_mut(|d| d.insert_temp(id, (zoom, pan, rot)));
    View {
        s: base.s * zoom,
        ox: c.x as f64 + (base.ox - c.x as f64) * zoom + pan.x as f64,
        oy: c.y as f64 + (base.oy - c.y as f64) * zoom + pan.y as f64,
        rot,
        c: [c.x as f64, c.y as f64],
    }
}

/// Zoom in, zoom out and (once moved) back-to-start buttons in the map's corner,
/// and a compass pointing north while the map is turned.
fn zoom_buttons(ui: &Ui, p: &egui::Painter, rect: Rect, show_reset: bool, rot: f64) {
    let fg = strong(ui);
    for i in 0..if show_reset { 3 } else { 2 } {
        let c = pos2(rect.right() - 22.0, rect.top() + 22.0 + i as f32 * 36.0);
        p.circle(c, 15.0, card_bg(ui).gamma_multiply(0.92), Stroke::new(1.0, border(ui)));
        let st = Stroke::new(2.0, fg);
        match i {
            0 => {
                p.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], st);
                p.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], st);
            }
            1 => {
                p.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], st);
            }
            _ => {
                p.circle_stroke(c, 6.0, Stroke::new(1.6, fg));
                p.circle_filled(c, 2.0, fg);
            }
        }
    }
    if rot != 0.0 {
        let c = compass_at(rect);
        p.circle(c, 15.0, card_bg(ui).gamma_multiply(0.92), Stroke::new(1.0, border(ui)));
        let north = turn(vec2(0.0, -10.0), rot);
        let side = turn(vec2(4.0, 0.0), rot);
        p.add(egui::Shape::convex_polygon(vec![c + north, c + side, c - side], RED, Stroke::NONE));
        p.add(egui::Shape::convex_polygon(vec![c - north, c - side, c + side], muted(ui), Stroke::NONE));
    }
}

/// A route over the offline map (or the plain backdrop where there is none).
/// `interactive` maps can be dragged, zoomed and turned.
/// `around` is the colour behind the map, used to round its corners.
pub fn route_map(ui: &mut Ui, track: &[TrackPoint], color: Color32, height: f32, interactive: bool, around: Color32) -> egui::Response {
    let sense = if interactive { Sense::click_and_drag() } else { Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);
    let p = ui.painter_at(rect);
    map_backdrop(ui, &p, rect, 10.0);
    let pts = merc(track);
    if pts.len() < 2 || !ui.is_rect_visible(rect) {
        round_corners(&p, rect, 14.0, around);
        return resp;
    }
    let base = fit_view(&[&pts], rect, 18.0);
    let view = if interactive { pan_zoom(ui, &resp, rect, base, true) } else { base };
    let dark = ui.visuals().dark_mode;
    let has_map = crate::offmap::draw(&p, rect, &view, dark);
    let line: Vec<Pos2> = pts.iter().map(|q| view.to_screen(*q)).collect();
    if !crate::game::route_line(&p, &line, color) {
        p.line(line.clone(), Stroke::new(10.0, color.gamma_multiply(0.22)));
        p.line(line.clone(), Stroke::new(3.5, color));
    }
    if !crate::game::start(&p, line[0]) {
        p.circle(line[0], 6.0, GREEN, Stroke::new(2.5, Color32::WHITE));
    }
    if !crate::game::finish(&p, apart(line[0], *line.last().unwrap())) {
        p.circle(*line.last().unwrap(), 6.0, RED, Stroke::new(2.5, Color32::WHITE));
    }
    crate::game::decorate(&p, rect, has_map);
    if has_map {
        crate::offmap::attribution(&p, rect, dark);
    } else if interactive {
        p.text(
            rect.left_bottom() + vec2(8.0, -6.0),
            Align2::LEFT_BOTTOM,
            "Belum ada peta untuk area ini",
            FontId::proportional(11.0),
            muted(ui),
        );
    }
    round_corners(&p, rect, 14.0, around);
    if interactive {
        let moved = view.s != base.s || view.ox != base.ox || view.oy != base.oy || view.rot != 0.0;
        zoom_buttons(ui, &p, rect, moved, view.rot);
    }
    resp
}

/// The map while recording (and before the start): the planned route faint underneath, the track
/// so far with where you are now, and where the ghost is.
pub fn live_map(
    ui: &mut Ui,
    track: &[TrackPoint],
    plan: &[TrackPoint],
    ghost: Option<TrackPoint>,
    color: Color32,
    height: f32,
    around: Color32,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    map_backdrop(ui, &p, rect, 10.0);
    let (tr, pl) = (merc(track), merc(plan));
    let gh = ghost.map(|g| merc(&[g])[0]);
    if tr.len() + pl.len() < 2 || !ui.is_rect_visible(rect) {
        round_corners(&p, rect, 14.0, around);
        return;
    }
    let ghost_path = gh.map(|g| vec![g]).unwrap_or_default();
    let view = fit_view(&[&tr, &pl, &ghost_path], rect, 22.0);
    let dark = ui.visuals().dark_mode;
    let has_map = crate::offmap::draw(&p, rect, &view, dark);
    if pl.len() > 1 {
        let line: Vec<Pos2> = pl.iter().map(|q| view.to_screen(*q)).collect();
        let faint = if dark { Color32::from_gray(200) } else { Color32::from_gray(90) };
        if !crate::game::plan_line(&p, &line) {
            p.line(line.clone(), Stroke::new(7.0, faint.gamma_multiply(0.35)));
        }
        if !crate::game::start(&p, line[0]) {
            p.circle(line[0], 5.0, GREEN, Stroke::new(2.0, Color32::WHITE));
        }
        if !crate::game::finish(&p, apart(line[0], *line.last().unwrap())) {
            p.circle(*line.last().unwrap(), 5.0, RED, Stroke::new(2.0, Color32::WHITE));
        }
    }
    if tr.len() > 1 {
        let line: Vec<Pos2> = tr.iter().map(|q| view.to_screen(*q)).collect();
        if !crate::game::route_line(&p, &line, color) {
            p.line(line.clone(), Stroke::new(10.0, color.gamma_multiply(0.22)));
            p.line(line, Stroke::new(3.5, color));
        }
    }
    if let Some(g) = gh {
        let at = view.to_screen(g);
        if !crate::game::ghost(&p, at) {
            p.circle(at, 7.5, GHOST, Stroke::new(2.5, Color32::WHITE));
            p.text(at - vec2(0.0, 12.0), Align2::CENTER_BOTTOM, "hantu", FontId::new(11.0, semi()), GHOST);
        }
    }
    if let Some(me) = tr.last() {
        let at = view.to_screen(*me);
        // Heading from a point some metres back, so GPS jitter does not spin the marker.
        let back = tr.iter().rev().map(|q| view.to_screen(*q)).find(|q| q.distance(at) > 6.0);
        let dir = back.map_or(Vec2::ZERO, |b| at - b);
        if !crate::game::me(&p, at, dir, color) {
            p.circle(at, 8.0, color, Stroke::new(3.0, Color32::WHITE));
        }
    }
    crate::game::decorate(&p, rect, has_map);
    if has_map {
        crate::offmap::attribution(&p, rect, dark);
    }
    round_corners(&p, rect, 14.0, around);
}

/// Where to put a finish marker so it does not hide the start of a loop.
fn apart(start: Pos2, finish: Pos2) -> Pos2 {
    if start.distance(finish) < 16.0 { start + vec2(20.0, 0.0) } else { finish }
}

/// The route planner's map: the offline map around `center` (latitude, longitude), about 4 km
/// across, with the route drawn so far. Returns where it was tapped, as (latitude, longitude).
pub fn plan_map(ui: &mut Ui, center: (f64, f64), route: &[[f64; 2]], color: Color32, around: Color32) -> Option<(f64, f64)> {
    let side = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click_and_drag());
    // A new starting place: start again from a plain north-up view there.
    let key = resp.id.with("plan_center");
    ui.data_mut(|d| {
        if d.get_temp::<(f64, f64)>(key) != Some(center) {
            d.remove::<(f64, Vec2, f64)>(resp.id.with("pan_zoom"));
            d.insert_temp(key, center);
        }
    });
    let p = ui.painter_at(rect);
    map_backdrop(ui, &p, rect, 10.0);
    let m = merc01(center.0, center.1);
    let base = centered_view([m[0] * EARTH_M, m[1] * EARTH_M], 4000.0, rect);
    // Taps add points, so a double tap must not also reset the view.
    let view = pan_zoom(ui, &resp, rect, base, false);
    let dark = ui.visuals().dark_mode;
    let has_map = crate::offmap::draw(&p, rect, &view, dark);
    let line: Vec<Pos2> = route
        .iter()
        .map(|q| {
            let w = merc01(q[0], q[1]);
            view.to_screen([w[0] * EARTH_M, w[1] * EARTH_M])
        })
        .collect();
    if line.len() > 1 && !crate::game::route_line(&p, &line, color) {
        p.line(line.clone(), Stroke::new(4.0, color));
    }
    for (i, q) in line.iter().enumerate() {
        if i == 0 && crate::game::start(&p, *q) {
            continue;
        }
        let c = if i == 0 { GREEN } else { Color32::WHITE };
        p.circle(*q, 5.0, c, Stroke::new(2.0, color));
    }
    crate::game::decorate(&p, rect, has_map);
    if has_map {
        crate::offmap::attribution(&p, rect, dark);
    } else {
        p.text(
            rect.left_bottom() + vec2(8.0, -6.0),
            Align2::LEFT_BOTTOM,
            "Belum ada peta offline untuk area ini",
            FontId::proportional(11.0),
            muted(ui),
        );
    }
    round_corners(&p, rect, 14.0, around);
    let moved = view.s != base.s || view.ox != base.ox || view.oy != base.oy || view.rot != 0.0;
    zoom_buttons(ui, &p, rect, moved, view.rot);
    if resp.clicked() {
        resp.interact_pointer_pos().map(|pos| unmerc(view.to_world(pos)))
    } else {
        None
    }
}

// ---------- charts ----------

/// Simple bar chart. `labels` go under each bar.
pub fn bars(ui: &mut Ui, values: &[f64], labels: &[String], color: Color32, height: f32, highlight_last: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    let max = values.iter().cloned().fold(0.0, f64::max).max(0.001);
    let n = values.len().max(1) as f32;
    let slot = rect.width() / n;
    let base = rect.bottom() - 16.0;
    for (i, v) in values.iter().enumerate() {
        let h = ((v / max) as f32 * (base - rect.top() - 4.0)).max(if *v > 0.0 { 3.0 } else { 1.0 });
        let x = rect.left() + slot * i as f32 + slot * 0.2;
        let r = Rect::from_min_max(pos2(x, base - h), pos2(x + slot * 0.6, base));
        let c = if highlight_last && i + 1 < values.len() {
            muted(ui).gamma_multiply(0.55)
        } else {
            color
        };
        p.rect_filled(r, 6.0, if *v > 0.0 { c } else { raised(ui) });
        if let Some(l) = labels.get(i) {
            p.text(
                pos2(x + slot * 0.3, rect.bottom() - 2.0),
                Align2::CENTER_BOTTOM,
                l,
                FontId::new(11.0, semi()),
                muted(ui),
            );
        }
    }
}

/// Filled line chart of (x, y) points.
pub fn area(ui: &mut Ui, pts: &[[f64; 2]], color: Color32, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    if pts.len() < 2 {
        return;
    }
    let p = ui.painter_at(rect);
    let (x0, x1) = (pts[0][0], pts.last().unwrap()[0].max(pts[0][0] + 1.0));
    let ymin = pts.iter().map(|q| q[1]).fold(f64::MAX, f64::min);
    let ymax = pts.iter().map(|q| q[1]).fold(f64::MIN, f64::max).max(ymin + 1.0);
    let map = |q: &[f64; 2]| {
        pos2(
            rect.left() + ((q[0] - x0) / (x1 - x0)) as f32 * rect.width(),
            rect.bottom() - 14.0 - ((q[1] - ymin) / (ymax - ymin)) as f32 * (rect.height() - 22.0),
        )
    };
    let step = (pts.len() / 200).max(1);
    let line: Vec<Pos2> = pts.iter().step_by(step).map(map).collect();
    let fill = color.gamma_multiply(0.25);
    for w in line.windows(2) {
        let quad = vec![
            w[0],
            w[1],
            pos2(w[1].x, rect.bottom() - 14.0),
            pos2(w[0].x, rect.bottom() - 14.0),
        ];
        p.add(egui::Shape::convex_polygon(quad, fill, Stroke::NONE));
    }
    p.line(line, Stroke::new(2.0, color));
    p.text(
        rect.left_bottom(),
        Align2::LEFT_BOTTOM,
        format!("{:.0} m", ymin),
        FontId::proportional(10.0),
        muted(ui),
    );
    p.text(
        rect.left_top(),
        Align2::LEFT_TOP,
        format!("{:.0} m", ymax),
        FontId::proportional(10.0),
        muted(ui),
    );
}

/// Progress bar in the accent colour with a label on the right.
pub fn progress(ui: &mut Ui, frac: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 5.0, raised(ui));
    let mut done = rect;
    done.set_width((rect.width() * frac.clamp(0.0, 1.0)).max(10.0));
    if frac > 0.0 {
        p.rect_filled(done, 5.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_view_maps_back_to_the_same_place() {
        let v = View { s: 0.37, ox: 120.0, oy: -40.0, rot: 1.1, c: [200.0, 150.0] };
        for p in [[0.0, 0.0], [512.5, -88.0], [-3000.0, 1234.5]] {
            let back = v.to_world(v.to_screen(p));
            assert!((back[0] - p[0]).abs() < 0.01 && (back[1] - p[1]).abs() < 0.01, "{p:?} came back as {back:?}");
        }
        // A quarter turn clockwise: what was east of the centre is now below it.
        let q = View { s: 1.0, ox: 0.0, oy: 0.0, rot: std::f64::consts::FRAC_PI_2, c: [0.0, 0.0] }.to_screen([10.0, 0.0]);
        assert!(q.x.abs() < 1e-4 && (q.y - 10.0).abs() < 1e-4, "{q:?}");
    }

    #[test]
    fn mercator_round_trip() {
        let m = merc01(-6.2383, 106.9756);
        let (lat, lon) = unmerc([m[0] * EARTH_M, m[1] * EARTH_M]);
        assert!((lat + 6.2383).abs() < 1e-9 && (lon - 106.9756).abs() < 1e-9);
    }

    #[test]
    fn centered_view_spans_the_asked_distance() {
        let m = merc01(-6.2, 106.8);
        let c = [m[0] * EARTH_M, m[1] * EARTH_M];
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0));
        let v = centered_view(c, 4000.0, rect);
        let (la, lo) = unmerc(v.to_world(pos2(0.0, 200.0)));
        let (lb, lb2) = unmerc(v.to_world(pos2(400.0, 200.0)));
        let a = TrackPoint { lat: la, lon: lo, ele: None, t: None };
        let b = TrackPoint { lat: lb, lon: lb2, ele: None, t: None };
        let d = crate::model::haversine(&a, &b);
        assert!((d - 4000.0).abs() < 20.0, "{d}");
    }
}
