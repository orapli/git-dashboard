mod app;
mod colors;
mod ui;

use crossterm::event::{self, Event, KeyEventKind};
use std::io;
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
