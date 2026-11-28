use crate::app::App;
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Download log");
    ui.horizontal(|ui| {
        ui.label("Search");
        ui.text_edit_singleline(&mut app.log_filter);
        if ui.button("Clear log").clicked() {
            app.logs.clear();
        }
    });
    egui::ScrollArea::vertical()
        .max_height(650.0)
        .show(ui, |ui| {
            for line in app.logs.iter().filter(|line| {
                app.log_filter.is_empty()
                    || line.to_lowercase().contains(&app.log_filter.to_lowercase())
            }) {
                ui.monospace(line);
            }
        });
}
