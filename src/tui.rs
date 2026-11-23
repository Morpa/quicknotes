//! Terminal lifecycle and the main event loop.

use std::io::{self, Stdout};
use std::panic;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::cursor;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use crate::app::App;
use crate::event;
use crate::external;
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
    let mut area = Rect::default();

    while !app.should_quit {
        let size = terminal.size().context("reading terminal size")?;
        let current = Rect::new(0, 0, size.width, size.height);
        if current != area {
            area = current;
            app.set_area(current);
        }

        terminal
            .draw(|frame| ui::draw(frame, app))
            .context("drawing the interface")?;

        match event::next_input(TICK)? {
            Some(event::Input::Key(key)) => app.handle_key(key)?,
            Some(event::Input::Paste(text)) => app.handle_paste(&text)?,
            None => app.tick(),
        }

        if let Some(request) = app.take_external_edit() {
            let outcome = run_external_editor(terminal, &request.body, &request.suffix);
            app.finish_external_edit(&request, outcome)?;
        }
    }

    Ok(())
}

/// Gives the terminal to the user's editor and takes it back afterwards. The
/// terminal is restored even when the editor fails to start.
fn run_external_editor(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    body: &str,
    suffix: &str,
) -> Result<String> {
    restore_terminal(terminal)?;
    let outcome = external::edit_text(&external::editor_command(), body, suffix);

    enable_raw_mode().context("enabling raw mode")?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
        EnableBracketedPaste,
        cursor::Hide
    )
    .context("entering the alternate screen")?;
    terminal.clear().context("redrawing after the editor")?;
    outcome
}

fn init_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enabling raw mode")?;

    let mut stdout = io::stdout();
    if let Err(error) = execute!(
        stdout,
        EnterAlternateScreen,
        EnableBracketedPaste,
        cursor::Hide
    ) {
        let _ = disable_raw_mode();
        return Err(error).context("entering the alternate screen");
    }

    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("creating the terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("disabling raw mode")?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableBracketedPaste,
        cursor::Show
    )
    .context("leaving the alternate screen")?;
    terminal.show_cursor().context("showing the cursor")?;
    Ok(())
}

fn install_panic_hook() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableBracketedPaste,
            cursor::Show
        );
        previous(info);
    }));
}
