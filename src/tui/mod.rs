mod app;
mod colors;
mod ui;

use crossterm::event::{self, Event, KeyEventKind};
use std::io;
use std::process::{Command, Stdio};
use std::time::Duration;

pub use app::{App, RepoTab, Screen};

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        orig_hook(info);
    }));

    let mut terminal = ratatui::init();
    let mut app = App::new();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result?;
    Ok(())
}

fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
) -> io::Result<()> {
    while !app.should_quit {
        app.drain_messages();
        if let Some(ext) = app.take_external() {
            ratatui::restore();
            let msg = run_external(&ext);
            *terminal = ratatui::init();
            match msg {
                Ok(s) => app.status = s,
                Err(e) => app.error = Some(e),
            }
            continue;
        }
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
    }
    Ok(())
}

fn run_external(ext: &app::ExternalDiff) -> Result<String, String> {
    if let Some(git_args) = &ext.pipe_git_diff {
        let mut git = Command::new("git")
            .args(git_args)
            .current_dir(&ext.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("git {}: {e}", git_args.join(" ")))?;
        let stdout = git.stdout.take().ok_or_else(|| "git stdout".to_string())?;
        let status = Command::new(&ext.program)
            .args(&ext.args)
            .current_dir(&ext.cwd)
            .stdin(stdout)
            .status()
            .map_err(|e| format!("{}: {e}", ext.program))?;
        let _ = git.wait();
        if status.success() {
            Ok(format!("{} ok", ext.program))
        } else {
            Err(format!("{} exited {status}", ext.program))
        }
    } else {
        let status = Command::new(&ext.program)
            .args(&ext.args)
            .current_dir(&ext.cwd)
            .status()
            .map_err(|e| format!("{}: {e}", ext.program))?;
        if status.success() {
            Ok(format!("{} ok", ext.program))
        } else {
            Err(format!("{} exited {status}", ext.program))
        }
    }
}
