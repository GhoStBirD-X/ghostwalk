//! Moments around activities: celebrating records, undoing a delete, the welcome screens,
//! and the picture card for sharing an activity.

use crate::app::{GhostApp, MapMode, Tab};
use crate::draw::{self, ACCENT};
use crate::model::{self, Activity};
use eframe::egui::{self, Align2, Color32, FontId, Id, LayerId, Order, Pos2, Rect, RichText, Stroke, Ui, pos2, vec2};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The share picture being shown, and its result once saved.
pub struct ShareCard {
    pub id: u64,
    result: Option<Arc<Mutex<Option<String>>>>,
}

impl ShareCard {
    pub fn new(id: u64) -> Self {
        Self { id, result: None }
    }
}

const BIG_EFFORTS: [(f64, &str); 4] = [(1000.0, "1K"), (5000.0, "5K"), (10_000.0, "10K"), (21_097.5, "setengah maraton")];

impl GhostApp {
    /// What `a` achieves compared with everything already saved: new bests and goals reached.
    pub fn wins(&self, a: &Activity) -> Vec<String> {
        let mut wins = Vec::new();
        let others: Vec<&Activity> = self.data.activities.iter().filter(|b| b.sport == a.sport && b.id != a.id).collect();
        if a.sport.has_distance() && !others.is_empty() {
            for (d, name) in BIG_EFFORTS {
                if let Some(t) = a.best_effort(d) {
                    let prev = others.iter().filter_map(|b| b.best_effort(d)).fold(f64::MAX, f64::min);
                    if prev < f64::MAX && t < prev - 0.5 {
                        wins.push(format!("Rekor {name} baru: {}", draw::clock(t)));
                    }
                }
            }
            let longest = others.iter().map(|b| b.distance_m).fold(0.0, f64::max);
            if a.distance_m >= 1000.0 && a.distance_m > longest {
                wins.push(format!(
                    "{} terjauh: {}",
                    a.sport.name(),
                    draw::dist(a.distance_m, self.data.settings.imperial)
                ));
            }
            let route = self.data.same_route(a);
            if let Some(best) = route.iter().map(|b| b.duration_s).reduce(f64::min) {
                if a.duration_s < best - 1.0 {
                    wins.push(format!("Tercepat di rute ini, {} lebih cepat", draw::clock(best - a.duration_s)));
                }
            }
        }
        for g in &self.data.goals {
            let before = self.data.goal_progress(g);
            let after = before + model::Data::goal_share(g, a);
            if before < g.target && after >= g.target {
                wins.push(format!(
                    "Target tercapai: {} {}",
                    g.sport.map(|s| s.name()).unwrap_or("Semua olahraga"),
                    g.period.name().to_lowercase()
                ));
            }
        }
        wins
    }

