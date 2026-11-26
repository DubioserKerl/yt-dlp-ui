#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod helpers;
mod model;
mod pages;

fn main() -> eframe::Result<()> {
    app::run()
}
