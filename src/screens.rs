//! Maps, goals and the "You" section with its pages.

use crate::app::{GhostApp, MapMode, Pick, Sub, Tab, hr_zone, parse_time};
use crate::draw::{self, ACCENT};
use crate::gpx;
use crate::mapdl::{self, Status};
use crate::offmap;
use crate::model::{self, Gear, Metric, Period, Route, Sport};
use chrono::{Datelike, Local, NaiveDate, Timelike};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, Ui, pos2, vec2};

impl GhostApp {
    // ---------- maps ----------

    pub fn maps(&mut self, ui: &mut Ui) {
        self.top_bar(ui, "Peta");
        ui.horizontal_wrapped(|ui| {
            for (m, l) in [
                (MapMode::Routes, "Rute saya"),
                (MapMode::Plan, "Buat rute"),
                (MapMode::Heat, "Peta jejak"),
                (MapMode::Offline, "Peta offline"),
            ] {
                if draw::chip(ui, self.map_mode == m, l).clicked() {
                    self.map_mode = m;
                }
            }
        });
        ui.add_space(6.0);
        match self.map_mode {
            MapMode::Routes => self.routes_list(ui),
            MapMode::Plan => self.route_planner(ui),
            MapMode::Heat => self.heatmap(ui),
            MapMode::Offline => self.offline_maps(ui),
        }
    }

