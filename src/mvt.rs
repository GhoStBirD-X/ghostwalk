//! A small reader for Mapbox Vector Tiles: just the layers, shapes, `kind`, `name` and `min_zoom`.
//! Spec: https://github.com/mapbox/vector-tile-spec/tree/master/2.1

pub const POINT: u8 = 1;
pub const LINE: u8 = 2;
pub const POLYGON: u8 = 3;

pub struct Layer {
    pub name: String,
    pub features: Vec<Feature>,
}

pub struct Feature {
    pub kind: String,
    pub name: String,
    pub min_zoom: f32,
    pub geom_type: u8,
    /// Rings or lines, in tile units from 0.0 to 1.0 (shapes may reach a little outside).
    pub parts: Vec<Vec<[f32; 2]>>,
}

enum Value {
    Str(String),
    Num(f64),
    Other,
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Reader { b, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.b.len()
    }

    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let byte = *self.b.get(self.pos)?;
            self.pos += 1;
            v |= ((byte & 0x7f) as u64) << shift;
            if byte < 0x80 {
                return Some(v);
            }
            shift += 7;
            if shift > 63 {
                return None;
            }
        }
    }

    fn bytes(&mut self) -> Option<&'a [u8]> {
        let n = self.varint()? as usize;
        let s = self.b.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(s)
    }

    /// Next field as (number, wire type).
    fn key(&mut self) -> Option<(u64, u8)> {
        let k = self.varint()?;
        Some((k >> 3, (k & 7) as u8))
    }

    fn skip(&mut self, wire: u8) -> Option<()> {
        match wire {
            0 => {
                self.varint()?;
            }
            1 => self.pos += 8,
            2 => {
                self.bytes()?;
            }
            5 => self.pos += 4,
            _ => return None,
        }
        (self.pos <= self.b.len()).then_some(())
    }

    fn fixed(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.b.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }
}

fn packed(b: &[u8]) -> Option<Vec<u32>> {
    let mut r = Reader::new(b);
    let mut out = Vec::new();
    while !r.done() {
        out.push(r.varint()? as u32);
    }
    Some(out)
}

fn zigzag(v: u32) -> i32 {
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

fn value(b: &[u8]) -> Option<Value> {
    let mut r = Reader::new(b);
    let mut out = Value::Other;
    while !r.done() {
        let (field, wire) = r.key()?;
        match (field, wire) {
            (1, 2) => out = Value::Str(String::from_utf8_lossy(r.bytes()?).into_owned()),
            (2, 5) => out = Value::Num(f32::from_le_bytes(r.fixed(4)?.try_into().ok()?) as f64),
            (3, 1) => out = Value::Num(f64::from_le_bytes(r.fixed(8)?.try_into().ok()?)),
            (4, 0) | (5, 0) => out = Value::Num(r.varint()? as i64 as f64),
            (6, 0) => {
                let v = r.varint()?;
                out = Value::Num(((v >> 1) as i64 ^ -((v & 1) as i64)) as f64);
            }
            _ => r.skip(wire)?,
        }
    }
    Some(out)
}

fn geometry(cmds: &[u32], extent: f32) -> Vec<Vec<[f32; 2]>> {
    let mut parts: Vec<Vec<[f32; 2]>> = Vec::new();
    let (mut x, mut y) = (0i32, 0i32);
    let mut i = 0;
    while i < cmds.len() {
        let id = cmds[i] & 7;
        let count = (cmds[i] >> 3) as usize;
        i += 1;
        match id {
            1 | 2 => {
                for _ in 0..count {
                    let (Some(&dx), Some(&dy)) = (cmds.get(i), cmds.get(i + 1)) else {
                        return parts;
                    };
                    x = x.wrapping_add(zigzag(dx));
                    y = y.wrapping_add(zigzag(dy));
                    i += 2;
                    if id == 1 || parts.is_empty() {
                        parts.push(Vec::new());
                    }
                    parts.last_mut().unwrap().push([x as f32 / extent, y as f32 / extent]);
                }
            }
            7 => {
                if let Some(p) = parts.last_mut() {
                    if let Some(&first) = p.first() {
                        p.push(first);
                    }
                }
            }
            _ => return parts,
        }
    }
    parts
}

fn layer(b: &[u8]) -> Option<Layer> {
    let mut r = Reader::new(b);
    let mut name = String::new();
    let mut keys: Vec<String> = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    let mut raw_features: Vec<&[u8]> = Vec::new();
    let mut extent = 4096.0f32;
    while !r.done() {
        let (field, wire) = r.key()?;
        match (field, wire) {
            (1, 2) => name = String::from_utf8_lossy(r.bytes()?).into_owned(),
            (2, 2) => raw_features.push(r.bytes()?),
            (3, 2) => keys.push(String::from_utf8_lossy(r.bytes()?).into_owned()),
            (4, 2) => values.push(value(r.bytes()?)?),
            (5, 0) => extent = r.varint()?.max(1) as f32,
            _ => r.skip(wire)?,
        }
    }
    let key_kind = keys.iter().position(|k| k == "kind");
    let key_name = keys.iter().position(|k| k == "name");
    let key_min = keys.iter().position(|k| k == "min_zoom");
    let mut features = Vec::with_capacity(raw_features.len());
    for fb in raw_features {
        let mut r = Reader::new(fb);
        let mut f = Feature {
            kind: String::new(),
            name: String::new(),
            min_zoom: 0.0,
            geom_type: 0,
            parts: Vec::new(),
        };
        while !r.done() {
            let (field, wire) = r.key()?;
            match (field, wire) {
                (2, 2) => {
                    let tags = packed(r.bytes()?)?;
                    for kv in tags.chunks_exact(2) {
                        let (k, v) = (Some(kv[0] as usize), values.get(kv[1] as usize));
                        match v {
                            Some(Value::Str(s)) if k == key_kind => f.kind = s.clone(),
                            Some(Value::Str(s)) if k == key_name => f.name = s.clone(),
                            Some(Value::Num(n)) if k == key_min => f.min_zoom = *n as f32,
                            _ => {}
                        }
                    }
                }
                (3, 0) => f.geom_type = r.varint()? as u8,
                (4, 2) => f.parts = geometry(&packed(r.bytes()?)?, extent),
                _ => r.skip(wire)?,
            }
        }
        if !f.parts.is_empty() {
            features.push(f);
        }
    }
    Some(Layer { name, features })
}

/// Decodes a (decompressed) tile. Broken layers are skipped.
pub fn decode(b: &[u8]) -> Vec<Layer> {
    let mut r = Reader::new(b);
    let mut out = Vec::new();
    while !r.done() {
        let Some((field, wire)) = r.key() else { break };
        if field == 3 && wire == 2 {
            let Some(lb) = r.bytes() else { break };
            if let Some(l) = layer(lb) {
                out.push(l);
            }
        } else if r.skip(wire).is_none() {
            break;
        }
    }
    out
}

/// Signed ring area in tile units; positive for outer rings (clockwise on screen).
pub fn ring_area(ring: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for w in ring.windows(2) {
        a += w[0][0] * w[1][1] - w[1][0] * w[0][1];
    }
    a / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_commands_from_the_spec() {
        // MoveTo(2,2) LineTo(2,10) LineTo(10,10) ClosePath, on a 10-unit tile.
        let cmds = [9, 4, 4, 18, 0, 16, 16, 0, 15];
        let parts = geometry(&cmds, 10.0);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0], vec![[0.2, 0.2], [0.2, 1.0], [1.0, 1.0], [0.2, 0.2]]);
        assert!(ring_area(&parts[0]) != 0.0);
    }
}
