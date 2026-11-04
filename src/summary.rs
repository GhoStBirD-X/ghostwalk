//! A look back at a month or a year: totals, a chart, highlights, and how it compares to the one before.

use crate::app::GhostApp;
use crate::draw::{self, ACCENT};
use crate::model::{Activity, Sport, Total};
use chrono::{Datelike, Local, Months, NaiveDate, Timelike};
use eframe::egui::{RichText, Sense, Ui, vec2};

const DAYS: [&str; 7] = ["Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu", "Minggu"];
const MONTHS_SHORT: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des"];

/// First day of the period `offset` steps back from now, and the first day after it.
fn range(year: bool, offset: i32) -> (NaiveDate, NaiveDate) {
    let today = Local::now().date_naive();
    if year {
        let y = today.year() + offset;
        (NaiveDate::from_ymd_opt(y, 1, 1).unwrap(), NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap())
    } else {
        let first = today.with_day(1).unwrap();
        let from = if offset < 0 {
            first - Months::new(offset.unsigned_abs())
        } else {
            first + Months::new(offset as u32)
        };
        (from, from + Months::new(1))
    }
}

fn within(a: &Activity, (from, to): (NaiveDate, NaiveDate)) -> bool {
    let d = a.date().date_naive();
    d >= from && d < to
}

fn part_of_day(h: u32) -> &'static str {
    match h {
        4..=10 => "Pagi",
        11..=14 => "Siang",
        15..=18 => "Sore",
        _ => "Malam",
    }
}