    /// A card with confetti after saving an activity that set a record or reached a goal.
    pub fn celebration(&mut self, ctx: &egui::Context) {
        let Some((lines, since)) = &self.celebrate else { return };
        let t = since.elapsed().as_secs_f32();
        if t > 8.0 {
            self.celebrate = None;
            return;
        }
        ctx.request_repaint();
        let ease = 1.0 - (1.0 - (t / 0.35).min(1.0)).powi(3);
        let screen = ctx.content_rect();
        ctx.layer_painter(LayerId::new(Order::Middle, Id::unique("celebrate_dim")))
            .rect_filled(screen, 0.0, Color32::from_black_alpha((150.0 * ease) as u8));
        let only_goals = lines.iter().all(|l| l.starts_with("Target"));
        let lines = lines.clone();
        let mut close = false;
        egui::Area::new(Id::unique("celebrate"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 20.0 * (1.0 - ease)))
            .show(ctx, |ui| {
                ui.multiply_opacity(ease);
                egui::Frame::new()
                    .fill(draw::card_bg(ui))
                    .stroke(Stroke::new(1.5, draw::lime(ui)))
                    .corner_radius(26.0)
                    .inner_margin(24.0)
                    .show(ui, |ui| {
                        ui.set_width(280.0);
                        ui.vertical_centered(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(64.0, 64.0), egui::Sense::hover());
                            ui.painter().circle_filled(r.center(), 32.0, draw::lime(ui).gamma_multiply(0.18));
                            big_star(ui.painter(), r.center(), 22.0, draw::lime(ui));
                            ui.label(draw::big(if only_goals { "Target tercapai!" } else { "Rekor baru!" }, 28.0, draw::lime(ui)));
                            ui.add_space(6.0);
                            for l in &lines {
                                ui.label(RichText::new(l).family(draw::semi()).size(15.0).color(draw::strong(ui)));
                            }
                            ui.add_space(10.0);
                            if draw::accent_button(ui, "Mantap!").clicked() {
                                close = true;
                            }
                        });
                    });
            });
        // Confetti falling from the top.
        let p = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("confetti")));
        let colors = [ACCENT, Color32::from_rgb(198, 255, 61), Color32::from_rgb(60, 140, 230), Color32::WHITE];
        for i in 0..44u32 {
            let seed = (i.wrapping_mul(2_654_435_761) >> 8) as f32 / 16_777_216.0;
            let x = screen.left() + screen.width() * ((i as f32 * 0.618 + seed) % 1.0);
            let y = screen.top() - 20.0 + (t * (160.0 + 140.0 * seed) + seed * 300.0) % (screen.height() + 40.0);
            let sway = (t * 3.0 + i as f32).sin() * 10.0;
            let r = Rect::from_center_size(pos2(x + sway, y), vec2(6.0, 10.0));
            p.rect_filled(r, 2.0, colors[i as usize % colors.len()].gamma_multiply((1.6 - t / 5.0).clamp(0.0, 1.0)));
        }
        if close {
            self.celebrate = None;
        }
    }

    /// Deletes an activity, keeping it for a few seconds so it can be brought back.
    pub fn delete_activity(&mut self, id: u64) {
        if let Some(i) = self.data.activities.iter().position(|a| a.id == id) {
            let a = self.data.activities.remove(i);
            self.undo = Some((a, Instant::now()));
            self.detail = None;
            self.edit = None;
            self.save();
        }
    }

    /// "Aktivitas dihapus · Urungkan" for six seconds after a delete. `lift` keeps it above the tabs.
    pub fn undo_toast(&mut self, ctx: &egui::Context, lift: f32) {
        let Some((_, since)) = &self.undo else { return };
        if since.elapsed().as_secs_f32() > 6.0 {
            self.undo = None;
            return;
        }
        ctx.request_repaint();
        let mut restore = false;
        egui::Area::new(Id::unique("undo"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -86.0 - lift))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(34, 34, 40))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(52, 52, 60)))
                    .corner_radius(22.0)
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Aktivitas dihapus").color(Color32::WHITE));
                            ui.add_space(8.0);
                            let b = egui::Button::new(
                                RichText::new("Urungkan").family(draw::semi()).color(Color32::from_rgb(198, 255, 61)),
                            )
                            .frame(false);
                            if ui.add(b).clicked() {
                                restore = true;
                            }
                        });
                    });
            });
        if restore {
            if let Some((mut a, _)) = self.undo.take() {
                if self.data.activities.iter().any(|b| b.id == a.id) {
                    a.id = self.data.new_id();
                }
                self.data.activities.push(a);
                self.save();
                self.say("Aktivitas dikembalikan");
            }
        }
    }

    /// Adds the sample activities next to the user's own gear, goals and routes.
    pub fn load_samples(&mut self) {
        self.data.add_samples();
        self.save();
    }

    fn finish_onboarding(&mut self) {
        self.data.settings.onboarded = true;
        self.save();
    }

    /// Three welcome screens on the first start: privacy, permissions, and a map for your area.
    pub fn onboarding(&mut self, ui: &mut Ui) {
        ui.add_space(24.0);
        let step = self.onboard_step;
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(110.0, 110.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 54.0, ACCENT.gamma_multiply(0.14));
            match step {
                0 => draw::ghost(ui.painter(), r.center(), 70.0, ACCENT, draw::bg(ui)),
                1 => draw::sport_icon(ui.painter(), model::Sport::Run, r.center(), 56.0, ACCENT),
                _ => draw::icon(ui.painter(), draw::Icon::Map, r.center(), ACCENT),
            }
            ui.add_space(10.0);
            let (title, sub) = match step {
                0 => ("Selamat datang di GhostWalk", "Pencatat olahraga yang pribadi dan offline."),
                1 => ("Izin yang dipakai", "Hanya untuk merekam di HP ini. Tidak ada yang dikirim ke mana pun."),
                _ => ("Peta wilayahmu", "Unduh peta tempat kamu biasa olahraga, lalu pakai tanpa internet."),
            };
            ui.label(draw::big(title, 28.0, draw::strong(ui)));
            ui.label(RichText::new(sub).size(15.0).color(draw::muted(ui)));
        });
        ui.add_space(18.0);
        let points: &[(&str, &str)] = match step {
            0 => &[
                ("Tanpa akun", "Tidak perlu email, nomor HP, atau tanggal lahir."),
                ("Data hanya di perangkat ini", "Tidak ada server. Cadangkan sendiri kapan pun."),
                ("Tanpa iklan dan pelacak", "Internet hanya dipakai saat kamu mengunduh peta."),
            ],
            1 if cfg!(target_os = "android") => &[
                ("Lokasi", "Untuk menggambar rute dan menghitung jarak."),
                ("Notifikasi", "Agar rekaman tetap jalan saat layar mati."),
                ("Aktivitas fisik", "Untuk menghitung langkah."),
            ],
            1 => &[("Di komputer tidak perlu izin", "Rekam dipakai sebagai stopwatch; rute bisa diimpor dari file GPX.")],
            _ => &[
                ("Per kecamatan atau kota", "Pilih beberapa wilayah sekaligus kalau sering keluar kota."),
                ("Sekali unduh", "Setelah itu peta dipakai tanpa internet."),
            ],
        };
        for (title, text) in points {
            draw::card(ui, |ui| {
                ui.label(RichText::new(*title).family(draw::semi()).size(16.0).color(draw::strong(ui)));
                ui.label(RichText::new(*text).size(13.5).color(draw::muted(ui)));
            });
            ui.add_space(8.0);
        }
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            match step {
                0 => {
                    if draw::accent_button(ui, "Lanjut").clicked() {
                        self.onboard_step = 1;
                    }
                }
                1 => {
                    if cfg!(target_os = "android") {
                        if draw::accent_button(ui, "Izinkan").clicked() {
                            #[cfg(target_os = "android")]
                            crate::android::ask_permissions();
                            self.onboard_step = 2;
                        }
                        if ui.button("Nanti saja").clicked() {
                            self.onboard_step = 2;
                        }
                    } else if draw::accent_button(ui, "Lanjut").clicked() {
                        self.onboard_step = 2;
                    }
                }
                _ => {
                    if draw::accent_button(ui, "Unduh peta sekarang").clicked() {
                        self.finish_onboarding();
                        self.tab = Tab::Maps;
                        self.map_mode = MapMode::Offline;
                    }
                    ui.add_space(4.0);
                    if ui.button("Coba dengan data contoh").clicked() {
                        self.load_samples();
                        self.finish_onboarding();
                    }
                    if ui.button("Mulai kosong").clicked() {
                        self.finish_onboarding();
                    }
                }
            }
            ui.add_space(12.0);
            let (r, _) = ui.allocate_exact_size(vec2(60.0, 10.0), egui::Sense::hover());
            for i in 0..3 {
                let c = pos2(r.left() + 10.0 + i as f32 * 20.0, r.center().y);
                let on = i == step;
                ui.painter().circle_filled(c, if on { 5.0 } else { 3.5 }, if on { ACCENT } else { draw::border(ui) });
            }
        });
    }

    /// The picture card for sharing an activity, with buttons under it.
    pub fn share_overlay(&mut self, ctx: &egui::Context) {
        let Some(card) = &self.share else { return };
        if let Some(msg) = card.result.as_ref().and_then(|r| r.lock().unwrap().take()) {
            self.share = None;
            self.say(msg);
            return;
        }
        let saving = card.result.is_some();
        let Some(a) = self.data.activities.iter().find(|a| a.id == card.id).cloned() else {
            self.share = None;
            return;
        };
        let screen = ctx.content_rect();
        ctx.layer_painter(LayerId::new(Order::Middle, Id::unique("share_dim")))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(190));
        let w = (screen.width() - 32.0).min(360.0).min((screen.height() - 150.0) * 9.0 / 16.0);
        let rect = Rect::from_center_size(screen.center() - vec2(0.0, 36.0), vec2(w, w * 16.0 / 9.0));
        let imp = self.data.settings.imperial;
        egui::Area::new(Id::unique("share_card"))
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| paint_share_card(ui, rect, &a, imp));
        let mut share = false;
        let mut close = false;
        egui::Area::new(Id::unique("share_buttons"))
            .order(Order::Foreground)
            .fixed_pos(pos2(rect.left(), rect.bottom() + 14.0))
            .show(ctx, |ui| {
                ui.set_width(rect.width());
                ui.horizontal(|ui| {
                    if saving {
                        ui.spinner();
                        ui.label(RichText::new("Menyimpan gambar…").color(Color32::WHITE));
                    } else {
                        let label = if cfg!(target_os = "android") { "Bagikan" } else { "Simpan gambar" };
                        if draw::accent_button(ui, label).clicked() {
                            share = true;
                        }
                        if ui.button("Tutup").clicked() {
                            close = true;
                        }
                    }
                });
            });
        if close {
            self.share = None;
            return;
        }
        if share {
            let result = Arc::new(Mutex::new(None));
            let out = result.clone();
            let ppp = ctx.pixels_per_point();
            let path = self.export_dir().join(format!("ghostwalk-{}.png", a.date().format("%Y%m%d-%H%M")));
            ctx.request_screenshot(move |img| {
                let msg = save_crop(&img, rect, ppp, &path);
                *out.lock().unwrap() = Some(msg);
            });
            if let Some(c) = &mut self.share {
                c.result = Some(result);
            }
        }
        ctx.request_repaint();
    }
}