    fn routes_list(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        if self.data.routes.is_empty() {
            draw::card(ui, |ui| {
                ui.label("Belum ada rute. Buat rute sendiri di \"Buat rute\".");
            });
        }
        let mut delete = None;
        let mut use_route = None;
        for r in self.data.routes.clone() {
            draw::card(ui, |ui| {
                ui.horizontal(|ui| {
                    draw::sport_badge(ui, r.sport, 28.0);
                    ui.label(RichText::new(&r.name).size(15.0).family(draw::semi()).color(draw::strong(ui)));
                });
                if !r.geo.is_empty() {
                    let around = draw::card_bg(ui);
                    draw::route_map(ui, &r.track(), draw::sport_color(r.sport), 150.0, false, around);
                } else {
                    // Routes from before they had a place: only their shape is known.
                    let pts: Vec<[f64; 2]> = r.points.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 130.0), Sense::hover());
                    let p = ui.painter_at(rect);
                    draw::map_backdrop(ui, &p, rect, 10.0);
                    if pts.len() > 1 {
                        let f = draw::fit(&[&pts], rect, 16.0);
                        let line: Vec<Pos2> = pts.iter().map(|q| f(*q)).collect();
                        p.line(line.clone(), Stroke::new(3.0, draw::sport_color(r.sport)));
                        p.circle_filled(line[0], 4.5, draw::GREEN);
                    }
                    ui.label(RichText::new("Rute lama: belum punya lokasi di peta.").size(11.5).color(draw::muted(ui)));
                }
                let d = r.distance_m();
                let speed = self.avg_speed(r.sport);
                ui.horizontal(|ui| {
                    draw::stat(ui, "Jarak", &draw::dist(d, imp));
                    ui.add_space(20.0);
                    draw::stat(ui, "Perkiraan waktu", &draw::dur(d / speed));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new("Hapus").color(draw::RED)).clicked() {
                            delete = Some(r.id);
                        }
                    });
                });
                if !r.geo.is_empty() {
                    let runs = self.data.ghost_choices(r.sport, Some(&r));
                    let best = runs.first().map(|a| (a.id, a.duration_s));
                    let n = runs.len();
                    if let Some((_, t)) = best {
                        ui.label(
                            RichText::new(format!("Sudah {n} kali dilari. Terbaik {}.", draw::clock(t)))
                                .size(12.5)
                                .color(draw::muted(ui)),
                        );
                    }
                    let text = if best.is_some() { "Lari rute ini lawan hantu terbaik" } else { "Lari rute ini" };
                    if draw::accent_button(ui, text).clicked() {
                        use_route = Some((r.sport, r.id, best.map(|b| b.0)));
                    }
                }
            });
            ui.add_space(8.0);
        }
        if let Some(id) = delete {
            self.data.routes.retain(|r| r.id != id);
            self.save();
        }
        if let Some((sport, id, ghost)) = use_route {
            self.race(sport, Some(id), ghost);
        }
    }

    /// Average speed for a sport from past activities, or a sensible default.
    fn avg_speed(&self, sport: Sport) -> f64 {
        let (d, t) = self
            .data
            .activities
            .iter()
            .filter(|a| a.sport == sport && a.distance_m > 0.0)
            .fold((0.0, 0.0), |(d, t), a| (d + a.distance_m, t + a.duration_s));
        if t > 0.0 {
            d / t
        } else {
            match sport {
                Sport::Ride => 7.0,
                Sport::Walk | Sport::Hike => 1.3,
                Sport::Swim => 0.7,
                _ => 2.9,
            }
        }
    }

    /// Where the route planner starts: the place chosen, the route so far, the latest activity,
    /// an installed map, or else central Jakarta.
    fn plan_start(&self) -> (f64, f64) {
        self.plan_center
            .or_else(|| self.draft.first().map(|p| (p[0], p[1])))
            .or_else(|| self.data.activities.iter().find_map(|a| a.track.first().map(|p| (p.lat, p.lon))))
            .or_else(offmap::first_center)
            .unwrap_or((-6.1754, 106.8272))
    }

    /// Moves the planner's map to `at`; with no points yet, `at` also becomes the start.
    fn plan_go(&mut self, at: (f64, f64), as_start: bool) {
        self.plan_center = Some(at);
        if as_start && self.draft.is_empty() {
            self.draft.push([at.0, at.1]);
        }
    }

    fn route_planner(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        draw::card(ui, |ui| {
            ui.label(RichText::new("Mulai dari mana?").size(14.0).family(draw::semi()).color(draw::strong(ui)));
            #[cfg(target_os = "android")]
            if ui.button("Lokasi saya").clicked() {
                // While recording the newest GPS point is best; otherwise what the phone last knew.
                let here = self
                    .rec
                    .track
                    .last()
                    .map(|p| (p.lat, p.lon))
                    .or_else(crate::android::last_location);
                match here {
                    Some(at) => self.plan_go(at, true),
                    None => self.say("Lokasi belum diketahui. Izinkan dan nyalakan Lokasi, lalu coba lagi."),
                }
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.plan_search)
                    .hint_text("Cari kecamatan/kota, atau koordinat -6.24, 106.99")
                    .desired_width(f32::INFINITY),
            );
            let mut go = None;
            if let Some(at) = mapdl::parse_coords(&self.plan_search) {
                if ui.button(format!("Mulai di {:.5}, {:.5}", at.0, at.1)).clicked() {
                    go = Some((at, true));
                }
            } else {
                for place in mapdl::search(&self.plan_search, 5) {
                    let text = format!("{} · {}", place.name, place.context);
                    if ui.button(RichText::new(text).size(12.5)).clicked() {
                        // A place name only says roughly where, so it moves the map but sets no start.
                        go = Some(((place.lat, place.lon), false));
                    }
                }
            }
            if let Some((at, as_start)) = go {
                self.plan_go(at, as_start);
                self.plan_search.clear();
            }
            ui.add_space(4.0);
            let turn_hint = if cfg!(target_os = "android") { "putar dengan dua jari" } else { "klik kanan + seret untuk memutar" };
            ui.label(
                RichText::new(format!(
                    "Ketuk peta untuk menambah titik; titik hijau adalah start. Geser dan zoom untuk melihat sekitar, {turn_hint}."
                ))
                .size(12.0)
                .color(draw::muted(ui)),
            );
            let center = self.plan_start();
            let around = draw::card_bg(ui);
            if let Some(at) = draw::plan_map(ui, center, &self.draft, draw::sport_color(self.draft_sport), around) {
                if self.plan_center.is_none() {
                    // Keep the map where it is now that the route has started.
                    self.plan_center = Some(center);
                }
                self.draft.push([at.0, at.1]);
            }
            let d = Route {
                id: 0,
                name: String::new(),
                sport: self.draft_sport,
                points: Vec::new(),
                geo: self.draft.clone(),
            }
            .distance_m();
            ui.horizontal(|ui| {
                draw::stat(ui, "Jarak", &draw::dist(d, imp));
                ui.add_space(20.0);
                draw::stat(ui, "Perkiraan waktu", &draw::dur(d / self.avg_speed(self.draft_sport)));
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Batalkan titik").clicked() {
                    self.draft.pop();
                }
                if ui.button("Tutup putaran").clicked() && self.draft.len() > 1 {
                    let first = self.draft[0];
                    self.draft.push(first);
                }
                if ui.button("Kosongkan").clicked() {
                    self.draft.clear();
                }
            });
            ui.horizontal_wrapped(|ui| {
                for s in [Sport::Run, Sport::Ride, Sport::Walk, Sport::Hike] {
                    if draw::chip(ui, self.draft_sport == s, s.name()).clicked() {
                        self.draft_sport = s;
                    }
                }
            });
            ui.add(egui::TextEdit::singleline(&mut self.draft_name).hint_text("Nama rute").desired_width(f32::INFINITY));
            if draw::accent_button(ui, "Simpan rute").clicked() {
                if self.draft.len() < 2 {
                    self.say("Tambahkan minimal dua titik");
                } else {
                    let id = self.data.new_id();
                    let name = if self.draft_name.trim().is_empty() {
                        format!("Rute {}", self.data.routes.len() + 1)
                    } else {
                        self.draft_name.trim().to_string()
                    };
                    self.data.routes.push(Route {
                        id,
                        name,
                        sport: self.draft_sport,
                        points: Vec::new(),
                        geo: std::mem::take(&mut self.draft),
                    });
                    self.draft_name.clear();
                    self.save();
                    self.say("Rute disimpan");
                    self.map_mode = MapMode::Routes;
                }
            }
        });
    }

    fn heatmap(&mut self, ui: &mut Ui) {
        let tracks: Vec<(Sport, Vec<[f64; 2]>)> = self
            .data
            .activities
            .iter()
            .filter(|a| a.track.len() > 1)
            .map(|a| (a.sport, draw::merc(&a.track)))
            .collect();
        draw::card(ui, |ui| {
            ui.label(
                RichText::new(format!("Semua jejak Anda ({} aktivitas) di satu peta.", tracks.len()))
                    .size(12.0)
                    .color(draw::muted(ui)),
            );
            let side = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(vec2(side, side * 1.1), Sense::hover());
            let p = ui.painter_at(rect);
            p.rect_filled(rect, 10.0, Color32::from_rgb(22, 24, 30));
            if !tracks.is_empty() {
                let refs: Vec<&[[f64; 2]]> = tracks.iter().map(|t| t.1.as_slice()).collect();
                let v = draw::fit_view(&refs, rect, 16.0);
                if offmap::draw(&p, rect, &v, true) {
                    p.rect_filled(rect, 0.0, Color32::from_black_alpha(110));
                    offmap::attribution(&p, rect, true);
                }
                for (s, t) in &tracks {
                    let line: Vec<Pos2> = t.iter().map(|q| v.to_screen(*q)).collect();
                    p.line(line.clone(), Stroke::new(5.0, draw::sport_color(*s).gamma_multiply(0.18)));
                    p.line(line, Stroke::new(1.6, Color32::from_rgb(255, 200, 120).gamma_multiply(0.8)));
                }
            }
            ui.horizontal_wrapped(|ui| {
                for s in Sport::ALL {
                    let n = tracks.iter().filter(|t| t.0 == s).count();
                    if n > 0 {
                        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 5.0, draw::sport_color(s));
                        ui.label(RichText::new(format!("{} ({n})", s.name())).size(12.0));
                    }
                }
            });
        });
    }

    // ---------- goals ----------

    pub fn goals(&mut self, ui: &mut Ui) {
        self.top_bar(ui, "Target");
        draw::heading(ui, "Target pribadi");
        let mut delete = None;
        for g in self.data.goals.clone() {
            let v = self.data.goal_progress(&g);
            draw::card(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!("{} · {}", g.sport.map(|s| s.name()).unwrap_or("Semua olahraga"), g.period.name()))
                                .size(14.0)
                                .strong().family(draw::semi())
                                .color(draw::strong(ui)),
                        );
                        ui.label(RichText::new(g.metric.name()).size(12.0).color(draw::muted(ui)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Hapus").clicked() {
                            delete = Some(g.id);
                        }
                        ui.label(RichText::new(format!("{:.1} / {:.0}", v, g.target)).strong().family(draw::semi()));
                    });
                });
                let frac = (v / g.target.max(0.001)) as f32;
                draw::progress(ui, frac, if frac >= 1.0 { draw::GREEN } else { ACCENT });
                if frac >= 1.0 {
                    ui.label(RichText::new("Tercapai!").color(draw::GREEN).size(12.0));
                }
            });
            ui.add_space(6.0);
        }
        if let Some(id) = delete {
            self.data.goals.retain(|g| g.id != id);
            self.save();
        }
        draw::card(ui, |ui| {
            ui.label(RichText::new("Target baru").size(14.0).family(draw::semi()).color(draw::strong(ui)));
            let g = &mut self.goal_draft;
            egui::ComboBox::from_id_salt("goal_sport")
                .selected_text(g.sport.map(|s| s.name()).unwrap_or("Semua olahraga"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut g.sport, None, "Semua olahraga");
                    for s in Sport::ALL {
                        ui.selectable_value(&mut g.sport, Some(s), s.name());
                    }
                });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("goal_period")
                    .selected_text(g.period.name())
                    .show_ui(ui, |ui| {
                        for p in [Period::Week, Period::Month, Period::Year] {
                            ui.selectable_value(&mut g.period, p, p.name());
                        }
                    });
                egui::ComboBox::from_id_salt("goal_metric")
                    .selected_text(g.metric.name())
                    .show_ui(ui, |ui| {
                        for m in [Metric::Distance, Metric::Time, Metric::Count, Metric::Elevation] {
                            ui.selectable_value(&mut g.metric, m, m.name());
                        }
                    });
            });
            ui.add(egui::Slider::new(&mut g.target, 1.0..=2000.0).logarithmic(true).text("target"));
            if draw::accent_button(ui, "Tambah target").clicked() {
                let mut ng = self.goal_draft.clone();
                ng.id = self.data.new_id();
                self.data.goals.push(ng);
                self.save();
                self.say("Target ditambahkan");
            }
        });

        draw::heading(ui, "Tantangan bulan ini");
        let month_km: f64 = self.data.in_period(Period::Month, None).map(|a| a.distance_m).sum::<f64>() / 1000.0;
        let month_run: f64 = self.data.in_period(Period::Month, Some(Sport::Run)).map(|a| a.distance_m).sum::<f64>() / 1000.0;
        let month_up: f64 = self.data.in_period(Period::Month, None).map(|a| a.elev_gain_m).sum();
        let month_n = self.data.in_period(Period::Month, None).count() as f64;
        let longest = self.data.in_period(Period::Month, None).map(|a| a.duration_s).fold(0.0, f64::max) / 60.0;
        let challenges = [
            ("Lari 50 km", month_run, 50.0, "km"),
            ("Bergerak 150 km", month_km, 150.0, "km"),
            ("Tanjakan 1.000 m", month_up, 1000.0, "m"),
            ("12 aktivitas", month_n, 12.0, "x"),
            ("Satu sesi 90 menit", longest, 90.0, "mnt"),
        ];
        for (name, v, target, unit) in challenges {
            draw::card(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                    let done = v >= target;
                    let p = ui.painter();
                    p.circle_filled(r.center(), 17.0, if done { draw::GREEN } else { ACCENT.gamma_multiply(0.2) });
                    p.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        format!("{:.0}%", (v / target * 100.0).min(100.0)),
                        FontId::proportional(10.5),
                        if done { Color32::WHITE } else { ACCENT },
                    );
                    ui.vertical(|ui| {
                        ui.label(RichText::new(name).family(draw::semi()).color(draw::strong(ui)));
                        ui.label(RichText::new(format!("{v:.0} / {target:.0} {unit}")).size(12.0).color(draw::muted(ui)));
                    });
                });
                draw::progress(ui, (v / target) as f32, if v >= target { draw::GREEN } else { ACCENT });
            });
            ui.add_space(6.0);
        }

        draw::heading(ui, "Lencana");
        let acts = &self.data.activities;
        let total_km: f64 = acts.iter().map(|a| a.distance_m).sum::<f64>() / 1000.0;
        let badges = [
            ("Langkah pertama", !acts.is_empty()),
            ("5K pertama", acts.iter().any(|a| a.sport == Sport::Run && a.distance_m >= 5000.0)),
            ("10K pertama", acts.iter().any(|a| a.sport == Sport::Run && a.distance_m >= 10_000.0)),
            ("Gowes 40 km", acts.iter().any(|a| a.sport == Sport::Ride && a.distance_m >= 40_000.0)),
            ("100 km total", total_km >= 100.0),
            ("500 km total", total_km >= 500.0),
            ("7 hari beruntun", self.data.streak_days() >= 7),
            ("4 minggu beruntun", self.data.streak_weeks() >= 4),
            ("Serba bisa (4 olahraga)", Sport::ALL.iter().filter(|s| acts.iter().any(|a| a.sport == **s)).count() >= 4),
            ("Burung pagi", acts.iter().filter(|a| a.date().hour() < 7).count() >= 5),
        ];
        draw::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (name, got) in badges {
                    let (fill, fg) = if got {
                        (ACCENT, Color32::WHITE)
                    } else {
                        (draw::border(ui), draw::muted(ui))
                    };
                    ui.add(
                        egui::Button::new(RichText::new(name).size(12.0).color(fg))
                            .fill(fill)
                            .stroke(Stroke::NONE)
                            .corner_radius(12.0),
                    )
                    .on_hover_text(if got { "Sudah didapat" } else { "Belum didapat" });
                }
            });
        });
    }

    // ---------- you ----------

    pub fn you(&mut self, ui: &mut Ui) {
        self.top_bar(ui, "Anda");
        let imp = self.data.settings.imperial;
        let name = if self.data.settings.nickname.trim().is_empty() {
            "Atlet tanpa nama".to_string()
        } else {
            self.data.settings.nickname.clone()
        };
        draw::card(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(60.0, 60.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 30.0, ACCENT.gamma_multiply(0.2));
                draw::ghost(ui.painter(), r.center(), 40.0, ACCENT, Color32::WHITE);
                ui.vertical(|ui| {
                    ui.label(RichText::new(name).size(18.0).family(draw::semi()).color(draw::strong(ui)));
                    ui.label(
                        RichText::new("Tanpa akun · data hanya di perangkat ini")
                            .size(12.0)
                            .color(draw::muted(ui)),
                    );
                });
            });
            ui.add_space(6.0);
            let acts = &self.data.activities;
            draw::stat_grid(
                ui,
                &[
                    ("Aktivitas", acts.len().to_string()),
                    ("Total jarak", draw::dist(acts.iter().map(|a| a.distance_m).sum(), imp)),
                    ("Total waktu", draw::dur(acts.iter().map(|a| a.duration_s).sum())),
                ],
                3,
            );
        });
        ui.add_space(8.0);

        // Last 12 weeks.
        draw::card(ui, |ui| {
            ui.label(RichText::new("12 minggu terakhir (jarak)").size(14.0).family(draw::semi()).color(draw::strong(ui)));
            let (vals, labels) = self.weekly(12, None);
            draw::bars(ui, &vals, &labels, ACCENT, 90.0, true);
        });
        ui.add_space(8.0);

        let items = [
            (Sub::Summary, "Ringkasan", "Bulan dan tahun dalam angka, hari paling aktif"),
            (Sub::Stats, "Statistik", "Total per olahraga, per minggu dan per tahun"),
            (Sub::Records, "Rekor pribadi", "1K, 5K, 10K, setengah maraton, terjauh"),
            (Sub::Calendar, "Kalender latihan", "Lihat hari-hari Anda bergerak"),
            (Sub::Gear, "Perlengkapan", "Sepatu & sepeda, berapa km sudah dipakai"),
            (Sub::Tools, "Alat bantu", "Hitung tempo, prediksi lomba, zona detak"),
            (Sub::Settings, "Pengaturan & privasi", "Satuan, tema, cadangan data, ekspor"),
        ];
        for (sub, title, desc) in items {
            let (_, clicked) = draw::card_click(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(title).size(15.0).family(draw::semi()).color(draw::strong(ui)));
                        ui.label(RichText::new(desc).size(12.0).color(draw::muted(ui)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("›").size(22.0).color(draw::muted(ui)));
                    });
                });
            });
            if clicked {
                self.sub = Some(sub);
            }
            ui.add_space(6.0);
        }
    }

    fn weekly(&self, n: i64, sport: Option<Sport>) -> (Vec<f64>, Vec<String>) {
        let today = Local::now().date_naive();
        let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
        let mut vals = vec![0.0; n as usize];
        let mut labels = Vec::new();
        for i in 0..n {
            let wk = monday - chrono::Duration::days(7 * (n - 1 - i));
            labels.push(if i % 3 == 0 { wk.format("%d/%m").to_string() } else { String::new() });
        }
        for a in &self.data.activities {
            if sport.is_some_and(|s| s != a.sport) {
                continue;
            }
            let d = a.date().date_naive();
            let back = if d >= monday { 0 } else { ((monday - d).num_days() + 6) / 7 };
            if back < n {
                vals[(n - 1 - back) as usize] += a.distance_m / 1000.0;
            }
        }
        (vals, labels)
    }

    pub fn sub_page(&mut self, ui: &mut Ui, sub: Sub) {
        let title = match sub {
            Sub::Summary => "Ringkasan",
            Sub::Stats => "Statistik",
            Sub::Records => "Rekor pribadi",
            Sub::Calendar => "Kalender latihan",
            Sub::Gear => "Perlengkapan",
            Sub::Tools => "Alat bantu",
            Sub::Settings => "Pengaturan & privasi",
        };
        if self.back_bar(ui, title) {
            self.sub = None;
            self.tab = Tab::You;
            return;
        }
        match sub {
            Sub::Summary => self.summary(ui),
            Sub::Stats => self.stats(ui),
            Sub::Records => self.records(ui),
            Sub::Calendar => self.calendar(ui),
            Sub::Gear => self.gear(ui),
            Sub::Tools => self.tools(ui),
            Sub::Settings => self.settings(ui),
        }
    }

    fn stats(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        ui.horizontal_wrapped(|ui| {
            if draw::chip(ui, self.stats_sport.is_none(), "Semua").clicked() {
                self.stats_sport = None;
            }
            for s in Sport::ALL {
                if draw::chip(ui, self.stats_sport == Some(s), s.name()).clicked() {
                    self.stats_sport = Some(s);
                }
            }
        });
        ui.add_space(6.0);
        let sport = self.stats_sport;
        draw::card(ui, |ui| {
            ui.label(RichText::new("Jarak per minggu (km)").family(draw::semi()).color(draw::strong(ui)));
            let (vals, labels) = self.weekly(16, sport);
            draw::bars(ui, &vals, &labels, sport.map(draw::sport_color).unwrap_or(ACCENT), 120.0, true);
        });
        ui.add_space(8.0);
        for (period, label) in [(Period::Week, "Minggu ini"), (Period::Month, "Bulan ini"), (Period::Year, "Tahun ini")] {
            let acts: Vec<_> = self.data.in_period(period, sport).collect();
            let dist: f64 = acts.iter().map(|a| a.distance_m).sum();
            let time: f64 = acts.iter().map(|a| a.duration_s).sum();
            let up: f64 = acts.iter().map(|a| a.elev_gain_m).sum();
            let n = acts.len();
            draw::card(ui, |ui| {
                ui.label(RichText::new(label).family(draw::semi()).color(draw::strong(ui)));
                draw::stat_grid(
                    ui,
                    &[
                        ("Aktivitas", n.to_string()),
                        ("Jarak", draw::dist(dist, imp)),
                        ("Waktu", draw::dur(time)),
                        ("Tanjakan", format!("{up:.0} m")),
                    ],
                    2,
                );
            });
            ui.add_space(6.0);
        }
        draw::card(ui, |ui| {
            ui.label(RichText::new("Pembagian waktu per olahraga").family(draw::semi()).color(draw::strong(ui)));
            let total: f64 = self.data.activities.iter().map(|a| a.duration_s).sum::<f64>().max(1.0);
            for s in Sport::ALL {
                let t: f64 = self.data.activities.iter().filter(|a| a.sport == s).map(|a| a.duration_s).sum();
                if t > 0.0 {
                    ui.horizontal(|ui| {
                        ui.add_sized([70.0, 16.0], egui::Label::new(RichText::new(s.name()).size(12.0)));
                        let w = ui.available_width() - 60.0;
                        let (r, _) = ui.allocate_exact_size(vec2(w * (t / total) as f32, 12.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, draw::sport_color(s));
                        ui.label(RichText::new(format!("{:.0}%", t / total * 100.0)).size(12.0));
                    });
                }
            }
        });
    }

    fn records(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        let runs: Vec<_> = self.data.activities.iter().filter(|a| a.sport == Sport::Run).collect();
        draw::card(ui, |ui| {
            ui.label(RichText::new("Usaha terbaik lari").family(draw::semi()).color(draw::strong(ui)));
            for (label, d) in [("400 m", 400.0), ("1 km", 1000.0), ("5 km", 5000.0), ("10 km", 10_000.0), ("Setengah maraton", 21_097.5), ("Maraton", 42_195.0)] {
                let best = runs
                    .iter()
                    .filter_map(|a| a.best_effort(d).map(|t| (t, a.date())))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                ui.horizontal(|ui| {
                    ui.add_sized([120.0, 20.0], egui::Label::new(RichText::new(label).size(13.0)));
                    match best {
                        Some((t, when)) => {
                            ui.label(RichText::new(draw::clock(t)).strong().monospace());
                            ui.label(RichText::new(format!("{} · {}", draw::pace(d / t, imp), when.format("%d/%m/%y"))).size(11.5).color(draw::muted(ui)));
                        }
                        None => {
                            ui.label(RichText::new("belum ada").color(draw::muted(ui)));
                        }
                    }
                });
            }
        });
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Rekor lain").family(draw::semi()).color(draw::strong(ui)));
            let acts = &self.data.activities;
            let maxby = |f: &dyn Fn(&model::Activity) -> f64, s: Option<Sport>| {
                acts.iter()
                    .filter(|a| s.is_none_or(|s| s == a.sport))
                    .max_by(|a, b| f(a).total_cmp(&f(b)))
            };
            let rows: Vec<(&str, String)> = vec![
                ("Lari terjauh", maxby(&|a| a.distance_m, Some(Sport::Run)).map(|a| draw::dist(a.distance_m, imp)).unwrap_or("-".into())),
                ("Gowes terjauh", maxby(&|a| a.distance_m, Some(Sport::Ride)).map(|a| draw::dist(a.distance_m, imp)).unwrap_or("-".into())),
                ("Tanjakan terbesar", maxby(&|a| a.elev_gain_m, None).map(|a| format!("{:.0} m", a.elev_gain_m)).unwrap_or("-".into())),
                ("Sesi terlama", maxby(&|a| a.duration_s, None).map(|a| draw::dur(a.duration_s)).unwrap_or("-".into())),
                ("Gowes tercepat", maxby(&|a| a.speed_mps(), Some(Sport::Ride)).map(|a| draw::speed(a.speed_mps(), imp)).unwrap_or("-".into())),
            ];
            for (l, v) in rows {
                ui.horizontal(|ui| {
                    ui.add_sized([140.0, 20.0], egui::Label::new(RichText::new(l).size(13.0)));
                    ui.label(RichText::new(v).strong().family(draw::semi()));
                });
            }
        });
    }

    fn calendar(&mut self, ui: &mut Ui) {
        let (y, m) = self.cal_month;
        let first = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or_else(|| Local::now().date_naive());
        ui.horizontal(|ui| {
            if ui.button("‹").clicked() {
                self.cal_month = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
            }
            ui.label(RichText::new(draw::month_year(y, m)).size(16.0).strong().family(draw::semi()));
            if ui.button("›").clicked() {
                self.cal_month = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            }
        });
        let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) }.unwrap_or(first);
        let days = (next - first).num_days();
        let offset = first.weekday().num_days_from_monday() as i64;
        let mut month_acts = Vec::new();
        draw::card(ui, |ui| {
            let w = ui.available_width() / 7.0;
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0 + 52.0 * ((offset + days + 6) / 7) as f32), Sense::hover());
            let p = ui.painter_at(rect);
            for (i, d) in ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"].iter().enumerate() {
                p.text(pos2(rect.left() + w * (i as f32 + 0.5), rect.top() + 10.0), Align2::CENTER_CENTER, d, FontId::proportional(11.0), draw::muted(ui));
            }
            let today = Local::now().date_naive();
            for day in 0..days {
                let date = first + chrono::Duration::days(day);
                let cell = offset + day;
                let c = pos2(rect.left() + w * ((cell % 7) as f32 + 0.5), rect.top() + 24.0 + 52.0 * (cell / 7) as f32 + 24.0);
                let acts: Vec<_> = self.data.activities.iter().filter(|a| a.date().date_naive() == date).collect();
                if date == today {
                    p.circle_stroke(c - vec2(0.0, 8.0), 12.0, Stroke::new(1.5, ACCENT));
                }
                p.text(c - vec2(0.0, 8.0), Align2::CENTER_CENTER, date.day().to_string(), FontId::proportional(13.0), draw::strong(ui));
                for (k, a) in acts.iter().take(3).enumerate() {
                    let dx = (k as f32 - (acts.len().min(3) as f32 - 1.0) / 2.0) * 9.0;
                    let r = 2.5 + ((a.duration_s / 3600.0).min(2.0) as f32) * 1.5;
                    p.circle_filled(c + vec2(dx, 12.0), r, draw::sport_color(a.sport));
                }
                month_acts.extend(acts.iter().map(|a| (a.id, a.title.clone(), a.sport, date)));
            }
        });
        ui.add_space(8.0);
        let total: f64 = self.data.activities.iter().filter(|a| a.date().year() == y && a.date().month() == m).map(|a| a.duration_s).sum();
        ui.label(RichText::new(format!("{} aktivitas · {}", month_acts.len(), draw::dur(total))).color(draw::muted(ui)));
        for (id, title, sport, date) in month_acts {
            ui.horizontal(|ui| {
                draw::sport_badge(ui, sport, 22.0);
                if ui.link(format!("{} – {}", date.format("%d/%m"), title)).clicked() {
                    self.detail = Some(id);
                }
            });
        }
    }

    fn gear(&mut self, ui: &mut Ui) {
        let mut changed = false;
        for i in 0..self.data.gear.len() {
            let g = self.data.gear[i].clone();
            let km = self.data.gear_km(g.id);
            draw::card(ui, |ui| {
                ui.horizontal(|ui| {
                    draw::sport_badge(ui, g.sport, 28.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(&g.name).family(draw::semi()).color(draw::strong(ui)));
                        ui.label(RichText::new(format!("{km:.1} km{}", if g.retired { " · pensiun" } else { "" })).size(12.0).color(draw::muted(ui)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(if g.retired { "Aktifkan" } else { "Pensiunkan" }).clicked() {
                            self.data.gear[i].retired = !g.retired;
                            changed = true;
                        }
                    });
                });
                if g.limit_km > 0.0 {
                    let f = (km / g.limit_km) as f32;
                    draw::progress(ui, f, if f > 0.9 { draw::RED } else { ACCENT });
                    ui.label(
                        RichText::new(if f > 0.9 {
                            format!("Hampir {:.0} km — saatnya ganti", g.limit_km)
                        } else {
                            format!("Batas pakai {:.0} km", g.limit_km)
                        })
                        .size(11.5)
                        .color(draw::muted(ui)),
                    );
                }
            });
            ui.add_space(6.0);
        }
        draw::card(ui, |ui| {
            ui.label(RichText::new("Tambah perlengkapan").family(draw::semi()).color(draw::strong(ui)));
            ui.add(egui::TextEdit::singleline(&mut self.gear_draft.0).hint_text("mis. Sepatu trail").desired_width(f32::INFINITY));
            ui.horizontal_wrapped(|ui| {
                for s in [Sport::Run, Sport::Ride, Sport::Walk, Sport::Hike] {
                    if draw::chip(ui, self.gear_draft.1 == s, s.name()).clicked() {
                        self.gear_draft.1 = s;
                    }
                }
            });
            ui.add(egui::Slider::new(&mut self.gear_draft.2, 0.0..=20_000.0).logarithmic(true).text("batas km (0 = tanpa)"));
            if draw::accent_button(ui, "Tambah").clicked() && !self.gear_draft.0.trim().is_empty() {
                let id = self.data.new_id();
                self.data.gear.push(Gear {
                    id,
                    name: self.gear_draft.0.trim().to_string(),
                    sport: self.gear_draft.1,
                    limit_km: self.gear_draft.2.round(),
                    retired: false,
                });
                self.gear_draft.0.clear();
                changed = true;
            }
        });
        if changed {
            self.save();
        }
    }

    fn tools(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        draw::card(ui, |ui| {
            ui.label(RichText::new("Kalkulator tempo").family(draw::semi()).color(draw::strong(ui)));
            ui.columns(2, |c| {
                c[0].label("Jarak (km)");
                c[0].text_edit_singleline(&mut self.calc.0);
                c[1].label("Waktu (j:m:d)");
                c[1].text_edit_singleline(&mut self.calc.1);
            });
            let d: Option<f64> = self.calc.0.trim().replace(',', ".").parse().ok();
            let t = parse_time(&self.calc.1);
            if let (Some(d), Some(t)) = (d, t) {
                if d > 0.0 && t > 0.0 {
                    let v = d * 1000.0 / t;
                    draw::stat_grid(ui, &[("Tempo", draw::pace(v, imp)), ("Kecepatan", draw::speed(v, imp))], 2);
                }
            }
        });
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Prediksi waktu lomba").family(draw::semi()).color(draw::strong(ui)));
            let base = self
                .data
                .activities
                .iter()
                .filter(|a| a.sport == Sport::Run)
                .filter_map(|a| a.best_effort(5000.0))
                .min_by(|a, b| a.total_cmp(b));
            match base {
                Some(t5) => {
                    ui.label(RichText::new(format!("Dari 5K terbaik Anda ({}), rumus Riegel:", draw::clock(t5))).size(12.0).color(draw::muted(ui)));
                    for (l, d) in [("10 km", 10_000.0), ("Setengah maraton", 21_097.5), ("Maraton", 42_195.0)] {
                        let t = t5 * (d / 5000.0f64).powf(1.06);
                        ui.horizontal(|ui| {
                            ui.add_sized([130.0, 18.0], egui::Label::new(l));
                            ui.label(RichText::new(draw::clock(t)).strong().monospace());
                            ui.label(RichText::new(draw::pace(d / t, imp)).size(11.5).color(draw::muted(ui)));
                        });
                    }
                }
                None => {
                    ui.label("Rekam lari minimal 5 km untuk melihat prediksi.");
                }
            }
        });
        ui.add_space(8.0);
        let max = self.data.settings.max_hr;
        draw::card(ui, |ui| {
            ui.label(RichText::new("Zona detak jantung").family(draw::semi()).color(draw::strong(ui)));
            ui.add(egui::Slider::new(&mut self.data.settings.max_hr, 140..=220).text("detak maks"));
            let colors = [
                Color32::from_rgb(150, 150, 160),
                Color32::from_rgb(60, 150, 230),
                draw::GREEN,
                Color32::from_rgb(240, 170, 40),
                draw::RED,
            ];
            let names = ["Pemulihan", "Daya tahan", "Aerobik", "Ambang", "Maksimal"];
            for z in 0..5 {
                let lo = (max as f32 * (0.5 + 0.1 * z as f32)) as u32;
                let hi = (max as f32 * (0.6 + 0.1 * z as f32)) as u32;
                let n = self.data.activities.iter().filter(|a| a.avg_hr.is_some_and(|h| hr_zone(h, max) == z + 1)).count();
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    ui.painter().rect_filled(r, 3.0, colors[z as usize]);
                    ui.label(RichText::new(format!("Z{} {}", z + 1, names[z as usize])).strong().family(draw::semi()));
                    ui.label(format!("{lo}–{hi} bpm"));
                    ui.label(RichText::new(format!("{n} aktivitas")).size(11.5).color(draw::muted(ui)));
                });
            }
        });
        if max != self.data.settings.max_hr {
            self.save();
        }
    }

    fn settings(&mut self, ui: &mut Ui) {
        let before = self.data.settings.clone();
        draw::card(ui, |ui| {
            ui.label(RichText::new("Profil (semua opsional)").family(draw::semi()).color(draw::strong(ui)));
            ui.label(RichText::new("Nama panggilan — hanya tampil di perangkat ini").size(12.0).color(draw::muted(ui)));
            ui.add(egui::TextEdit::singleline(&mut self.data.settings.nickname).hint_text("boleh dikosongkan").desired_width(f32::INFINITY));
            ui.add(egui::Slider::new(&mut self.data.settings.weight_kg, 30.0..=160.0).text("berat untuk kalori (kg)"));
        });
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Tampilan").family(draw::semi()).color(draw::strong(ui)));
            ui.horizontal(|ui| {
                if draw::chip(ui, !self.data.settings.imperial, "Kilometer").clicked() {
                    self.data.settings.imperial = false;
                }
                if draw::chip(ui, self.data.settings.imperial, "Mil").clicked() {
                    self.data.settings.imperial = true;
                }
            });
            ui.horizontal(|ui| {
                if draw::chip(ui, !self.data.settings.dark, "Terang").clicked() {
                    self.data.settings.dark = false;
                }
                if draw::chip(ui, self.data.settings.dark, "Gelap").clicked() {
                    self.data.settings.dark = true;
                }
            });
        });
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Privasi").family(draw::semi()).color(draw::GREEN));
            for line in [
                "Tidak perlu akun, email, nomor HP atau tanggal lahir.",
                "Internet hanya dipakai saat Anda mengunduh peta. Lokasi, rute, dan aktivitas Anda tidak pernah dikirim ke mana pun.",
                "Tidak ada iklan, pelacak, atau analitik.",
                "Semua aktivitas disimpan di satu file di perangkat ini:",
            ] {
                ui.label(RichText::new(format!("• {line}")).size(12.5));
            }
            ui.label(RichText::new(model::data_file().display().to_string()).monospace().size(11.0).color(draw::muted(ui)));
        });
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Peta offline").family(draw::semi()).color(draw::strong(ui)));
            ui.label(
                RichText::new(format!("{} peta terpasang. Unduh atau hapus peta di tab Peta.", offmap::list().len()))
                    .size(12.5)
                    .color(draw::muted(ui)),
            );
            if ui.button("Buka Peta offline").clicked() {
                self.sub = None;
                self.tab = Tab::Maps;
                self.map_mode = MapMode::Offline;
            }
        });
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Data Anda").family(draw::semi()).color(draw::strong(ui)));
            ui.horizontal_wrapped(|ui| {
                if ui.button("Cadangkan (JSON)").clicked() {
                    let path = self.export_dir().join(format!("cadangan-{}.json", Local::now().format("%Y%m%d-%H%M")));
                    let r = std::fs::create_dir_all(self.export_dir())
                        .and_then(|_| std::fs::write(&path, serde_json::to_vec_pretty(&self.data).unwrap_or_default()));
                    match r {
                        Ok(()) => self.publish(&path, "application/json", false),
                        Err(e) => self.say(format!("Gagal: {e}")),
                    }
                }
                if ui.button("Ekspor tabel (CSV)").clicked() {
                    let path = self.export_dir().join("aktivitas.csv");
                    let r = std::fs::create_dir_all(self.export_dir()).and_then(|_| std::fs::write(&path, gpx::csv(&self.data.activities)));
                    match r {
                        Ok(()) => self.publish(&path, "text/csv", false),
                        Err(e) => self.say(format!("Gagal: {e}")),
                    }
                }
            });
            ui.label(RichText::new("Pulihkan dari cadangan").size(12.0).color(draw::muted(ui)));
            if cfg!(target_os = "android") {
                self.pick_button(ui, Pick::Backup, "Pilih file cadangan");
            } else {
                ui.add(egui::TextEdit::singleline(&mut self.restore_path).hint_text("lokasi file cadangan .json").desired_width(f32::INFINITY));
                if ui.button("Pulihkan").clicked() {
                    let path = std::path::PathBuf::from(self.restore_path.trim());
                    self.restore_backup(&path);
                }
            }
            ui.separator();
            if self.data.has_samples && ui.button("Hapus data contoh").clicked() {
                // Only the sample items; activities, gear, goals and routes added since stay.
                self.data.remove_samples();
                self.save();
                self.say("Data contoh dihapus");
            }
            if !self.confirm_wipe {
                if ui.button(RichText::new("Hapus semua data").color(draw::RED)).clicked() {
                    self.confirm_wipe = true;
                }
            } else {
                ui.label(RichText::new("Yakin? Ini tidak bisa dibatalkan.").color(draw::RED));
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Ya, hapus semua").color(draw::RED)).clicked() {
                        let s = self.data.settings.clone();
                        // Keep counting ids on, so nothing old can come back under a reused id.
                        self.data = model::Data { settings: s, next_id: self.data.next_id, ..Default::default() };
                        self.undo = None;
                        self.confirm_wipe = false;
                        self.save();
                        self.say("Semua data dihapus");
                    }
                    if ui.button("Batal").clicked() {
                        self.confirm_wipe = false;
                    }
                });
            }
        });
        let s = &self.data.settings;
        if s.nickname != before.nickname || s.imperial != before.imperial || s.dark != before.dark || s.weight_kg != before.weight_kg {
            self.save();
        }
    }

    /// Replaces all data with a backup file made by "Cadangkan (JSON)".
    pub fn restore_backup(&mut self, path: &std::path::Path) {
        match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|s| serde_json::from_str::<model::Data>(&s).map_err(|e| e.to_string()))
        {
            Ok(mut d) => {
                d.fix_next_id();
                self.data = d;
                // The deleted activity waiting for "Urungkan" belonged to the old data.
                self.undo = None;
                self.save();
                self.say("Data dipulihkan");
            }
            Err(e) => self.say(format!("Gagal memulihkan: {e}")),
        }
    }

    // ---------- offline maps ----------

    fn offline_maps(&mut self, ui: &mut Ui) {
        enum Act {
            None,
            Check,
            Download(Vec<std::sync::Arc<mapdl::Plan>>),
            Cancel,
            Clear,
        }
        let mut act = Act::None;
        draw::card(ui, |ui| {
            ui.label(RichText::new("Peta offline").family(draw::semi()).color(draw::strong(ui)));
            ui.label(
                RichText::new("Peta tampil di bawah rute Anda dan dipakai tanpa internet.")
                    .size(12.0)
                    .color(draw::muted(ui)),
            );
            let maps = offmap::list();
            if maps.is_empty() {
                ui.label(RichText::new("Belum ada peta terpasang.").size(12.5));
            }
            for (name, size, path) in maps {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&name).size(13.5).color(draw::strong(ui)));
                    ui.label(RichText::new(mb(size)).size(12.0).color(draw::muted(ui)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.map_delete.as_ref() == Some(&path) {
                            if ui.button("Batal").clicked() {
                                self.map_delete = None;
                            }
                            if ui.button(RichText::new("Ya, hapus").color(draw::RED)).clicked() {
                                let _ = std::fs::remove_file(&path);
                                offmap::reload();
                                self.map_delete = None;
                                self.say("Peta dihapus");
                            }
                        } else if ui.button(RichText::new("Hapus").color(draw::RED)).clicked() {
                            self.map_delete = Some(path.clone());
                        }
                    });
                });
            }

            ui.separator();
            ui.label(RichText::new("Unduh peta wilayah").family(draw::semi()).color(draw::strong(ui)));
            ui.label(
                RichText::new("Cari kecamatan atau kabupaten/kota. Anda bisa menambahkan beberapa wilayah sekaligus.")
                    .size(12.0)
                    .color(draw::muted(ui)),
            );
            let busy = self.map_task.as_ref().is_some_and(|t| t.busy());
            let queue_before = self.map_queue.clone();
            ui.add_enabled_ui(!busy, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.map_search)
                        .hint_text("Cari: Cibitung, Kota Bandung, Tambun…")
                        .desired_width(f32::INFINITY),
                );
                let hits = mapdl::search(&self.map_search, 8);
                if self.map_search.trim().chars().count() >= 2 && hits.is_empty() {
                    ui.label(
                        RichText::new("Tidak ditemukan. Coba nama lain, atau masukkan koordinat di bawah.")
                            .size(12.0)
                            .color(draw::muted(ui)),
                    );
                }
                for place in hits {
                    let area = place.area();
                    let added = self.map_queue.iter().any(|a| a.name == area.name && a.lat == area.lat);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            ui.label(RichText::new(&place.name).family(draw::semi()).color(draw::strong(ui)));
                            ui.label(RichText::new(&place.context).size(12.0).color(draw::muted(ui)));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if added {
                                ui.label(RichText::new("Ditambahkan").size(12.0).color(draw::lime(ui)));
                            } else if ui.button("+ Tambah").clicked() {
                                self.map_queue.push(area);
                            }
                        });
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.map_coords)
                            .hint_text("atau koordinat, mis. -6.27, 107.07")
                            .desired_width(ui.available_width() - 90.0),
                    );
                    if ui.button("Tambah").clicked() {
                        match mapdl::parse_coords(&self.map_coords) {
                            Some((lat, lon)) => {
                                self.map_queue.push(mapdl::Area {
                                    name: format!("Titik {lat:.3}, {lon:.3}"),
                                    lat,
                                    lon,
                                    radius_km: 5.0,
                                });
                                self.map_coords.clear();
                            }
                            None => self.say("Koordinat tidak dikenali. Contoh: -6.27, 107.07"),
                        }
                    }
                });
            });

            let status = self.map_task.as_ref().map(|t| t.status.clone());
            let status = status.as_ref().map(|s| s.lock().unwrap());
            let sizes: Vec<u64> = match status.as_deref() {
                Some(Status::Ready(plans)) => plans.iter().map(|p| p.bytes).collect(),
                _ => Vec::new(),
            };
            if !self.map_queue.is_empty() {
                ui.add_space(4.0);
                draw::overline(ui, &format!("Daftar unduhan ({})", self.map_queue.len()));
                let mut remove = None;
                for (i, area) in self.map_queue.iter_mut().enumerate() {
                    egui::Frame::new()
                        .fill(draw::bg(ui))
                        .stroke(egui::Stroke::new(1.0, draw::border(ui)))
                        .corner_radius(14.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&area.name).family(draw::semi()).color(draw::strong(ui)));
                                if let Some(b) = sizes.get(i) {
                                    ui.label(RichText::new(format!("± {}", mb(*b))).size(12.0).color(draw::muted(ui)));
                                } else if area.installed() {
                                    ui.label(RichText::new("sudah terpasang").size(12.0).color(draw::muted(ui)));
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add_enabled(!busy, egui::Button::new(RichText::new("×").size(20.0)).frame(false)).clicked() {
                                        remove = Some(i);
                                    }
                                });
                            });
                            ui.add_enabled_ui(!busy, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    for r in mapdl::RADII_KM {
                                        if draw::chip(ui, area.radius_km == r, &format!("{r:.0} km")).clicked() {
                                            area.radius_km = r;
                                        }
                                    }
                                });
                            });
                        });
                }
                if let Some(i) = remove {
                    self.map_queue.remove(i);
                }
            }
            if self.map_queue != queue_before {
                act = Act::Clear;
            }
            match status.as_deref() {
                Some(Status::Checking { done, count }) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Memeriksa ukuran ({done}/{count})…"));
                        if ui.button("Batal").clicked() {
                            act = Act::Cancel;
                        }
                    });
                }
                Some(Status::Ready(plans)) => {
                    let total: u64 = plans.iter().map(|p| p.bytes).sum();
                    ui.label(
                        RichText::new(format!("Total ± {} untuk {} wilayah", mb(total), plans.len()))
                            .family(draw::semi())
                            .color(draw::strong(ui)),
                    );
                    if total > 300 << 20 {
                        ui.label(
                            RichText::new("Cukup besar. Pastikan memakai Wi-Fi dan ruang penyimpanan cukup.")
                                .size(12.0)
                                .color(ACCENT),
                        );
                    }
                    ui.horizontal(|ui| {
                        if draw::accent_button(ui, "Unduh semua").clicked() {
                            act = Act::Download(plans.clone());
                        }
                        if ui.button("Batal").clicked() {
                            act = Act::Clear;
                        }
                    });
                }
                Some(Status::Downloading { index, count, name, done, total }) => {
                    ui.label(format!("Mengunduh {} dari {}: {name}", index + 1, count));
                    draw::progress(ui, *done as f32 / (*total).max(1) as f32, ACCENT);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} dari {}", mb(*done), mb(*total))).size(12.5).color(draw::muted(ui)));
                        if ui.button("Batal").clicked() {
                            act = Act::Cancel;
                        }
                    });
                }
                other => {
                    match other {
                        Some(Status::Done(names)) => {
                            ui.label(RichText::new(format!("Siap dipakai: {}", names.join(", "))).color(draw::GREEN));
                        }
                        Some(Status::Failed(e)) => {
                            ui.label(RichText::new(format!("Gagal: {e}")).color(draw::RED));
                        }
                        Some(Status::Cancelled) => {
                            ui.label("Dibatalkan.");
                        }
                        _ => {}
                    }
                    if !self.map_queue.is_empty() && draw::accent_button(ui, "Cek ukuran").clicked() {
                        act = Act::Check;
                    }
                }
            }
            let finished = matches!(status.as_deref(), Some(Status::Done(_)));
            drop(status);
            if finished && !self.map_queue.is_empty() {
                self.map_queue.clear();
            }
            ui.label(
                RichText::new(
                    "Hanya saat Anda menekan tombol, aplikasi meminta potongan peta wilayah pilihan Anda dari \
                     build.protomaps.com (data OpenStreetMap). Lokasi GPS, rute, dan aktivitas tidak pernah dikirim. \
                     Daftar wilayah: GeoNames (CC BY 4.0).",
                )
                .size(11.5)
                .color(draw::muted(ui)),
            );

            ui.separator();
            ui.label(RichText::new("Impor file peta").family(draw::semi()).color(draw::strong(ui)));
            ui.label(
                RichText::new(
                    "File .pmtiles berisi peta vektor Protomaps, misalnya hasil \"pmtiles extract\" di komputer. \
                     Pilih cakupan kota/kabupaten agar ukurannya kecil. Peta gambar (raster) belum didukung.",
                )
                .size(11.5)
                .color(draw::muted(ui)),
            );
            if cfg!(target_os = "android") {
                self.pick_button(ui, Pick::Map, "Pilih file .pmtiles");
            } else {
                ui.add(egui::TextEdit::singleline(&mut self.map_import).hint_text("lokasi file .pmtiles").desired_width(f32::INFINITY));
                if ui.button("Impor").clicked() {
                    match mapdl::import(&self.map_import) {
                        Ok(name) => {
                            self.map_import.clear();
                            self.say(format!("Peta {name} dipasang"));
                        }
                        Err(e) => self.say(format!("Gagal mengimpor: {e}")),
                    }
                }
            }
        });
        match act {
            Act::None => {}
            Act::Check => self.map_task = Some(mapdl::check(ui.ctx(), self.map_queue.clone())),
            Act::Download(plans) => self.map_task = Some(mapdl::download(ui.ctx(), plans)),
            Act::Cancel => {
                if let Some(t) = &self.map_task {
                    t.cancel();
                }
            }
            Act::Clear => {
                if !self.map_task.as_ref().is_some_and(|t| t.busy()) {
                    self.map_task = None;
                }
            }
        }
    }
}

/// "12,3 MB"
fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0).replace('.', ",")
}
