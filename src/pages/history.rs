use crate::app::App;
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Download history");
    if app.history.is_empty() {
        ui.label("No completed downloads yet");
    }
    for entry in app.history.iter().rev() {
        ui.horizontal(|ui| {
            ui.label("✓");
            ui.label(&entry.url);
            ui.label(&entry.progress);
        });
    }
}
