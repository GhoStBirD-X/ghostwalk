//! Recording: the clock, GPS, auto-pause, steps and intervals, and the Record screen.

use crate::app::{Form, GhostApp, Tab, default_title};
use crate::draw::{self, ACCENT};
use crate::model::{self, Interval, Phase, Sport, TrackPoint};
use chrono::{Local, TimeZone};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Stroke, Ui, vec2};
use serde::{Deserialize, Serialize};
use std::time::Instant;

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// "2026-11-02 07:30", as the activity form shows it.
fn when_text(start: i64) -> String {
    Local.timestamp_opt(start, 0).single().unwrap_or_else(Local::now).format("%Y-%m-%d %H:%M").to_string()
}

/// A recording in progress is kept here, so it survives the phone closing the app.
fn snapshot_file() -> std::path::PathBuf {
    model::data_dir().join("rekaman.json")
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    sport: Sport,
    state: RecState,
    laps: Vec<f64>,
    track: Vec<TrackPoint>,
    steps: u32,
    interval: Option<Interval>,
    auto_pause: bool,
    start_ms: i64,
    paused_s: f64,
    paused_since_ms: Option<i64>,
    #[serde(default)]
    route: Option<u64>,
    #[serde(default)]
    ghost: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum RecState {
    Idle,
    /// The wall clock is used (not a monotonic one) so time keeps counting while the phone sleeps.
    Running { since_ms: i64, before: f64 },
    Paused { total: f64 },
    Done { total: f64 },
}

pub struct Recorder {
    pub sport: Sport,
    pub state: RecState,
    pub laps: Vec<f64>,
    pub form: Form,
    /// Positions from the phone's GPS (empty on computers). `t` is recording time.
    pub track: Vec<TrackPoint>,
    /// 0 = no permission, 1 = waiting for signal, 2 = receiving, 3 = Location off; -1 = no GPS on this device.
    pub gps: i32,
    /// When the app last asked the phone to start GPS.
    pub gps_asked: Option<Instant>,
    pub steps: u32,
    /// The interval plan for this recording, if any.
    pub interval: Option<Interval>,
    pub auto_pause: bool,
    /// Unix ms at the start, and seconds spent paused by hand, to put GPS fixes on the recording clock.
    start_ms: i64,
    paused_s: f64,
    paused_since_ms: Option<i64>,
    /// Steps counted before the app was restarted in the middle of this recording.
    steps_before: u32,
    /// "Buang" was tapped once; a second tap throws the recording away.
    confirm_discard: bool,
    /// The saved route to follow and the activity to race, chosen before the start.
    pub route: Option<u64>,
    pub ghost: Option<u64>,
    /// Filled in from those by `arm`: the route's points and metres along it, and the ghost.
    pub plan: Vec<TrackPoint>,
    pub plan_cum: Vec<f64>,
    pub ghost_run: Option<model::Ghost>,
    /// Metres along the route reached so far, and how far the last position was from it.
    pub route_at: f64,
    pub off_m: Option<f64>,
    /// What the copy in `snapshot_file` was written for: (state, laps), when, and whether it exists.
    saved_key: Option<(u8, usize)>,
    saved_at: Option<Instant>,
    saved_file: bool,
    save_failed: bool,
}

impl Recorder {
    pub fn new(sport: Sport) -> Self {
        Self {
            sport,
            state: RecState::Idle,
            laps: Vec::new(),
            form: Form::blank(sport),
            track: Vec::new(),
            gps: if cfg!(target_os = "android") { 1 } else { -1 },
            gps_asked: None,
            steps: 0,
            interval: None,
            auto_pause: false,
            start_ms: 0,
            paused_s: 0.0,
            paused_since_ms: None,
            steps_before: 0,
            confirm_discard: false,
            route: None,
            ghost: None,
            plan: Vec::new(),
            plan_cum: Vec::new(),
            ghost_run: None,
            route_at: 0.0,
            off_m: None,
            saved_key: None,
            saved_at: None,
            saved_file: false,
            save_failed: false,
        }
    }

    /// Picks up a recording that was still going (or not saved yet) when the app closed.
    pub fn restore(settings: &model::Settings) -> Option<Self> {
        let s: Snapshot = serde_json::from_str(&std::fs::read_to_string(snapshot_file()).ok()?).ok()?;
        if matches!(s.state, RecState::Idle) {
            return None;
        }
        let mut r = Self::new(s.sport);
        r.state = s.state;
        r.laps = s.laps;
        r.track = s.track;
        r.steps = s.steps;
        r.steps_before = s.steps;
        r.interval = s.interval;
        r.auto_pause = s.auto_pause;
        r.start_ms = s.start_ms;
        r.paused_s = s.paused_s;
        r.paused_since_ms = s.paused_since_ms;
        r.route = s.route;
        r.ghost = s.ghost;
        r.saved_file = true;
        #[cfg(target_os = "android")]
        if r.active() {
            crate::android::coach_begin(
                settings.voice && r.sport.has_distance(),
                r.sport.uses_speed(),
                r.auto_pause,
                r.sport.min_moving_speed(),
                r.interval.map(|i| (i.work_s, i.rest_s, i.reps)),
            );
            let first = r.track.first().and_then(|p| p.t).unwrap_or(-1.0);
            crate::android::coach_carry_over(r.distance(), r.elapsed(), r.moving(), first);
            if matches!(r.state, RecState::Paused { .. }) {
                crate::android::coach_pause(true);
            }
        }
        let _ = settings;
        Some(r)
    }

    /// Writes the recording to `snapshot_file` when it changes and every 10 s while running,
    /// and removes the file once the recording is saved or thrown away.
    pub fn keep_safe(&mut self) {
        let tag = match self.state {
            RecState::Idle => {
                if self.saved_file {
                    let _ = std::fs::remove_file(snapshot_file());
                    self.saved_file = false;
                }
                self.saved_key = None;
                return;
            }
            RecState::Running { .. } => 1,
            RecState::Paused { .. } => 2,
            RecState::Done { .. } => 3,
        };
        let key = (tag, self.laps.len());
        let waited = self.saved_at.is_none_or(|t| t.elapsed().as_secs() >= 10);
        let changed = self.saved_key != Some(key);
        // After a failed write, wait before trying again rather than retrying every frame.
        let due = if self.save_failed { waited } else { changed || (tag == 1 && waited) };
        if !due {
            return;
        }
        let snap = Snapshot {
            sport: self.sport,
            state: self.state,
            laps: self.laps.clone(),
            track: self.track.clone(),
            steps: self.steps,
            interval: self.interval,
            auto_pause: self.auto_pause,
            start_ms: self.start_ms,
            paused_s: self.paused_s,
            paused_since_ms: self.paused_since_ms,
            route: self.route,
            ghost: self.ghost,
        };
        let path = snapshot_file();
        let tmp = path.with_extension("json.tmp");
        let ok = std::fs::create_dir_all(model::data_dir())
            .and_then(|_| std::fs::write(&tmp, serde_json::to_vec(&snap).unwrap_or_default()))
            .and_then(|_| std::fs::rename(&tmp, &path))
            .is_ok();
        self.saved_at = Some(Instant::now());
        self.save_failed = !ok;
        if ok {
            self.saved_key = Some(key);
            self.saved_file = true;
        }
    }

    pub fn active(&self) -> bool {
        matches!(self.state, RecState::Running { .. } | RecState::Paused { .. })
    }

    pub fn begin(&mut self, settings: &model::Settings) {
        let now = now_ms();
        self.state = RecState::Running { since_ms: now, before: 0.0 };
        self.start_ms = now;
        self.paused_s = 0.0;
        self.paused_since_ms = None;
        self.laps.clear();
        self.track.clear();
        self.steps = 0;
        self.steps_before = 0;
        self.confirm_discard = false;
        self.gps_asked = None;
        self.route_at = 0.0;
        self.off_m = None;
        self.interval = settings.interval.on.then_some(settings.interval);
        self.auto_pause = settings.auto_pause && self.sport.has_distance();
        #[cfg(target_os = "android")]
        crate::android::coach_begin(
            settings.voice && self.sport.has_distance(),
            self.sport.uses_speed(),
            self.auto_pause,
            self.sport.min_moving_speed(),
            self.interval.map(|i| (i.work_s, i.rest_s, i.reps)),
        );
    }

    /// Looks up the chosen route and ghost in `data`, dropping a choice that no longer exists.
    pub fn arm(&mut self, data: &model::Data) {
        let route = self.route.and_then(|id| data.routes.iter().find(|r| r.id == id && r.geo.len() >= 2));
        self.route = route.map(|r| r.id);
        self.plan = route.map(|r| r.track()).unwrap_or_default();
        self.plan_cum = model::cumulative(&self.plan);
        self.ghost_run = self
            .ghost
            .and_then(|id| data.activities.iter().find(|a| a.id == id))
            .and_then(|a| model::Ghost::new(a.id, &a.track));
        self.ghost = self.ghost_run.as_ref().map(|g| g.id);
    }

    /// Hands the route and ghost to the phone's coach, for spoken updates with the screen off.
    pub fn send_plan(&self) {
        #[cfg(target_os = "android")]
        {
            let route: Vec<f64> = self.plan.iter().flat_map(|p| [p.lat, p.lon]).collect();
            let ghost: Vec<f64> = self
                .ghost_run
                .as_ref()
                .map(|g| g.cum.iter().zip(&g.times).flat_map(|(d, t)| [*d, *t]).collect())
                .unwrap_or_default();
            crate::android::coach_plan(&route, &ghost);
        }
    }

    /// The clock for racing the ghost: from the first GPS position, as the ghost's own clock is,
    /// so the wait for a signal is not counted. None before the first position.
    pub fn race_clock(&self) -> Option<f64> {
        let t0 = self.track.first()?.t?;
        Some((self.elapsed() - t0).max(0.0))
    }

    /// The route actually shown: the planned one, or else the ghost's.
    pub fn shown_plan(&self) -> &[TrackPoint] {
        match &self.ghost_run {
            Some(g) if self.plan.is_empty() => &g.track,
            _ => &self.plan,
        }
    }

    /// Keeps up where on the route the newest position is.
    pub fn follow(&mut self) {
        let Some(last) = self.track.last().copied() else {
            self.off_m = None;
            return;
        };
        if self.plan.len() < 2 || !self.active() {
            self.off_m = None;
            return;
        }
        let (mut along, mut off) = model::locate_on(&self.plan, &self.plan_cum, &last, self.route_at, 300.0);
        if off > 150.0 {
            // Joined the route somewhere else (a shortcut, or a start halfway along it).
            let (a, o) = model::locate_on(&self.plan, &self.plan_cum, &last, 0.0, f64::MAX);
            if o < 50.0 {
                (along, off) = (a, o);
            }
        }
        if off < 100.0 {
            self.route_at = along;
        }
        self.off_m = Some(off);
    }

    pub fn pause(&mut self) {
        self.state = RecState::Paused { total: self.elapsed() };
        self.paused_since_ms = Some(now_ms());
        self.confirm_discard = false;
        #[cfg(target_os = "android")]
        crate::android::coach_pause(true);
    }

    pub fn resume(&mut self) {
        if let RecState::Paused { total } = self.state {
            let now = now_ms();
            self.paused_s += self.paused_since_ms.take().map_or(0.0, |p| (now - p) as f64 / 1000.0);
            self.state = RecState::Running { since_ms: now, before: total };
            self.confirm_discard = false;
            #[cfg(target_os = "android")]
            crate::android::coach_pause(false);
        }
    }

    /// Stops GPS and coaching (phones).
    pub fn stop(&mut self) {
        // Forget "receiving", or the next recording would never ask the phone to start GPS again.
        if self.gps == 2 {
            self.gps = 1;
        }
        self.gps_asked = None;
        #[cfg(target_os = "android")]
        crate::android::gps_stop();
    }

    pub fn distance(&self) -> f64 {
        model::track_stats(&self.track).0
    }

    /// Clock time, leaving out time paused by hand.
    pub fn elapsed(&self) -> f64 {
        match self.state {
            RecState::Idle => 0.0,
            RecState::Running { since_ms, before } => before + (now_ms() - since_ms).max(0) as f64 / 1000.0,
            RecState::Paused { total } | RecState::Done { total } => total,
        }
    }

    /// The time that counts: with auto-pause, standing still is left out.
    pub fn moving(&self) -> f64 {
        let e = self.elapsed();
        if self.auto_pause {
            (e - model::stopped_time(&self.track, self.sport.min_moving_speed())).max(0.0)
        } else {
            e
        }
    }

    /// True while auto-pause sees no movement in the last few seconds of GPS.
    pub fn standing_still(&self) -> bool {
        if !self.auto_pause || !matches!(self.state, RecState::Running { .. }) {
            return false;
        }
        let Some(last) = self.track.last().and_then(|p| p.t) else { return false };
        let Some(from) = self.track.iter().rev().find(|p| p.t.is_some_and(|t| last - t >= 8.0)) else {
            return false;
        };
        let dt = last - from.t.unwrap_or(last);
        dt > 0.0 && model::haversine(from, self.track.last().unwrap()) / dt < self.sport.min_moving_speed()
    }

    /// Reads new GPS positions and steps (phones only) and keeps the good positions while recording.
    pub fn poll(&mut self) {
        #[cfg(target_os = "android")]
        {
            let running = matches!(self.state, RecState::Running { .. });
            if running {
                if self.gps != 2 && self.gps_asked.is_none_or(|t| t.elapsed().as_secs() >= 3) {
                    self.gps_asked = Some(Instant::now());
                    crate::android::gps_start();
                }
                // Also while receiving, so a lost signal or Location switched off shows up.
                self.gps = crate::android::gps_state();
            }
            for [lat, lon, ele, acc, ms] in crate::android::gps_drain() {
                if running && acc <= 35.0 {
                    let t = (ms - self.start_ms as f64) / 1000.0 - self.paused_s;
                    self.track.push(TrackPoint { lat, lon, ele: Some(ele), t: Some(t.max(0.0)) });
                }
            }
            if self.active() {
                self.steps = self.steps_before + crate::android::steps();
            }
        }
    }
}

/// "1:05" for interval countdowns.
fn mmss(s: f64) -> String {
    let s = s.max(0.0).ceil() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

const WORK_CHOICES: [u32; 6] = [30, 60, 90, 120, 180, 300];
const REST_CHOICES: [u32; 6] = [0, 30, 60, 90, 120, 180];

impl GhostApp {
    pub fn record(&mut self, ui: &mut Ui) {
        if self.rec.active() {
            self.record_focus(ui);
            return;
        }
        self.top_bar(ui, "Rekam");
        if let RecState::Done { .. } = self.rec.state {
            self.save_form(ui);
            return;
        }
        self.record_start(ui);
        self.record_extras(ui);
    }

    /// Before recording: sport, interval plan and options, and the start button.
    fn record_start(&mut self, ui: &mut Ui) {
        draw::card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for s in Sport::ALL {
                    if draw::chip(ui, self.rec.sport == s, s.name()).clicked() && self.rec.sport != s {
                        self.rec.sport = s;
                        // Routes and ghosts belong to one sport.
                        self.rec.route = None;
                        self.rec.ghost = None;
                        self.rec.arm(&self.data);
                    }
                }
            });
            ui.add_space(6.0);
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 190.0), Sense::hover());
            let p = ui.painter();
            let c = r.center();
            p.circle_filled(c, 86.0, ACCENT.gamma_multiply(0.06));
            p.circle_stroke(c, 76.0, Stroke::new(8.0, draw::raised(ui)));
            draw::sport_icon(p, self.rec.sport, c - vec2(0.0, 24.0), 34.0, draw::sport_color(self.rec.sport));
            p.text(c + vec2(0.0, 22.0), Align2::CENTER_CENTER, "00:00", FontId::new(40.0, draw::heavy()), draw::strong(ui));
            p.text(c + vec2(0.0, 52.0), Align2::CENTER_CENTER, "SIAP", FontId::new(12.0, draw::semi()), draw::muted(ui));

            let s = &mut self.data.settings;
            let before = (s.voice, s.auto_pause, s.interval);
            ui.horizontal_wrapped(|ui| {
                if self.rec.sport.has_distance() {
                    if cfg!(target_os = "android") && draw::chip(ui, s.voice, "Suara tiap km").clicked() {
                        s.voice = !s.voice;
                    }
                    if draw::chip(ui, s.auto_pause, "Jeda otomatis").clicked() {
                        s.auto_pause = !s.auto_pause;
                    }
                }
                if draw::chip(ui, s.interval.on, "Interval").clicked() {
                    s.interval.on = !s.interval.on;
                }
            });
            if s.interval.on {
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(draw::bg(ui))
                    .corner_radius(14.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let row = |ui: &mut Ui, label: &str, value: &mut u32, choices: &[u32]| {
                            ui.label(RichText::new(label).size(12.5).color(draw::muted(ui)));
                            ui.horizontal_wrapped(|ui| {
                                for c in choices {
                                    let text = if *c == 0 { "Tanpa".to_string() } else { mmss(*c as f64) };
                                    if draw::chip(ui, *value == *c, &text).clicked() {
                                        *value = *c;
                                    }
                                }
                            });
                        };
                        row(ui, "Lari (menit:detik)", &mut s.interval.work_s, &WORK_CHOICES);
                        row(ui, "Jalan santai", &mut s.interval.rest_s, &REST_CHOICES);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Jumlah set").size(12.5).color(draw::muted(ui)));
                            ui.add(egui::DragValue::new(&mut s.interval.reps).range(1..=30).speed(0.1));
                            let total = (s.interval.work_s + s.interval.rest_s) * s.interval.reps;
                            ui.label(RichText::new(format!("total {}", mmss(total as f64))).size(12.5).color(draw::muted(ui)));
                        });
                    });
            }
            let changed = before != (s.voice, s.auto_pause, s.interval);
            if changed {
                self.save();
            }
            self.record_plan(ui);
            ui.add_space(6.0);
            if draw::accent_button(ui, "Mulai").clicked() {
                let settings = self.data.settings.clone();
                self.rec.arm(&self.data);
                self.rec.begin(&settings);
                self.rec.send_plan();
            }
            if self.rec.gps < 0 {
                ui.label(
                    RichText::new("Di komputer tidak ada GPS: stopwatch mencatat waktu, jarak diisi saat selesai.")
                        .size(11.5)
                        .color(draw::muted(ui)),
                );
            }
        });
    }

    /// Before the start: a saved route to follow and an earlier activity to race (the "ghost").
    fn record_plan(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        let sport = self.rec.sport;
        let routes: Vec<(u64, String)> = self
            .data
            .routes
            .iter()
            .filter(|r| r.sport == sport && r.geo.len() >= 2)
            .map(|r| (r.id, r.name.clone()))
            .collect();
        let route = self.rec.route.and_then(|id| self.data.routes.iter().find(|r| r.id == id)).cloned();
        let ghosts: Vec<(u64, String)> = self
            .data
            .ghost_choices(sport, route.as_ref())
            .iter()
            .take(6)
            .map(|a| {
                let t = a.track.last().and_then(|p| p.t).unwrap_or(0.0) - a.track[0].t.unwrap_or(0.0);
                (a.id, format!("{} · {} · {}", a.date().format("%d/%m"), draw::dist(a.distance_m, imp), draw::clock(t)))
            })
            .collect();
        if routes.is_empty() && ghosts.is_empty() {
            return;
        }
        let (old_route, old_ghost) = (self.rec.route, self.rec.ghost);
        ui.add_space(6.0);
        if !routes.is_empty() {
            ui.label(RichText::new("Rute").size(12.5).color(draw::muted(ui)));
            ui.horizontal_wrapped(|ui| {
                if draw::chip(ui, self.rec.route.is_none(), "Bebas").clicked() {
                    self.rec.route = None;
                }
                for (id, name) in &routes {
                    if draw::chip(ui, self.rec.route == Some(*id), name).clicked() {
                        self.rec.route = Some(*id);
                    }
                }
            });
        }
        if self.rec.route != old_route {
            // A new route: race the best time on it, if there is one.
            let r = self.rec.route.and_then(|id| self.data.routes.iter().find(|r| r.id == id));
            self.rec.ghost = r.and_then(|r| self.data.ghost_choices(sport, Some(r)).first().map(|a| a.id));
            self.rec.arm(&self.data);
            return;
        }
        ui.add_space(4.0);
        ui.label(RichText::new("Lawan hantu (lari Anda sebelumnya)").size(12.5).color(draw::muted(ui)));
        if ghosts.is_empty() {
            ui.label(
                RichText::new("Belum ada lari di rute ini. Lari ini nanti bisa jadi hantunya.").size(11.5).color(draw::muted(ui)),
            );
        } else {
            ui.horizontal_wrapped(|ui| {
                if draw::chip(ui, self.rec.ghost.is_none(), "Tanpa hantu").clicked() {
                    self.rec.ghost = None;
                }
                for (i, (id, text)) in ghosts.iter().enumerate() {
                    let text = if i == 0 && route.is_some() { format!("Terbaik {text}") } else { text.clone() };
                    if draw::chip(ui, self.rec.ghost == Some(*id), &text).clicked() {
                        self.rec.ghost = Some(*id);
                    }
                }
            });
        }
        if self.rec.ghost != old_ghost {
            self.rec.arm(&self.data);
        }
        let shown = self.rec.shown_plan();
        if shown.len() > 1 {
            ui.add_space(6.0);
            let around = draw::card_bg(ui);
            let ghost_at = self.rec.ghost_run.as_ref().map(|g| g.track[0]);
            draw::live_map(ui, &[], shown, ghost_at, draw::sport_color(sport), 150.0, around);
            let mut line = format!("Jarak {}", draw::dist(model::cumulative(shown).last().copied().unwrap_or(0.0), imp));
            if let Some(g) = &self.rec.ghost_run {
                line += &format!(" · waktu hantu {}", draw::clock(g.total_s()));
            }
            ui.label(RichText::new(line).size(12.5).color(draw::muted(ui)));
        }
    }

    /// While recording: how the race with the ghost stands, and how much of the route is left.
    fn record_race(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        let d = self.rec.distance();
        let clock = self.rec.race_clock();
        if let Some(g) = &self.rec.ghost_run {
            let t = clock.unwrap_or(0.0);
            let gap = if clock.is_some() { g.gap(d, t) } else { 0.0 };
            let (head, sub, color) = if clock.is_none() {
                ("HANTU SIAP", "mulai berlomba saat GPS dapat sinyal".to_string(), draw::GHOST)
            } else if d >= g.total_m() {
                if gap <= 0.0 {
                    ("MENGALAHKAN HANTU", format!("{} lebih cepat", draw::clock(-gap)), draw::lime(ui))
                } else {
                    ("KALAH DARI HANTU", format!("{} lebih lambat", draw::clock(gap)), draw::RED)
                }
            } else if t >= g.total_s() {
                ("HANTU SUDAH FINISH", format!("sisa {}", draw::dist(g.total_m() - d, imp)), draw::RED)
            } else {
                let lead = g.dist_at(t) - d;
                let side = if lead > 0.0 { "depan" } else { "belakang" };
                let sub = format!("hantu {} di {side} Anda", draw::dist(lead.abs(), imp));
                if gap <= 0.0 {
                    ("DI DEPAN HANTU", sub, draw::lime(ui))
                } else {
                    ("DI BELAKANG HANTU", sub, ACCENT)
                }
            };
            let sign = match clock {
                None => "",
                Some(_) if gap <= 0.0 => "-",
                Some(_) => "+",
            };
            egui::Frame::new()
                .fill(draw::GHOST.gamma_multiply(0.14))
                .stroke(Stroke::new(1.0, draw::GHOST.gamma_multiply(0.6)))
                .corner_radius(18.0)
                .inner_margin(14.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(head).family(draw::heavy()).size(16.0).color(color));
                            ui.label(RichText::new(sub).size(12.5).color(draw::muted(ui)));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(draw::big(format!("{sign}{}", draw::clock(gap.abs())), 36.0, color));
                        });
                    });
                });
            ui.add_space(8.0);
        }
        if self.rec.plan.len() > 1 {
            let total = self.rec.plan_cum.last().copied().unwrap_or(0.0);
            let left = (total - self.rec.route_at).max(0.0);
            let off = self.rec.off_m.filter(|o| *o > 50.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Sisa rute {}", draw::dist(left, imp))).family(draw::semi()).color(draw::strong(ui)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(o) = off {
                        draw::pill(ui, &format!("Keluar rute {:.0} m", o), draw::RED, true);
                    }
                });
            });
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
            let f = if total > 0.0 { (self.rec.route_at / total).clamp(0.0, 1.0) as f32 } else { 0.0 };
            ui.painter().rect_filled(r, 4.0, draw::raised(ui));
            ui.painter().rect_filled(egui::Rect::from_min_size(r.min, vec2(r.width() * f, r.height())), 4.0, draw::sport_color(self.rec.sport));
            ui.add_space(8.0);
        }
    }

    /// While recording: big numbers you can read on the move, and controls that are hard to hit by accident.
    fn record_focus(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        let running = matches!(self.rec.state, RecState::Running { .. });
        let moving = self.rec.moving();
        let d = self.rec.distance();
        let still = self.rec.standing_still();

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            draw::sport_icon(ui.painter(), self.rec.sport, r.center(), 26.0, draw::sport_color(self.rec.sport));
            ui.label(RichText::new(self.rec.sport.name()).family(draw::semi()).size(18.0).color(draw::strong(ui)));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, color) = if !running {
                    ("Dijeda", ACCENT)
                } else if still {
                    ("Jeda otomatis", ACCENT)
                } else {
                    match self.rec.gps {
                        -1 => ("Merekam", draw::lime(ui)),
                        2 => ("GPS aktif", draw::lime(ui)),
                        0 => ("Izin lokasi?", ACCENT),
                        3 => ("Lokasi mati", ACCENT),
                        _ => ("Mencari GPS", draw::muted(ui)),
                    }
                };
                draw::pill(ui, text, color, true);
            });
        });
        ui.add_space(8.0);

        if let Some(plan) = self.rec.interval {
            let (label, left, color, set) = match plan.phase(self.rec.elapsed()) {
                Phase::Work { rep, left_s } => ("LARI", Some(left_s), ACCENT, format!("Set {rep}/{}", plan.reps)),
                Phase::Rest { rep, left_s } => {
                    ("JALAN SANTAI", Some(left_s), Color32::from_rgb(60, 140, 230), format!("Set {rep}/{}", plan.reps))
                }
                Phase::Done => ("INTERVAL SELESAI", None, draw::lime(ui), String::new()),
            };
            egui::Frame::new()
                .fill(color.gamma_multiply(0.18))
                .stroke(Stroke::new(1.0, color.gamma_multiply(0.6)))
                .corner_radius(18.0)
                .inner_margin(14.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(label).family(draw::heavy()).size(18.0).color(color));
                            ui.label(RichText::new(set).size(12.5).color(draw::muted(ui)));
                        });
                        if let Some(left) = left {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(draw::big(mmss(left), 40.0, draw::strong(ui)));
                            });
                        }
                    });
                });
            ui.add_space(8.0);
        }

        self.record_race(ui);
        let has_distance = self.rec.sport.has_distance();
        if has_distance {
            let txt = draw::dist(d, imp);
            let (num, unit) = txt.split_once(' ').unwrap_or((&txt, ""));
            ui.vertical_centered(|ui| {
                ui.label(draw::big(num, 88.0, draw::strong(ui)));
                ui.label(RichText::new(unit.to_uppercase()).family(draw::semi()).size(14.0).color(draw::muted(ui)));
            });
        }
        ui.add_space(6.0);
        let speed = if moving > 0.0 && d >= 50.0 { d / moving } else { 0.0 };
        let (pl, pv) = draw::pace_or_speed(self.rec.sport, speed, imp);
        let mut items = vec![("Waktu", draw::clock(moving))];
        if has_distance {
            items.push((pl, pv));
        }
        if self.rec.sport.has_steps() && self.rec.gps >= 0 {
            items.push(("Langkah", self.rec.steps.to_string()));
            let cadence = if moving > 30.0 { self.rec.steps as f64 / (moving / 60.0) } else { 0.0 };
            items.push(("Langkah/menit", format!("{cadence:.0}")));
        }
        ui.columns(2, |c| {
            for (i, (l, v)) in items.iter().enumerate() {
                let ui = &mut c[i % 2];
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(*l).size(13.0).color(draw::muted(ui)));
                    ui.label(draw::big(v.as_str(), 34.0, draw::strong(ui)));
                    ui.add_space(2.0);
                });
            }
        });

        #[cfg(target_os = "android")]
        ui.horizontal_wrapped(|ui| match self.rec.gps {
            0 => {
                ui.label(RichText::new("Izinkan lokasi agar rute tercatat.").size(12.5).color(ACCENT));
                if ui.button("Izinkan lokasi").clicked() {
                    self.rec.gps_asked = None;
                }
                if ui.button("Buka pengaturan app").clicked() {
                    crate::android::gps_open_app_settings();
                }
            }
            3 => {
                ui.label(RichText::new("Lokasi di HP sedang mati.").size(12.5).color(ACCENT));
                if ui.button("Nyalakan Lokasi").clicked() {
                    crate::android::gps_open_location_settings();
                }
            }
            _ => {}
        });

        ui.add_space(6.0);
        // A big round pause/resume button, with laps or discard on the side.
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
        let c = row.center();
        let big = egui::Rect::from_center_size(c, egui::Vec2::splat(88.0));
        let big_resp = ui.interact(big, ui.scope_id().with("rec_big"), Sense::click());
        let p = ui.painter();
        p.circle_filled(c, 48.0, ACCENT.gamma_multiply(if big_resp.hovered() { 0.3 } else { 0.18 }));
        p.circle_filled(c, 40.0, ACCENT);
        if running {
            for dx in [-8.0, 8.0] {
                p.rect_filled(egui::Rect::from_center_size(c + vec2(dx, 0.0), vec2(8.0, 26.0)), 2.0, Color32::WHITE);
            }
        } else {
            let tri = vec![c + vec2(-8.0, -14.0), c + vec2(-8.0, 14.0), c + vec2(15.0, 0.0)];
            p.add(egui::Shape::convex_polygon(tri, Color32::WHITE, Stroke::NONE));
        }
        let side = egui::Rect::from_center_size(c + vec2(row.width() * 0.32, 0.0), vec2(96.0, 44.0));
        let side_text = if running {
            "Putaran"
        } else if self.rec.confirm_discard {
            "Ya, buang"
        } else {
            "Buang"
        };
        let side_resp = ui.interact(side, ui.scope_id().with("rec_side"), Sense::click());
        p.rect_filled(side, 22.0, draw::raised(ui));
        p.text(
            side.center(),
            Align2::CENTER_CENTER,
            side_text,
            FontId::new(15.0, draw::semi()),
            if running { draw::strong(ui) } else { draw::RED },
        );
        if running && !self.rec.laps.is_empty() {
            p.text(
                c - vec2(row.width() * 0.32, 0.0),
                Align2::CENTER_CENTER,
                format!("Putaran {}", self.rec.laps.len() + 1),
                FontId::new(13.0, draw::semi()),
                draw::muted(ui),
            );
        }
        if big_resp.clicked() {
            if running {
                self.rec.pause();
            } else {
                self.rec.resume();
            }
        }
        if side_resp.clicked() {
            if running {
                // Moving time, like the activity's duration, so the last lap can be worked out later.
                self.rec.laps.push(self.rec.moving());
            } else if !self.rec.confirm_discard {
                // One stray tap in a pocket must not lose the whole recording.
                self.rec.confirm_discard = true;
                self.say("Ketuk sekali lagi untuk membuang rekaman");
            } else {
                self.rec.confirm_discard = false;
                self.rec.stop();
                self.rec.state = RecState::Idle;
                self.rec.track.clear();
                self.say("Rekaman dibuang");
            }
        }
        ui.add_space(12.0);
        if draw::slide_to_confirm(ui, "rec_finish", "Geser untuk selesai") {
            self.finish_recording();
        }
        // Below the controls, and not draggable, so the page still scrolls with a thumb on it.
        if self.rec.track.len() > 1 || self.rec.shown_plan().len() > 1 {
            ui.add_space(12.0);
            let around = draw::bg(ui);
            let ghost_at = self.rec.ghost_run.as_ref().map(|g| g.pos_at(self.rec.race_clock().unwrap_or(0.0)));
            let color = draw::sport_color(self.rec.sport);
            draw::live_map(ui, &self.rec.track, self.rec.shown_plan(), ghost_at, color, 200.0, around);
        }
        if running {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn finish_recording(&mut self) {
        let total = self.rec.elapsed();
        self.rec.state = RecState::Done { total };
        self.rec.confirm_discard = false;
        self.rec.stop();
        self.rec.form = self.recording_form();
    }

    /// The form for a finished recording, filled in from what was recorded.
    pub fn recording_form(&self) -> Form {
        let mut f = Form::blank(self.rec.sport);
        if self.rec.track.len() > 1 {
            let (d, gain, _) = model::track_stats(&self.rec.track);
            f.dist_km = format!("{:.2}", d / 1000.0);
            f.elev = format!("{gain:.0}");
        }
        let start = self.rec.start_ms.div_euclid(1000);
        f.title = default_title(self.rec.sport, start);
        f.when = when_text(start);
        f.time = draw::clock(self.rec.moving());
        f.gear = self.data.gear.iter().find(|g| g.sport == self.rec.sport && !g.retired).map(|g| g.id);
        f
    }

    /// After recording: name it, check the numbers, save.
    fn save_form(&mut self, ui: &mut Ui) {
        let RecState::Done { total } = self.rec.state else { return };
        let moving = self.rec.moving();
        let mut form = self.rec.form.clone();
        let mut close = false;
        draw::card(ui, |ui| {
            draw::heading(ui, "Simpan aktivitas");
            if self.rec.auto_pause && moving + 1.0 < total {
                ui.label(
                    RichText::new(format!(
                        "Waktu bergerak {} (berhenti {} tidak dihitung).",
                        draw::clock(moving),
                        draw::clock(total - moving)
                    ))
                    .size(12.5)
                    .color(draw::muted(ui)),
                );
            }
            self.form_ui(ui, &mut form, false);
            ui.horizontal(|ui| {
                if draw::accent_button(ui, "Simpan").clicked() {
                    // The real start of the recording, not the moment Simpan is pressed.
                    let start = self.rec.start_ms.div_euclid(1000);
                    let mut a = self.empty_activity(form.sport, start);
                    a.duration_s = moving;
                    a.laps = self.rec.laps.clone();
                    a.steps = (self.rec.steps > 0).then_some(self.rec.steps);
                    // Fields left as filled in keep their exact values; changed ones are used as typed.
                    let mut f = form.clone();
                    if f.time.trim() == draw::clock(moving) {
                        f.time.clear();
                    }
                    let keep_start = f.when.trim() == when_text(start);
                    match f.apply(&mut a) {
                        Ok(()) => {
                            if keep_start {
                                a.start = start;
                            }
                            let verdict = self.ghost_verdict();
                            a.track = std::mem::take(&mut self.rec.track);
                            let id = a.id;
                            self.add_activity(a);
                            self.say("Aktivitas tersimpan");
                            match verdict {
                                Some((true, msg)) => match &mut self.celebrate {
                                    Some((wins, _)) => wins.push(msg),
                                    None => self.celebrate = Some((vec![msg], Instant::now())),
                                },
                                Some((false, msg)) => self.say(msg),
                                None => {}
                            }
                            self.detail = Some(id);
                            self.tab = Tab::Home;
                            close = true;
                        }
                        Err(e) => self.say(e),
                    }
                }
                let label = if self.rec.confirm_discard { "Ya, buang" } else { "Buang" };
                if ui.button(RichText::new(label).color(draw::RED)).clicked() {
                    // Same as while recording: a whole run must not go with one stray tap.
                    if self.rec.confirm_discard {
                        close = true;
                        self.say("Rekaman dibuang");
                    } else {
                        self.rec.confirm_discard = true;
                        self.say("Ketuk sekali lagi untuk membuang rekaman");
                    }
                }
            });
        });
        self.rec.form = form;
        if close {
            self.rec.confirm_discard = false;
            self.rec.route = None;
            self.rec.ghost = None;
            self.rec.arm(&self.data);
            self.rec.state = RecState::Idle;
            self.rec.laps.clear();
            self.rec.track.clear();
        }
    }

    /// Whether the finished recording beat its ghost, with the line to show; None if it did not go the distance.
    fn ghost_verdict(&self) -> Option<(bool, String)> {
        let g = self.rec.ghost_run.as_ref()?;
        let me = model::Ghost::new(0, &self.rec.track)?;
        // A few metres short of the ghost's finish still counts.
        if me.total_m() < g.total_m() * 0.97 {
            return None;
        }
        let at = g.total_m().min(me.total_m());
        let gap = me.time_at(at)? - g.time_at(at)?;
        Some(if gap <= 0.0 {
            (true, format!("Mengalahkan hantu, {} lebih cepat", draw::clock(-gap)))
        } else {
            (false, format!("Kalah {} dari hantu. Coba lagi!", draw::clock(gap)))
        })
    }

    /// Manual entry and GPX import, below the start card.
    fn record_extras(&mut self, ui: &mut Ui) {
        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tambah manual").size(15.0).family(draw::semi()).color(draw::strong(ui)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(if self.show_manual { "Tutup" } else { "Buka" }).clicked() {
                        self.show_manual = !self.show_manual;
                    }
                });
            });
            ui.label(RichText::new("Untuk latihan di treadmill, gym atau yang lupa direkam.").size(11.5).color(draw::muted(ui)));
            if self.show_manual {
                let mut f = self.manual.clone();
                self.form_ui(ui, &mut f, true);
                if draw::accent_button(ui, "Tambah").clicked() {
                    let mut a = self.empty_activity(f.sport, Local::now().timestamp());
                    match f.apply(&mut a) {
                        Ok(()) => {
                            self.add_activity(a);
                            self.say("Aktivitas ditambahkan");
                            f = Form::blank(f.sport);
                        }
                        Err(e) => self.say(e),
                    }
                }
                self.manual = f;
            }
        });

        ui.add_space(8.0);
        draw::card(ui, |ui| {
            ui.label(RichText::new("Impor file GPX").size(15.0).family(draw::semi()).color(draw::strong(ui)));
            if cfg!(target_os = "android") {
                ui.label(
                    RichText::new("Dari jam olahraga atau app lain. Pilih file .gpx dari Download, Drive, atau WhatsApp.")
                        .size(11.5)
                        .color(draw::muted(ui)),
                );
                self.pick_button(ui, crate::app::Pick::Gpx, "Pilih file GPX");
            } else {
                ui.label(
                    RichText::new("Dari jam olahraga atau HP. Tulis lokasi file, atau tarik file ke jendela ini.")
                        .size(11.5)
                        .color(draw::muted(ui)),
                );
                ui.add(egui::TextEdit::singleline(&mut self.import_path).hint_text("/home/anda/Unduhan/lari.gpx").desired_width(f32::INFINITY));
                if draw::accent_button(ui, "Impor").clicked() {
                    let p = std::path::PathBuf::from(self.import_path.trim());
                    self.import_gpx(&p);
                }
            }
        });
        let dropped: Vec<std::path::PathBuf> =
            ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped {
            self.import_gpx(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_phases_follow_the_plan() {
        let plan = Interval { on: true, work_s: 60, rest_s: 30, reps: 2 };
        assert_eq!(plan.phase(10.0), Phase::Work { rep: 1, left_s: 50.0 });
        assert_eq!(plan.phase(70.0), Phase::Rest { rep: 1, left_s: 20.0 });
        assert_eq!(plan.phase(95.0), Phase::Work { rep: 2, left_s: 55.0 });
        assert_eq!(plan.phase(181.0), Phase::Done);
    }

    #[test]
    fn standing_still_is_left_out_of_moving_time() {
        let mut r = Recorder::new(Sport::Run);
        r.auto_pause = true;
        // 60 s running 3 m/s east, then 60 s standing still.
        for i in 0..=120 {
            let x = if i <= 60 { i as f64 * 3.0 } else { 180.0 };
            r.track.push(TrackPoint { lat: 0.0, lon: x / 111_320.0, ele: None, t: Some(i as f64) });
        }
        r.state = RecState::Running { since_ms: now_ms() - 120_000, before: 0.0 };
        let m = r.moving();
        assert!((m - 60.0).abs() < 2.0, "moving {m}");
        assert!(r.standing_still());
    }

    #[test]
    fn a_new_recording_asks_for_gps_again() {
        let mut r = Recorder::new(Sport::Run);
        r.begin(&model::Settings::default());
        r.gps = 2;
        r.stop();
        r.state = RecState::Idle;
        r.begin(&model::Settings::default());
        assert_ne!(r.gps, 2, "a stale \"receiving\" would keep GPS off for the next recording");
    }
}