/// Saves the card's part of a screenshot as a PNG and, on phones, puts it in the gallery and opens sharing.
fn save_crop(img: &egui::ColorImage, rect: Rect, ppp: f32, path: &std::path::Path) -> String {
    let [w, h] = img.size;
    let x0 = ((rect.left() * ppp).round().max(0.0) as usize).min(w);
    let y0 = ((rect.top() * ppp).round().max(0.0) as usize).min(h);
    let x1 = ((rect.right() * ppp).round() as usize).clamp(x0, w);
    let y1 = ((rect.bottom() * ppp).round() as usize).clamp(y0, h);
    let mut buf = image::RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
    for y in y0..y1 {
        for x in x0..x1 {
            let c = img.pixels[y * w + x];
            buf.put_pixel((x - x0) as u32, (y - y0) as u32, image::Rgba(c.to_array()));
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = buf.save(path) {
        return format!("Gagal menyimpan gambar: {e}");
    }
    #[cfg(target_os = "android")]
    if let Some(place) = crate::android::publish(path, "image/png", true) {
        return format!("Gambar disimpan di {place}");
    }
    format!("Gambar disimpan ke {}", path.display())
}

fn big_star(p: &egui::Painter, c: Pos2, r: f32, color: Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    for i in 0..10 {
        let rr = if i % 2 == 0 { r } else { r * 0.42 };
        let a = (-90.0 + 36.0 * i as f32).to_radians();
        mesh.colored_vertex(c + vec2(a.cos(), a.sin()) * rr, color);
    }
    for i in 0..10u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
    }
    p.add(mesh);
}

/// The 9:16 picture: route on the map, the big numbers, and the GhostWalk mark.
fn paint_share_card(ui: &mut Ui, rect: Rect, a: &Activity, imp: bool) {
    let p = ui.painter().clone();
    let top = Color32::from_rgb(20, 20, 26);
    let bottom = Color32::from_rgb(74, 32, 14);
    let mut mesh = egui::Mesh::default();
    for (pt, c) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.colored_vertex(pt, c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(mesh);
    let s = rect.width() / 360.0;
    let pad = 20.0 * s;
    let white = Color32::from_rgb(246, 246, 248);
    let soft = Color32::from_rgb(170, 170, 182);

    draw::ghost(&p, rect.left_top() + vec2(pad + 12.0 * s, pad + 14.0 * s), 26.0 * s, ACCENT, top);
    p.text(
        rect.left_top() + vec2(pad + 32.0 * s, pad + 14.0 * s),
        Align2::LEFT_CENTER,
        "GhostWalk",
        FontId::new(20.0 * s, draw::heavy()),
        white,
    );

    let map = Rect::from_min_size(rect.left_top() + vec2(pad, 62.0 * s), vec2(rect.width() - 2.0 * pad, rect.width() * 0.92));
    if a.track.len() > 1 {
        ui.scope_builder(egui::UiBuilder::new().max_rect(map), |ui| {
            draw::route_map(ui, &a.track, ACCENT, map.height(), false, top);
        });
    } else {
        p.rect_filled(map, 16.0 * s, Color32::from_rgb(30, 30, 36));
        draw::sport_icon(&p, a.sport, map.center(), 90.0 * s, draw::sport_color(a.sport));
    }

    let mut y = map.bottom() + 22.0 * s;
    p.text(pos2(rect.left() + pad, y), Align2::LEFT_TOP, &a.title, FontId::new(24.0 * s, draw::heavy()), white);
    y += 32.0 * s;
    p.text(
        pos2(rect.left() + pad, y),
        Align2::LEFT_TOP,
        format!("{} · {}", draw::date_long(&a.date()), a.sport.name()),
        FontId::new(13.0 * s, draw::semi()),
        soft,
    );
    y += 34.0 * s;
    let mut stats = Vec::new();
    if a.sport.has_distance() {
        stats.push(("Jarak", draw::dist(a.distance_m, imp)));
    }
    stats.push(("Waktu", draw::clock(a.duration_s)));
    if a.sport.has_distance() {
        let (l, v) = draw::pace_or_speed(a.sport, a.speed_mps(), imp);
        stats.push((l, v));
    } else {
        stats.push(("Usaha", format!("{}/10", a.effort)));
    }
    let col = (rect.width() - 2.0 * pad) / stats.len() as f32;
    for (i, (label, value)) in stats.iter().enumerate() {
        let x = rect.left() + pad + col * i as f32;
        p.text(pos2(x, y), Align2::LEFT_TOP, *label, FontId::new(12.0 * s, draw::semi()), soft);
        p.text(pos2(x, y + 16.0 * s), Align2::LEFT_TOP, value, FontId::new(24.0 * s, draw::heavy()), white);
    }
    p.text(
        pos2(rect.center().x, rect.bottom() - pad),
        Align2::CENTER_BOTTOM,
        "offline · pribadi · tanpa akun",
        FontId::new(11.0 * s, draw::semi()),
        soft,
    );
}
