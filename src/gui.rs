use crate::app;
use crate::config;
use crate::i18n;

pub fn run() -> Result<(), eframe::Error> {
    // Crash visibility: GUI apps have no console, so panics vanish otherwise.
    // Append panic info to crash.log in the config directory for debugging.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let log_path = config::get_config_dir().join("crash.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            use std::io::Write;
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(f, "[{now}] panic: {info}");
            let _ = writeln!(f, "{}", std::backtrace::Backtrace::force_capture());
        }
        default_hook(info);
    }));

    // Window icon: same artwork as the installer icon, embedded as raw RGBA
    // (regenerate with scripts/generate_icon.py)
    let icon = egui::IconData {
        rgba: include_bytes!("../assets/icon-64.rgba").to_vec(),
        width: 64,
        height: 64,
    };

    let startup_lang = config::load_preferences().language;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([800.0, 550.0])
            .with_title(i18n::t(startup_lang, "app_title"))
            .with_icon(icon),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "Git Dashboard",
        options,
        Box::new(|cc| Ok(Box::new(app::GitDashboardApp::new(cc)))),
    )
}
