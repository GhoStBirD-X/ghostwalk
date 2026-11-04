//! Game-style maps: the "Petualang" (storybook adventure) and "Pemburu" (expedition field map)
//! themes draw their own route trails, markers, frame and area banner over the offline map.
//! Every other theme returns false from these, and the plain look is drawn instead.

use crate::draw;
use crate::model::MapTheme;
use crate::offmap;
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};

const GOLD: Color32 = Color32::from_rgb(214, 170, 76);
const GOLD_DARK: Color32 = Color32::from_rgb(120, 82, 30);
const INK: Color32 = Color32::from_rgb(64, 36, 18);
const AMBER: Color32 = Color32::from_rgb(236, 170, 70);
const BONE: Color32 = Color32::from_rgb(232, 220, 186);
const SOOT: Color32 = Color32::from_rgb(24, 26, 20);

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Adventurer,
    Hunter,
}

fn look() -> Option<Look> {
    match offmap::theme() {
        MapTheme::Adventurer => Some(Look::Adventurer),
        MapTheme::Hunter => Some(Look::Hunter),
        _ => None,
    }
}

/// Points every `step` pixels along `line`, with the direction of travel there.
fn along(line: &[Pos2], step: f32, offset: f32) -> Vec<(Pos2, Vec2)> {
    let mut out = Vec::new();
    let mut next = offset;
    let mut walked = 0.0;
    for w in line.windows(2) {
        let d = w[1] - w[0];
        let len = d.length();
        if len <= 0.0 {
            continue;
        }
        let dir = d / len;
        while next <= walked + len {
            out.push((w[0] + dir * (next - walked), dir));
            next += step;
        }
        walked += len;
    }
    out
}

/// A planned route still to follow: dotted gold steps, or footprints.
pub fn plan_line(p: &Painter, line: &[Pos2]) -> bool {
    match look() {
        None => false,
        Some(Look::Adventurer) => {
            for (q, _) in along(line, 9.0, 4.0) {
                p.circle(q, 2.6, GOLD, Stroke::new(1.2, GOLD_DARK));
            }
            true
        }
        Some(Look::Hunter) => {
            footprints(p, line, AMBER.gamma_multiply(0.85));
            true
        }
    }
}

fn footprints(p: &Painter, line: &[Pos2], color: Color32) {
    for (i, (q, dir)) in along(line, 14.0, 6.0).into_iter().enumerate() {
        let side = vec2(-dir.y, dir.x) * if i % 2 == 0 { 3.8 } else { -3.8 };
        let c = q + side;
        // An oval print pointing along the way, with a toe dot ahead of it.
        let pts: Vec<Pos2> = (0..12)
            .map(|k| {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                c + dir * (a.cos() * 3.6) + vec2(-dir.y, dir.x) * (a.sin() * 2.2)
            })
            .collect();
        p.add(Shape::convex_polygon(pts, color, Stroke::new(0.8, SOOT.gamma_multiply(0.6))));
        p.circle_filled(c + dir * 5.6, 1.4, color);
    }
}

/// The route already run, or a route on its own (`color` is the sport's colour).
pub fn route_line(p: &Painter, line: &[Pos2], color: Color32) -> bool {
    match look() {
        None => false,
        Some(Look::Adventurer) => {
            p.line(line.to_vec(), Stroke::new(6.0, GOLD_DARK));
            p.line(line.to_vec(), Stroke::new(3.2, GOLD));
            let _ = color;
            true
        }
        Some(Look::Hunter) => {
            p.line(line.to_vec(), Stroke::new(8.0, AMBER.gamma_multiply(0.18)));
            p.line(line.to_vec(), Stroke::new(3.0, AMBER));
            true
        }
    }
}

/// Draws rows of "pixels" from a picture where each character picks a colour ('.' is empty).
fn sprite(p: &Painter, c: Pos2, px: f32, rows: &[&str], colors: &[(char, Color32)]) {
    let h = rows.len() as f32;
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0) as f32;
    let origin = c - vec2(w * px / 2.0, h * px / 2.0);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if let Some((_, col)) = colors.iter().find(|k| k.0 == ch) {
                let r = Rect::from_min_size(origin + vec2(x as f32 * px, y as f32 * px), vec2(px + 0.2, px + 0.2));
                p.rect_filled(r, 0.0, *col);
            }
        }
    }
}

/// Where the route starts.
pub fn start(p: &Painter, c: Pos2) -> bool {
    match look() {
        None => false,
        Some(Look::Adventurer) => {
            // A pennant on a pole.
            sprite(
                p,
                c - vec2(0.0, 8.0),
                2.0,
                &["kRRRR.", "kRRRRR", "kRRRR.", "kR....", "k.....", "k.....", "k.....", "kk...."],
                &[('k', INK), ('R', Color32::from_rgb(178, 44, 40))],
            );
            true
        }
        Some(Look::Hunter) => {
            // A camp tent.
            let base = c + vec2(0.0, 4.0);
            p.add(Shape::convex_polygon(
                vec![base + vec2(-8.0, 0.0), base + vec2(0.0, -11.0), base + vec2(8.0, 0.0)],
                Color32::from_rgb(196, 120, 60),
                Stroke::new(1.5, SOOT),
            ));
            p.add(Shape::convex_polygon(vec![base + vec2(-2.5, 0.0), base + vec2(0.0, -5.0), base + vec2(2.5, 0.0)], SOOT, Stroke::NONE));
            true
        }
    }
}

