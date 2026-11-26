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
    ui.separator();

    let baseline = app.config.settings.clone();
    let mut remove_index = None;
    let mut start_indexes = Vec::new();
    for (index, entry) in app.staging.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(if entry.error.is_empty() {
                    "◷"
                } else {
                    "⚠ Failed"
                });
                ui.label(&entry.url);
                if ui.button("Download").clicked() {
                    start_indexes.push(index);
                }
                if ui.button("Remove").clicked() {
                    remove_index = Some(index);
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
    if let Some(index) = remove_index {
        app.staging.remove(index);
    }
    for index in start_indexes.into_iter().rev() {
        app.start_download(index);
    }
}
