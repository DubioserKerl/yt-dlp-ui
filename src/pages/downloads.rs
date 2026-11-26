use crate::{app::App, helpers::progress_fraction};
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
                ui.label(&entry.url);
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
                    egui::ProgressBar::new(progress_fraction(&entry.progress))
                        .text(&entry.progress),
                );
            }
        });
    }
    if let Some(id) = cancel_id {
        app.cancel_download(id);
    }
}
