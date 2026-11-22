//! Application state and the actions that mutate it.

use std::time::{Duration, Instant};

use std::path::PathBuf;

use anyhow::Result;
use chrono::{Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Wrap};

use crate::clipboard;
use crate::config::{Config, Icons};
use crate::db::{Database, Item, ItemQuery, Kind, NewItem, SortOrder, Stats};
use crate::event::{self, Action};
use crate::external;
use crate::markdown;
use crate::ui::editor::{Editor, Field, PRIORITY_LABELS, parse_due};
use crate::ui::layout::{self, Visibility};
use crate::ui::palette::{self, Command, Palette};
use crate::ui::theme::Theme;

pub const STATUS_TTL: Duration = Duration::from_secs(4);
/// Idle time before the editor silently persists its buffers.
pub const AUTOSAVE_DELAY: Duration = Duration::from_millis(1_200);
/// Sink for text copied from an item; the system clipboard unless replaced.
pub type CopyFn = Box<dyn FnMut(&str) -> Result<()>>;

/// Deleted items kept for undo.
pub const UNDO_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Sidebar,
    List,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Edit,
    ConfirmDelete,
    ConfirmDiscard,
    /// Typing in the search bar.
    Search,
    Palette,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone)]
pub struct Status {
    pub text: String,
    pub tone: Tone,
    pub at: Instant,
}

impl Status {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingDelete {
    pub id: i64,
    pub title: String,
}

/// A request to edit an item's body outside the TUI; the terminal loop owns
/// the actual suspend, spawn and resume.
#[derive(Debug, Clone)]
pub struct ExternalEdit {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub suffix: String,
}

#[derive(Debug, Clone, Copy)]
enum Select {
    Keep,
    First,
    Id(i64),
    /// Same row index, so completing a todo keeps the cursor in place.
    Position(usize),
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub db: Database,
    pub section: Kind,
    pub items: Vec<Item>,
    pub selected: usize,
    pub list_offset: usize,
    pub list_height: usize,
    pub focus: Pane,
    pub stats: Stats,
    pub status: Option<Status>,
    pub should_quit: bool,
    pub area: Rect,
    pub visibility: Visibility,
    pub preview_scroll: usize,
    pub preview_max_scroll: usize,
    pub preview_width: u16,
    pub preview_height: usize,
    pub mode: Mode,
    pub editor: Option<Editor>,
    pub pending_delete: Option<PendingDelete>,
    pub sort: SortOrder,
    /// Full-text query applied to the lists; empty means no search.
    pub search: String,
    pub palette: Option<Palette>,
    /// First visible line of the help overlay on narrow terminals.
    pub help_scroll: usize,
    /// Where `T` saves the chosen theme; `None` keeps it per session.
    pub config_path: Option<PathBuf>,
    /// Most recently deleted items, newest last.
    pub deleted: Vec<Item>,
    external_edit: Option<ExternalEdit>,
    /// Writes text to the system clipboard; replaced in tests.
    pub clipboard: CopyFn,
}

impl App {
    pub fn new(config: Config, theme: Theme, db: Database) -> Result<Self> {
        let stats = db.stats()?;
        let section = Kind::Note;
        let items = db.list_items(&ItemQuery {
            kind: Some(section),
            ..ItemQuery::default()
        })?;

        let mut app = Self {
            config,
            theme,
            db,
            section,
            items,
            selected: 0,
            list_offset: 0,
            list_height: 1,
            focus: Pane::List,
            stats,
            status: None,
            should_quit: false,
            area: Rect::default(),
            visibility: Visibility::for_width(80),
            preview_scroll: 0,
            preview_max_scroll: 0,
            preview_width: 0,
            preview_height: 0,
            mode: Mode::Normal,
            editor: None,
            pending_delete: None,
            sort: SortOrder::default(),
            search: String::new(),
            palette: None,
            help_scroll: 0,
            config_path: None,
            deleted: Vec::new(),
            external_edit: None,
            clipboard: Box::new(clipboard::copy),
        };
        app.ensure_visible();
        Ok(app)
    }

