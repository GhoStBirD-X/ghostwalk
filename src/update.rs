//! Looking for a newer GhostWalk release on GitHub, only when the person switched it on.
//! Nothing about the person is sent: just a request for the latest release, and the APK itself.

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/GhoStBirD-X/ghostwalk/releases/latest";
pub const RELEASES: &str = "https://github.com/GhoStBirD-X/ghostwalk/releases/latest";
const APK: &str = "GhostWalk.apk";

/// A release newer than this app.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    apk_url: String,
    sums_url: Option<String>,
    pub size: u64,
}

pub enum Status {
    Checking,
    UpToDate,
    Available(Release),
    Downloading { release: Release, done: u64 },
    /// The APK is on the device and matched SHA256SUMS: (release, file, its SHA-256).
    Ready(Release, PathBuf, String),
    Failed(String),
}

pub struct Task {
    pub status: Arc<Mutex<Status>>,
}

impl Task {
    pub fn busy(&self) -> bool {
        matches!(*self.status.lock().unwrap(), Status::Checking | Status::Downloading { .. })
    }
}

type Ctx = eframe::egui::Context;

fn spawn(ctx: &Ctx, start: Status, job: impl FnOnce(&Mutex<Status>, &Ctx) -> Status + Send + 'static) -> Task {
    let status = Arc::new(Mutex::new(start));
    let (s, ctx) = (status.clone(), ctx.clone());
    std::thread::spawn(move || {
        let end = job(&s, &ctx);
        *s.lock().unwrap() = end;
        ctx.request_repaint();
    });
    Task { status }
}

fn agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(300)))
            .user_agent(concat!("GhostWalk/", env!("CARGO_PKG_VERSION"), " (update check)"))
            .build(),
    )
}

fn net(e: ureq::Error) -> io::Error {
    io::Error::other(format!("tidak bisa terhubung: {e}"))
}

/// (major, minor, patch) from "v1.2.3" or "1.2.3".
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim().trim_start_matches(['v', 'V']).split('.').map(|p| p.parse::<u32>().ok());
    let v = (it.next()??, it.next()??, it.next().unwrap_or(Some(0))?);
    it.next().is_none().then_some(v)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse_version(candidate), parse_version(current)), (Some(a), Some(b)) if a > b)
}

/// The SHA-256 listed for `name` in a SHA256SUMS file ("<hex>  <name>" lines).
pub fn sum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.trim().split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hash.to_ascii_lowercase())
    })
}

fn latest(agent: &ureq::Agent) -> io::Result<Option<Release>> {
    let raw = agent
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(net)?
        .body_mut()
        .with_config()
        .limit(4 << 20)
        .read_to_vec()
        .map_err(net)?;
    let json: serde_json::Value = serde_json::from_slice(&raw).map_err(io::Error::other)?;
    let version = json["tag_name"].as_str().unwrap_or_default().trim_start_matches('v').to_string();
    if !is_newer(&version, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let assets = json["assets"].as_array().cloned().unwrap_or_default();
    let url_of = |name: &str| {
        assets
            .iter()
            .find(|a| a["name"].as_str() == Some(name))
            .and_then(|a| Some((a["browser_download_url"].as_str()?.to_string(), a["size"].as_u64().unwrap_or(0))))
    };
    let (apk_url, size) = url_of(APK).ok_or_else(|| io::Error::other("rilis terbaru belum punya file APK"))?;
    Ok(Some(Release {
        version,
        notes: json["body"].as_str().unwrap_or_default().trim().to_string(),
        apk_url,
        sums_url: url_of("SHA256SUMS").map(|u| u.0),
        size,
    }))
}

/// Asks GitHub for the latest release.
pub fn check(ctx: &Ctx) -> Task {
    spawn(ctx, Status::Checking, |_, _| match latest(&agent()) {
        Ok(Some(r)) => Status::Available(r),
        Ok(None) => Status::UpToDate,
        Err(e) => Status::Failed(e.to_string()),
    })
}

fn dir() -> PathBuf {
    crate::model::data_dir().join("update")
}

/// Downloads the APK of `release` and its SHA256SUMS entry. The phone checks the file against
/// that sum, its package name and its signature before it is installed.
pub fn download(ctx: &Ctx, release: Release) -> Task {
    let start = Status::Downloading { release: release.clone(), done: 0 };
    spawn(ctx, start, move |status, ctx| {
        let r = (|| -> io::Result<(PathBuf, String)> {
            let agent = agent();
            let sums_url = release
                .sums_url
                .as_deref()
                .ok_or_else(|| io::Error::other("rilis ini tidak punya SHA256SUMS, jadi tidak bisa diperiksa"))?;
            let sums = agent.get(sums_url).call().map_err(net)?.body_mut().with_config().limit(64 << 10).read_to_string().map_err(net)?;
            let sum = sum_for(&sums, APK).ok_or_else(|| io::Error::other("SHA256SUMS tidak memuat GhostWalk.apk"))?;
            std::fs::create_dir_all(dir())?;
            let path = dir().join(APK);
            let tmp = path.with_extension("part");
            let mut resp = agent.get(&release.apk_url).call().map_err(net)?;
            let mut body = resp.body_mut().with_config().limit(200 << 20).reader();
            let mut out = std::fs::File::create(&tmp)?;
            let mut buf = vec![0u8; 1 << 16];
            let mut done = 0u64;
            loop {
                let n = io::Read::read(&mut body, &mut buf)?;
                if n == 0 {
                    break;
                }
                io::Write::write_all(&mut out, &buf[..n])?;
                done += n as u64;
                *status.lock().unwrap() = Status::Downloading { release: release.clone(), done };
                ctx.request_repaint();
            }
            drop(out);
            if release.size > 0 && done != release.size {
                return Err(io::Error::other("unduhan terputus, coba lagi"));
            }
            std::fs::rename(&tmp, &path)?;
            Ok((path, sum))
        })();
        match r {
            Ok((path, sum)) => Status::Ready(release, path, sum),
            Err(e) => Status::Failed(e.to_string()),
        }
    })
}

/// Removes a downloaded APK once it is no longer needed.
pub fn clean() {
    let _ = std::fs::remove_dir_all(dir());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("v0.5.0", "0.4.0"));
        assert!(is_newer("1.0", "0.9.0"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.9", "0.4.0"));
        assert!(!is_newer("nightly", "0.4.0"));
        assert_eq!(parse_version("1.2.3.4"), None);
    }

    #[test]
    fn sums_are_found_by_file_name() {
        let h = "a".repeat(64);
        let sums = format!("{}  other.txt\n{}  GhostWalk.apk\n", "b".repeat(64), h.to_uppercase());
        assert_eq!(sum_for(&sums, "GhostWalk.apk"), Some(h.clone()));
        assert_eq!(sum_for(&format!("{h} *GhostWalk.apk"), "GhostWalk.apk"), Some(h));
        assert_eq!(sum_for("abc  GhostWalk.apk", "GhostWalk.apk"), None, "not a SHA-256");
        assert_eq!(sum_for(&sums, "missing.apk"), None);
    }
}
