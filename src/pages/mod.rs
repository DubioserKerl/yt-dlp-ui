mod downloads;
mod history;
mod options;
mod staging;

use crate::{app::App, model::Page};
use eframe::egui;

pub(crate) fn show(app: &mut App, page: &Page, ui: &mut egui::Ui) {
    match page {
        Page::Staging => staging::show(app, ui),
        Page::Downloads => downloads::show(app, ui),
        Page::History => history::show(app, ui),
        Page::Options => options::show(app, ui),
    }
}
