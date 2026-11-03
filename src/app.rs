//! The app window: tab bar, home feed, recording and the activity page.

use crate::draw::{self, ACCENT, Icon};
use crate::gpx;
use crate::model::{self, Activity, Data, Sport};
use crate::record::{RecState, Recorder};
use chrono::{Datelike, Local, NaiveDateTime, TimeZone, Timelike};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, Ui, pos2, vec2};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Home,
    Maps,
    Record,
    Goals,
    You,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sub {
    Summary,
    Stats,
    Records,
    Calendar,
    Gear,
    Tools,
    Settings,
}

/// What a file chosen in the phone's file picker is for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Gpx,
    Map,
    Backup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MapMode {
    Routes,
    Plan,
    Heat,
    Offline,
}

/// Fields for adding, finishing or editing an activity.
#[derive(Clone)]
pub struct Form {
    pub sport: Sport,
    pub title: String,
    pub when: String,
    pub dist_km: String,
    pub time: String,
    pub elev: String,
    pub hr: String,
    pub effort: u8,
    pub notes: String,
    pub gear: Option<u64>,
}

impl Form {
    pub fn blank(sport: Sport) -> Self {
        Self {
            sport,
            title: String::new(),
            when: Local::now().format("%Y-%m-%d %H:%M").to_string(),
            dist_km: String::new(),
            time: String::new(),
            elev: String::new(),
            hr: String::new(),
            effort: 5,
            notes: String::new(),
            gear: None,
        }
    }

    pub fn from(a: &Activity) -> Self {
        Self {
            sport: a.sport,
            title: a.title.clone(),
            when: a.date().format("%Y-%m-%d %H:%M").to_string(),
            dist_km: format!("{:.2}", a.distance_m / 1000.0),
            time: draw::clock(a.duration_s),
            elev: format!("{:.0}", a.elev_gain_m),
            hr: a.avg_hr.map(|h| h.to_string()).unwrap_or_default(),
            effort: a.effort,
            notes: a.notes.clone(),
            gear: a.gear,
        }
    }

    /// Copies the form into an activity; returns an error message if a field is wrong.
    pub fn apply(&self, a: &mut Activity) -> Result<(), String> {
        let num = |s: &str| s.trim().replace(',', ".").parse::<f64>();
        a.sport = self.sport;
        a.title = if self.title.trim().is_empty() {
            default_title(self.sport, a.start)
        } else {
            self.title.trim().to_string()
        };
        if let Ok(t) = NaiveDateTime::parse_from_str(self.when.trim(), "%Y-%m-%d %H:%M") {
            if let Some(t) = t.and_local_timezone(Local).single() {
                a.start = t.timestamp();
            }
        } else {
            return Err("Tanggal harus seperti 2026-11-02 07:30".into());
        }
        if !self.dist_km.trim().is_empty() {
            a.distance_m = num(&self.dist_km).map_err(|_| "Jarak harus angka")? * 1000.0;
        }
        if !self.time.trim().is_empty() {
            a.duration_s = parse_time(&self.time).ok_or("Waktu harus seperti 45:00 atau 1:05:30")?;
        }
        if a.duration_s <= 0.0 {
            return Err("Isi lama aktivitas".into());
        }
        a.elev_gain_m = if self.elev.trim().is_empty() {
            a.elev_gain_m
        } else {
            num(&self.elev).map_err(|_| "Tanjakan harus angka")?
        };
        a.avg_hr = self.hr.trim().parse().ok();
        a.effort = self.effort;
        a.notes = self.notes.clone();
        a.gear = self.gear;
        Ok(())
    }
}

pub fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<f64> = s
        .trim()
        .split(':')
        .map(|p| p.trim().replace(',', ".").parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    Some(match parts.as_slice() {
        [m] => m * 60.0,
        [m, s] => m * 60.0 + s,
        [h, m, s] => h * 3600.0 + m * 60.0 + s,
        _ => return None,
    })
}

pub fn default_title(sport: Sport, start: i64) -> String {
    let h = Local
        .timestamp_opt(start, 0)
        .single()
        .map(|t| t.hour())
        .unwrap_or(12);
    let part = match h {
        4..=10 => "pagi",
        11..=14 => "siang",
        15..=18 => "sore",
        _ => "malam",
    };
    format!("{} {}", sport.name(), part)
}

struct Shot {
    path: PathBuf,
    requested: bool,
    done: Arc<AtomicBool>,
}

pub struct GhostApp {
    pub data: Data,
    pub persist: bool,
    pub tab: Tab,
    pub detail: Option<u64>,
    pub edit: Option<Form>,
    pub sub: Option<Sub>,
    pub rec: Recorder,
    pub manual: Form,
    pub show_manual: bool,
    pub import_path: String,
    pub toast: Option<(String, f64)>,
    pub feed_filter: Option<Sport>,
    pub map_mode: MapMode,
    pub draft: Vec<[f32; 2]>,
    pub draft_name: String,
    pub draft_sport: Sport,
    pub goal_draft: model::Goal,
    pub gear_draft: (String, Sport, f64),
    pub calc: (String, String),
    pub cal_month: (i32, u32),
    pub stats_sport: Option<Sport>,
    pub confirm_wipe: bool,
    pub restore_path: String,
    pub map_search: String,
    pub map_coords: String,
    pub map_queue: Vec<crate::mapdl::Area>,
    pub map_task: Option<crate::mapdl::Task>,
    pub map_import: String,
    pub map_delete: Option<PathBuf>,
    /// Shown once after the app closed by itself.
    pub crash_report: Option<String>,
    /// A card with confetti for records set by the activity just saved.
    pub celebrate: Option<(Vec<String>, Instant)>,
    /// The last deleted activity, while it can still be brought back.
    pub undo: Option<(Activity, Instant)>,
    pub onboard_step: u8,
    pub share: Option<crate::moments::ShareCard>,
    pub summary_year: bool,
    pub summary_offset: i32,
    /// Waiting for the phone's file picker.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub picking: Option<Pick>,
    /// Room taken by the phone's bars and keyboard (left, top, right, bottom, keyboard), in points.
    insets: [f32; 5],
    /// When the app first asked the phone for `insets`.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    insets_at: Option<Instant>,
    applied_dark: Option<bool>,
    keyboard_shown: bool,
    frames: u64,
    shot: Option<Shot>,
}

