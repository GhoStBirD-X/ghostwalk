//! Downloading the map of one area, on request only.
//!
//! The only thing sent is which map pieces are wanted (an area the person picked by hand, never
//! their GPS position). Pieces come from the free OpenStreetMap builds at build.protomaps.com,
//! which are meant to be cut into smaller offline extracts like this.

use crate::offmap;
use crate::pmtiles::{self, COMP_GZIP, Entry, HEADER_LEN, Header};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const BUILDS: &str = "https://build-metadata.protomaps.dev/builds.json";
const SOURCE: &str = "https://build.protomaps.com/";
const MIN_ZOOM: u8 = 10;
const MAX_ZOOM: u8 = 15;
/// Pieces closer together than this are fetched in one request.
const GAP: u64 = 256 << 10;
const CHUNK: u64 = 16 << 20;

// ---------- picking areas ----------

/// A kabupaten/kota or kecamatan from the built-in list (GeoNames, CC BY 4.0).
pub struct Place {
    pub name: String,
    /// "Kab. Bekasi, Jawa Barat" or "dekat Kab. Bekasi, Jawa Barat".
    pub context: String,
    pub lat: f64,
    pub lon: f64,
    pub kecamatan: bool,
    key: String,
}

impl Place {
    /// A sensible download size: a kecamatan is small, a kabupaten/kota larger.
    pub fn area(&self) -> Area {
        Area {
            name: self.name.clone(),
            lat: self.lat,
            lon: self.lon,
            radius_km: if self.kecamatan { 5.0 } else { 10.0 },
        }
    }
}

pub fn places() -> &'static [Place] {
    static P: OnceLock<Vec<Place>> = OnceLock::new();
    P.get_or_init(|| {
        let mut text = String::new();
        let raw: &[u8] = include_bytes!("../assets/wilayah.tsv.gz");
        let _ = flate2::read::GzDecoder::new(raw).read_to_string(&mut text);
        text.lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                let [kind, name, context, lat, lon] = f.as_slice() else { return None };
                let key = ["kec. ", "kab. ", "kota "]
                    .iter()
                    .fold(name.to_lowercase(), |k, p| k.strip_prefix(p).map(str::to_string).unwrap_or(k));
                Some(Place {
                    name: name.to_string(),
                    context: context.to_string(),
                    lat: lat.parse().ok()?,
                    lon: lon.parse().ok()?,
                    kecamatan: *kind == "C",
                    key,
                })
            })
            .collect()
    })
}

/// Places whose name matches `query`: starts-with first, then kabupaten/kota, then shorter names.
pub fn search(query: &str, limit: usize) -> Vec<&'static Place> {
    let q = query.trim().to_lowercase();
    if q.chars().count() < 2 {
        return Vec::new();
    }
    let mut hits: Vec<(u8, &Place)> = places()
        .iter()
        .filter_map(|p| {
            let rank = if p.key.starts_with(&q) {
                0
            } else if p.key.split([' ', '-']).any(|w| w.starts_with(&q)) {
                1
            } else if p.key.contains(&q) {
                2
            } else {
                return None;
            };
            Some((rank, p))
        })
        .collect();
    hits.sort_by_key(|(rank, p)| (*rank, p.kecamatan, p.key.len()));
    hits.into_iter().take(limit).map(|h| h.1).collect()
}

