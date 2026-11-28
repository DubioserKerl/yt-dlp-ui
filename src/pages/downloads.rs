use crate::{app::App, helpers::progress_fraction};
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Downloads");
    ui.horizontal(|ui| {
        if ui
            .button(if app.queue_paused {
                "Resume queue"
            } else {
                "Pause after active download"
            })
            .clicked()
        {
            app.toggle_queue_pause();
        }
        if ui.button("Open log viewer").clicked() {
            app.open_logs();
        }
    });
    if app.downloads.is_empty() {
        ui.label("No active or queued downloads");
    }
    let mut cancel_id = None;
    let mut move_request = None;
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
                if entry.status == "Queued" {
                    if ui
                        .add_enabled(app.can_move_download(entry.id, -1), egui::Button::new("↑"))
                        .clicked()
                    {
                        move_request = Some((entry.id, -1));
                    }
                    if ui
                        .add_enabled(app.can_move_download(entry.id, 1), egui::Button::new("↓"))
                        .clicked()
                    {
                        move_request = Some((entry.id, 1));
                    }
                }
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
                ui.horizontal(|ui| {
                    if !entry.speed.is_empty() {
                        ui.label(format!("Speed: {}", entry.speed));
                    }
                    if !entry.eta.is_empty() {
                        ui.label(format!("ETA: {}", entry.eta));
                    }
                });
            }
        });
    }
    if let Some(id) = cancel_id {
        app.cancel_download(id);
    }
    if let Some((id, offset)) = move_request {
        app.move_download(id, offset);
    }
}