impl GhostApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        let mut persist = screenshot.is_none();
        let mut load_error = None;
        let data = if persist {
            match model::load() {
                Ok(Some(d)) => d,
                Ok(None) => {
                    let mut d = Data::default();
                    d.settings.onboarded = false;
                    let _ = model::save(&d);
                    d
                }
                Err((msg, kept)) => {
                    // Never write over a file that could not be read unless a copy was put aside.
                    persist = kept;
                    load_error = Some(msg);
                    Data::default()
                }
            }
        } else {
            model::sample_data()
        };
        let crash_report = if screenshot.is_none() {
            let crash = std::fs::read_to_string(model::crash_file()).ok();
            match (load_error, crash) {
                (Some(a), Some(b)) => Some(format!("{a}\n\n{b}")),
                (a, b) => a.or(b),
            }
        } else {
            None
        };
        let rec = if persist {
            Recorder::restore(&data.settings).unwrap_or_else(|| Recorder::new(Sport::Run))
        } else {
            Recorder::new(Sport::Run)
        };
        let tab = if rec.active() || matches!(rec.state, RecState::Done { .. }) { Tab::Record } else { Tab::Home };
        draw::install_fonts(&cc.egui_ctx);
        let now = Local::now();
        let mut app = Self {
            data,
            persist,
            tab,
            detail: None,
            edit: None,
            sub: None,
            rec,
            manual: Form::blank(Sport::Run),
            show_manual: false,
            import_path: String::new(),
            toast: None,
            feed_filter: None,
            map_mode: MapMode::Routes,
            draft: Vec::new(),
            draft_name: String::new(),
            draft_sport: Sport::Run,
            goal_draft: model::Goal {
                id: 0,
                sport: Some(Sport::Run),
                period: model::Period::Week,
                metric: model::Metric::Distance,
                target: 20.0,
            },
            gear_draft: (String::new(), Sport::Run, 700.0),
            calc: ("10".into(), "55:00".into()),
            cal_month: (now.year(), now.month()),
            stats_sport: None,
            confirm_wipe: false,
            restore_path: String::new(),
            map_search: String::new(),
            map_coords: String::new(),
            map_queue: Vec::new(),
            map_task: None,
            map_import: String::new(),
            map_delete: None,
            crash_report,
            picking: None,
            celebrate: None,
            undo: None,
            onboard_step: 0,
            share: None,
            summary_year: false,
            summary_offset: 0,
            insets: [0.0; 5],
            insets_at: None,
            applied_dark: None,
            keyboard_shown: false,
            frames: 0,
            shot: screenshot.map(|path| Shot {
                path,
                requested: false,
                done: Arc::new(AtomicBool::new(false)),
            }),
        };
        if matches!(app.rec.state, RecState::Done { .. }) {
            // A finished recording that was not saved yet when the app closed.
            app.rec.form = app.recording_form();
        }
        app
    }

    /// Opens a given screen at start (handy for checking screens one by one).
    pub fn open_start_screen(&mut self, name: &str) {
        match name {
            "maps" => self.tab = Tab::Maps,
            "plan" => (self.tab, self.map_mode) = (Tab::Maps, MapMode::Plan),
            "heat" => (self.tab, self.map_mode) = (Tab::Maps, MapMode::Heat),
            "offline" => (self.tab, self.map_mode) = (Tab::Maps, MapMode::Offline),
            "record" => self.tab = Tab::Record,
            "goals" => self.tab = Tab::Goals,
            "you" => self.tab = Tab::You,
            "stats" => (self.tab, self.sub) = (Tab::You, Some(Sub::Stats)),
            "records" => (self.tab, self.sub) = (Tab::You, Some(Sub::Records)),
            "calendar" => (self.tab, self.sub) = (Tab::You, Some(Sub::Calendar)),
            "gear" => (self.tab, self.sub) = (Tab::You, Some(Sub::Gear)),
            "tools" => (self.tab, self.sub) = (Tab::You, Some(Sub::Tools)),
            "settings" => (self.tab, self.sub) = (Tab::You, Some(Sub::Settings)),
            "summary" => (self.tab, self.sub) = (Tab::You, Some(Sub::Summary)),
            "onboarding" => self.data.settings.onboarded = false,
            "share" => self.share = self.data.activities.first().map(|a| crate::moments::ShareCard::new(a.id)),
            "celebrate" => {
                self.celebrate = Some((vec!["Rekor 5K baru: 24:31".into(), "Tercepat di rute ini, 0:42 lebih cepat".into()], Instant::now()))
            }
            "recording" => {
                // A recording in progress, with a finished track standing in for live GPS.
                self.tab = Tab::Record;
                let settings = self.data.settings.clone();
                self.rec.begin(&settings);
                self.rec.gps = 2;
                self.rec.steps = 4210;
                if let Some(a) = self.data.activities.iter().find(|a| a.sport == Sport::Run && a.track.len() > 1) {
                    self.rec.track = a.track.clone();
                    let since_ms = chrono::Utc::now().timestamp_millis() - (a.duration_s * 1000.0) as i64;
                    self.rec.state = RecState::Running { since_ms, before: 0.0 };
                }
            }
            "detail" => self.detail = self.data.activities.first().map(|a| a.id),
            _ => {}
        }
    }

    pub fn save(&mut self) {
        self.data.sort();
        if self.persist {
            if let Err(e) = model::save(&self.data) {
                self.say(format!("Gagal menyimpan: {e}"));
            }
        }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 3.0));
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        self.frames += 1;
        ctx.request_repaint();
        if shot.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if self.frames >= 120 && !shot.requested {
            shot.requested = true;
            let path = shot.path.clone();
            let done = shot.done.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    Some(buf) => {
                        if let Err(e) = buf.save(&path) {
                            eprintln!("could not save screenshot: {e}");
                        }
                    }
                    None => eprintln!("screenshot had an unexpected size"),
                }
                done.store(true, Ordering::SeqCst);
            });
        }
    }
}

