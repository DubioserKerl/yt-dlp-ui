use crate::{app::App, helpers::render_overrides};
use eframe::egui;

pub(super) fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.heading("Download staging");
    ui.label("YouTube links copied to the clipboard appear here. You can also paste a link below.");
    let mut url = String::new();
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut url);
        if ui.button("Add URL").clicked() {
            app.add_url(&url);
        }
    });
    let mut download_all = false;
    ui.add_enabled_ui(!app.staging.is_empty(), |ui| {
        if ui.button("Download all").clicked() {
            download_all = true;
        }
    });
    ui.separator();

    let baseline = app.config.settings.clone();
    let mut remove_id = None;
    let mut start_ids = Vec::new();
    for entry in app.staging.iter_mut() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(if entry.error.is_empty() {
                    "◷"
                } else {
                    "⚠ Failed"
                });
                ui.label(&entry.url);
                if ui.button("Download").clicked() {
                    start_ids.push(entry.id);
                }
                if ui.button("Remove").clicked() {
                    remove_id = Some(entry.id);
                }
            });
            if !entry.error.is_empty() {
                ui.colored_label(egui::Color32::RED, &entry.error);
                if ui.button("Show error").clicked() {
                    app.notice = entry.error.clone();
                }
            }
            ui.collapsing("Override arguments for this download", |ui| {
                render_overrides(ui, &mut entry.overrides, &baseline);
                ui.label("Additional arguments");
                ui.text_edit_singleline(&mut entry.extra_args);
            });
        });
    }
    if let Some(id) = remove_id {
        if let Some(index) = app.staging.iter().position(|entry| entry.id == id) {
            app.staging.remove(index);
        }
    }
    if download_all {
        start_ids = app.staging.iter().map(|entry| entry.id).collect();
    }
    for id in start_ids {
        if let Some(index) = app.staging.iter().position(|entry| entry.id == id) {
            app.start_download(index);
        }
    }
}
