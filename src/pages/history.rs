use crate::{app::App, model::Entry};
use eframe::egui;
use std::collections::BTreeMap;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.heading("Download history");
        if !app.history.is_empty() && ui.button("Clear all").clicked() {
            app.clear_history();
        }
    });

    if app.history.is_empty() {
        ui.label("No completed downloads yet");
        return;
    }

    let mut dated_entries = BTreeMap::<i64, BTreeMap<u32, BTreeMap<u32, Vec<Entry>>>>::new();
    let mut undated_entries = Vec::new();
    for entry in &app.history {
        if let Some((year, month, day)) = date_parts(entry.completed_at) {
            dated_entries
                .entry(year)
                .or_default()
                .entry(month)
                .or_default()
                .entry(day)
                .or_default()
                .push(entry.clone());
        } else {
            undated_entries.push(entry.clone());
        }
    }

    let mut remove_id = None;
    for (year, months) in dated_entries.iter().rev() {
        egui::CollapsingHeader::new(year.to_string())
            .id_salt(("history-year", year))
            .default_open(false)
            .show(ui, |ui| {
                for (month, days) in months.iter().rev() {
                    egui::CollapsingHeader::new(month_name(*month))
                        .id_salt(("history-month", year, month))
                        .default_open(false)
                        .show(ui, |ui| {
                            for (day, entries) in days.iter().rev() {
                                egui::CollapsingHeader::new(format!("Day {day}"))
                                    .id_salt(("history-day", year, month, day))
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for entry in entries.iter().rev() {
                                            history_entry(ui, entry, &mut remove_id);
                                        }
                                    });
                            }
                        });
                }
            });
    }

    if !undated_entries.is_empty() {
        egui::CollapsingHeader::new("Undated")
            .id_salt("history-undated")
            .default_open(false)
            .show(ui, |ui| {
                for entry in undated_entries.iter().rev() {
                    history_entry(ui, entry, &mut remove_id);
                }
            });
    }

    if let Some(id) = remove_id {
        app.remove_history_entry(id);
    }
}

fn history_entry(ui: &mut egui::Ui, entry: &Entry, remove_id: &mut Option<u64>) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if !entry.title.is_empty() {
                    ui.strong(&entry.title);
                }
                ui.label(&entry.url);
                ui.weak(&entry.progress);
            });
            if ui.button("Delete").clicked() {
                *remove_id = Some(entry.id);
            }
        });
    });
}

fn date_parts(timestamp: i64) -> Option<(i64, u32, u32)> {
    if timestamp <= 0 {
        return None;
    }

    let days = timestamp.div_euclid(86_400);
    let shifted_days = days + 719_468;
    let era = shifted_days.div_euclid(146_097);
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);

    Some((year, month as u32, day as u32))
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown month",
    }
}
