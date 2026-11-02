//! Reading and writing GPX files and CSV tables, without any network access.

use crate::model::{Activity, TrackPoint};
use chrono::{DateTime, TimeZone, Utc};

fn attr(tag: &str, name: &str) -> Option<f64> {
    let key = format!("{name}=");
    let i = tag.find(&key)? + key.len();
    let rest = &tag[i..];
    let q = rest.chars().next()?;
    let rest = &rest[1..];
    let end = rest.find(q)?;
    rest[..end].trim().parse().ok()
}

fn inner<'a>(s: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let i = s.find(&open)?;
    let after = &s[i..];
    let gt = after.find('>')? + 1;
    let close = format!("</{tag}>");
    let end = after.find(&close)?;
    (gt <= end).then(|| after[gt..end].trim())
}

/// Parses a GPX file. Returns (name, start unix seconds, points).
pub fn parse(text: &str) -> Result<(Option<String>, Option<i64>, Vec<TrackPoint>), String> {
    let mut pts = Vec::new();
    let mut first_time: Option<DateTime<Utc>> = None;
    let mut rest = text;
    while let Some(i) = rest.find("<trkpt").or_else(|| rest.find("<rtept")) {
        rest = &rest[i..];
        let tag_end = rest.find('>').ok_or("GPX rusak")?;
        let tag = &rest[..tag_end];
        let self_closing = tag.ends_with('/');
        let body_end = if self_closing {
            tag_end
        } else {
            rest.find("</trkpt>")
                .or_else(|| rest.find("</rtept>"))
                .unwrap_or(tag_end)
        };
        let body = &rest[tag_end.min(body_end)..body_end];
        let (Some(lat), Some(lon)) = (attr(tag, "lat"), attr(tag, "lon")) else {
            rest = &rest[tag_end..];
            continue;
        };
        let ele = inner(body, "ele").and_then(|e| e.parse().ok());
        let time = inner(body, "time").and_then(|t| DateTime::parse_from_rfc3339(t).ok());
        let t = time.map(|tm| {
            let tm = tm.with_timezone(&Utc);
            let f = *first_time.get_or_insert(tm);
            (tm - f).num_milliseconds() as f64 / 1000.0
        });
        pts.push(TrackPoint { lat, lon, ele, t });
        rest = &rest[body_end.max(tag_end)..];
    }
    if pts.len() < 2 {
        return Err("Tidak ada titik rute di file ini".into());
    }
    let name = inner(text, "name").map(|n| n.replace("<![CDATA[", "").replace("]]>", ""));
    Ok((name, first_time.map(|t| t.timestamp()), pts))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn write(a: &Activity) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"GhostWalk\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n",
    );
    s += &format!("<trk><name>{}</name><type>{}</type><trkseg>\n", esc(&a.title), a.sport.name());
    for p in &a.track {
        s += &format!("<trkpt lat=\"{:.7}\" lon=\"{:.7}\">", p.lat, p.lon);
        if let Some(e) = p.ele {
            s += &format!("<ele>{e:.1}</ele>");
        }
        if let Some(t) = p.t {
            if let Some(tm) = Utc.timestamp_opt(a.start + t as i64, 0).single() {
                s += &format!("<time>{}</time>", tm.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            }
        }
        s += "</trkpt>\n";
    }
    s += "</trkseg></trk>\n</gpx>\n";
    s
}

pub fn csv(acts: &[Activity]) -> String {
    let mut s = String::from("tanggal,olahraga,judul,jarak_km,durasi_menit,tanjakan_m,detak_rata,usaha,catatan\n");
    for a in acts {
        s += &format!(
            "{},{},\"{}\",{:.2},{:.1},{:.0},{},{},\"{}\"\n",
            a.date().format("%Y-%m-%d %H:%M"),
            a.sport.name(),
            a.title.replace('"', "'"),
            a.distance_m / 1000.0,
            a.duration_s / 60.0,
            a.elev_gain_m,
            a.avg_hr.map(|h| h.to_string()).unwrap_or_default(),
            a.effort,
            a.notes.replace('"', "'").replace('\n', " "),
        );
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        let d = crate::model::sample_data();
        let a = d.activities.iter().find(|a| a.track.len() > 1).unwrap();
        let (name, start, pts) = super::parse(&super::write(a)).unwrap();
        assert_eq!(name.as_deref(), Some(a.title.as_str()));
        assert_eq!(start, Some(a.start));
        assert_eq!(pts.len(), a.track.len());
        let (dist, _, dur) = crate::model::track_stats(&pts);
        assert!((dist - a.distance_m).abs() < 5.0);
        assert!((dur - a.duration_s).abs() < 3.0);
    }
}