    /// Dispatches a raw key press according to the active mode.
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Normal => {
                if let Some(action) = event::map_key(key) {
                    self.update(action)?;
                }
            }
            Mode::Edit => self.handle_editor_key(key)?,
            Mode::ConfirmDelete | Mode::ConfirmDiscard => self.handle_confirm_key(key)?,
            Mode::Search => self.handle_search_key(key)?,
            Mode::Palette => self.handle_palette_key(key)?,
            Mode::Help => match key.code {
                KeyCode::Up => self.help_scroll = self.help_scroll.saturating_sub(1),
                KeyCode::Down => {
                    self.help_scroll = (self.help_scroll + 1).min(palette::help_height());
                }
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(8),
                KeyCode::PageDown => {
                    self.help_scroll = (self.help_scroll + 8).min(palette::help_height());
                }
                _ => self.mode = Mode::Normal,
            },
        }
        Ok(())
    }

    pub fn update(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Quit => self.should_quit = true,
            Action::Back => self.back()?,
            Action::NextSection => {
                self.focus = Pane::List;
                self.change_section(self.section.next())?;
            }
            Action::PrevSection => {
                self.focus = Pane::List;
                self.change_section(self.section.prev())?;
            }
            Action::Up => self.move_up()?,
            Action::Down => self.move_down()?,
            Action::Left => self.cycle_focus(false),
            Action::Right => self.cycle_focus(true),
            Action::First => self.jump(true)?,
            Action::Last => self.jump(false)?,
            Action::PageUp => self.page(true)?,
            Action::PageDown => self.page(false)?,
            Action::Refresh => self.refresh()?,
            Action::New => self.open_new_editor(),
            Action::Edit => self.open_selected_editor(),
            Action::Delete => self.request_delete(),
            Action::ToggleDone => self.toggle_done()?,
            Action::CyclePriority => self.cycle_priority()?,
            Action::CycleSort => self.cycle_sort()?,
            Action::Copy => self.copy_selected(),
            Action::CycleTheme => self.cycle_theme(),
            Action::TogglePin => self.toggle_pin()?,
            Action::Undo => self.undo_delete()?,
            Action::ExternalEdit => self.request_external_edit(),
            Action::Search => self.mode = Mode::Search,
            Action::Palette => {
                self.palette = Some(Palette::new());
                self.mode = Mode::Palette;
            }
            Action::Help => {
                self.help_scroll = 0;
                self.mode = Mode::Help;
            }
        }
        Ok(())
    }

    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
        self.visibility = Visibility::for_width(area.width);

        let frame = layout::compute(area, self.visibility);
        self.list_height = usize::from(frame.list_rows.height).max(1);
        self.preview_width = frame.preview_body.width;
        self.preview_height = usize::from(frame.preview_body.height);

        self.normalize_focus();
        self.ensure_visible();
        self.refresh_preview_bounds();
    }

    pub fn tick(&mut self) {
        if self
            .status
            .as_ref()
            .is_some_and(|status| status.at.elapsed() >= STATUS_TTL)
        {
            self.status = None;
        }

        self.try_autosave();
    }

    pub fn set_status(&mut self, text: impl Into<String>, tone: Tone) {
        self.status = Some(Status::new(text, tone));
    }

    pub fn selected_item(&self) -> Option<&Item> {
        self.items.get(self.selected)
    }

    pub fn icon(&self, kind: Kind) -> &'static str {
        match self.config.icons {
            Icons::Nerd => match kind {
                Kind::Note => "󰈙",
                Kind::Todo => "󰄱",
                Kind::Snippet => "󰅴",
            },
            Icons::Ascii => "",
        }
    }

    pub fn section_count(&self, kind: Kind) -> u64 {
        match kind {
            Kind::Note => self.stats.notes,
            Kind::Todo => self.stats.open_todos,
            Kind::Snippet => self.stats.snippets,
        }
    }

    pub fn list_summary(&self) -> String {
        if !self.search.is_empty() {
            return format!("{} found · /{}", self.items.len(), self.search);
        }

        match self.section {
            Kind::Note => self.stats.notes.to_string(),
            Kind::Todo if self.stats.done_todos > 0 => {
                format!(
                    "{} open · {} done",
                    self.stats.open_todos, self.stats.done_todos
                )
            }
            Kind::Todo => format!("{} open", self.stats.open_todos),
            Kind::Snippet => self.stats.snippets.to_string(),
        }
    }

    pub fn checkbox(&self, item: &Item) -> &'static str {
        match item.kind {
            Kind::Todo if item.done => match self.config.icons {
                Icons::Nerd => "[✓]",
                Icons::Ascii => "[x]",
            },
            Kind::Todo => "[ ]",
            _ => "   ",
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('s') => return self.save_editor(false),
                KeyCode::Char('p') => {
                    if let Some(editor) = &mut self.editor {
                        editor.split = !editor.split;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        let mut cancel = false;
        let mut changed = false;

        if let Some(editor) = &mut self.editor {
            match key.code {
                KeyCode::Esc => cancel = true,
                KeyCode::Tab if editor.field == Field::Body => changed = editor.input(key),
                KeyCode::Tab => editor.next_field(true),
                KeyCode::BackTab => editor.next_field(false),
                KeyCode::Enter if editor.field != Field::Body => editor.next_field(true),
                _ => changed = editor.input(key),
            }

            if changed {
                editor.dirty = true;
                editor.last_edit = Instant::now();
                editor.error = None;
            }
        }

        if cancel {
            self.request_cancel();
        }
        Ok(())
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.cancel_confirm();
            return Ok(());
        }

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => match self.mode {
                Mode::ConfirmDelete => self.confirm_delete()?,
                Mode::ConfirmDiscard => self.discard_editor()?,
                Mode::Normal | Mode::Edit | Mode::Search | Mode::Palette | Mode::Help => {}
            },
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Char('q') => {
                self.cancel_confirm();
            }
            _ => {}
        }
        Ok(())
    }

    fn open_new_editor(&mut self) {
        self.editor = Some(Editor::new(self.section, None, &self.theme));
        self.mode = Mode::Edit;
    }

    fn open_selected_editor(&mut self) {
        let item = self.selected_item().cloned();
        let kind = item.as_ref().map_or(self.section, |item| item.kind);
        self.editor = Some(Editor::new(kind, item.as_ref(), &self.theme));
        self.mode = Mode::Edit;
    }

    fn request_cancel(&mut self) {
        let modified = self.editor.as_ref().is_some_and(Editor::is_modified);
        if modified {
            self.mode = Mode::ConfirmDiscard;
        } else {
            self.editor = None;
            self.mode = Mode::Normal;
        }
    }

    fn request_delete(&mut self) {
        match self.selected_item() {
            Some(item) => {
                self.pending_delete = Some(PendingDelete {
                    id: item.id,
                    title: item.title.clone(),
                });
                self.mode = Mode::ConfirmDelete;
            }
            None => self.set_status("Nothing to delete", Tone::Warning),
        }
    }

    fn confirm_delete(&mut self) -> Result<()> {
        let Some(pending) = self.pending_delete.take() else {
            self.mode = Mode::Normal;
            return Ok(());
        };

        let snapshot = self.db.get_item(pending.id)?;
        let deleted = self.db.delete_item(pending.id)?;
        if deleted && let Some(item) = snapshot {
            self.deleted.push(item);
            if self.deleted.len() > UNDO_LIMIT {
                self.deleted.remove(0);
            }
        }
        self.mode = Mode::Normal;
        self.reload(Select::Keep)?;

        if deleted {
            self.set_status(
                format!("Deleted · {} · u to undo", pending.title),
                Tone::Success,
            );
        }
        Ok(())
    }

    fn cancel_confirm(&mut self) {
        self.pending_delete = None;
        self.mode = if self.editor.is_some() {
            Mode::Edit
        } else {
            Mode::Normal
        };
    }

    fn discard_editor(&mut self) -> Result<()> {
        let Some(editor) = self.editor.take() else {
            self.mode = Mode::Normal;
            return Ok(());
        };
        self.mode = Mode::Normal;

        if editor.is_modified() {
            match &editor.baseline {
                Some(baseline) => self.db.update_item(baseline)?,
                None => {
                    if let Some(id) = editor.item_id {
                        self.db.delete_item(id)?;
                    }
                }
            }
        }

        self.reload(Select::Keep)?;
        self.set_status("Changes discarded", Tone::Normal);
        Ok(())
    }

    /// Saves the editor buffers. Autosave keeps the editor open; an explicit
    /// save closes it and selects the saved row.
    fn save_editor(&mut self, autosave: bool) -> Result<()> {
        let Some(editor) = self.editor.as_ref() else {
            return Ok(());
        };

        let title = editor.title_text().trim().to_string();
        if title.is_empty() {
            if autosave {
                return Ok(());
            }
            if let Some(editor) = &mut self.editor {
                editor.error = Some("Title cannot be empty".to_string());
            }
            return Ok(());
        }

        let body = editor.body_text();
        let language = if editor.kind == Kind::Snippet {
            editor.normalized_language()
        } else {
            None
        };
        let kind = editor.kind;
        let item_id = editor.item_id;
        let priority = editor.priority;
        let due_input = editor.due_text();

        // `None` keeps the stored due date: autosave must not drop it while
        // the field holds a half-typed value.
        let due: Option<Option<NaiveDate>> = if kind == Kind::Todo {
            match parse_due(&due_input, Local::now().date_naive()) {
                Ok(due) => Some(due),
                Err(_) if autosave => None,
                Err(message) => {
                    if let Some(editor) = &mut self.editor {
                        editor.error = Some(message);
                    }
                    return Ok(());
                }
            }
        } else {
            None
        };

        let saved = match item_id {
            Some(id) => {
                let Some(mut item) = self.db.get_item(id)? else {
                    anyhow::bail!("item {id} no longer exists");
                };
                item.title = title;
                item.body = body;
                item.language = language;
                if kind == Kind::Todo {
                    if let Some(due) = due {
                        item.due = due;
                    }
                    item.priority = priority;
                }
                self.db.update_item(&item)?;
                item
            }
            None => self.db.insert_item(&NewItem {
                kind,
                title,
                body,
                language,
                due: due.flatten(),
                priority: if kind == Kind::Todo { priority } else { 0 },
            })?,
        };

        if let Some(editor) = &mut self.editor {
            editor.item_id = Some(saved.id);
            editor.dirty = false;
            editor.error = None;
        }

        if autosave {
            return Ok(());
        }

        self.mode = Mode::Normal;
        self.editor = None;
        self.reload(Select::Id(saved.id))?;
        self.set_status(format!("Saved · {}", saved.title), Tone::Success);
        Ok(())
    }

    fn try_autosave(&mut self) {
        if self.mode != Mode::Edit {
            return;
        }

        let Some(editor) = self.editor.as_ref() else {
            return;
        };
        if !editor.dirty || editor.last_edit.elapsed() < AUTOSAVE_DELAY {
            return;
        }
        if editor.title_text().trim().is_empty() {
            return;
        }

        if let Err(error) = self.save_editor(true) {
            tracing::warn!(error = %format!("{error:#}"), "autosave failed");
            if let Some(editor) = &mut self.editor {
                editor.last_edit = Instant::now();
            }
        }
    }

    fn reload(&mut self, select: Select) -> Result<()> {
        let previous = self.selected_item().map(|item| item.id);

        self.stats = self.db.stats()?;
        self.items = self.db.list_items(&self.query())?;

        self.selected = match select {
            Select::First => 0,
            Select::Keep => previous
                .and_then(|id| self.items.iter().position(|item| item.id == id))
                .unwrap_or(self.selected),
            Select::Id(id) => self
                .items
                .iter()
                .position(|item| item.id == id)
                .unwrap_or(0),
            Select::Position(index) => index,
        };
        self.selected = self.selected.min(self.items.len().saturating_sub(1));

        self.ensure_visible();
        self.refresh_preview_bounds();
        Ok(())
    }

    fn query(&self) -> ItemQuery {
        ItemQuery {
            kind: Some(self.section),
            search: Some(self.search.clone()).filter(|query| !query.trim().is_empty()),
            sort: self.sort,
            ..ItemQuery::default()
        }
    }

    fn back(&mut self) -> Result<()> {
        if !self.search.is_empty() {
            self.search.clear();
            self.reload(Select::First)?;
            self.set_status("Search cleared", Tone::Normal);
            return Ok(());
        }
        self.should_quit = true;
        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.end_search(true)?,
                KeyCode::Char('u') => self.edit_search(|search| search.clear())?,
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.end_search(true)?,
            KeyCode::Enter => self.end_search(false)?,
            KeyCode::Backspace => self.edit_search(|search| {
                search.pop();
            })?,
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Char(character) => self.edit_search(|search| search.push(character))?,
            _ => {}
        }
        Ok(())
    }

    /// Applies an edit to the query and refreshes the results as you type.
    fn edit_search(&mut self, edit: impl FnOnce(&mut String)) -> Result<()> {
        edit(&mut self.search);
        self.list_offset = 0;
        self.preview_scroll = 0;
        self.reload(Select::First)
    }

    fn end_search(&mut self, clear: bool) -> Result<()> {
        self.mode = Mode::Normal;
        if clear && !self.search.is_empty() {
            self.search.clear();
            self.reload(Select::First)?;
        }
        Ok(())
    }

    fn handle_palette_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(palette) = &mut self.palette else {
            self.mode = Mode::Normal;
            return Ok(());
        };

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') | KeyCode::Char('k') => self.close_palette(),
                KeyCode::Char('u') => palette.clear_query(),
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => self.close_palette(),
            KeyCode::Up => palette.move_cursor(-1),
            KeyCode::Down => palette.move_cursor(1),
            KeyCode::Backspace => palette.pop(),
            KeyCode::Enter => {
                let command = palette.current();
                self.close_palette();
                if let Some(command) = command {
                    self.run_command(command)?;
                }
            }
            KeyCode::Char(character) => palette.push(character),
            _ => {}
        }
        Ok(())
    }

    fn close_palette(&mut self) {
        self.palette = None;
        self.mode = Mode::Normal;
    }

    pub fn run_command(&mut self, command: Command) -> Result<()> {
        match command {
            Command::New => self.update(Action::New),
            Command::Edit => self.update(Action::Edit),
            Command::Delete => self.update(Action::Delete),
            Command::ToggleDone => self.update(Action::ToggleDone),
            Command::CyclePriority => self.update(Action::CyclePriority),
            Command::CycleSort => self.update(Action::CycleSort),
            Command::CycleTheme => self.update(Action::CycleTheme),
            Command::TogglePin => self.update(Action::TogglePin),
            Command::Undo => self.update(Action::Undo),
            Command::ExternalEdit => self.update(Action::ExternalEdit),
            Command::Copy => self.update(Action::Copy),
            Command::Search => self.update(Action::Search),
            Command::Refresh => self.update(Action::Refresh),
            Command::Help => self.update(Action::Help),
            Command::Quit => self.update(Action::Quit),
            Command::Go(section) => {
                self.focus = Pane::List;
                self.change_section(section)
            }
            Command::ClearFilters => {
                self.search.clear();
                self.reload(Select::First)?;
                self.set_status("Search cleared", Tone::Normal);
                Ok(())
            }
        }
    }

    fn toggle_done(&mut self) -> Result<()> {
        let Some(item) = self.selected_item() else {
            self.set_status("Nothing selected", Tone::Warning);
            return Ok(());
        };
        if item.kind != Kind::Todo {
            self.set_status("Only todos can be completed", Tone::Warning);
            return Ok(());
        }

        let (id, title) = (item.id, item.title.clone());
        let done = self.db.toggle_done(id)?;
        self.reload(Select::Position(self.selected))?;

        let message = if done {
            format!("Done · {title}")
        } else {
            format!("Reopened · {title}")
        };
        self.set_status(message, Tone::Success);
        Ok(())
    }

    fn cycle_priority(&mut self) -> Result<()> {
        let Some(item) = self.selected_item() else {
            self.set_status("Nothing selected", Tone::Warning);
            return Ok(());
        };
        if item.kind != Kind::Todo {
            self.set_status("Only todos have a priority", Tone::Warning);
            return Ok(());
        }

        let (id, title) = (item.id, item.title.clone());
        let next = (item.priority + 1) % PRIORITY_LABELS.len() as u8;
        let priority = self.db.set_priority(id, next)?;
        self.reload(Select::Id(id))?;

        self.set_status(
            format!(
                "Priority · {} · {title}",
                PRIORITY_LABELS[usize::from(priority)]
            ),
            Tone::Success,
        );
        Ok(())
    }

    fn cycle_sort(&mut self) -> Result<()> {
        self.sort = self.sort.next();
        self.reload(Select::Keep)?;
        self.set_status(format!("Sort · {}", self.sort.label()), Tone::Normal);
        Ok(())
    }

    /// Switches Wave → Dragon → Lotus for this session; `config.toml` is the
    /// place to make it permanent.
    fn cycle_theme(&mut self) {
        self.config.theme = self.theme.name.next();
        self.theme = Theme::from_config(&self.config);
        self.refresh_preview_bounds();

        let name = self.theme.name;
        match &self.config_path {
            None => self.set_status(format!("Theme · {name} · this session"), Tone::Normal),
            Some(path) => match Config::save_theme(path, self.config.theme) {
                Ok(()) => self.set_status(format!("Theme · {name} · saved"), Tone::Success),
                Err(error) => {
                    tracing::warn!(error = %format!("{error:#}"), "could not save the theme");
                    self.set_status(
                        format!("Theme · {name} · could not save to config.toml"),
                        Tone::Warning,
                    );
                }
            },
        }
    }

    fn toggle_pin(&mut self) -> Result<()> {
        let Some(item) = self.selected_item() else {
            self.set_status("Nothing selected", Tone::Warning);
            return Ok(());
        };

        let (id, title, pinned) = (item.id, item.title.clone(), !item.pinned);
        self.db.set_pinned(id, pinned)?;
        self.reload(Select::Id(id))?;

        let verb = if pinned { "Pinned" } else { "Unpinned" };
        self.set_status(format!("{verb} · {title}"), Tone::Success);
        Ok(())
    }

    fn undo_delete(&mut self) -> Result<()> {
        let Some(item) = self.deleted.pop() else {
            self.set_status("Nothing to undo", Tone::Warning);
            return Ok(());
        };

        match self.db.restore_item(&item) {
            Ok(restored) => {
                // Show the item again: switch to its section if needed.
                self.change_section(restored.kind)?;
                self.reload(Select::Id(restored.id))?;
                self.set_status(format!("Restored · {}", restored.title), Tone::Success);
            }
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "undo failed");
                self.set_status("Could not restore the item", Tone::Danger);
            }
        }
        Ok(())
    }

    fn request_external_edit(&mut self) {
        let Some(item) = self.selected_item() else {
            self.set_status("Nothing selected", Tone::Warning);
            return;
        };

        self.external_edit = Some(ExternalEdit {
            id: item.id,
            title: item.title.clone(),
            body: item.body.clone(),
            suffix: external::extension(item.kind, item.language.as_deref()),
        });
    }

    /// Hands the pending external edit to the terminal loop, once.
    pub fn take_external_edit(&mut self) -> Option<ExternalEdit> {
        self.external_edit.take()
    }

    /// Applies the outcome of an external edit: the new text, or the reason
    /// the editor could not run.
    pub fn finish_external_edit(
        &mut self,
        request: &ExternalEdit,
        outcome: Result<String>,
    ) -> Result<()> {
        let text = match outcome {
            Ok(text) => text,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "external editor failed");
                self.set_status(format!("Editor failed: {error:#}"), Tone::Danger);
                return Ok(());
            }
        };

        // Editors add a final newline; do not store one the body never had.
        let body = if request.body.ends_with('\n') {
            text
        } else {
            text.trim_end_matches(['\n', '\r']).to_string()
        };
        if body == request.body {
            self.set_status("No changes", Tone::Normal);
            return Ok(());
        }

        let Some(mut item) = self.db.get_item(request.id)? else {
            self.set_status("The item was deleted meanwhile", Tone::Warning);
            return Ok(());
        };
        item.body = body;
        self.db.update_item(&item)?;
        self.reload(Select::Id(request.id))?;
        self.set_status(format!("Updated · {}", request.title), Tone::Success);
        Ok(())
    }

    fn copy_selected(&mut self) {
        let Some(item) = self.selected_item() else {
            self.set_status("Nothing to copy", Tone::Warning);
            return;
        };
        if item.body.is_empty() {
            self.set_status("Nothing to copy: the body is empty", Tone::Warning);
            return;
        }

        let text = item.body.clone();
        let title = item.title.clone();
        let lines = text.lines().count();

        match (self.clipboard)(&text) {
            Ok(()) => self.set_status(format!("Copied · {title} ({lines} lines)"), Tone::Success),
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "clipboard write failed");
                self.set_status("Clipboard unavailable", Tone::Danger);
            }
        }
    }

    fn refresh(&mut self) -> Result<()> {
        self.reload(Select::Keep)?;
        self.set_status(format!("Refreshed · {}", self.items.len()), Tone::Success);
        Ok(())
    }

    fn change_section(&mut self, section: Kind) -> Result<()> {
        if section == self.section {
            return Ok(());
        }

        self.section = section;
        self.list_offset = 0;
        self.preview_scroll = 0;
        self.reload(Select::First)
    }

    fn move_up(&mut self) -> Result<()> {
        match self.focus {
            Pane::Sidebar => self.change_section(self.section.prev()),
            Pane::List => {
                self.move_selection(-1);
                Ok(())
            }
            Pane::Preview => {
                self.scroll_preview(-1);
                Ok(())
            }
        }
    }

    fn move_down(&mut self) -> Result<()> {
        match self.focus {
            Pane::Sidebar => self.change_section(self.section.next()),
            Pane::List => {
                self.move_selection(1);
                Ok(())
            }
            Pane::Preview => {
                self.scroll_preview(1);
                Ok(())
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }

        let last = self.items.len() - 1;
        self.selected = if delta < 0 {
            self.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected + delta as usize).min(last)
        };

        self.preview_scroll = 0;
        self.ensure_visible();
        self.refresh_preview_bounds();
    }

    fn scroll_preview(&mut self, delta: isize) {
        self.preview_scroll = if delta < 0 {
            self.preview_scroll.saturating_sub(delta.unsigned_abs())
        } else {
            (self.preview_scroll + delta as usize).min(self.preview_max_scroll)
        };
    }

    fn jump(&mut self, first: bool) -> Result<()> {
        match self.focus {
            Pane::Sidebar => {
                let section = if first { Kind::Note } else { Kind::Snippet };
                self.change_section(section)
            }
            Pane::List => {
                self.selected = if first {
                    0
                } else {
                    self.items.len().saturating_sub(1)
                };
                self.preview_scroll = 0;
                self.ensure_visible();
                self.refresh_preview_bounds();
                Ok(())
            }
            Pane::Preview => {
                self.preview_scroll = if first { 0 } else { self.preview_max_scroll };
                Ok(())
            }
        }
    }

    fn page(&mut self, up: bool) -> Result<()> {
        match self.focus {
            Pane::Sidebar => Ok(()),
            Pane::List => {
                let step = self.list_height.max(1) as isize;
                self.move_selection(if up { -step } else { step });
                Ok(())
            }
            Pane::Preview => {
                self.scroll_preview(if up { -10 } else { 10 });
                Ok(())
            }
        }
    }

    fn cycle_focus(&mut self, forward: bool) {
        let panes = self.visible_panes();
        if panes.len() < 2 {
            return;
        }

        let current = panes
            .iter()
            .position(|pane| *pane == self.focus)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % panes.len()
        } else {
            (current + panes.len() - 1) % panes.len()
        };
        self.focus = panes[next];
    }

    fn visible_panes(&self) -> Vec<Pane> {
        let mut panes = Vec::new();
        if self.visibility.sidebar {
            panes.push(Pane::Sidebar);
        }
        panes.push(Pane::List);
        if self.visibility.preview {
            panes.push(Pane::Preview);
        }
        panes
    }

    fn normalize_focus(&mut self) {
        if !self.visibility.sidebar && self.focus == Pane::Sidebar {
            self.focus = Pane::List;
        }
        if !self.visibility.preview && self.focus == Pane::Preview {
            self.focus = Pane::List;
        }
    }

    fn ensure_visible(&mut self) {
        let height = self.list_height.max(1);
        if self.selected < self.list_offset {
            self.list_offset = self.selected;
        } else if self.selected >= self.list_offset + height {
            self.list_offset = self.selected + 1 - height;
        }

        let max_offset = self.items.len().saturating_sub(height);
        self.list_offset = self.list_offset.min(max_offset);
    }

    fn refresh_preview_bounds(&mut self) {
        if self.preview_width == 0 || self.preview_height == 0 {
            self.preview_max_scroll = 0;
            self.preview_scroll = 0;
            return;
        }

        let lines = match self.selected_item() {
            Some(item) => {
                let text = markdown::render(
                    &item.body,
                    item.kind,
                    item.language.as_deref(),
                    &self.theme,
                    self.preview_width,
                );
                let paragraph = if item.kind == Kind::Snippet {
                    Paragraph::new(text)
                } else {
                    Paragraph::new(text).wrap(Wrap { trim: false })
                };
                paragraph.line_count(self.preview_width)
            }
            None => 0,
        };

        self.preview_max_scroll = lines.saturating_sub(self.preview_height);
        self.preview_scroll = self.preview_scroll.min(self.preview_max_scroll);
    }
}
