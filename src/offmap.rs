//! Offline maps: the map files kept on this device, and drawing them under routes.

use crate::draw::{EARTH_M, View};
use crate::mvt;
use crate::pmtiles::Archive;
use eframe::egui::{self, Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, epaint::Vertex, pos2};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use crate::model::MapTheme;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

pub const ATTRIBUTION: &str = "© OpenStreetMap";
/// Tiles kept decoded in memory.
const CACHE_TILES: usize = 160;
/// Never draw more tiles than this in one map.
const MAX_TILES: u32 = 48;

/// The map style for every map drawn, as an index into `MapTheme::ALL`.
static THEME: AtomicU8 = AtomicU8::new(0);

/// Sets the map style; the app calls this each frame with `Settings::theme`.
pub fn set_theme(t: MapTheme) {
    let i = MapTheme::ALL.iter().position(|x| *x == t).unwrap_or(0);
    THEME.store(i as u8, Ordering::Relaxed);
}

pub fn theme() -> MapTheme {
    MapTheme::ALL.get(THEME.load(Ordering::Relaxed) as usize).copied().unwrap_or_default()
}

/// The place name nearest the middle of the last map drawn, for a game-style banner.
static AREA: Mutex<String> = Mutex::new(String::new());

pub fn area_name() -> String {
    AREA.lock().map(|a| a.clone()).unwrap_or_default()
}

/// Where map files live.
pub fn dir() -> PathBuf {
    crate::model::data_dir().join("peta")
}

pub struct MapFile {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    /// West, south, east, north in degrees.
    pub bounds: [f64; 4],
    archive: Archive,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Fill {
    Earth,
    Green,
    Sand,
    Water,
    Building,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
    River,
    Stream,
    Rail,
    Path,
    Minor,
    Major,
    Highway,
}

const FILL_ORDER: [Fill; 5] = [Fill::Earth, Fill::Green, Fill::Sand, Fill::Water, Fill::Building];

struct MeshPart {
    pts: Vec<[f32; 2]>,
    idx: Vec<u32>,
}

/// One decoded tile, ready to draw. Points are in tile units (0..1).
struct Tile {
    fills: HashMap<Fill, MeshPart>,
    lines: Vec<(Line, Vec<[f32; 2]>)>,
    labels: Vec<([f32; 2], String, f32)>,
}

#[derive(Default)]
struct Store {
    loaded: bool,
    maps: Vec<MapFile>,
    cache: HashMap<(usize, u8, u32, u32), Option<Arc<Tile>>>,
    order: VecDeque<(usize, u8, u32, u32)>,
}

fn store() -> MutexGuard<'static, Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    let mut s = S.get_or_init(|| Mutex::new(Store::default())).lock().unwrap_or_else(|e| e.into_inner());
    if !s.loaded {
        s.load();
    }
    s
}

impl Store {
    fn load(&mut self) {
        self.loaded = true;
        self.maps.clear();
        self.cache.clear();
        self.order.clear();
        let Ok(rd) = std::fs::read_dir(dir()) else { return };
        let mut paths: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "pmtiles"))
            .collect();
        paths.sort();
        for path in paths {
            if let Ok(mut archive) = Archive::open(&path) {
                let name = archive.name().unwrap_or_else(|| file_label(&path));
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let bounds = archive.header.bounds;
                self.maps.push(MapFile { path, name, size, bounds, archive });
            }
        }
    }

    fn tile(&mut self, map: usize, z: u8, x: u32, y: u32) -> Option<Arc<Tile>> {
        let key = (map, z, x, y);
        if let Some(t) = self.cache.get(&key) {
            return t.clone();
        }
        let t = self.maps[map].archive.tile(z, x, y).map(|b| Arc::new(build_tile(&b)));
        self.cache.insert(key, t.clone());
        self.order.push_back(key);
        while self.order.len() > CACHE_TILES {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            }
        }
        t
    }
}

