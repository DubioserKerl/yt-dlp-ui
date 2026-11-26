use crate::app::App;
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Downloads");
    if app.downloads.is_empty() {
        ui.label("No active or queued downloads");
    }
    let mut cancel_id = None;
    for entry in &app.downloads {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if !entry.title.is_empty() {
                        ui.strong(&entry.title);
                    }
                    ui.label(&entry.url);
                });
                ui.strong(&entry.status);
                if entry.status != "Queued"
                    && entry.status != "Cancelling…"
                    && ui.button("Cancel").clicked()
                {
                    cancel_id = Some(entry.id);
                }
            });
            if entry.status == "Queued" {
                ui.label("Waiting for the previous download to finish");
            } else {
                ui.add(
                    egui::ProgressBar::new(entry.download_progress)
                        .text(format!("{:.0}% overall", entry.download_progress * 100.0)),
                );
                if !entry.progress.contains('%') && !entry.progress.is_empty() {
                    ui.weak(&entry.progress);
                }
            }
        });
    }
    if let Some(id) = cancel_id {
        app.cancel_download(id);
    }
}
