// Windows: GUI subsystem so launching the app doesn't open a console window
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> Result<(), eframe::Error> {
    git_dashboard::gui::run()
}