fn file_label(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().replace(['-', '_'], " ")).unwrap_or_default()
}

/// Forget what is loaded, e.g. after a map was added or removed.
pub fn reload() {
    store().load();
}

/// (name, size in bytes, file) for every installed map.
pub fn list() -> Vec<(String, u64, PathBuf)> {
    store().maps.iter().map(|m| (m.name.clone(), m.size, m.path.clone())).collect()
}

// ---------- turning a tile into shapes ----------

fn fill_class(layer: &str, kind: &str) -> Option<Fill> {
    match layer {
        "earth" => Some(Fill::Earth),
        "water" => Some(Fill::Water),
        "buildings" => Some(Fill::Building),
        "landcover" => match kind {
            "forest" | "grassland" | "scrub" => Some(Fill::Green),
            "barren" => Some(Fill::Sand),
            _ => None,
        },
        "landuse" => match kind {
            "park" | "forest" | "wood" | "grass" | "garden" | "golf_course" | "nature_reserve" | "national_park"
            | "playground" | "pitch" | "recreation_ground" | "cemetery" | "meadow" | "village_green" | "scrub"
            | "wetland" | "orchard" | "farmland" => Some(Fill::Green),
            "beach" | "sand" | "bare_rock" => Some(Fill::Sand),
            _ => None,
        },
        _ => None,
    }
}

fn line_class(layer: &str, kind: &str) -> Option<Line> {
    match (layer, kind) {
        ("roads", "highway") => Some(Line::Highway),
        ("roads", "major_road") => Some(Line::Major),
        ("roads", "minor_road") => Some(Line::Minor),
        ("roads", "path") => Some(Line::Path),
        ("roads", "rail") => Some(Line::Rail),
        ("water", "river" | "canal") => Some(Line::River),
        ("water", "stream" | "ditch" | "drain") => Some(Line::Stream),
        _ => None,
    }
}