/// Reads "-6.27, 107.07" or "-6,27 107,07" as (latitude, longitude).
pub fn parse_coords(s: &str) -> Option<(f64, f64)> {
    let s = s.trim();
    let parts: Vec<String> = if s.contains('.') {
        s.split([',', ';', ' ', '\t']).filter(|p| !p.is_empty()).map(str::to_string).collect()
    } else {
        s.split([';', ' ', '\t']).filter(|p| !p.is_empty()).map(|p| p.replace(',', ".")).collect()
    };
    let [lat, lon] = parts.as_slice() else { return None };
    let (lat, lon) = (lat.parse::<f64>().ok()?, lon.parse::<f64>().ok()?);
    ((-85.0..=85.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

pub const RADII_KM: [f64; 5] = [3.0, 5.0, 10.0, 20.0, 40.0];

/// An area to download: a box around a centre.
#[derive(Clone, PartialEq)]
pub struct Area {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub radius_km: f64,
}

impl Area {
    /// West, south, east, north.
    pub fn bounds(&self) -> [f64; 4] {
        let dlat = self.radius_km / 111.32;
        let dlon = self.radius_km / (111.32 * self.lat.to_radians().cos().max(0.1));
        [self.lon - dlon, self.lat - dlat, self.lon + dlon, self.lat + dlat]
    }

    pub fn label(&self) -> String {
        format!("{} ({} km)", self.name, self.radius_km as u32)
    }

    /// True when this exact area is already on the device.
    pub fn installed(&self) -> bool {
        offmap::dir().join(file_name(self)).exists()
    }
}

/// What a size check found: everything needed to download without asking the server again.
pub struct Plan {
    pub area: Area,
    url: String,
    /// Tile id → (offset, length) in the source file.
    tiles: BTreeMap<u64, (u64, u32)>,
    data_off: u64,
    /// Bytes of map data the new file will hold.
    pub bytes: u64,
}

pub enum Status {
    Checking { done: usize, count: usize },
    /// One plan per area, in the same order.
    Ready(Vec<Arc<Plan>>),
    Downloading { index: usize, count: usize, name: String, done: u64, total: u64 },
    Done(Vec<String>),
    Failed(String),
    Cancelled,
}

pub struct Task {
    pub status: Arc<Mutex<Status>>,
    cancel: Arc<AtomicBool>,
}

impl Task {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn busy(&self) -> bool {
        matches!(*self.status.lock().unwrap(), Status::Checking { .. } | Status::Downloading { .. })
    }
}

type Ctx = eframe::egui::Context;

fn spawn(ctx: &Ctx, start: Status, job: impl FnOnce(&Mutex<Status>, &AtomicBool, &Ctx) -> Status + Send + 'static) -> Task {
    let status = Arc::new(Mutex::new(start));
    let cancel = Arc::new(AtomicBool::new(false));
    let (s, c, ctx) = (status.clone(), cancel.clone(), ctx.clone());
    std::thread::spawn(move || {
        let end = job(&s, &c, &ctx);
        *s.lock().unwrap() = if c.load(Ordering::Relaxed) { Status::Cancelled } else { end };
        ctx.request_repaint();
    });
    Task { status, cancel }
}

/// Looks up how big each area's map is. Only the map index is downloaded.
pub fn check(ctx: &Ctx, areas: Vec<Area>) -> Task {
    let count = areas.len();
    spawn(ctx, Status::Checking { done: 0, count }, move |status, cancel, ctx| {
        let mut plans = Vec::new();
        for (i, area) in areas.into_iter().enumerate() {
            let name = area.label();
            match plan(area, cancel) {
                Ok(p) => plans.push(Arc::new(p)),
                Err(e) => return Status::Failed(format!("{name}: {e}")),
            }
            *status.lock().unwrap() = Status::Checking { done: i + 1, count };
            ctx.request_repaint();
        }
        Status::Ready(plans)
    })
}

/// Downloads checked areas one after another, each into its own map file.
pub fn download(ctx: &Ctx, plans: Vec<Arc<Plan>>) -> Task {
    let count = plans.len();
    let first = Status::Downloading { index: 0, count, name: String::new(), done: 0, total: 0 };
    spawn(ctx, first, move |status, cancel, ctx| {
        let mut done_names = Vec::new();
        for (index, plan) in plans.iter().enumerate() {
            let name = plan.area.label();
            let total = plan.bytes;
            let progress = |done: u64| {
                *status.lock().unwrap() = Status::Downloading { index, count, name: name.clone(), done, total };
                ctx.request_repaint();
            };
            progress(0);
            match fetch(plan, cancel, &progress) {
                Ok(label) => {
                    offmap::reload();
                    done_names.push(label);
                }
                Err(e) => {
                    let mut msg = format!("{name}: {e}");
                    if !done_names.is_empty() {
                        msg += &format!(". Sudah terpasang: {}", done_names.join(", "));
                    }
                    return Status::Failed(msg);
                }
            }
        }
        Status::Done(done_names)
    })
}

// ---------- network ----------

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(90)))
            .user_agent("GhostWalk/0.1 (offline map extract)")
            .build(),
    )
}

