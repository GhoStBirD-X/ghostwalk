//! GhostWalk: a private, offline activity tracker for computers and Android phones.

mod app;
mod draw;
mod gpx;
mod mapdl;
mod model;
mod moments;
mod mvt;
mod offmap;
mod pmtiles;
mod record;
mod screens;
mod summary;

#[cfg(target_os = "android")]
pub mod android;

use eframe::egui;

/// Saves the reason when the app crashes, so the next start can show it.
fn keep_crash_reports() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::create_dir_all(model::data_dir());
        let when = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = std::fs::write(model::crash_file(), format!("{when}\n{info}"));
        previous(info);
    }));
}

/// Starts the computer version. `screenshot` saves a picture after ~120 frames and exits.
pub fn run_desktop(screenshot: Option<std::path::PathBuf>) -> eframe::Result {
    if screenshot.is_none() {
        keep_crash_reports();
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GhostWalk")
            .with_app_id("ghostwalk")
            .with_inner_size([400.0, 860.0])
            .with_min_inner_size([340.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "GhostWalk",
        options,
        Box::new(move |cc| {
            let mut app = app::GhostApp::new(cc, screenshot);
            if let Ok(screen) = std::env::var("GHOSTWALK_SCREEN") {
                app.open_start_screen(&screen);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: winit::platform::android::activity::AndroidApp) {
    if let Some(dir) = android_app.internal_data_path() {
        // SAFETY: set once at start-up, before any other thread reads the environment.
        unsafe { std::env::set_var("GHOSTWALK_DATA", dir) };
    }
    keep_crash_reports();
    android::init(android_app.clone());
    let options = eframe::NativeOptions {
        android_app: Some(android_app),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "GhostWalk",
        options,
        Box::new(|cc| Ok(Box::new(app::GhostApp::new(cc, None)))),
    );
}
