//! Data kept only on this device: activities, gear, goals, planned routes and settings.

use chrono::{Datelike, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Sport {
    Run,
    Ride,
    Walk,
    Hike,
    Swim,
    Workout,
}

impl Sport {
    pub const ALL: [Sport; 6] = [
        Sport::Run,
        Sport::Ride,
        Sport::Walk,
        Sport::Hike,
        Sport::Swim,
        Sport::Workout,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Sport::Run => "Lari",
            Sport::Ride => "Sepeda",
            Sport::Walk => "Jalan",
            Sport::Hike => "Hiking",
            Sport::Swim => "Renang",
            Sport::Workout => "Latihan",
        }
    }

    /// Rough MET value for calorie estimates.
    pub fn met(self) -> f64 {
        match self {
            Sport::Run => 9.8,
            Sport::Ride => 7.5,
            Sport::Walk => 3.8,
            Sport::Hike => 6.0,
            Sport::Swim => 8.0,
            Sport::Workout => 5.0,
        }
    }

    /// Speed sports show km/h, the rest show pace per km.
    pub fn uses_speed(self) -> bool {
        matches!(self, Sport::Ride)
    }

    pub fn has_distance(self) -> bool {
        !matches!(self, Sport::Workout)
    }

    /// Below this speed (m/s) the recording counts as standing still, for auto-pause.
    pub fn min_moving_speed(self) -> f64 {
        match self {
            Sport::Ride => 1.5,
            _ => 0.6,
        }
    }

    /// Sports where a phone's step counter means something.
    pub fn has_steps(self) -> bool {
        matches!(self, Sport::Run | Sport::Walk | Sport::Hike)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct TrackPoint {
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,
    /// Seconds since the activity start.
    pub t: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Activity {
    pub id: u64,
    pub sport: Sport,
    pub title: String,
    /// Unix seconds.
    pub start: i64,
    /// Moving time in seconds.
    pub duration_s: f64,
    pub distance_m: f64,
    pub elev_gain_m: f64,
    pub avg_hr: Option<u32>,
    /// Perceived effort 1..=10.
    pub effort: u8,
    pub notes: String,
    pub gear: Option<u64>,
    pub track: Vec<TrackPoint>,
    pub favorite: bool,
    /// Lap split times in seconds since start.
    #[serde(default)]
    pub laps: Vec<f64>,
    /// Steps counted by the phone while recording.
    #[serde(default)]
    pub steps: Option<u32>,
}

impl Activity {
    pub fn speed_mps(&self) -> f64 {
        if self.duration_s > 0.0 {
            self.distance_m / self.duration_s
        } else {
            0.0
        }
    }

    pub fn date(&self) -> chrono::DateTime<Local> {
        Local
            .timestamp_opt(self.start, 0)
            .single()
            .unwrap_or_else(Local::now)
    }

    /// Steps per minute of moving time.
    pub fn cadence(&self) -> Option<f64> {
        let steps = self.steps.filter(|s| *s > 0)?;
        (self.duration_s > 60.0).then(|| steps as f64 / (self.duration_s / 60.0))
    }

    /// Pace (or speed) along the track: (distance m, metres per second), smoothed over ~200 m.
    pub fn speed_profile(&self) -> Vec<[f64; 2]> {
        let pts = &self.track;
        if pts.len() < 3 || pts[0].t.is_none() {
            return Vec::new();
        }
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().unwrap() + haversine(&w[0], &w[1]));
        }
        let mut out = Vec::new();
        let mut j = 0;
        let step = (cum.last().unwrap() / 120.0).max(25.0);
        let mut next = 0.0;
        for i in 0..pts.len() {
            if cum[i] < next {
                continue;
            }
            next = cum[i] + step;
            while j < pts.len() - 1 && cum[j] - cum[i] < 200.0 {
                j += 1;
            }
            let dt = pts[j].t.unwrap_or(0.0) - pts[i].t.unwrap_or(0.0);
            let dd = cum[j] - cum[i];
            if dt > 0.0 && dd > 50.0 {
                out.push([cum[i], dd / dt]);
            }
        }
        out
    }

    pub fn calories(&self, weight_kg: f32) -> f64 {
        self.sport.met() * weight_kg as f64 * self.duration_s / 3600.0
    }

    /// Time (seconds) for each full kilometre, from the recorded track.
    pub fn splits(&self) -> Vec<f64> {
        let mut out = Vec::new();
        if self.track.len() < 2 || self.track[0].t.is_none() {
            return out;
        }
        let mut acc = 0.0;
        let mut next = 1000.0;
        let mut last_t = 0.0;
        for w in self.track.windows(2) {
            let d = haversine(&w[0], &w[1]);
            let (t0, t1) = (w[0].t.unwrap_or(0.0), w[1].t.unwrap_or(0.0));
            while acc + d >= next && d > 0.0 {
                let f = (next - acc) / d;
                let t = t0 + (t1 - t0) * f;
                out.push(t - last_t);
                last_t = t;
                next += 1000.0;
            }
            acc += d;
        }
        out
    }

    /// (distance m, elevation m) along the track.
    pub fn elevation_profile(&self) -> Vec<[f64; 2]> {
        let mut acc = 0.0;
        let mut out = Vec::new();
        for (i, p) in self.track.iter().enumerate() {
            if i > 0 {
                acc += haversine(&self.track[i - 1], p);
            }
            if let Some(e) = p.ele {
                out.push([acc, e]);
            }
        }
        out
    }

    /// Fastest time over `dist` metres anywhere on the track (seconds).
    pub fn best_effort(&self, dist: f64) -> Option<f64> {
        if self.distance_m < dist {
            return None;
        }
        let pts = &self.track;
        if pts.len() < 2 || pts[0].t.is_none() {
            // No timing detail: assume an even pace.
            return Some(dist / self.speed_mps().max(0.01));
        }
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().unwrap() + haversine(&w[0], &w[1]));
        }
        let mut best: Option<f64> = None;
        let mut j = 0;
        for i in 0..pts.len() {
            while j < pts.len() && cum[j] - cum[i] < dist {
                j += 1;
            }
            if j >= pts.len() {
                break;
            }
            let t = pts[j].t.unwrap_or(0.0) - pts[i].t.unwrap_or(0.0);
            if t > 0.0 && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        }
        best
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Gear {
    pub id: u64,
    pub name: String,
    pub sport: Sport,
    pub limit_km: f64,
    pub retired: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Period {
    Week,
    Month,
    Year,
}

impl Period {
    pub fn name(self) -> &'static str {
        match self {
            Period::Week => "Minggu ini",
            Period::Month => "Bulan ini",
            Period::Year => "Tahun ini",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Metric {
    Distance,
    Time,
    Count,
    Elevation,
}

impl Metric {
    pub fn name(self) -> &'static str {
        match self {
            Metric::Distance => "Jarak (km)",
            Metric::Time => "Waktu (jam)",
            Metric::Count => "Jumlah aktivitas",
            Metric::Elevation => "Tanjakan (m)",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Goal {
    pub id: u64,
    pub sport: Option<Sport>,
    pub period: Period,
    pub metric: Metric,
    pub target: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Route {
    pub id: u64,
    pub name: String,
    pub sport: Sport,
    /// Older routes only: points in metres on a local flat plane, with no place on the map.
    #[serde(default)]
    pub points: Vec<[f32; 2]>,
    /// Points as [latitude, longitude]. Empty for older routes.
    #[serde(default)]
    pub geo: Vec<[f64; 2]>,
}

impl Route {
    pub fn distance_m(&self) -> f64 {
        if !self.geo.is_empty() {
            return self.track().windows(2).map(|w| haversine(&w[0], &w[1])).sum();
        }
        self.points
            .windows(2)
            .map(|w| ((w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])) as f64)
            .sum()
    }

    /// The route's points as a track (no times), for drawing on the map.
    pub fn track(&self) -> Vec<TrackPoint> {
        self.geo.iter().map(|g| TrackPoint { lat: g[0], lon: g[1], ele: None, t: None }).collect()
    }
}

/// Flat points in metres (x east, y south) placed on the map with the first point at `origin` (latitude, longitude).
pub fn flat_to_geo(points: &[[f32; 2]], origin: (f64, f64)) -> Vec<[f64; 2]> {
    let k = 111_320.0;
    let cos = origin.0.to_radians().cos().max(0.01);
    let (x0, y0) = points.first().map_or((0.0, 0.0), |p| (p[0] as f64, p[1] as f64));
    points
        .iter()
        .map(|p| [origin.0 - (p[1] as f64 - y0) / k, origin.1 + (p[0] as f64 - x0) / (k * cos)])
        .collect()
}

/// Run/rest blocks, e.g. 1 minute running and 1 minute walking, 8 times.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct Interval {
    pub on: bool,
    pub work_s: u32,
    pub rest_s: u32,
    pub reps: u32,
}

impl Default for Interval {
    fn default() -> Self {
        Self { on: false, work_s: 60, rest_s: 60, reps: 8 }
    }
}

/// Where a recording is within an interval workout.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Work { rep: u32, left_s: f64 },
    Rest { rep: u32, left_s: f64 },
    Done,
}

impl Interval {
    pub fn phase(&self, t: f64) -> Phase {
        let cycle = (self.work_s + self.rest_s).max(1) as f64;
        let rep = (t / cycle).floor() as u32;
        if rep >= self.reps {
            return Phase::Done;
        }
        let into = t - rep as f64 * cycle;
        if into < self.work_s as f64 {
            Phase::Work { rep: rep + 1, left_s: self.work_s as f64 - into }
        } else {
            Phase::Rest { rep: rep + 1, left_s: cycle - into }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Optional, never required.
    pub nickname: String,
    pub imperial: bool,
    pub dark: bool,
    pub max_hr: u32,
    pub weight_kg: f32,
    /// Spoken distance and pace every kilometre (phones).
    pub voice: bool,
    /// Stop the clock while standing still.
    pub auto_pause: bool,
    pub interval: Interval,
    /// False until the welcome screens were seen; existing data counts as seen.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            imperial: false,
            dark: true,
            max_hr: 190,
            weight_kg: 65.0,
            voice: true,
            auto_pause: true,
            interval: Interval::default(),
            onboarded: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Data {
    pub activities: Vec<Activity>,
    pub gear: Vec<Gear>,
    pub goals: Vec<Goal>,
    pub routes: Vec<Route>,
    pub settings: Settings,
    pub next_id: u64,
    /// True while the sample activities from the first start are still there.
    #[serde(default)]
    pub has_samples: bool,
    /// Ids of the sample activities, gear, goals and routes, so only those are removed later.
    #[serde(default)]
    pub sample_ids: Vec<u64>,
}

impl Data {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Makes sure `new_id` never hands out an id that is already in use.
    pub fn fix_next_id(&mut self) {
        let used = self
            .activities
            .iter()
            .map(|a| a.id)
            .chain(self.gear.iter().map(|g| g.id))
            .chain(self.goals.iter().map(|g| g.id))
            .chain(self.routes.iter().map(|r| r.id))
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(used);
    }

    /// Adds the sample activities, gear, goals and route next to what is already there, with fresh ids.
    pub fn add_samples(&mut self) {
        let s = sample_data();
        let mut map = std::collections::HashMap::new();
        for g in s.gear {
            let id = self.new_id();
            map.insert(g.id, id);
            self.gear.push(Gear { id, ..g });
        }
        for a in s.activities {
            let id = self.new_id();
            let gear = a.gear.and_then(|g| map.get(&g).copied());
            self.activities.push(Activity { id, gear, ..a });
            self.sample_ids.push(id);
        }
        for g in s.goals {
            let id = self.new_id();
            self.goals.push(Goal { id, ..g });
            self.sample_ids.push(id);
        }
        for r in s.routes {
            let id = self.new_id();
            self.routes.push(Route { id, ..r });
            self.sample_ids.push(id);
        }
        self.sample_ids.extend(map.values());
        self.has_samples = true;
        self.sort();
    }

    /// Removes only the sample items; anything the user added stays.
    pub fn remove_samples(&mut self) {
        let ids: std::collections::HashSet<u64> = self.sample_ids.drain(..).collect();
        self.activities.retain(|a| !ids.contains(&a.id));
        self.gear.retain(|g| !ids.contains(&g.id));
        self.goals.retain(|g| !ids.contains(&g.id));
        self.routes.retain(|r| !ids.contains(&r.id));
        for a in &mut self.activities {
            if a.gear.is_some_and(|g| ids.contains(&g)) {
                a.gear = None;
            }
        }
        self.has_samples = false;
    }

    pub fn sort(&mut self) {
        self.activities.sort_by(|a, b| b.start.cmp(&a.start));
    }

    pub fn gear_km(&self, id: u64) -> f64 {
        self.activities
            .iter()
            .filter(|a| a.gear == Some(id))
            .map(|a| a.distance_m)
            .sum::<f64>()
            / 1000.0
    }

    pub fn in_period(&self, period: Period, sport: Option<Sport>) -> impl Iterator<Item = &Activity> {
        let from = period_start(period);
        self.activities.iter().filter(move |a| {
            a.date().date_naive() >= from && sport.is_none_or(|s| s == a.sport)
        })
    }

    /// How much `a` adds to a goal (0 when it is outside the goal's period or sport).
    pub fn goal_share(g: &Goal, a: &Activity) -> f64 {
        if a.date().date_naive() < period_start(g.period) || g.sport.is_some_and(|s| s != a.sport) {
            return 0.0;
        }
        match g.metric {
            Metric::Distance => a.distance_m / 1000.0,
            Metric::Time => a.duration_s / 3600.0,
            Metric::Count => 1.0,
            Metric::Elevation => a.elev_gain_m,
        }
    }

    pub fn goal_progress(&self, g: &Goal) -> f64 {
        let it = self.in_period(g.period, g.sport);
        match g.metric {
            Metric::Distance => it.map(|a| a.distance_m).sum::<f64>() / 1000.0,
            Metric::Time => it.map(|a| a.duration_s).sum::<f64>() / 3600.0,
            Metric::Count => it.count() as f64,
            Metric::Elevation => it.map(|a| a.elev_gain_m).sum::<f64>(),
        }
    }

    /// Earlier activities of the same sport over the same route as `a`, oldest first.
    pub fn same_route(&self, a: &Activity) -> Vec<&Activity> {
        let mut v: Vec<&Activity> = self
            .activities
            .iter()
            .filter(|b| b.id != a.id && b.sport == a.sport && same_route(a, b))
            .collect();
        v.sort_by_key(|b| b.start);
        v
    }

    /// Consecutive days (ending today or yesterday) with at least one activity.
    pub fn streak_days(&self) -> u32 {
        let days: std::collections::HashSet<_> =
            self.activities.iter().map(|a| a.date().date_naive()).collect();
        let mut d = Local::now().date_naive();
        if !days.contains(&d) {
            d -= chrono::Duration::days(1);
        }
        let mut n = 0;
        while days.contains(&d) {
            n += 1;
            d -= chrono::Duration::days(1);
        }
        n
    }

    /// Consecutive weeks (ending this week) with at least one activity.
    pub fn streak_weeks(&self) -> u32 {
        let weeks: std::collections::HashSet<_> = self
            .activities
            .iter()
            .map(|a| a.date().date_naive().iso_week())
            .map(|w| (w.year(), w.week()))
            .collect();
        let mut d = Local::now().date_naive();
        let mut n = 0;
        let key = |d: chrono::NaiveDate| (d.iso_week().year(), d.iso_week().week());
        if !weeks.contains(&key(d)) {
            d -= chrono::Duration::days(7);
        }
        while weeks.contains(&key(d)) {
            n += 1;
            d -= chrono::Duration::days(7);
        }
        n
    }
}

/// First day of the current week (Monday), month or year.
pub fn period_start(period: Period) -> chrono::NaiveDate {
    let today = Local::now().date_naive();
    match period {
        Period::Week => today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64),
        Period::Month => today.with_day(1).unwrap_or(today),
        Period::Year => today.with_ordinal(1).unwrap_or(today),
    }
}

pub fn haversine(a: &TrackPoint, b: &TrackPoint) -> f64 {
    let r = 6_371_000.0;
    let (la1, la2) = (a.lat.to_radians(), b.lat.to_radians());
    let dla = la2 - la1;
    let dlo = (b.lon - a.lon).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

/// Seconds spent standing still: track pieces slower than `min_speed`, or long gaps without movement.
pub fn stopped_time(track: &[TrackPoint], min_speed: f64) -> f64 {
    let mut stopped = 0.0;
    for w in track.windows(2) {
        let (Some(t0), Some(t1)) = (w[0].t, w[1].t) else { continue };
        let dt = t1 - t0;
        if dt > 0.0 && haversine(&w[0], &w[1]) / dt < min_speed {
            stopped += dt;
        }
    }
    stopped
}

/// True when two tracks follow the same route: similar length, same start and end, same path.
pub fn same_route(a: &Activity, b: &Activity) -> bool {
    let (ta, tb) = (&a.track, &b.track);
    if ta.len() < 10 || tb.len() < 10 || a.distance_m < 300.0 {
        return false;
    }
    if (a.distance_m - b.distance_m).abs() > 0.15 * a.distance_m.max(b.distance_m) {
        return false;
    }
    let near = |p: &TrackPoint, q: &TrackPoint, m: f64| haversine(p, q) <= m;
    if !near(&ta[0], &tb[0], 150.0) || !near(ta.last().unwrap(), tb.last().unwrap(), 150.0) {
        return false;
    }
    let step_b = (tb.len() / 400).max(1);
    let samples = 12;
    (0..samples).all(|i| {
        let p = &ta[i * (ta.len() - 1) / (samples - 1)];
        tb.iter().step_by(step_b).any(|q| near(p, q, 100.0))
    })
}

pub fn track_stats(track: &[TrackPoint]) -> (f64, f64, f64) {
    let mut dist = 0.0;
    let mut gain = 0.0;
    for w in track.windows(2) {
        dist += haversine(&w[0], &w[1]);
        if let (Some(a), Some(b)) = (w[0].ele, w[1].ele) {
            if b > a {
                gain += b - a;
            }
        }
    }
    let dur = match (track.first().and_then(|p| p.t), track.last().and_then(|p| p.t)) {
        (Some(a), Some(b)) => b - a,
        _ => 0.0,
    };
    (dist, gain, dur)
}

// ---------- storage ----------

pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("GHOSTWALK_DATA") {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(d).join("ghostwalk");
    }
    if let Some(h) = std::env::var_os("HOME") {
        return PathBuf::from(h).join(".local/share/ghostwalk");
    }
    PathBuf::from("ghostwalk-data")
}

/// Why the app last closed by itself, if it did.
pub fn crash_file() -> PathBuf {
    data_dir().join("error-terakhir.txt")
}

pub fn data_file() -> PathBuf {
    data_dir().join("data.json")
}

/// `Ok(None)` when there is no data yet. When the file cannot be read, the error says why and
/// whether a copy was put aside; without a copy the app must not save over the file.
pub fn load() -> Result<Option<Data>, (String, bool)> {
    let path = data_file();
    let s = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err((format!("Data tidak bisa dibaca: {e}"), false)),
    };
    match serde_json::from_str::<Data>(&s) {
        Ok(mut d) => {
            // Sample data from before `sample_ids` existed: it replaced everything, so its ids were the first ones.
            if d.has_samples && d.sample_ids.is_empty() {
                d.sample_ids = (1..=sample_data().next_id).collect();
            }
            d.fix_next_id();
            Ok(Some(d))
        }
        Err(e) => {
            let aside = data_dir().join(format!("data-rusak-{}.json", Local::now().format("%Y%m%d-%H%M%S")));
            let kept = std::fs::copy(&path, &aside).is_ok();
            let msg = if kept {
                format!("Data tidak bisa dibaca ({e}). Salinannya disimpan di {}.", aside.display())
            } else {
                format!("Data tidak bisa dibaca ({e}). Perubahan tidak disimpan agar file lama tetap utuh.")
            };
            Err((msg, kept))
        }
    }
}

pub fn save(d: &Data) -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    let tmp = data_dir().join("data.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(d)?)?;
    std::fs::rename(tmp, data_file())
}

// ---------- sample data ----------

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % 10_000) as f64 / 10_000.0
    }
}

fn sample_track(rng: &mut Rng, dist_m: f64, speed: f64, shape: f64) -> Vec<TrackPoint> {
    let (clat, clon) = (-6.2 + rng.f() * 0.01, 106.82 + rng.f() * 0.01);
    let wob = 0.15 + 0.25 * rng.f();
    let lobes = 2.0 + (shape * 3.0).floor();
    let phase = rng.f() * std::f64::consts::TAU;
    // Base radius so the loop's length is roughly the target distance.
    let r = dist_m / (2.0 * std::f64::consts::PI * (1.0 + wob * 0.6));
    let n = (dist_m / 15.0).clamp(40.0, 2000.0) as usize;
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let th = i as f64 / n as f64 * std::f64::consts::TAU;
        let rr = r * (1.0 + wob * (lobes * th + phase).sin());
        let (x, y) = (rr * th.cos() * 1.3, rr * th.sin() * 0.8);
        pts.push(TrackPoint {
            lat: clat + y / 111_320.0,
            lon: clon + x / (111_320.0 * clat.to_radians().cos()),
            ele: Some(18.0 + 14.0 * (th * 2.0 + phase).sin() + 6.0 * (th * 5.0).cos()),
            t: None,
        });
    }
    let mut t = 0.0;
    pts[0].t = Some(0.0);
    for i in 1..pts.len() {
        let d = haversine(&pts[i - 1], &pts[i]);
        let v = speed * (0.88 + 0.24 * rng.f());
        t += d / v;
        pts[i].t = Some(t);
    }
    pts
}

pub fn sample_data() -> Data {
    let mut d = Data {
        has_samples: true,
        ..Default::default()
    };
    let shoes = d.new_id();
    d.gear.push(Gear {
        id: shoes,
        name: "Sepatu lari biru".into(),
        sport: Sport::Run,
        limit_km: 700.0,
        retired: false,
    });
    let bike = d.new_id();
    d.gear.push(Gear {
        id: bike,
        name: "Sepeda balap".into(),
        sport: Sport::Ride,
        limit_km: 0.0,
        retired: false,
    });
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let now = Local::now();
    let plan: [(i64, Sport, &str, f64, f64, u8); 14] = [
        (0, Sport::Run, "Lari pagi", 6200.0, 3.05, 6),
        (1, Sport::Walk, "Jalan sore di taman", 3400.0, 1.4, 2),
        (2, Sport::Ride, "Gowes akhir pekan", 32_500.0, 7.6, 7),
        (3, Sport::Run, "Interval 400 m", 5000.0, 3.4, 8),
        (5, Sport::Hike, "Hiking bukit", 8900.0, 1.25, 7),
        (6, Sport::Run, "Lari santai", 4100.0, 2.8, 4),
        (8, Sport::Workout, "Latihan kekuatan", 0.0, 0.0, 6),
        (9, Sport::Run, "Lari jauh", 12_300.0, 2.95, 7),
        (11, Sport::Ride, "Gowes ke pantai", 41_000.0, 7.1, 6),
        (13, Sport::Walk, "Jalan pagi", 2800.0, 1.35, 2),
        (15, Sport::Run, "Tempo run", 8000.0, 3.25, 8),
        (18, Sport::Swim, "Renang kolam", 1500.0, 0.75, 5),
        (21, Sport::Run, "Lari 10K", 10_050.0, 3.1, 9),
        (26, Sport::Run, "Lari pemulihan", 3600.0, 2.6, 3),
    ];
    for (i, (days, sport, title, dist, speed, effort)) in plan.into_iter().enumerate() {
        let hour = [6, 17, 7, 6, 8, 18][i % 6];
        let start = (now - chrono::Duration::days(days))
            .date_naive()
            .and_hms_opt(hour, 15, 0)
            .and_then(|t| t.and_local_timezone(Local).single())
            .map(|t| t.timestamp())
            .unwrap_or(now.timestamp());
        let (track, dur, dist_m, gain) = if sport == Sport::Workout {
            (Vec::new(), 2700.0, 0.0, 0.0)
        } else if sport == Sport::Swim {
            (Vec::new(), dist / speed, dist, 0.0)
        } else {
            let tr = sample_track(&mut rng, dist, speed, i as f64 / 14.0);
            let (dm, g, du) = track_stats(&tr);
            (tr, du, dm, g)
        };
        let id = d.new_id();
        d.activities.push(Activity {
            id,
            sport,
            title: title.into(),
            start,
            duration_s: dur,
            distance_m: dist_m,
            elev_gain_m: gain,
            avg_hr: Some(120 + effort as u32 * 6),
            effort,
            notes: String::new(),
            gear: match sport {
                Sport::Run => Some(shoes),
                Sport::Ride => Some(bike),
                _ => None,
            },
            track,
            favorite: i == 12,
            laps: Vec::new(),
            steps: matches!(sport, Sport::Run | Sport::Walk | Sport::Hike)
                .then(|| (dur / 60.0 * if sport == Sport::Run { 168.0 } else { 112.0 }) as u32),
        });
    }
    let g1 = d.new_id();
    d.goals.push(Goal {
        id: g1,
        sport: Some(Sport::Run),
        period: Period::Week,
        metric: Metric::Distance,
        target: 25.0,
    });
    let g2 = d.new_id();
    d.goals.push(Goal {
        id: g2,
        sport: None,
        period: Period::Month,
        metric: Metric::Time,
        target: 20.0,
    });
    let r = d.new_id();
    d.routes.push(Route {
        id: r,
        name: "Putaran taman kota".into(),
        sport: Sport::Run,
        points: Vec::new(),
        geo: flat_to_geo(
            &[[0.0, 0.0], [900.0, 150.0], [1500.0, 900.0], [1100.0, 1700.0], [200.0, 1500.0], [0.0, 0.0]],
            (-6.195, 106.822),
        ),
    });
    d.sort();
    d.sample_ids = (1..=d.next_id).collect();
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loop_activity(id: u64, shift_m: f64, scale: f64) -> Activity {
        let track: Vec<TrackPoint> = (0..=100)
            .map(|i| {
                let a = i as f64 / 100.0 * std::f64::consts::TAU;
                let (x, y) = (800.0 * scale * a.cos() + shift_m, 500.0 * scale * a.sin());
                TrackPoint { lat: -6.2 + y / 111_320.0, lon: 107.0 + x / 110_700.0, ele: None, t: Some(i as f64 * 10.0) }
            })
            .collect();
        let (distance_m, _, duration_s) = track_stats(&track);
        Activity {
            id,
            sport: Sport::Run,
            title: String::new(),
            start: id as i64,
            duration_s,
            distance_m,
            elev_gain_m: 0.0,
            avg_hr: None,
            effort: 5,
            notes: String::new(),
            gear: None,
            track,
            favorite: false,
            laps: Vec::new(),
            steps: None,
        }
    }

    #[test]
    fn the_same_loop_is_the_same_route() {
        let a = loop_activity(1, 0.0, 1.0);
        assert!(same_route(&a, &loop_activity(2, 30.0, 1.0)), "a few metres of GPS drift");
        assert!(!same_route(&a, &loop_activity(3, 0.0, 1.6)), "a bigger loop");
        assert!(!same_route(&a, &loop_activity(4, 2000.0, 1.0)), "somewhere else");
    }

    #[test]
    fn samples_come_and_go_without_touching_the_users_own_things() {
        let mut d = Data::default();
        let shoes = d.new_id();
        d.gear.push(Gear { id: shoes, name: "Sepatu saya".into(), sport: Sport::Run, limit_km: 600.0, retired: false });
        d.add_samples();
        assert_eq!(d.gear.len(), 3, "own shoes kept next to the sample gear");
        let mut own = loop_activity(d.new_id(), 0.0, 1.0);
        own.gear = d.gear.iter().find(|g| g.id != shoes).map(|g| g.id);
        let own_id = own.id;
        d.activities.push(own);
        d.remove_samples();
        assert_eq!(d.activities.len(), 1);
        assert_eq!(d.activities[0].id, own_id);
        assert_eq!(d.activities[0].gear, None, "sample gear is gone, so the link is cleared");
        assert_eq!(d.gear.len(), 1);
        assert!(d.goals.is_empty() && d.routes.is_empty() && !d.has_samples);
    }

    #[test]
    fn routes_on_the_map_measure_real_distance() {
        let flat = [[0.0, 0.0], [1000.0, 0.0], [1000.0, 1000.0]];
        let old = Route { id: 1, name: String::new(), sport: Sport::Run, points: flat.to_vec(), geo: Vec::new() };
        let new = Route { geo: flat_to_geo(&flat, (-6.2, 106.8)), points: Vec::new(), ..old.clone() };
        assert!((old.distance_m() - 2000.0).abs() < 0.1);
        assert!((new.distance_m() - 2000.0).abs() < 5.0, "{}", new.distance_m());
        // East is +longitude and south (y down) is -latitude.
        assert!(new.geo[1][1] > new.geo[0][1] && new.geo[2][0] < new.geo[1][0]);
        let json = r#"{"id":3,"name":"lama","sport":"Run","points":[[0.0,0.0],[3.0,4.0]]}"#;
        let r: Route = serde_json::from_str(json).unwrap();
        assert!(r.geo.is_empty() && (r.distance_m() - 5.0).abs() < 1e-6, "older routes still load");
    }

    #[test]
    fn goal_share_counts_only_matching_activities() {
        let g = Goal { id: 1, sport: Some(Sport::Ride), period: Period::Week, metric: Metric::Count, target: 3.0 };
        let mut a = loop_activity(1, 0.0, 1.0);
        a.start = Local::now().timestamp();
        assert_eq!(Data::goal_share(&g, &a), 0.0);
        a.sport = Sport::Ride;
        assert_eq!(Data::goal_share(&g, &a), 1.0);
    }
}
