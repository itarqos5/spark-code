#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod gui;
fn main() -> iced::Result {
    gui::run()
}
