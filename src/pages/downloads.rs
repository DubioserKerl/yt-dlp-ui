use crate::{app::App, helpers::progress_fraction};
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Active downloads");
    if app.active.is_empty() {
        ui.label("No active downloads");
    }
    for entry in &app.active {
        ui.group(|ui| {
            ui.label(&entry.url);
            ui.label(&entry.status);
            ui.add(
                egui::ProgressBar::new(progress_fraction(&entry.progress)).text(&entry.progress),
            );
        });
    }
}
