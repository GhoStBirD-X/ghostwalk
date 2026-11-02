//! Colours, number formatting and the hand-drawn pieces: maps, charts and icons.

use crate::model::{Sport, TrackPoint};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};

pub const ACCENT: Color32 = Color32::from_rgb(255, 92, 38);
pub const GREEN: Color32 = Color32::from_rgb(46, 170, 90);
pub const RED: Color32 = Color32::from_rgb(220, 60, 60);

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

/// Barlow for everything, with egui's own fonts behind it for symbols.
pub fn install_fonts(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    let fallback = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for (name, bytes) in [
        ("barlow", &include_bytes!("../assets/fonts/Barlow-Medium.ttf")[..]),
        ("barlow-semi", &include_bytes!("../assets/fonts/Barlow-SemiBold.ttf")[..]),
        ("barlow-heavy", &include_bytes!("../assets/fonts/Barlow-ExtraBold.ttf")[..]),
    ] {
        f.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    }
    for (family, first) in [(FontFamily::Proportional, "barlow"), (semi(), "barlow-semi"), (heavy(), "barlow-heavy")] {
        let mut list = vec![first.to_string()];
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
    p.text(rect.center(), Align2::CENTER_CENTER, sport.letter(), FontId::new(size * 0.5, heavy()), c);
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

/// Flat metres (x east, y south) from lat/lon.
pub fn project(track: &[TrackPoint]) -> Vec<[f64; 2]> {
    let Some(first) = track.first() else {
        return Vec::new();
    };
    let k = first.lat.to_radians().cos();
    track
        .iter()
        .map(|p| [p.lon * 111_320.0 * k, -p.lat * 111_320.0])
        .collect()
}

pub fn map_backdrop(ui: &Ui, p: &egui::Painter, rect: Rect, radius: f32) {
    p.rect_filled(rect, radius, map_bg(ui));
    let line = pick(ui, Color32::from_rgb(34, 35, 40), Color32::from_rgb(220, 228, 216));
    let road = pick(ui, Color32::from_rgb(42, 44, 50), Color32::from_rgb(250, 250, 246));
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
    let park = pick(ui, Color32::from_rgb(28, 40, 32), Color32::from_rgb(205, 228, 196));
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

/// Where map positions land on screen: `screen = world * s + offset`.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub s: f64,
    pub ox: f64,
    pub oy: f64,
}

impl View {
    pub fn to_screen(&self, p: [f64; 2]) -> Pos2 {
        pos2((p[0] * self.s + self.ox) as f32, (p[1] * self.s + self.oy) as f32)
    }

    pub fn to_world(&self, p: Pos2) -> [f64; 2] {
        [(p.x as f64 - self.ox) / self.s, (p.y as f64 - self.oy) / self.s]
    }
}

pub fn fit(paths: &[&[[f64; 2]]], rect: Rect, pad: f32) -> impl Fn([f64; 2]) -> Pos2 + use<> {
    let v = fit_view(paths, rect, pad);
    move |p: [f64; 2]| v.to_screen(p)
}

/// The view that shows every path inside `rect`.
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
    View { s, ox, oy }
}

/// Applies dragging, pinching and the +/- buttons to `base`; the zoom is remembered per map.
fn pan_zoom(ui: &Ui, resp: &egui::Response, rect: Rect, base: View) -> View {
    let id = resp.id.with("pan_zoom");
    let (mut zoom, mut pan): (f64, Vec2) = ui.data(|d| d.get_temp(id)).unwrap_or((1.0, Vec2::ZERO));
    let c = rect.center();
    let zoom_at = |zoom: &mut f64, pan: &mut Vec2, factor: f64, at: Pos2| {
        let z2 = (*zoom * factor).clamp(0.25, 64.0);
        *pan = (at - c) - (at - c - *pan) * (z2 / *zoom) as f32;
        *zoom = z2;
    };
    if resp.dragged() {
        pan += resp.drag_delta();
    }
    if resp.hovered() {
        let zd = ui.input(|i| i.zoom_delta()) as f64;
        if zd != 1.0 {
            let at = ui.input(|i| i.pointer.hover_pos()).unwrap_or(c);
            zoom_at(&mut zoom, &mut pan, zd, at);
        }
    }
    if resp.double_clicked() {
        (zoom, pan) = (1.0, Vec2::ZERO);
    }
    for (i, factor) in [(0, 2.0), (1, 0.5), (2, 0.0)] {
        if i == 2 && zoom == 1.0 && pan == Vec2::ZERO {
            continue;
        }
        let r = Rect::from_center_size(pos2(rect.right() - 22.0, rect.top() + 22.0 + i as f32 * 36.0), Vec2::splat(30.0));
        if ui.interact(r, id.with(i), Sense::click()).clicked() {
            if factor == 0.0 {
                (zoom, pan) = (1.0, Vec2::ZERO);
            } else {
                zoom_at(&mut zoom, &mut pan, factor, c);
            }
        }
    }
    ui.data_mut(|d| d.insert_temp(id, (zoom, pan)));
    View {
        s: base.s * zoom,
        ox: c.x as f64 + (base.ox - c.x as f64) * zoom + pan.x as f64,
        oy: c.y as f64 + (base.oy - c.y as f64) * zoom + pan.y as f64,
    }
}

/// Zoom in, zoom out and (once moved) back-to-route buttons in the map's corner.
fn zoom_buttons(ui: &Ui, p: &egui::Painter, rect: Rect, show_reset: bool) {
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
}

/// A route over the offline map (or the plain backdrop where there is none).
/// `interactive` maps can be dragged and zoomed.
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
    let view = if interactive { pan_zoom(ui, &resp, rect, base) } else { base };
    let dark = ui.visuals().dark_mode;
    let has_map = crate::offmap::draw(&p, rect, &view, dark);
    let line: Vec<Pos2> = pts.iter().map(|q| view.to_screen(*q)).collect();
    p.line(line.clone(), Stroke::new(10.0, color.gamma_multiply(0.22)));
    p.line(line.clone(), Stroke::new(3.5, color));
    p.circle(line[0], 6.0, GREEN, Stroke::new(2.5, Color32::WHITE));
    p.circle(*line.last().unwrap(), 6.0, RED, Stroke::new(2.5, Color32::WHITE));
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
        let moved = view.s != base.s || view.ox != base.ox || view.oy != base.oy;
        zoom_buttons(ui, &p, rect, moved);
    }
    resp
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