fn net(e: ureq::Error) -> io::Error {
    io::Error::other(format!("tidak bisa mengunduh: {e}"))
}

fn get_range(agent: &ureq::Agent, url: &str, off: u64, len: u64) -> io::Result<Vec<u8>> {
    let mut resp = agent
        .get(url)
        .header("Range", &format!("bytes={}-{}", off, off + len - 1))
        .call()
        .map_err(net)?;
    if resp.status() != 206 {
        return Err(io::Error::other("server peta tidak mendukung unduhan sebagian"));
    }
    let b = resp.body_mut().with_config().limit(len + 1024).read_to_vec().map_err(net)?;
    if b.len() as u64 != len {
        return Err(io::Error::other("unduhan terputus, coba lagi"));
    }
    Ok(b)
}

/// The newest map build with the layer layout this app draws.
fn latest_build(agent: &ureq::Agent) -> io::Result<String> {
    let raw = agent
        .get(BUILDS)
        .call()
        .map_err(net)?
        .body_mut()
        .with_config()
        .limit(16 << 20)
        .read_to_vec()
        .map_err(net)?;
    let json: serde_json::Value = serde_json::from_slice(&raw).map_err(io::Error::other)?;
    json.as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["version"].as_str().is_some_and(|v| v.starts_with("4.")))
        .filter_map(|b| b["key"].as_str())
        .filter(|k| k.ends_with(".pmtiles"))
        .max()
        .map(|k| format!("{SOURCE}{k}"))
        .ok_or_else(|| io::Error::other("tidak ada peta yang tersedia saat ini"))
}

fn wanted_ids(b: [f64; 4]) -> Vec<u64> {
    let mut ids = Vec::new();
    for z in MIN_ZOOM..=MAX_ZOOM {
        let (x0, y0) = pmtiles::lonlat_to_tile(b[0], b[3], z);
        let (x1, y1) = pmtiles::lonlat_to_tile(b[2], b[1], z);
        for y in y0..=y1 {
            for x in x0..=x1 {
                ids.push(pmtiles::tile_id(z, x, y));
            }
        }
    }
    ids.sort_unstable();
    ids
}

fn plan(area: Area, cancel: &AtomicBool) -> io::Result<Plan> {
    let agent = agent();
    let url = latest_build(&agent)?;
    let head = get_range(&agent, &url, 0, 16384)?;
    let h = Header::parse(&head)?;
    if h.tile_type != pmtiles::TYPE_MVT {
        return Err(io::Error::other("format peta di server berubah"));
    }
    let root_raw = match head.get(h.root_off as usize..(h.root_off + h.root_len) as usize) {
        Some(b) => b.to_vec(),
        None => get_range(&agent, &url, h.root_off, h.root_len)?,
    };
    let mut dirs: HashMap<u64, Vec<Entry>> = HashMap::new();
    let root = pmtiles::parse_dir(&pmtiles::decompress(h.internal_comp, &root_raw)?)?;
    let mut tiles = BTreeMap::new();
    for id in wanted_ids(area.bounds()) {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("dibatalkan"));
        }
        let mut entry = pmtiles::find(&root, id);
        for _ in 0..4 {
            let Some(e) = entry else { break };
            if e.run_length > 0 {
                tiles.insert(id, (e.offset, e.length));
                break;
            }
            if !dirs.contains_key(&e.offset) {
                let raw = get_range(&agent, &url, h.leaf_off + e.offset, e.length as u64)?;
                dirs.insert(e.offset, pmtiles::parse_dir(&pmtiles::decompress(h.internal_comp, &raw)?)?);
            }
            entry = pmtiles::find(&dirs[&e.offset], id);
        }
    }
    if tiles.is_empty() {
        return Err(io::Error::other("area ini tidak punya data peta"));
    }
    let mut seen = std::collections::HashSet::new();
    let bytes = tiles.values().filter(|t| seen.insert(t.0)).map(|t| t.1 as u64).sum();
    Ok(Plan { area, url, tiles, data_off: h.data_off, bytes })
}

