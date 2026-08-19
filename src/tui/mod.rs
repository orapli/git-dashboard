mod app;
mod colors;
mod ui;

use crossterm::event::{self, DisableMouseCapture, Event, KeyEventKind};
use crossterm::terminal::{self, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use std::io::{self, Write};
use std::path::PathBuf;
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

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.should_quit {
        app.drain_messages();
        if let Some(ext) = app.take_external() {
            leave_tui();
            let msg = run_external(&ext);
            *terminal = ratatui::init();
            drain_pending_keys();
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

fn leave_tui() {
    let _ = terminal::disable_raw_mode();
    let mut out = io::stdout();
    let _ = execute!(
        out,
        LeaveAlternateScreen,
        DisableMouseCapture,
        cursor::Show
    );
    let _ = out.flush();
}

fn drain_pending_keys() {
    while event::poll(Duration::from_millis(0)).unwrap_or(false) {
        let _ = event::read();
    }
}

fn run_external(ext: &app::ExternalDiff) -> Result<String, String> {
    let program = resolve_program(&ext.program)?;
    let cmdline = format!(
        "{} {}  (cwd {})",
        program.display(),
        ext.args.join(" "),
        ext.cwd.display()
    );
    eprintln!("\n--- git-dashboard-tui: {cmdline}\n");
    let _ = io::stderr().flush();

    let status = if let Some(git_args) = &ext.pipe_git_diff {
        let mut git = Command::new("git")
            .args(git_args)
            .current_dir(&ext.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("git {}: {e}", git_args.join(" ")))?;
        let stdout = git.stdout.take().ok_or_else(|| "git stdout".to_string())?;
        let status = Command::new(&program)
            .args(&ext.args)
            .current_dir(&ext.cwd)
            .stdin(stdout)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("{cmdline}: {e}"))?;
        let _ = git.wait();
        status
    } else {
        Command::new(&program)
            .args(&ext.args)
            .current_dir(&ext.cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("{cmdline}: {e}"))?
    };

    if status.success() {
        Ok(cmdline)
    } else {
        eprintln!("\n{cmdline}\nexited {status}");
        eprint!("Enter で git-dashboard-tui に戻る / press Enter to return: ");
        let _ = io::stderr().flush();
        let _ = io::stdin().read_line(&mut String::new());
        Err(format!("{cmdline}  →  {status}"))
    }
}

fn resolve_program(name: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(name);
    if p.is_absolute() && p.is_file() {
        return Ok(p);
    }
    if name.contains('/') {
        return Ok(p);
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(name);
            if cand.is_file() {
                return Ok(cand);
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let extras = [
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/opt/homebrew/opt/hunk/bin"),
    ];
    for dir in extras {
        let cand = dir.join(name);
        if cand.is_file() {
            return Ok(cand);
        }
    }
    if let Some(home) = home {
        for dir in [
            home.join(".hunk"),
            home.join(".hunk/bin"),
            home.join(".local/bin"),
        ] {
            let cand = dir.join(name);
            if cand.is_file() {
                return Ok(cand);
            }
        }
    }
    Err(format!(
        "command not found: {name} (PATH に hunk がありません。which hunk を確認)"
    ))
}