fn build_tile(bytes: &[u8]) -> Tile {
    let mut t = Tile { fills: HashMap::new(), lines: Vec::new(), labels: Vec::new() };
    for layer in mvt::decode(bytes) {
        for f in &layer.features {
            match f.geom_type {
                mvt::POLYGON => {
                    if let Some(class) = fill_class(&layer.name, &f.kind) {
                        let mesh = t.fills.entry(class).or_insert_with(|| MeshPart { pts: Vec::new(), idx: Vec::new() });
                        triangulate(&f.parts, mesh);
                    }
                }
                mvt::LINE => {
                    if let Some(class) = line_class(&layer.name, &f.kind) {
                        for part in &f.parts {
                            if part.len() > 1 {
                                t.lines.push((class, part.clone()));
                            }
                        }
                    }
                }
                mvt::POINT if layer.name == "places" && !f.name.is_empty() => {
                    if matches!(f.kind.as_str(), "locality" | "neighbourhood" | "macrohood") {
                        for part in &f.parts {
                            let p = part[0];
                            if (0.0..1.0).contains(&p[0]) && (0.0..1.0).contains(&p[1]) {
                                t.labels.push((p, f.name.clone(), f.min_zoom));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // Draw big roads last so they sit on top.
    t.lines.sort_by_key(|l| l.0 as u8);
    t
}

/// Outer rings start a new polygon; the holes that follow belong to it.
fn triangulate(parts: &[Vec<[f32; 2]>], out: &mut MeshPart) {
    let mut flush = |coords: &mut Vec<f32>, holes: &mut Vec<usize>| {
        if coords.len() >= 6 {
            if let Ok(tris) = earcutr::earcut(coords, holes, 2) {
                let base = out.pts.len() as u32;
                out.pts.extend(coords.chunks_exact(2).map(|c| [c[0], c[1]]));
                out.idx.extend(tris.into_iter().map(|i| base + i as u32));
            }
        }
        coords.clear();
        holes.clear();
    };
    let mut coords: Vec<f32> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    for ring in parts {
        let area = mvt::ring_area(ring);
        if area == 0.0 || ring.len() < 4 {
            continue;
        }
        let ring = &ring[..ring.len() - 1];
        if area > 0.0 {
            flush(&mut coords, &mut holes);
        } else if coords.is_empty() {
            continue;
        } else {
            holes.push(coords.len() / 2);
        }
        coords.extend(ring.iter().flat_map(|p| [p[0], p[1]]));
    }
    flush(&mut coords, &mut holes);
}

// ---------- drawing ----------

struct Palette {
    sea: Color32,
    earth: Color32,
    green: Color32,
    sand: Color32,
    building: Color32,
    casing: Color32,
    road: Color32,
    highway: Color32,
    path: Color32,
    rail: Color32,
    label: Color32,
    halo: Color32,
    /// Scales every line.
    widths: f32,
}

fn palette(dark: bool) -> Palette {
    palette_for(theme(), dark)
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn palette_for(theme: MapTheme, dark: bool) -> Palette {
    match theme {
        MapTheme::Standard if dark => Palette {
            sea: rgb(17, 29, 40),
            earth: rgb(25, 26, 29),
            green: rgb(25, 38, 29),
            sand: rgb(40, 38, 33),
            building: rgb(35, 36, 41),
            casing: rgb(19, 19, 22),
            road: rgb(54, 56, 63),
            highway: rgb(96, 78, 52),
            path: rgb(62, 60, 57),
            rail: rgb(58, 58, 64),
            label: rgb(150, 150, 162),
            halo: rgb(19, 19, 22),
            widths: 1.0,
        },
        MapTheme::Standard => Palette {
            sea: rgb(170, 211, 230),
            earth: rgb(242, 239, 233),
            green: rgb(205, 232, 192),
            sand: rgb(242, 232, 200),
            building: rgb(222, 216, 208),
            casing: rgb(214, 206, 196),
            road: Color32::WHITE,
            highway: rgb(255, 206, 128),
            path: rgb(196, 180, 160),
            rail: rgb(170, 170, 176),
            label: rgb(70, 70, 80),
            halo: rgb(250, 248, 244),
            widths: 1.0,
        },
        MapTheme::Neon => Palette {
            sea: rgb(16, 8, 38),
            earth: rgb(12, 9, 24),
            green: rgb(14, 30, 34),
            sand: rgb(30, 18, 38),
            building: rgb(32, 18, 52),
            casing: rgb(30, 14, 54),
            road: rgb(0, 112, 132),
            highway: rgb(196, 36, 140),
            path: rgb(78, 60, 140),
            rail: rgb(110, 46, 160),
            label: rgb(150, 245, 255),
            halo: rgb(12, 9, 24),
            widths: 0.7,
        },
        MapTheme::Treasure => Palette {
            sea: rgb(150, 178, 170),
            earth: rgb(232, 214, 172),
            green: rgb(196, 194, 140),
            sand: rgb(240, 222, 170),
            building: rgb(208, 182, 136),
            casing: rgb(120, 82, 46),
            road: rgb(244, 230, 196),
            highway: rgb(196, 120, 64),
            path: rgb(140, 96, 56),
            rail: rgb(110, 80, 52),
            label: rgb(84, 50, 24),
            halo: rgb(240, 226, 190),
            widths: 1.0,
        },
        MapTheme::Pixel => Palette {
            sea: rgb(64, 120, 232),
            earth: rgb(110, 184, 76),
            green: rgb(56, 140, 52),
            sand: rgb(232, 212, 120),
            building: rgb(150, 150, 150),
            casing: rgb(40, 40, 40),
            road: rgb(200, 200, 200),
            highway: rgb(250, 200, 60),
            path: rgb(150, 110, 70),
            rail: rgb(80, 80, 80),
            label: Color32::WHITE,
            halo: rgb(30, 30, 30),
            widths: 1.5,
        },
        MapTheme::Adventurer => Palette {
            sea: rgb(126, 162, 160),
            earth: rgb(228, 208, 164),
            green: rgb(186, 190, 126),
            sand: rgb(236, 218, 170),
            building: rgb(210, 184, 136),
            casing: rgb(122, 86, 48),
            road: rgb(244, 232, 200),
            highway: rgb(176, 112, 58),
            path: rgb(152, 108, 62),
            rail: rgb(104, 74, 46),
            label: rgb(78, 44, 22),
            halo: rgb(240, 226, 190),
            widths: 1.0,
        },
        MapTheme::Hunter => Palette {
            sea: rgb(40, 64, 72),
            earth: rgb(60, 64, 50),
            green: rgb(54, 76, 46),
            sand: rgb(88, 80, 58),
            building: rgb(76, 78, 62),
            casing: rgb(32, 34, 26),
            road: rgb(112, 106, 82),
            highway: rgb(150, 142, 110),
            path: rgb(98, 92, 70),
            rail: rgb(92, 92, 72),
            label: rgb(232, 220, 186),
            halo: rgb(30, 32, 24),
            widths: 0.9,
        },
        MapTheme::Radar => Palette {
            sea: rgb(0, 26, 18),
            earth: rgb(4, 12, 8),
            green: rgb(4, 22, 12),
            sand: rgb(10, 20, 10),
            building: rgb(10, 36, 22),
            casing: rgb(4, 12, 8),
            road: rgb(40, 170, 90),
            highway: rgb(120, 255, 150),
            path: rgb(24, 110, 60),
            rail: rgb(30, 90, 50),
            label: rgb(120, 255, 160),
            halo: rgb(4, 12, 8),
            widths: 0.8,
        },
        MapTheme::Snow => Palette {
            sea: rgb(120, 170, 220),
            earth: rgb(250, 252, 255),
            green: rgb(222, 236, 244),
            sand: rgb(240, 244, 248),
            building: rgb(204, 214, 228),
            casing: rgb(110, 130, 160),
            road: Color32::WHITE,
            highway: rgb(70, 130, 210),
            path: rgb(130, 150, 180),
            rail: rgb(90, 100, 120),
            label: rgb(20, 40, 70),
            halo: Color32::WHITE,
            widths: 1.2,
        },
    }
}

/// The colour behind a map for `theme`, where no map file covers the area; None keeps the app's own.
pub fn backdrop(dark: bool) -> Option<(Color32, Color32, Color32, Color32)> {
    let t = theme();
    if t == MapTheme::Standard {
        return None;
    }
    let p = palette_for(t, dark);
    Some((p.earth, p.building, p.road, p.green))
}

/// A small sample of `theme`: land, a park, water and two roads, for choosing between themes.
pub fn swatch(p: &Painter, rect: Rect, theme: MapTheme, dark: bool) {
    let pal = palette_for(theme, dark);
    p.rect_filled(rect, 8.0, pal.earth);
    let (w, h) = (rect.width(), rect.height());
    let at = |x: f32, y: f32| pos2(rect.left() + w * x, rect.top() + h * y);
    p.rect_filled(Rect::from_min_max(at(0.08, 0.1), at(0.42, 0.45)), 4.0, pal.green);
    p.rect_filled(Rect::from_min_max(at(0.6, 0.62), at(0.94, 0.92)), 4.0, pal.sea);
    for (a, b, c, wd) in [(at(0.0, 0.55), at(1.0, 0.5), pal.highway, 5.0), (at(0.5, 0.0), at(0.52, 1.0), pal.road, 3.5)] {
        p.line_segment([a, b], Stroke::new(wd * pal.widths + 1.6, pal.casing));
        p.line_segment([a, b], Stroke::new(wd * pal.widths, c));
    }
    p.circle_filled(at(0.25, 0.75), 3.0, pal.label);
}

/// Line width in pixels; `zoom` is the (fractional) map zoom level.
fn line_width(class: Line, zoom: f32) -> f32 {
    let base = match class {
        Line::Highway => 9.0,
        Line::Major => 7.0,
        Line::Minor => 4.5,
        Line::River => 5.0,
        Line::Path | Line::Stream | Line::Rail => 1.6,
    };
    (base * 2f32.powf(zoom - 15.0)).clamp(0.6, base * 2.0)
}

/// The middle of the first installed map, as (latitude, longitude).
pub fn first_center() -> Option<(f64, f64)> {
    store().maps.first().map(|m| {
        let [w, s, e, n] = m.bounds;
        ((s + n) / 2.0, (w + e) / 2.0)
    })
}

/// Draws installed maps under everything else. Returns false when no map covers this view.
pub fn draw(p: &Painter, rect: Rect, v: &View, dark: bool) -> bool {
    let mut s = store();
    if s.maps.is_empty() {
        return false;
    }
    let zoom = ((v.s * EARTH_M / 256.0).log2()) as f32;
    // All four corners, since a turned map shows more than the corners of a north-up one.
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()].map(|q| v.to_world(q));
    let (w0x, w1x) = corners.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q[0]), b.max(q[0])));
    let (w0y, w1y) = corners.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q[1]), b.max(q[1])));
    let lon_lat = |wx: f64, wy: f64| {
        let lon = wx / EARTH_M * 360.0 - 180.0;
        let lat = (std::f64::consts::PI * (1.0 - 2.0 * wy / EARTH_M)).sinh().atan().to_degrees();
        (lon, lat)
    };
    let (west, north) = lon_lat(w0x, w0y);
    let (east, south) = lon_lat(w1x, w1y);

    // (z, x, y, tile) for every visible tile some map has.
    let mut tiles: Vec<(u8, u32, u32, Arc<Tile>)> = Vec::new();
    for m in 0..s.maps.len() {
        let [bw, bs, be, bn] = s.maps[m].bounds;
        if be < west || bw > east || bn < south || bs > north {
            continue;
        }
        let h = &s.maps[m].archive.header;
        let z = (zoom.floor().max(0.0) as u8).clamp(h.min_zoom, h.max_zoom);
        let n = (1u64 << z) as f64;
        let tx = |wx: f64| (wx / EARTH_M * n).floor().clamp(0.0, n - 1.0) as u32;
        let (x0, x1, y0, y1) = (tx(w0x), tx(w1x), tx(w0y), tx(w1y));
        if (x1 - x0 + 1) * (y1 - y0 + 1) > MAX_TILES {
            continue;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                if tiles.iter().any(|t| (t.0, t.1, t.2) == (z, x, y)) {
                    continue;
                }
                if let Some(t) = s.tile(m, z, x, y) {
                    tiles.push((z, x, y, t));
                }
            }
        }
    }
    drop(s);
    if tiles.is_empty() {
        return false;
    }

    let pal = palette(dark);
    let place = |z: u8, x: u32, y: u32| {
        let size = EARTH_M / (1u64 << z) as f64;
        let (ox, oy) = (x as f64 * size, y as f64 * size);
        move |q: [f32; 2]| v.to_screen([ox + q[0] as f64 * size, oy + q[1] as f64 * size])
    };
    // Sea under each tile; land comes from the "earth" shapes. Outside the map the backdrop stays.
    for (z, x, y, _) in &tiles {
        let to = place(*z, *x, *y);
        let mut sea = egui::Mesh::default();
        for q in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            sea.colored_vertex(to(q), pal.sea);
        }
        sea.add_triangle(0, 1, 2);
        sea.add_triangle(0, 2, 3);
        p.add(Shape::mesh(sea));
    }

    for class in FILL_ORDER {
        let color = match class {
            Fill::Earth => pal.earth,
            Fill::Green => pal.green,
            Fill::Sand => pal.sand,
            Fill::Water => pal.sea,
            Fill::Building => pal.building,
        };
        if class == Fill::Building && zoom < 14.0 {
            continue;
        }
        for (z, x, y, t) in &tiles {
            let Some(part) = t.fills.get(&class) else { continue };
            let to = place(*z, *x, *y);
            let mut mesh = egui::Mesh::default();
            mesh.indices = part.idx.clone();
            mesh.vertices = part
                .pts
                .iter()
                .map(|q| Vertex { pos: to(*q), uv: egui::epaint::WHITE_UV, color })
                .collect();
            p.add(Shape::mesh(mesh));
        }
    }

    let stroke_for = |class: Line, casing: bool| -> Option<Stroke> {
        let w = line_width(class, zoom) * pal.widths;
        let (color, w) = match class {
            Line::River | Line::Stream if !casing => (pal.sea, w),
            Line::Rail if !casing => (pal.rail, w),
            Line::Path if !casing => (pal.path, w),
            Line::Minor | Line::Major | Line::Highway if casing => (pal.casing, w + 1.6),
            Line::Minor | Line::Major if !casing => (pal.road, w),
            Line::Highway if !casing => (pal.highway, w),
            _ => return None,
        };
        Some(Stroke::new(w, color))
    };
    for casing in [true, false] {
        for (z, x, y, t) in &tiles {
            let to = place(*z, *x, *y);
            for (class, pts) in &t.lines {
                if *class == Line::Path && zoom < 13.0 {
                    continue;
                }
                if let Some(st) = stroke_for(*class, casing) {
                    p.add(Shape::line(pts.iter().map(|q| to(*q)).collect(), st));
                }
            }
        }
    }

    let mut used: Vec<Rect> = Vec::new();
    let mut nearest: Option<(f32, &String)> = None;
    let (family, upper) = match theme() {
        MapTheme::Adventurer => (crate::draw::serif(), false),
        MapTheme::Hunter => (crate::draw::semi(), true),
        _ => (egui::FontFamily::Proportional, false),
    };
    for (z, x, y, t) in &tiles {
        let to = place(*z, *x, *y);
        for (q, name, min_zoom) in &t.labels {
            let at = to(*q);
            let d = at.distance(rect.center());
            // The closest name, even just outside the map, so a small map still gets one.
            if d < rect.width() && nearest.is_none_or(|n| d < n.0) {
                nearest = Some((d, name));
            }
            if *min_zoom > zoom + 0.5 {
                continue;
            }
            let font = FontId::new(if family == egui::FontFamily::Proportional { 11.5 } else { 12.5 }, family.clone());
            let shown = if upper { name.to_uppercase() } else { name.clone() };
            let name = &shown;
            let galley = p.layout_no_wrap(name.clone(), font.clone(), pal.label);
            let r = Align2::CENTER_CENTER.anchor_size(at, galley.size()).expand(3.0);
            if !rect.contains_rect(r) || used.iter().any(|u| u.intersects(r)) {
                continue;
            }
            used.push(r);
            for d in [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]] {
                p.text(at + egui::vec2(d[0], d[1]), Align2::CENTER_CENTER, name, font.clone(), pal.halo);
            }
            p.text(at, Align2::CENTER_CENTER, name, font, pal.label);
        }
    }
    if let Ok(mut a) = AREA.lock() {
        *a = nearest.map(|n| n.1.clone()).unwrap_or_default();
    }
    true
}

/// The small credit line every map must show.
pub fn attribution(p: &Painter, rect: Rect, dark: bool) {
    let font = FontId::proportional(9.5);
    let pal = palette(dark);
    let at: Pos2 = pos2(rect.right() - 6.0, rect.bottom() - 4.0);
    let g = p.layout_no_wrap(ATTRIBUTION.into(), font.clone(), pal.label);
    let r = Align2::RIGHT_BOTTOM.anchor_size(at, g.size()).expand(2.0);
    p.rect_filled(r, 3.0, pal.halo.gamma_multiply(0.85));
    p.text(at, Align2::RIGHT_BOTTOM, ATTRIBUTION, font, pal.label);
}
