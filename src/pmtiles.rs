//! Reading and writing PMTiles v3 map files: one file holding every map tile of an area.
//! Spec: https://github.com/protomaps/PMTiles/blob/main/spec/v3/spec.md

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub const HEADER_LEN: usize = 127;

pub const COMP_NONE: u8 = 1;
pub const COMP_GZIP: u8 = 2;
pub const TYPE_MVT: u8 = 1;

#[derive(Clone, Debug, Default)]
pub struct Header {
    pub root_off: u64,
    pub root_len: u64,
    pub meta_off: u64,
    pub meta_len: u64,
    pub leaf_off: u64,
    pub leaf_len: u64,
    pub data_off: u64,
    pub data_len: u64,
    pub addressed: u64,
    pub entries: u64,
    pub contents: u64,
    pub clustered: bool,
    pub internal_comp: u8,
    pub tile_comp: u8,
    pub tile_type: u8,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// West, south, east, north in degrees.
    pub bounds: [f64; 4],
    pub center_zoom: u8,
    pub center: [f64; 2],
}

fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

fn deg_at(b: &[u8], i: usize) -> f64 {
    i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as f64 / 1e7
}

impl Header {
    pub fn parse(b: &[u8]) -> io::Result<Header> {
        if b.len() < HEADER_LEN || &b[..7] != b"PMTiles" {
            return Err(bad("bukan file PMTiles"));
        }
        if b[7] != 3 {
            return Err(bad("versi PMTiles tidak didukung (perlu versi 3)"));
        }
        Ok(Header {
            root_off: u64_at(b, 8),
            root_len: u64_at(b, 16),
            meta_off: u64_at(b, 24),
            meta_len: u64_at(b, 32),
            leaf_off: u64_at(b, 40),
            leaf_len: u64_at(b, 48),
            data_off: u64_at(b, 56),
            data_len: u64_at(b, 64),
            addressed: u64_at(b, 72),
            entries: u64_at(b, 80),
            contents: u64_at(b, 88),
            clustered: b[96] == 1,
            internal_comp: b[97],
            tile_comp: b[98],
            tile_type: b[99],
            min_zoom: b[100],
            max_zoom: b[101],
            bounds: [deg_at(b, 102), deg_at(b, 106), deg_at(b, 110), deg_at(b, 114)],
            center_zoom: b[118],
            center: [deg_at(b, 119), deg_at(b, 123)],
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER_LEN);
        b.extend_from_slice(b"PMTiles");
        b.push(3);
        for v in [
            self.root_off,
            self.root_len,
            self.meta_off,
            self.meta_len,
            self.leaf_off,
            self.leaf_len,
            self.data_off,
            self.data_len,
            self.addressed,
            self.entries,
            self.contents,
        ] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[
            self.clustered as u8,
            self.internal_comp,
            self.tile_comp,
            self.tile_type,
            self.min_zoom,
            self.max_zoom,
        ]);
        for d in self.bounds {
            b.extend_from_slice(&((d * 1e7) as i32).to_le_bytes());
        }
        b.push(self.center_zoom);
        for d in self.center {
            b.extend_from_slice(&((d * 1e7) as i32).to_le_bytes());
        }
        b
    }
}

pub fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

pub fn decompress(comp: u8, b: &[u8]) -> io::Result<Vec<u8>> {
    match comp {
        COMP_NONE | 0 => Ok(b.to_vec()),
        COMP_GZIP => {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(b).read_to_end(&mut out)?;
            Ok(out)
        }
        _ => Err(bad("kompresi peta tidak didukung (perlu gzip)")),
    }
}

pub fn gzip(b: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let _ = e.write_all(b);
    e.finish().unwrap_or_default()
}

// ---------- tile ids ----------

/// Tile id along the Hilbert curve, counting every tile of the lower zoom levels first.
pub fn tile_id(z: u8, x: u32, y: u32) -> u64 {
    let base = ((1u64 << (2 * z as u64)) - 1) / 3;
    let n = 1u64 << z;
    let (mut x, mut y) = (x as u64, y as u64);
    let mut d = 0u64;
    let mut s = n / 2;
    while s > 0 {
        let rx = ((x & s) > 0) as u64;
        let ry = ((y & s) > 0) as u64;
        d += s * s * ((3 * rx) ^ ry);
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - (x & (s - 1));
                y = s - 1 - (y & (s - 1));
            }
            std::mem::swap(&mut x, &mut y);
        }
        s /= 2;
    }
    base + d
}

/// Tile column and row holding a point, at zoom `z`.
pub fn lonlat_to_tile(lon: f64, lat: f64, z: u8) -> (u32, u32) {
    let n = (1u64 << z) as f64;
    let [mx, my] = crate::draw::merc01(lat, lon);
    let clamp = |v: f64| (v * n).floor().clamp(0.0, n - 1.0) as u32;
    (clamp(mx), clamp(my))
}

// ---------- directories ----------

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub tile_id: u64,
    pub offset: u64,
    pub length: u32,
    /// 0 means the entry points at another directory.
    pub run_length: u32,
}