impl GhostApp {
    pub fn summary(&mut self, ui: &mut Ui) {
        let imp = self.data.settings.imperial;
        ui.horizontal(|ui| {
            for (year, label) in [(false, "Bulanan"), (true, "Tahunan")] {
                if draw::chip(ui, self.summary_year == year, label).clicked() && self.summary_year != year {
                    self.summary_year = year;
                    self.summary_offset = 0;
                }
            }
        });
        let span = range(self.summary_year, self.summary_offset);
        let title = if self.summary_year {
            span.0.year().to_string()
        } else {
            draw::month_year(span.0.year(), span.0.month())
        };
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add(eframe::egui::Button::new(RichText::new("‹").size(22.0)).min_size(vec2(40.0, 36.0))).clicked() {
                self.summary_offset -= 1;
            }
            ui.label(RichText::new(&title).family(draw::heavy()).size(22.0).color(draw::strong(ui)));
            ui.add_enabled_ui(self.summary_offset < 0, |ui| {
                if ui.add(eframe::egui::Button::new(RichText::new("›").size(22.0)).min_size(vec2(40.0, 36.0))).clicked() {
                    self.summary_offset += 1;
                }
            });
        });
        ui.add_space(6.0);

        let acts: Vec<&Activity> = self.data.activities.iter().filter(|a| within(a, span)).collect();
        let mut prev_span = range(self.summary_year, self.summary_offset - 1);
        if self.summary_offset == 0 {
            // A period still running is compared with the same number of days of the one before.
            let days_in = (Local::now().date_naive() - span.0).num_days() + 1;
            prev_span.1 = prev_span.1.min(prev_span.0 + chrono::Duration::days(days_in));
        }
        let prev_km: f64 = self.data.activities.iter().filter(|a| within(a, prev_span)).map(|a| a.distance_m).total();
        if acts.is_empty() {
            draw::card(ui, |ui| {
                ui.label("Belum ada aktivitas di periode ini.");
            });
            return;
        }
        let dist: f64 = acts.iter().map(|a| a.distance_m).total();
        let time: f64 = acts.iter().map(|a| a.duration_s).total();
        let elev: f64 = acts.iter().map(|a| a.elev_gain_m).total();
        let steps: u64 = acts.iter().filter_map(|a| a.steps).map(u64::from).sum();

        draw::card(ui, |ui| {
            ui.horizontal(|ui| {
                draw::overline(ui, "Total");
                if prev_km > 0.0 {
                    let change = (dist - prev_km) / prev_km * 100.0;
                    let before = if self.summary_year { "tahun lalu" } else { "bulan lalu" };
                    ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                        let (text, color) = if change >= 0.0 {
                            (format!("+{change:.0}% dari {before}"), draw::lime(ui))
                        } else {
                            (format!("{change:.0}% dari {before}"), draw::muted(ui))
                        };
                        draw::pill(ui, &text, color, false);
                    });
                }
            });
            let km = draw::dist(dist, imp);
            let (num, unit) = km.split_once(' ').unwrap_or((&km, ""));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(draw::big(num, 52.0, draw::strong(ui)));
                ui.label(draw::big(unit, 22.0, draw::muted(ui)));
            });
            let mut items = vec![
                ("Waktu", draw::dur(time)),
                ("Aktivitas", acts.len().to_string()),
                ("Tanjakan", format!("{elev:.0} m")),
            ];
            if steps > 0 {
                items.push(("Langkah", steps.to_string()));
            }
            draw::stat_grid(ui, &items, 3);

            // One bar per day (month) or per month (year).
            let (slots, labels): (usize, Vec<String>) = if self.summary_year {
                (12, MONTHS_SHORT.iter().map(|m| m[..1].to_string()).collect())
            } else {
                let n = (span.1 - span.0).num_days() as usize;
                (n, (1..=n).map(|d| if d == 1 || d % 5 == 0 { d.to_string() } else { String::new() }).collect())
            };
            let mut vals = vec![0.0; slots];
            for a in &acts {
                let d = a.date().date_naive();
                let i = if self.summary_year { d.month0() as usize } else { (d - span.0).num_days() as usize };
                if let Some(v) = vals.get_mut(i) {
                    *v += if a.distance_m > 0.0 { a.distance_m / 1000.0 } else { a.duration_s / 600.0 };
                }
            }
            draw::bars(ui, &vals, &labels, ACCENT, 90.0, false);
        });

        // Highlights.
        let mut by_day = [0.0f64; 7];
        let mut by_part: Vec<(&str, u32)> = Vec::new();
        let mut by_sport: Vec<(Sport, u32)> = Vec::new();
        let mut days = std::collections::HashSet::new();
        for a in &acts {
            let t = a.date();
            by_day[t.weekday().num_days_from_monday() as usize] += a.duration_s;
            let part = part_of_day(t.hour());
            match by_part.iter_mut().find(|p| p.0 == part) {
                Some(p) => p.1 += 1,
                None => by_part.push((part, 1)),
            }
            match by_sport.iter_mut().find(|p| p.0 == a.sport) {
                Some(p) => p.1 += 1,
                None => by_sport.push((a.sport, 1)),
            }
            days.insert(t.date_naive());
        }
        let best_day = (0..7).max_by(|a, b| by_day[*a].total_cmp(&by_day[*b])).unwrap_or(0);
        by_part.sort_by_key(|p| std::cmp::Reverse(p.1));
        by_sport.sort_by_key(|p| std::cmp::Reverse(p.1));
        let longest = acts.iter().max_by(|a, b| a.distance_m.total_cmp(&b.distance_m));
        let fastest = acts
            .iter()
            .filter(|a| a.sport == Sport::Run && a.distance_m >= 3000.0)
            .max_by(|a, b| a.speed_mps().total_cmp(&b.speed_mps()));
        let total_days = (span.1.min(Local::now().date_naive().succ_opt().unwrap()) - span.0).num_days().max(1);

        ui.add_space(8.0);
        draw::card(ui, |ui| {
            draw::heading(ui, "Sorotan");
            let mut rows: Vec<(String, String)> = vec![
                ("Hari aktif".into(), format!("{} dari {} hari", days.len(), total_days)),
                ("Hari paling aktif".into(), DAYS[best_day].into()),
            ];
            if let Some((part, _)) = by_part.first() {
                rows.push(("Waktu favorit".into(), part.to_string()));
            }
            if let Some((sport, n)) = by_sport.first() {
                rows.push(("Olahraga favorit".into(), format!("{} ({n}x)", sport.name())));
            }
            if let Some(a) = longest.filter(|a| a.distance_m > 0.0) {
                rows.push(("Terjauh".into(), format!("{} · {}", a.title, draw::dist(a.distance_m, imp))));
            }
            if let Some(a) = fastest {
                rows.push(("Lari tercepat".into(), format!("{} · {}", a.title, draw::pace(a.speed_mps(), imp))));
            }
            for (label, value) in rows {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 18.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, ACCENT);
                    ui.label(RichText::new(label).size(13.5).color(draw::muted(ui)));
                    ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                        ui.label(RichText::new(value).family(draw::semi()).size(14.0).color(draw::strong(ui)));
                    });
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_whole_periods() {
        let (from, to) = range(false, -1);
        assert_eq!(from.day(), 1);
        assert_eq!(to.day(), 1);
        assert_eq!((to - Months::new(1)), from);
        let (from, to) = range(true, 0);
        assert_eq!((from.month(), from.day()), (1, 1));
        assert_eq!(to.year(), from.year() + 1);
    }
}