/// Where the route ends.
pub fn finish(p: &Painter, c: Pos2) -> bool {
    match look() {
        None => false,
        Some(Look::Adventurer) => {
            // A treasure chest.
            sprite(
                p,
                c,
                2.0,
                &[".kkkkkkk.", "kbbbbbbbk", "kbbbbbbbk", "kkkkYkkkk", "kbbbYbbbk", "kbbbbbbbk", "kkkkkkkkk"],
                &[('k', INK), ('b', Color32::from_rgb(150, 88, 40)), ('Y', GOLD)],
            );
            true
        }
        Some(Look::Hunter) => {
            target(p, c, 8.0, Color32::from_rgb(214, 70, 52));
            true
        }
    }
}

/// A quarry crest: a ring with three claw slashes.
fn target(p: &Painter, c: Pos2, r: f32, color: Color32) {
    p.circle(c, r, SOOT.gamma_multiply(0.85), Stroke::new(2.0, color));
    for k in -1..=1 {
        let x = k as f32 * r * 0.38;
        p.line_segment([c + vec2(x - r * 0.2, -r * 0.55), c + vec2(x + r * 0.2, r * 0.55)], Stroke::new(1.8, color));
    }
}

/// You, heading along `dir` (screen direction, may be zero).
pub fn me(p: &Painter, c: Pos2, dir: Vec2, color: Color32) -> bool {
    match look() {
        None => false,
        Some(Look::Adventurer) => {
            p.circle_filled(c + vec2(0.0, 9.0), 6.0, Color32::from_black_alpha(60));
            // A small hooded traveller with a walking staff.
            sprite(
                p,
                c,
                2.0,
                &[
                    "..ggg..s", ".ggggg.s", ".gfffg.s", ".gfefe.s", "..fff..s", ".ccccc.s", "cccccccs", ".ccccc.s", ".cc.cc..",
                    ".kk.kk..",
                ],
                &[
                    ('g', Color32::from_rgb(52, 92, 64)),
                    ('f', Color32::from_rgb(240, 200, 160)),
                    ('e', INK),
                    ('c', Color32::from_rgb(170, 52, 44)),
                    ('s', Color32::from_rgb(120, 80, 40)),
                    ('k', INK),
                ],
            );
            let _ = (dir, color);
            true
        }
        Some(Look::Hunter) => {
            let d = if dir.length() > 0.0 { dir.normalized() } else { vec2(0.0, -1.0) };
            let side = vec2(-d.y, d.x);
            // A soft cone of where you are heading, then the arrow.
            let cone: Vec<Pos2> = (0..=8)
                .map(|k| {
                    let a = (k as f32 / 8.0 - 0.5) * 1.1;
                    c + (d * a.cos() + side * a.sin()) * 30.0
                })
                .chain(std::iter::once(c))
                .collect();
            p.add(Shape::convex_polygon(cone, AMBER.gamma_multiply(0.22), Stroke::NONE));
            p.add(Shape::convex_polygon(
                vec![c + d * 10.0, c - d * 6.0 + side * 7.0, c - d * 2.5, c - d * 6.0 - side * 7.0],
                BONE,
                Stroke::new(1.8, SOOT),
            ));
            true
        }
    }
}

/// The ghost being raced.
pub fn ghost(p: &Painter, c: Pos2) -> bool {
    match look() {
        None => false,
        Some(Look::Adventurer) => {
            sprite(
                p,
                c,
                2.0,
                &[".wwww.", "wwwwww", "wkwwkw", "wwwwww", "wwwwww", "w.ww.w"],
                &[('w', Color32::from_rgb(214, 206, 255)), ('k', INK)],
            );
            p.text(c - vec2(0.0, 9.0), Align2::CENTER_BOTTOM, "hantu", FontId::new(12.0, draw::serif()), INK);
            true
        }
        Some(Look::Hunter) => {
            target(p, c, 9.0, draw::GHOST);
            p.text(c - vec2(0.0, 12.0), Align2::CENTER_BOTTOM, "BURUAN", FontId::new(10.5, draw::heavy()), draw::GHOST);
            true
        }
    }
}

