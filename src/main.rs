#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod appearance;
mod gui;
mod icons;
fn main() -> iced::Result {
    gui::run()
}

mod window_chrome;