fn read_varint(b: &[u8], pos: &mut usize) -> io::Result<u64> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let byte = *b.get(*pos).ok_or_else(|| bad("direktori peta rusak"))?;
        *pos += 1;
        v |= ((byte & 0x7f) as u64) << shift;
        if byte < 0x80 {
            return Ok(v);
        }
        shift += 7;
        if shift > 63 {
            return Err(bad("direktori peta rusak"));
        }
    }
}

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// Reads an already decompressed directory.
pub fn parse_dir(b: &[u8]) -> io::Result<Vec<Entry>> {
    let mut pos = 0;
    let n = read_varint(b, &mut pos)? as usize;
    if n > b.len() {
        return Err(bad("direktori peta rusak"));
    }
    let mut e = vec![Entry { tile_id: 0, offset: 0, length: 0, run_length: 0 }; n];
    let mut last = 0;
    for x in e.iter_mut() {
        last += read_varint(b, &mut pos)?;
        x.tile_id = last;
    }
    for x in e.iter_mut() {
        x.run_length = read_varint(b, &mut pos)? as u32;
    }
    for x in e.iter_mut() {
        x.length = read_varint(b, &mut pos)? as u32;
    }
    for i in 0..n {
        let v = read_varint(b, &mut pos)?;
        e[i].offset = if v == 0 && i > 0 { e[i - 1].offset + e[i - 1].length as u64 } else { v.saturating_sub(1) };
    }
    Ok(e)
}

/// Writes a directory (uncompressed). Entries must be sorted by tile id.
pub fn write_dir(e: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    write_varint(&mut out, e.len() as u64);
    let mut last = 0;
    for x in e {
        write_varint(&mut out, x.tile_id - last);
        last = x.tile_id;
    }
    for x in e {
        write_varint(&mut out, x.run_length as u64);
    }
    for x in e {
        write_varint(&mut out, x.length as u64);
    }
    for (i, x) in e.iter().enumerate() {
        if i > 0 && x.offset == e[i - 1].offset + e[i - 1].length as u64 {
            write_varint(&mut out, 0);
        } else {
            write_varint(&mut out, x.offset + 1);
        }
    }
    out
}

/// The entry covering `id`: either the tile itself or the leaf directory to look in next.
pub fn find(e: &[Entry], id: u64) -> Option<Entry> {
    let i = e.partition_point(|x| x.tile_id <= id);
    let x = *e.get(i.checked_sub(1)?)?;
    if x.run_length == 0 || id < x.tile_id + x.run_length as u64 {
        Some(x)
    } else {
        None
    }
}

// ---------- local files ----------

/// A map file on this device.
pub struct Archive {
    file: File,
    pub header: Header,
    root: Vec<Entry>,
    leaves: HashMap<u64, Vec<Entry>>,
}

impl Archive {
    pub fn open(path: &Path) -> io::Result<Archive> {
        let mut file = File::open(path)?;
        let mut hb = [0u8; HEADER_LEN];
        file.read_exact(&mut hb)?;
        let header = Header::parse(&hb)?;
        if header.tile_type != TYPE_MVT {
            return Err(bad("peta ini berisi gambar, bukan peta vektor (MVT)"));
        }
        let raw = read_at(&mut file, header.root_off, header.root_len)?;
        let root = parse_dir(&decompress(header.internal_comp, &raw)?)?;
        Ok(Archive { file, header, root, leaves: HashMap::new() })
    }

    /// The decompressed tile, or None when the file has no tile there.
    pub fn tile(&mut self, z: u8, x: u32, y: u32) -> Option<Vec<u8>> {
        let id = tile_id(z, x, y);
        let mut entry = find(&self.root, id)?;
        for _ in 0..4 {
            if entry.run_length > 0 {
                let raw = read_at(&mut self.file, self.header.data_off + entry.offset, entry.length as u64).ok()?;
                return decompress(self.header.tile_comp, &raw).ok();
            }
            if !self.leaves.contains_key(&entry.offset) {
                let raw = read_at(&mut self.file, self.header.leaf_off + entry.offset, entry.length as u64).ok()?;
                let dir = parse_dir(&decompress(self.header.internal_comp, &raw).ok()?).ok()?;
                self.leaves.insert(entry.offset, dir);
            }
            entry = find(&self.leaves[&entry.offset], id)?;
        }
        None
    }

    /// The `name` stored in the file's metadata, if any.
    pub fn name(&mut self) -> Option<String> {
        if self.header.meta_len == 0 || self.header.meta_len > 1 << 20 {
            return None;
        }
        let raw = read_at(&mut self.file, self.header.meta_off, self.header.meta_len).ok()?;
        let json: serde_json::Value = serde_json::from_slice(&decompress(self.header.internal_comp, &raw).ok()?).ok()?;
        json.get("name")?.as_str().map(str::to_string)
    }
}

fn read_at(f: &mut File, off: u64, len: u64) -> io::Result<Vec<u8>> {
    if len > 64 << 20 {
        return Err(bad("bagian peta terlalu besar"));
    }
    f.seek(SeekFrom::Start(off))?;
    let mut b = vec![0u8; len as usize];
    f.read_exact(&mut b)?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_ids_follow_the_spec() {
        assert_eq!(tile_id(0, 0, 0), 0);
        assert_eq!(tile_id(1, 0, 0), 1);
        assert_eq!(tile_id(1, 0, 1), 2);
        assert_eq!(tile_id(1, 1, 1), 3);
        assert_eq!(tile_id(1, 1, 0), 4);
        assert_eq!(tile_id(2, 0, 0), 5);
        assert_eq!(tile_id(12, 3423, 1763), 19078479);
    }

    #[test]
    fn directories_round_trip() {
        let e = vec![
            Entry { tile_id: 5, offset: 0, length: 10, run_length: 1 },
            Entry { tile_id: 6, offset: 10, length: 7, run_length: 3 },
            Entry { tile_id: 20, offset: 0, length: 10, run_length: 1 },
        ];
        let back = parse_dir(&write_dir(&e)).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!((back[1].offset, back[1].run_length), (10, 3));
        assert_eq!(back[2].offset, 0);
        assert_eq!(find(&back, 8).unwrap().tile_id, 6);
        assert!(find(&back, 9).is_none());
    }
}