/// Over a finished map: the paper or survey grid, the frame, and the name of the area.
pub fn decorate(p: &Painter, rect: Rect, has_map: bool) {
    let Some(look) = look() else { return };
    match look {
        Look::Adventurer => {
            // Paper grain, fixed to the sheet.
            let mut seed: u32 = 0x9e37_79b9;
            for _ in 0..(rect.area() / 260.0) as usize {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let x = rect.left() + (seed % 10_000) as f32 / 10_000.0 * rect.width();
                let y = rect.top() + ((seed / 10_000) % 10_000) as f32 / 10_000.0 * rect.height();
                p.circle_filled(pos2(x, y), 0.7, INK.gamma_multiply(0.08));
            }
            // Darker edges, like old paper.
            for i in 0..8 {
                let r = rect.shrink(i as f32 * 2.0);
                p.rect_stroke(r, 10.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(90, 56, 24, 30 - i * 3)), egui::StrokeKind::Inside);
            }
            // A gold double frame with diamond corners.
            p.rect_stroke(rect.shrink(3.0), 8.0, Stroke::new(2.5, GOLD), egui::StrokeKind::Inside);
            p.rect_stroke(rect.shrink(7.0), 6.0, Stroke::new(1.0, GOLD_DARK), egui::StrokeKind::Inside);
            for c in [rect.left_top() + vec2(7.0, 7.0), rect.right_top() + vec2(-7.0, 7.0), rect.left_bottom() + vec2(7.0, -7.0), rect.right_bottom() + vec2(-7.0, -7.0)] {
                diamond(p, c, 5.0, GOLD, GOLD_DARK);
            }
        }
        Look::Hunter => {
            // A survey grid with zone letters and numbers.
            let step = 64.0;
            let grid = Stroke::new(1.0, BONE.gamma_multiply(0.10));
            let font = FontId::new(9.5, draw::semi());
            let mut x = rect.left() + step;
            let mut col = 0u8;
            while x < rect.right() {
                p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], grid);
                p.text(pos2(x - step / 2.0, rect.bottom() - 4.0), Align2::CENTER_BOTTOM, ((b'A' + col) as char).to_string(), font.clone(), BONE.gamma_multiply(0.5));
                x += step;
                col += 1;
            }
            let mut y = rect.top() + step;
            let mut row = 1;
            while y < rect.bottom() {
                p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], grid);
                p.text(pos2(rect.left() + 5.0, y - step / 2.0), Align2::LEFT_CENTER, row.to_string(), font.clone(), BONE.gamma_multiply(0.5));
                y += step;
                row += 1;
            }
            // Corner brackets.
            let s = Stroke::new(3.0, BONE.gamma_multiply(0.85));
            let l = 18.0;
            let r = rect.shrink(5.0);
            for (c, dx, dy) in [(r.left_top(), 1.0, 1.0), (r.right_top(), -1.0, 1.0), (r.left_bottom(), 1.0, -1.0), (r.right_bottom(), -1.0, -1.0)] {
                p.line(vec![c + vec2(dx * l, 0.0), c, c + vec2(0.0, dy * l)], s);
            }
        }
    }
    let area = if has_map { offmap::area_name() } else { String::new() };
    if !area.is_empty() {
        banner(p, rect, look, &area);
    }
}

fn diamond(p: &Painter, c: Pos2, r: f32, fill: Color32, edge: Color32) {
    p.add(Shape::convex_polygon(vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)], fill, Stroke::new(1.0, edge)));
}

/// The name of the area at the top of the map.
fn banner(p: &Painter, rect: Rect, look: Look, name: &str) {
    match look {
        Look::Adventurer => {
            let font = FontId::new(15.0, draw::serif());
            let g = p.layout_no_wrap(name.to_string(), font, Color32::from_rgb(250, 236, 200));
            let w = (g.size().x + 34.0).min(rect.width() - 30.0);
            let top = rect.top() + 12.0;
            let r = Rect::from_center_size(pos2(rect.center().x, top + 13.0), vec2(w, 24.0));
            // A ribbon with folded tails.
            for side in [-1.0f32, 1.0] {
                let x = if side < 0.0 { r.left() } else { r.right() };
                p.add(Shape::convex_polygon(
                    vec![pos2(x, r.top() + 4.0), pos2(x + side * 14.0, r.top() + 4.0), pos2(x + side * 8.0, r.center().y + 2.0), pos2(x + side * 14.0, r.bottom() + 4.0), pos2(x, r.bottom() + 4.0)],
                    Color32::from_rgb(110, 30, 28),
                    Stroke::new(1.0, GOLD_DARK),
                ));
            }
            p.rect(r, 2.0, Color32::from_rgb(150, 42, 38), Stroke::new(1.5, GOLD), egui::StrokeKind::Inside);
            p.galley(pos2(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
        }
        Look::Hunter => {
            let font = FontId::new(12.0, draw::heavy());
            let text = name.to_uppercase();
            let g = p.layout_no_wrap(text, font, BONE);
            let r = Rect::from_min_size(rect.left_top() + vec2(12.0, 12.0), vec2(g.size().x + 30.0, 22.0));
            p.rect(r, 3.0, SOOT.gamma_multiply(0.85), Stroke::new(1.0, AMBER.gamma_multiply(0.7)), egui::StrokeKind::Inside);
            diamond(p, pos2(r.left() + 11.0, r.center().y), 4.0, AMBER, SOOT);
            p.galley(pos2(r.left() + 20.0, r.center().y - g.size().y / 2.0), g, BONE);
        }
    }
}