/// A file name made only of plain letters, numbers and dashes.
fn file_name(area: &Area) -> String {
    let mut s: String = area
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let ns = if area.lat < 0.0 { 's' } else { 'n' };
    let ew = if area.lon < 0.0 { 'w' } else { 'e' };
    format!(
        "{}-{}km-{ns}{}-{ew}{}.pmtiles",
        s.trim_matches('-'),
        area.radius_km as u32,
        (area.lat.abs() * 100.0).round() as u32,
        (area.lon.abs() * 100.0).round() as u32
    )
}

fn fetch(plan: &Plan, cancel: &AtomicBool, progress: &dyn Fn(u64)) -> io::Result<String> {
    let agent = agent();
    std::fs::create_dir_all(offmap::dir())?;
    let final_path = offmap::dir().join(file_name(&plan.area));
    let part_path = final_path.with_extension("part");
    let data_path = final_path.with_extension("data");
    let result = (|| {
        // Unique pieces in file order, grouped into requests.
        let mut pieces: Vec<(u64, u32)> = plan.tiles.values().copied().collect();
        pieces.sort_unstable();
        pieces.dedup();
        let mut groups: Vec<(u64, u64, Vec<(u64, u32)>)> = Vec::new();
        for (off, len) in pieces {
            match groups.last_mut() {
                Some(g) if off <= g.0 + g.1 + GAP && off + len as u64 - g.0 <= CHUNK => {
                    g.1 = g.1.max(off + len as u64 - g.0);
                    g.2.push((off, len));
                }
                _ => groups.push((off, len as u64, vec![(off, len)])),
            }
        }
        let mut data = File::create(&data_path)?;
        let mut new_off: HashMap<u64, u64> = HashMap::new();
        let mut written = 0u64;
        for (start, len, members) in groups {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::other("dibatalkan"));
            }
            let buf = get_range(&agent, &plan.url, plan.data_off + start, len)?;
            for (off, l) in members {
                let i = (off - start) as usize;
                data.write_all(&buf[i..i + l as usize])?;
                new_off.insert(off, written);
                written += l as u64;
            }
            progress(written);
        }
        drop(data);

        let entries: Vec<Entry> = plan
            .tiles
            .iter()
            .map(|(&id, &(off, len))| Entry { tile_id: id, offset: new_off[&off], length: len, run_length: 1 })
            .collect();
        let dir = pmtiles::gzip(&pmtiles::write_dir(&entries));
        let label = plan.area.label();
        let meta = pmtiles::gzip(
            serde_json::json!({
                "name": label,
                "attribution": "© OpenStreetMap contributors",
                "source": plan.url,
            })
            .to_string()
            .as_bytes(),
        );
        let b = plan.area.bounds();
        let h = Header {
            root_off: HEADER_LEN as u64,
            root_len: dir.len() as u64,
            meta_off: (HEADER_LEN + dir.len()) as u64,
            meta_len: meta.len() as u64,
            leaf_off: (HEADER_LEN + dir.len() + meta.len()) as u64,
            leaf_len: 0,
            data_off: (HEADER_LEN + dir.len() + meta.len()) as u64,
            data_len: written,
            addressed: entries.len() as u64,
            entries: entries.len() as u64,
            contents: new_off.len() as u64,
            clustered: false,
            internal_comp: COMP_GZIP,
            tile_comp: COMP_GZIP,
            tile_type: pmtiles::TYPE_MVT,
            min_zoom: MIN_ZOOM,
            max_zoom: MAX_ZOOM,
            bounds: b,
            center_zoom: 13,
            center: [plan.area.lon, plan.area.lat],
        };
        let mut out = File::create(&part_path)?;
        out.write_all(&h.write())?;
        out.write_all(&dir)?;
        out.write_all(&meta)?;
        io::copy(&mut File::open(&data_path)?, &mut out)?;
        out.sync_all()?;
        drop(out);
        std::fs::rename(&part_path, &final_path)?;
        Ok(label)
    })();
    let _ = std::fs::remove_file(&data_path);
    if result.is_err() {
        let _ = std::fs::remove_file(&part_path);
    }
    result
}