impl eframe::App for GhostApp {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        // While taking the check picture, ignore mouse and keys so the picture is always the same.
        if self.shot.is_some() {
            raw_input.events.clear();
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.applied_dark != Some(self.data.settings.dark) {
            let dark = self.data.settings.dark;
            ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
            self.applied_dark = Some(dark);
            let mut style = (*ctx.global_style()).clone();
            draw::theme(&mut style, dark);
            ctx.set_global_style(style);
            #[cfg(target_os = "android")]
            crate::android::edge_to_edge(dark);
        }
        #[cfg(target_os = "android")]
        {
            // The phone answers in the background, so the first frames may still see zeros.
            let started = *self.insets_at.get_or_insert_with(Instant::now);
            let ppp = ctx.pixels_per_point();
            let fresh = crate::android::insets().map(|px| px as f32 / ppp);
            if fresh != self.insets || (fresh == [0.0; 5] && started.elapsed().as_secs() < 3) {
                ctx.request_repaint();
            }
            self.insets = fresh;
        }
        let [_, top, _, bottom, keyboard] = self.insets;
        // GPS and steps keep coming in whichever page is open.
        self.rec.poll();
        if self.persist {
            self.rec.keep_safe();
        }
        #[cfg(target_os = "android")]
        if let Some(kind) = self.picking {
            ctx.request_repaint();
            if let Some(path) = crate::android::picked_file() {
                self.picking = None;
                if !path.is_empty() {
                    self.open_picked(kind, &PathBuf::from(path));
                }
            }
        }
        self.handle_screenshot(&ctx);

        if top > 0.0 {
            egui::Panel::top("status_bar")
                .exact_size(top)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(draw::bg(ui)))
                .show(ui, |_| {});
        }
        // While the keyboard is up it covers the tabs; leave room so the page can scroll above it.
        if keyboard > 0.0 {
            egui::Panel::bottom("keyboard")
                .exact_size(keyboard)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(draw::bg(ui)))
                .show(ui, |_| {});
        } else if !self.data.settings.onboarded || self.recording_screen() {
            // No tabs on the welcome screens or while recording (so a pocket tap can't leave the page).
            egui::Panel::bottom("system_bar")
                .exact_size(bottom.max(8.0))
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(draw::bg(ui)))
                .show(ui, |_| {});
        } else {
            egui::Panel::bottom("tabs")
                .exact_size(72.0 + bottom)
                .resizable(false)
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(if ui.visuals().dark_mode {
                    Color32::from_rgb(16, 16, 19)
                } else {
                    Color32::WHITE
                }))
                .show(ui, |ui| self.tab_bar(ui));
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(draw::bg(ui)).inner_margin(egui::Margin::symmetric(14, 0)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt((self.tab as u8, self.sub.map(|s| s as u8), self.detail))
                    .show(ui, |ui| {
                        // Keep a phone-like column even when the window is wide.
                        let full = ui.available_width();
                        let w = full.min(560.0);
                        ui.horizontal_top(|ui| {
                            ui.add_space((full - w) / 2.0);
                            ui.vertical(|ui| {
                                ui.set_width(w);
                                self.page(ui);
                            });
                        });
                    });
            });

        self.celebration(&ctx);
        self.share_overlay(&ctx);
        self.undo_toast(&ctx, bottom + keyboard);
        if let Some((msg, left)) = &mut self.toast {
            *left -= ctx.input(|i| i.stable_dt).min(0.1) as f64;
            let msg = msg.clone();
            egui::Area::new(egui::Id::unique("toast"))
                .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -86.0 - bottom - keyboard))
                .show(&ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(34, 34, 40))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(52, 52, 60)))
                        .corner_radius(22.0)
                        .inner_margin(egui::Margin::symmetric(16, 10))
                        .show(ui, |ui| ui.label(RichText::new(msg).color(Color32::WHITE)));
                });
            if *left <= 0.0 {
                self.toast = None;
            }
            ctx.request_repaint();
        }
        // Phone back button closes the open page.
        if ctx.input(|i| i.key_pressed(egui::Key::BrowserBack)) {
            if self.detail.is_some() || self.edit.is_some() {
                self.detail = None;
                self.edit = None;
            } else if self.sub.is_some() {
                self.sub = None;
            }
        }
        let wants_keys = ctx.egui_wants_keyboard_input();
        if wants_keys != self.keyboard_shown {
            self.keyboard_shown = wants_keys;
            #[cfg(target_os = "android")]
            crate::android::keyboard(wants_keys);
        }
        if matches!(self.rec.state, RecState::Running { .. }) {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

impl GhostApp {
    /// True while the Record page shows a recording in progress.
    fn recording_screen(&self) -> bool {
        self.tab == Tab::Record && self.rec.active() && self.detail.is_none() && self.sub.is_none()
    }

    fn page(&mut self, ui: &mut Ui) {
        ui.add_space(10.0);
        if !self.data.settings.onboarded {
            self.onboarding(ui);
        } else if let Some(id) = self.detail {
            self.activity_page(ui, id);
        } else if let Some(sub) = self.sub {
            self.sub_page(ui, sub);
        } else {
            match self.tab {
                Tab::Home => self.home(ui),
                Tab::Maps => self.maps(ui),
                Tab::Record => self.record(ui),
                Tab::Goals => self.goals(ui),
                Tab::You => self.you(ui),
            }
        }
        ui.add_space(24.0);
    }

    fn tab_bar(&mut self, ui: &mut Ui) {
        let tabs = [
            (Tab::Home, Icon::Home, "Beranda"),
            (Tab::Maps, Icon::Map, "Peta"),
            (Tab::Record, Icon::Record, "Rekam"),
            (Tab::Goals, Icon::Goals, "Target"),
            (Tab::You, Icon::You, "Anda"),
        ];
        let rect = ui.max_rect();
        ui.painter().hline(rect.x_range(), rect.top(), Stroke::new(1.0, draw::border(ui)));
        // Keep the tabs as wide as the page column on big windows.
        let width = rect.width().min(560.0);
        let left = rect.center().x - width / 2.0;
        let w = width / tabs.len() as f32;
        for (i, (tab, ic, label)) in tabs.into_iter().enumerate() {
            let r = egui::Rect::from_min_size(pos2(left + w * i as f32, rect.top()), vec2(w, rect.height()));
            let resp = ui.interact(r, ui.scope_id().with(("tab", i)), Sense::click());
            let active = self.tab == tab;
            let color = if active {
                ACCENT
            } else if resp.hovered() {
                draw::strong(ui)
            } else {
                draw::muted(ui)
            };
            let p = ui.painter();
            let icon_at = pos2(r.center().x, r.top() + 25.0);
            if tab == Tab::Record {
                p.circle_filled(icon_at, 23.0, ACCENT.gamma_multiply(0.2));
                p.circle_filled(icon_at, 18.0, ACCENT);
                draw::icon(p, ic, icon_at, Color32::WHITE);
            } else {
                if active {
                    p.rect_filled(
                        egui::Rect::from_center_size(pos2(r.center().x, r.top() + 1.5), vec2(28.0, 3.0)),
                        2.0,
                        ACCENT,
                    );
                }
                draw::icon(p, ic, icon_at, color);
            }
            p.text(
                pos2(r.center().x, r.top() + 56.0),
                Align2::CENTER_CENTER,
                label,
                FontId::new(11.5, if active { draw::semi() } else { egui::FontFamily::Proportional }),
                color,
            );
            if resp.clicked() {
                self.tab = tab;
                self.detail = None;
                self.sub = None;
                self.edit = None;
            }
        }
    }

    pub fn top_bar(&mut self, ui: &mut Ui, title: &str) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(28.0, 32.0), Sense::hover());
            draw::ghost(ui.painter(), r.center(), 28.0, ACCENT, draw::bg(ui));
            ui.label(draw::big(title, 26.0, draw::strong(ui)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.rec.active() {
                    let text = format!("Merekam {}", draw::clock(self.rec.moving()));
                    if draw::pill(ui, &text, ACCENT, true).interact(Sense::click()).clicked() {
                        self.tab = Tab::Record;
                        self.detail = None;
                        self.sub = None;
                    }
                } else {
                    draw::pill(ui, "Offline", draw::lime(ui), true).on_hover_text(
                        "Tidak ada akun. Internet hanya dipakai saat Anda mengunduh peta. Semua data hanya ada di perangkat ini.",
                    );
                }
            });
        });
        ui.add_space(10.0);
    }

    pub fn back_bar(&mut self, ui: &mut Ui, title: &str) -> bool {
        let mut back = false;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("‹").size(22.0).color(draw::strong(ui)))
                        .fill(draw::raised(ui))
                        .corner_radius(18.0)
                        .min_size(vec2(36.0, 36.0)),
                )
                .clicked()
            {
                back = true;
            }
            ui.add_space(4.0);
            ui.label(RichText::new(title).family(draw::semi()).size(18.0).color(draw::strong(ui)));
        });
        ui.add_space(10.0);
        back
    }

    // ---------- home ----------

    fn home(&mut self, ui: &mut Ui) {
        self.top_bar(ui, "GhostWalk");
        if let Some(report) = self.crash_report.clone() {
            draw::card(ui, |ui| {
                ui.label(RichText::new("App sempat tertutup karena error").family(draw::semi()).color(draw::RED));
                ui.label(
                    RichText::new("Kirim tangkapan layar atau salinan teks ini ke pembuat app agar bisa diperbaiki.")
                        .size(12.5)
                        .color(draw::muted(ui)),
                );
                ui.add(egui::Label::new(RichText::new(&report).monospace().size(11.5)).selectable(true));
                ui.horizontal(|ui| {
                    if ui.button("Salin").clicked() {
                        ui.ctx().copy_text(report.clone());
                        self.say("Disalin");
                    }
                    if ui.button("Tutup").clicked() {
                        let _ = std::fs::remove_file(model::crash_file());
                        self.crash_report = None;
                    }
                });
            });
            ui.add_space(8.0);
        }
        let imp = self.data.settings.imperial;

        // This week, with one bar per day.
        let today = Local::now().date_naive();
        let monday = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
        let mut per_day = [0.0f64; 7];
        let (mut dist, mut time, mut count, mut elev) = (0.0, 0.0, 0, 0.0);
        for a in self.data.in_period(model::Period::Week, None) {
            let d = (a.date().date_naive() - monday).num_days();
            if (0..7).contains(&d) {
                per_day[d as usize] += a.duration_s / 60.0;
            }
            dist += a.distance_m;
            time += a.duration_s;
            elev += a.elev_gain_m;
            count += 1;
        }
        let streak = self.data.streak_days();
        let wstreak = self.data.streak_weeks();
        draw::card(ui, |ui| {
            ui.horizontal(|ui| {
                draw::overline(ui, "Minggu ini");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if streak > 0 {
                        draw::pill(ui, &format!("{streak} hari beruntun"), draw::lime(ui), false)
                            .on_hover_text(format!("{wstreak} minggu beruntun"));
                    }
                });
            });
            let km = draw::dist(dist, imp);
            let (num, unit) = km.split_once(' ').unwrap_or((&km, ""));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(draw::big(num, 52.0, draw::strong(ui)));
                ui.label(draw::big(unit, 22.0, draw::muted(ui)));
            });
            draw::stat_grid(
                ui,
                &[
                    ("Waktu", draw::dur(time)),
                    ("Aktivitas", count.to_string()),
                    ("Tanjakan", format!("{elev:.0} m")),
                ],
                3,
            );
            let labels: Vec<String> = ["S", "S", "R", "K", "J", "S", "M"].iter().map(|s| s.to_string()).collect();
            draw::bars(ui, &per_day, &labels, ACCENT, 84.0, false);
        });

        // Goals preview.
        if let Some(g) = self.data.goals.first().cloned() {
            ui.add_space(8.0);
            let (_, clicked) = draw::card_click(ui, |ui| {
                let v = self.data.goal_progress(&g);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Target").size(13.0).family(draw::semi()).color(draw::strong(ui)));
                    ui.label(
                        RichText::new(format!(
                            "{} · {} {}",
                            g.sport.map(|s| s.name()).unwrap_or("Semua"),
                            g.period.name().to_lowercase(),
                            g.metric.name().to_lowercase()
                        ))
                        .size(12.0)
                        .color(draw::muted(ui)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.0}/{:.0}", v, g.target)).size(12.0).strong().family(draw::semi()));
                    });
                });
                draw::progress(ui, (v / g.target.max(0.001)) as f32, ACCENT);
            });
            if clicked {
                self.tab = Tab::Goals;
            }
        }

        // Filter chips.
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if draw::chip(ui, self.feed_filter.is_none(), "Semua").clicked() {
                self.feed_filter = None;
            }
            for s in Sport::ALL {
                if self.data.activities.iter().any(|a| a.sport == s)
                    && draw::chip(ui, self.feed_filter == Some(s), s.name()).clicked()
                {
                    self.feed_filter = Some(s);
                }
            }
        });
        ui.add_space(6.0);

        if self.data.activities.is_empty() {
            draw::card(ui, |ui| {
                ui.label("Belum ada aktivitas. Tekan Rekam untuk mulai, atau impor file GPX.");
                ui.horizontal_wrapped(|ui| {
                    if draw::accent_button(ui, "Mulai rekam").clicked() {
                        self.tab = Tab::Record;
                    }
                    if ui.button("Isi data contoh").clicked() {
                        self.load_samples();
                    }
                });
            });
        }

        let ids: Vec<u64> = self
            .data
            .activities
            .iter()
            .filter(|a| self.feed_filter.is_none_or(|s| s == a.sport))
            .map(|a| a.id)
            .collect();
        for id in ids {
            if self.activity_card(ui, id) {
                self.detail = Some(id);
                self.edit = None;
            }
            ui.add_space(8.0);
        }
    }

    /// Returns true when the card was tapped.
    fn activity_card(&mut self, ui: &mut Ui, id: u64) -> bool {
        let imp = self.data.settings.imperial;
        let nick = if self.data.settings.nickname.trim().is_empty() {
            "Anda".to_string()
        } else {
            self.data.settings.nickname.clone()
        };
        let Some(a) = self.data.activities.iter().find(|a| a.id == id).cloned() else {
            return false;
        };
        let mut fav_clicked = false;
        let (_, clicked) = draw::card_click(ui, |ui| {
            ui.horizontal(|ui| {
                draw::sport_badge(ui, a.sport, 40.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(&a.title).family(draw::semi()).size(17.0).color(draw::strong(ui)));
                    ui.label(
                        RichText::new(format!("{} · {} · {}", nick, draw::date_short(&a.date()), a.sport.name()))
                            .size(12.5)
                            .color(draw::muted(ui)),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if draw::star(ui, a.favorite).on_hover_text("Favorit").clicked() {
                        fav_clicked = true;
                    }
                });
            });
            ui.add_space(4.0);
            let mut items = Vec::new();
            if a.sport.has_distance() {
                items.push(("Jarak", draw::dist(a.distance_m, imp)));
                let (l, v) = draw::pace_or_speed(a.sport, a.speed_mps(), imp);
                items.push((l, v));
            } else {
                items.push(("Usaha", format!("{}/10", a.effort)));
                items.push(("Kalori", format!("{:.0}", a.calories(self.data.settings.weight_kg))));
            }
            items.push(("Waktu", draw::dur(a.duration_s)));
            draw::stat_grid(ui, &items, 3);
            if a.track.len() > 1 {
                let around = draw::card_bg(ui);
                draw::route_map(ui, &a.track, draw::sport_color(a.sport), 180.0, false, around);
            }
        });
        if fav_clicked {
            if let Some(x) = self.data.activities.iter_mut().find(|x| x.id == id) {
                x.favorite = !x.favorite;
            }
            self.save();
            return false;
        }
        clicked
    }

    // ---------- activity page ----------

    fn activity_page(&mut self, ui: &mut Ui, id: u64) {
        let Some(a) = self.data.activities.iter().find(|a| a.id == id).cloned() else {
            self.detail = None;
            return;
        };
        if self.back_bar(ui, "Aktivitas") {
            self.detail = None;
            self.edit = None;
            return;
        }
        let imp = self.data.settings.imperial;

        if let Some(mut form) = self.edit.take() {
            let mut keep = true;
            draw::card(ui, |ui| {
                draw::heading(ui, "Ubah aktivitas");
                self.form_ui(ui, &mut form, true);
                ui.horizontal(|ui| {
                    if draw::accent_button(ui, "Simpan").clicked() {
                        let mut copy = a.clone();
                        match form.apply(&mut copy) {
                            Ok(()) => {
                                if let Some(x) = self.data.activities.iter_mut().find(|x| x.id == id) {
                                    *x = copy;
                                }
                                self.save();
                                self.say("Perubahan disimpan");
                                keep = false;
                            }
                            Err(e) => self.say(e),
                        }
                    }
                    if ui.button("Batal").clicked() {
                        keep = false;
                    }
                });
            });
            if keep {
                self.edit = Some(form);
            }
            return;
        }

        if a.track.len() > 1 {
            let around = draw::bg(ui);
            draw::route_map(ui, &a.track, draw::sport_color(a.sport), 330.0, true, around);
            ui.add_space(-26.0);
        }
        draw::card(ui, |ui| {
            // A handle, so the card reads as a sheet lying on the map.
            let (h, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
            ui.painter().rect_filled(egui::Rect::from_center_size(h.center(), vec2(40.0, 5.0)), 3.0, draw::border(ui));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                draw::sport_badge(ui, a.sport, 44.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(RichText::new(&a.title).size(21.0).family(draw::heavy()).color(draw::strong(ui)));
                    ui.label(
                        RichText::new(format!("{} · {}", draw::date_long(&a.date()), a.sport.name()))
                            .size(12.5)
                            .color(draw::muted(ui)),
                    );
                });
            });
            ui.add_space(10.0);
            let mut items: Vec<(&str, String)> = Vec::new();
            if a.sport.has_distance() {
                items.push(("Jarak", draw::dist(a.distance_m, imp)));
                let (l, v) = draw::pace_or_speed(a.sport, a.speed_mps(), imp);
                items.push((l, v));
            }
            items.push(("Waktu bergerak", draw::clock(a.duration_s)));
            if a.sport.has_distance() {
                items.push(("Tanjakan", format!("{:.0} m", a.elev_gain_m)));
            }
            items.push(("Kalori (perkiraan)", format!("{:.0}", a.calories(self.data.settings.weight_kg))));
            items.push(("Usaha", format!("{}/10", a.effort)));
            if let Some(hr) = a.avg_hr {
                items.push(("Detak jantung", format!("{hr} bpm")));
                let zone = hr_zone(hr, self.data.settings.max_hr);
                items.push(("Zona", format!("Z{zone}")));
            }
            if let Some(steps) = a.steps.filter(|s| *s > 0) {
                items.push(("Langkah", steps.to_string()));
            }
            if let Some(c) = a.cadence() {
                items.push(("Langkah/menit", format!("{c:.0}")));
            }
            if let Some(g) = a.gear.and_then(|g| self.data.gear.iter().find(|x| x.id == g)) {
                items.push(("Perlengkapan", g.name.clone()));
            }
            draw::stat_grid(ui, &items, 2);
            if !a.notes.is_empty() {
                ui.separator();
                ui.label(&a.notes);
            }
        });

        // How this one compares with earlier times on the same route.
        let route = self.data.same_route(&a);
        if !route.is_empty() {
            ui.add_space(8.0);
            draw::card(ui, |ui| {
                draw::heading(ui, "Rute ini");
                let best = route.iter().map(|b| b.duration_s).fold(a.duration_s, f64::min);
                let n = route.len() + 1;
                ui.label(RichText::new(format!("Sudah {n} kali lewat rute ini.")).size(13.5).color(draw::muted(ui)));
                if a.duration_s <= best + 0.5 {
                    draw::pill(ui, "Tercepat di rute ini", draw::lime(ui), true);
                } else {
                    ui.label(
                        RichText::new(format!(
                            "{} lebih lambat dari waktu terbaik ({})",
                            draw::clock(a.duration_s - best),
                            draw::clock(best)
                        ))
                        .family(draw::semi())
                        .color(draw::strong(ui)),
                    );
                }
                let mut all: Vec<&Activity> = route.clone();
                all.push(&a);
                all.sort_by_key(|b| b.start);
                let vals: Vec<f64> = all.iter().map(|b| b.duration_s / 60.0).collect();
                let labels: Vec<String> = all.iter().map(|b| b.date().format("%d/%m").to_string()).collect();
                draw::bars(ui, &vals, &labels, draw::sport_color(a.sport), 80.0, false);
            });
        }

        let profile = a.speed_profile();
        if profile.len() > 4 {
            ui.add_space(8.0);
            draw::card(ui, |ui| {
                draw::heading(ui, if a.sport.uses_speed() { "Kecepatan" } else { "Tempo" });
                let sport = a.sport;
                draw::speed_chart(ui, &profile, draw::sport_color(a.sport), 130.0, move |v| {
                    draw::pace_or_speed(sport, v, imp).1
                });
            });
        }

        let splits = a.splits();
        if !splits.is_empty() {
            ui.add_space(8.0);
            draw::card(ui, |ui| {
                draw::heading(ui, "Split per km");
                let best = splits.iter().cloned().fold(f64::MAX, f64::min);
                let worst = splits.iter().cloned().fold(0.0, f64::max);
                for (i, s) in splits.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:>2}", i + 1)).monospace().color(draw::muted(ui)));
                        ui.label(RichText::new(draw::pace(1000.0 / s, false)).monospace());
                        let w = ui.available_width() - 8.0;
                        let frac = if worst > 0.0 && *s > 0.0 { (best / s).clamp(0.0, 1.0) as f32 } else { 1.0 };
                        let (r, _) = ui.allocate_exact_size(vec2(w * frac, 12.0), Sense::hover());
                        let c = if *s == best { ACCENT } else { draw::sport_color(a.sport).gamma_multiply(0.55) };
                        ui.painter().rect_filled(r, 3.0, c);
                    });
                }
            });
        }

        let prof = a.elevation_profile();
        if prof.len() > 2 {
            ui.add_space(8.0);
            draw::card(ui, |ui| {
                draw::heading(ui, "Profil ketinggian");
                draw::area(ui, &prof, Color32::from_rgb(120, 130, 140), 110.0);
            });
        }
        if !a.laps.is_empty() {
            ui.add_space(8.0);
            draw::card(ui, |ui| {
                draw::heading(ui, "Putaran");
                let mut prev = 0.0;
                for (i, t) in a.laps.iter().enumerate() {
                    ui.label(format!("Putaran {}: {}", i + 1, draw::clock(t - prev)));
                    prev = *t;
                }
                // The stretch from the last lap press to the finish.
                if a.duration_s > prev + 1.0 {
                    ui.label(format!("Putaran {}: {}", a.laps.len() + 1, draw::clock(a.duration_s - prev)));
                }
            });
        }

        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            if draw::accent_button(ui, "Bagikan").clicked() {
                self.share = Some(crate::moments::ShareCard::new(a.id));
            }
            if ui.button("Ubah").clicked() {
                self.edit = Some(Form::from(&a));
            }
            if ui.button(if a.favorite { "Hapus favorit" } else { "Jadikan favorit" }).clicked() {
                if let Some(x) = self.data.activities.iter_mut().find(|x| x.id == id) {
                    x.favorite = !x.favorite;
                }
                self.save();
            }
            if a.track.len() > 1 && ui.button("Simpan GPX").clicked() {
                let name = format!("aktivitas-{}.gpx", a.date().format("%Y%m%d-%H%M"));
                let path = self.export_dir().join(name);
                let r = std::fs::create_dir_all(self.export_dir()).and_then(|_| std::fs::write(&path, gpx::write(&a)));
                match r {
                    Ok(()) => self.publish(&path, "application/gpx+xml", true),
                    Err(e) => self.say(format!("Gagal: {e}")),
                }
            }
            if a.track.len() > 1 && ui.button("Jadikan rute").clicked() {
                let pts = draw::project(&a.track);
                let (x0, y0) = (pts[0][0], pts[0][1]);
                let step = (pts.len() / 60).max(1);
                let points = pts.iter().step_by(step).map(|p| [(p[0] - x0) as f32, (p[1] - y0) as f32]).collect();
                let rid = self.data.new_id();
                self.data.routes.push(model::Route {
                    id: rid,
                    name: format!("Rute: {}", a.title),
                    sport: a.sport,
                    points,
                });
                self.save();
                self.say("Rute disimpan di Peta");
            }
            if ui.button(RichText::new("Hapus").color(draw::RED)).clicked() {
                self.delete_activity(id);
            }
        });
    }

    /// Tells where an exported file went; on phones it is first copied to Download/GhostWalk
    /// (or the gallery for pictures), optionally opening the share sheet.
    pub fn publish(&mut self, path: &std::path::Path, mime: &str, share: bool) {
        #[cfg(target_os = "android")]
        if let Some(place) = crate::android::publish(path, mime, share) {
            self.say(format!("Disimpan di {place}"));
            return;
        }
        let _ = (mime, share);
        self.say(format!("Disimpan ke {}", path.display()));
    }

    pub fn export_dir(&self) -> PathBuf {
        if let Some(d) = std::env::var_os("GHOSTWALK_EXPORT") {
            return PathBuf::from(d);
        }
        model::data_dir().join("ekspor")
    }

    // ---------- forms ----------

    pub fn form_ui(&mut self, ui: &mut Ui, f: &mut Form, with_sport: bool) {
        let w = ui.available_width();
        if with_sport {
            ui.horizontal_wrapped(|ui| {
                for s in Sport::ALL {
                    if draw::chip(ui, f.sport == s, s.name()).clicked() {
                        f.sport = s;
                    }
                }
            });
        }
        let field = |ui: &mut Ui, label: &str, val: &mut String, hint: &str| {
            ui.label(RichText::new(label).size(12.0).color(draw::muted(ui)));
            ui.add(egui::TextEdit::singleline(val).hint_text(hint).desired_width(w));
        };
        field(ui, "Judul", &mut f.title, "mis. Lari pagi");
        field(ui, "Tanggal & jam", &mut f.when, "2026-11-02 07:30");
        ui.columns(2, |c| {
            if f.sport.has_distance() {
                c[0].label(RichText::new("Jarak (km)").size(12.0));
                c[0].add(egui::TextEdit::singleline(&mut f.dist_km).hint_text("5,0"));
            }
            c[1].label(RichText::new("Waktu (j:m:d)").size(12.0));
            c[1].add(egui::TextEdit::singleline(&mut f.time).hint_text("30:00"));
        });
        ui.columns(2, |c| {
            c[0].label(RichText::new("Tanjakan (m)").size(12.0));
            c[0].add(egui::TextEdit::singleline(&mut f.elev).hint_text("0"));
            c[1].label(RichText::new("Detak rata-rata").size(12.0));
            c[1].add(egui::TextEdit::singleline(&mut f.hr).hint_text("opsional"));
        });
        ui.label(RichText::new("Seberapa berat? (1 ringan – 10 maksimal)").size(12.0).color(draw::muted(ui)));
        ui.add(egui::Slider::new(&mut f.effort, 1..=10));
        let gear: Vec<(u64, String)> = self
            .data
            .gear
            .iter()
            .filter(|g| !g.retired)
            .map(|g| (g.id, g.name.clone()))
            .collect();
        if !gear.is_empty() {
            let cur = f
                .gear
                .and_then(|id| gear.iter().find(|g| g.0 == id))
                .map(|g| g.1.clone())
                .unwrap_or_else(|| "Tanpa perlengkapan".into());
            egui::ComboBox::from_label("Perlengkapan")
                .selected_text(cur)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut f.gear, None, "Tanpa perlengkapan");
                    for (id, n) in &gear {
                        ui.selectable_value(&mut f.gear, Some(*id), n);
                    }
                });
        }
        ui.label(RichText::new("Catatan").size(12.0).color(draw::muted(ui)));
        ui.add(egui::TextEdit::multiline(&mut f.notes).desired_rows(2).desired_width(w));
    }

    pub fn empty_activity(&mut self, sport: Sport, start: i64) -> Activity {
        Activity {
            id: self.data.new_id(),
            sport,
            title: String::new(),
            start,
            duration_s: 0.0,
            distance_m: 0.0,
            elev_gain_m: 0.0,
            avg_hr: None,
            effort: 5,
            notes: String::new(),
            gear: None,
            track: Vec::new(),
            favorite: false,
            laps: Vec::new(),
            steps: None,
        }
    }

    /// Adds a new activity, saves, and celebrates any record it sets.
    pub fn add_activity(&mut self, a: Activity) {
        let wins = self.wins(&a);
        self.data.activities.push(a);
        self.save();
        if !wins.is_empty() {
            self.celebrate = Some((wins, Instant::now()));
        }
    }

    /// A button that opens the phone's file picker (phones only).
    pub fn pick_button(&mut self, ui: &mut Ui, kind: Pick, text: &str) {
        if self.picking == Some(kind) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Menunggu file…");
            });
        } else if draw::accent_button(ui, text).clicked() {
            self.picking = Some(kind);
            #[cfg(target_os = "android")]
            crate::android::pick_file();
        }
    }

    /// Handles a file from the phone's file picker, then removes the app's temporary copy.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn open_picked(&mut self, kind: Pick, path: &std::path::Path) {
        match kind {
            Pick::Gpx => self.import_gpx(path),
            Pick::Backup => self.restore_backup(path),
            Pick::Map => match crate::mapdl::import(&path.to_string_lossy()) {
                Ok(name) => self.say(format!("Peta {name} dipasang")),
                Err(e) => self.say(format!("Gagal mengimpor: {e}")),
            },
        }
        let _ = std::fs::remove_file(path);
    }

    pub fn import_gpx(&mut self, path: &std::path::Path) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.say(format!("Tidak bisa membuka file: {e}"));
                return;
            }
        };
        match gpx::parse(&text) {
            Ok((name, start, track)) => {
                let (dist, gain, dur) = model::track_stats(&track);
                let sport = if dist > 0.0 && dur > 0.0 && dist / dur > 4.5 {
                    Sport::Ride
                } else if dist > 0.0 && dur > 0.0 && dist / dur < 1.8 {
                    Sport::Walk
                } else {
                    Sport::Run
                };
                let start = start.unwrap_or_else(|| Local::now().timestamp());
                let mut a = self.empty_activity(sport, start);
                a.title = name.unwrap_or_else(|| default_title(sport, start));
                a.distance_m = dist;
                a.elev_gain_m = gain;
                a.duration_s = if dur > 0.0 { dur } else { dist / 2.8 };
                a.track = track;
                let id = a.id;
                self.add_activity(a);
                self.say("GPX berhasil diimpor");
                self.detail = Some(id);
                self.tab = Tab::Home;
            }
            Err(e) => self.say(e),
        }
    }
}

pub fn hr_zone(hr: u32, max: u32) -> u32 {
    let f = hr as f32 / max.max(1) as f32;
    match f {
        x if x < 0.6 => 1,
        x if x < 0.7 => 2,
        x if x < 0.8 => 3,
        x if x < 0.9 => 4,
        _ => 5,
    }
}
