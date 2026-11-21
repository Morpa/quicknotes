//! Terminal lifecycle and the main event loop.

use std::io::{self, Stdout};
use std::panic;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::cursor;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use crate::app::App;
use crate::event;
use crate::ui;

const TICK: Duration = Duration::from_millis(200);

pub fn run(mut app: App) -> Result<()> {
    install_panic_hook();

    let mut terminal = init_terminal()?;
    let outcome = run_loop(&mut terminal, &mut app);
    let restored = restore_terminal(&mut terminal);

    outcome.and(restored)
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    while !app.should_quit {
        let size = terminal.size().context("reading terminal size")?;
        app.set_area(Rect::new(0, 0, size.width, size.height));

        terminal
            .draw(|frame| ui::draw(frame, app))
            .context("drawing the interface")?;

        if let Some(action) = event::next_action(TICK)? {
            app.update(action)?;
        } else {
            app.tick();
        }
    }

    Ok(())
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enabling raw mode")?;

    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, cursor::Hide) {
        let _ = disable_raw_mode();
        return Err(error).context("entering the alternate screen");
    }

    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("creating the terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("disabling raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)
        .context("leaving the alternate screen")?;
    terminal.show_cursor().context("showing the cursor")?;
    Ok(())
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, cursor::Show);
        previous(info);
    }));
}