/// Copies a map file the person already has into the app, after checking it can be read.
pub fn import(path: &str) -> io::Result<String> {
    let src = PathBuf::from(path.trim());
    let mut a = pmtiles::Archive::open(&src)?;
    let name = a.name().unwrap_or_else(|| src.file_stem().unwrap_or_default().to_string_lossy().into_owned());
    std::fs::create_dir_all(offmap::dir())?;
    let dest = offmap::dir().join(src.file_name().ok_or_else(|| pmtiles::bad("nama file tidak valid"))?);
    if dest.with_extension("pmtiles") != dest {
        return Err(pmtiles::bad("file peta harus berakhiran .pmtiles"));
    }
    std::fs::copy(&src, &dest)?;
    offmap::reload();
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_make_plain_file_names() {
        let a = Area { name: "Jakarta Selatan".into(), lat: -6.26, lon: 106.81, radius_km: 10.0 };
        assert_eq!(file_name(&a), "jakarta-selatan-10km-s626-e10681.pmtiles");
        let b = a.bounds();
        assert!(b[0] < 106.81 && b[2] > 106.81 && b[1] < -6.26 && b[3] > -6.26);
    }

    /// Downloads a real 5 km area into $GHOSTWALK_DATA. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn downloads_a_small_area() {
        let area = Area { name: "Jakarta Pusat".into(), lat: -6.1865, lon: 106.8341, radius_km: 5.0 };
        let p = plan(area, &AtomicBool::new(false)).unwrap();
        assert!(p.bytes > 0);
        let name = fetch(&p, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(name, "Jakarta Pusat (5 km)");
        let path = offmap::dir().join(file_name(&p.area));
        let mut a = pmtiles::Archive::open(&path).unwrap();
        let (x, y) = pmtiles::lonlat_to_tile(106.8341, -6.1865, 14);
        let tile = a.tile(14, x, y).expect("centre tile");
        let layers = crate::mvt::decode(&tile);
        assert!(layers.iter().any(|l| l.name == "roads" && !l.features.is_empty()));
        println!("{} bytes, {} tiles", std::fs::metadata(&path).unwrap().len(), p.tiles.len());
    }

    #[test]
    fn finds_kecamatan_by_name() {
        let hits = search("cibitung", 5);
        assert!(hits.iter().any(|p| p.name == "Kec. Cibitung" && p.context.contains("Bekasi")));
        let hits = search("bekasi", 5);
        assert!(!hits[0].kecamatan, "kabupaten/kota come first");
        assert!(search("x", 5).is_empty());
    }

    #[test]
    fn reads_both_coordinate_styles() {
        assert_eq!(parse_coords("-6.27, 107.07"), Some((-6.27, 107.07)));
        assert_eq!(parse_coords("-6,27 107,07"), Some((-6.27, 107.07)));
        assert_eq!(parse_coords("bekasi"), None);
        assert_eq!(parse_coords("-95, 107"), None);
    }

    #[test]
    fn wanted_tiles_cover_every_zoom() {
        let a = Area { name: "x".into(), lat: -6.9, lon: 107.6, radius_km: 5.0 };
        let ids = wanted_ids(a.bounds());
        let (x, y) = pmtiles::lonlat_to_tile(107.6, -6.9, MAX_ZOOM);
        assert!(ids.binary_search(&pmtiles::tile_id(MAX_ZOOM, x, y)).is_ok());
        let (x, y) = pmtiles::lonlat_to_tile(107.6, -6.9, MIN_ZOOM);
        assert!(ids.binary_search(&pmtiles::tile_id(MIN_ZOOM, x, y)).is_ok());
    }
}
