//! Application state and the actions that mutate it.

use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{Local, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::widgets::{Paragraph, Wrap};

use crate::clipboard;
use crate::config::{Config, Icons};
use crate::db::{Database, Item, ItemQuery, Kind, NewItem, SortOrder, Stats, normalize_tag};
use crate::event::{self, Action};
use crate::markdown;
use crate::ui::editor::{Editor, Field, PRIORITY_LABELS, parse_due};
use crate::ui::layout::{self, Visibility};
use crate::ui::picker::{RowKind, TagPicker, TagPurpose};
use crate::ui::theme::Theme;

pub const STATUS_TTL: Duration = Duration::from_secs(4);
/// Idle time before the editor silently persists its buffers.
pub const AUTOSAVE_DELAY: Duration = Duration::from_millis(1_200);
/// Sink for text copied from an item; the system clipboard unless replaced.
pub type CopyFn = Box<dyn FnMut(&str) -> Result<()>>;

/// Tags listed in the sidebar; the rest are reachable through the picker.
pub const SIDEBAR_TAGS: usize = 8;

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
    Tags,
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
    pub tags: Vec<(String, u64)>,
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
    pub tag_filter: Option<String>,
    /// Row highlighted in the sidebar: sections first, then tags.
    pub sidebar_row: usize,
    pub tag_picker: Option<TagPicker>,
    /// Writes text to the system clipboard; replaced in tests.
    pub clipboard: CopyFn,
}

impl App {
    pub fn new(config: Config, theme: Theme, db: Database) -> Result<Self> {
        let stats = db.stats()?;
        let tags = db.all_tags()?;
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
            tags,
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
            tag_filter: None,
            sidebar_row: 0,
            tag_picker: None,
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
            Mode::Tags => self.handle_picker_key(key)?,
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
            Action::Edit => self.edit_or_filter()?,
            Action::Delete => self.request_delete(),
            Action::ToggleDone => self.toggle_done()?,
            Action::CyclePriority => self.cycle_priority()?,
            Action::CycleSort => self.cycle_sort()?,
            Action::Copy => self.copy_selected(),
            Action::PickTags => self.open_tag_picker(false),
            Action::FilterTags => self.open_tag_picker(true),
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
        if let Some(tag) = &self.tag_filter {
            return format!("{} with #{tag}", self.items.len());
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

    /// Tags shown in the sidebar, in display order.
    pub fn sidebar_tags(&self) -> &[(String, u64)] {
        &self.tags[..self.tags.len().min(SIDEBAR_TAGS)]
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
                KeyCode::Char('t') => {
                    if let Some(editor) = &mut self.editor {
                        editor.focus(Field::Tags);
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
                Mode::Normal | Mode::Edit | Mode::Tags => {}
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

        let deleted = self.db.delete_item(pending.id)?;
        self.mode = Mode::Normal;
        self.reload(Select::Keep)?;

        if deleted {
            self.set_status(format!("Deleted · {}", pending.title), Tone::Success);
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
        let tags = editor.tags_list();
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
                item.tags = tags;
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
                tags,
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
        self.tags = self.db.all_tags()?;
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
        self.clamp_sidebar_row();

        self.ensure_visible();
        self.refresh_preview_bounds();
        Ok(())
    }

    fn query(&self) -> ItemQuery {
        ItemQuery {
            kind: Some(self.section),
            tag: self.tag_filter.clone(),
            sort: self.sort,
            ..ItemQuery::default()
        }
    }

    fn section_row(section: Kind) -> usize {
        Kind::ALL
            .iter()
            .position(|kind| *kind == section)
            .unwrap_or(0)
    }

    /// Keeps the sidebar cursor on an existing row after tags change.
    fn clamp_sidebar_row(&mut self) {
        let rows = Kind::ALL.len() + self.sidebar_tags().len();
        if self.sidebar_row >= rows {
            self.sidebar_row = Self::section_row(self.section);
        }
    }

    fn back(&mut self) -> Result<()> {
        if self.tag_filter.take().is_some() {
            self.reload(Select::First)?;
            self.set_status("Filter cleared", Tone::Normal);
            return Ok(());
        }
        self.should_quit = true;
        Ok(())
    }

    /// Enter edits the item, except on a sidebar tag where it toggles the filter.
    fn edit_or_filter(&mut self) -> Result<()> {
        if self.focus == Pane::Sidebar && self.sidebar_row >= Kind::ALL.len() {
            let tag = self
                .sidebar_tags()
                .get(self.sidebar_row - Kind::ALL.len())
                .map(|(tag, _)| tag.clone());
            if let Some(tag) = tag {
                let next = (self.tag_filter.as_deref() != Some(tag.as_str())).then_some(tag);
                return self.set_tag_filter(next);
            }
        }

        self.open_selected_editor();
        Ok(())
    }

    fn set_tag_filter(&mut self, tag: Option<String>) -> Result<()> {
        self.tag_filter = tag;
        self.list_offset = 0;
        self.preview_scroll = 0;
        self.reload(Select::First)?;

        match &self.tag_filter {
            Some(tag) => {
                let message = format!("Filter · #{tag} · {}", self.items.len());
                self.set_status(message, Tone::Success);
            }
            None => self.set_status("Filter cleared", Tone::Normal),
        }
        Ok(())
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

    fn open_tag_picker(&mut self, filter: bool) {
        let picker = if filter {
            let selected = self.tag_filter.iter().cloned().collect();
            TagPicker::new(TagPurpose::Filter, self.tags.clone(), selected)
        } else {
            let Some(item) = self.selected_item() else {
                self.set_status("Select an item to tag", Tone::Warning);
                return;
            };
            TagPicker::new(
                TagPurpose::Assign { item_id: item.id },
                self.tags.clone(),
                item.tags.clone(),
            )
        };

        self.tag_picker = Some(picker);
        self.mode = Mode::Tags;
    }

    fn close_tag_picker(&mut self) {
        self.tag_picker = None;
        self.mode = Mode::Normal;
    }

    fn handle_picker_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(picker) = &mut self.tag_picker else {
            self.mode = Mode::Normal;
            return Ok(());
        };

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.close_tag_picker(),
                KeyCode::Char('u') => picker.clear_query(),
                _ => {}
            }
            return Ok(());
        }

        let assign = matches!(picker.purpose, TagPurpose::Assign { .. });
        match key.code {
            KeyCode::Esc => self.close_tag_picker(),
            KeyCode::Up => picker.move_cursor(-1),
            KeyCode::Down => picker.move_cursor(1),
            KeyCode::PageUp => picker.move_cursor(-8),
            KeyCode::PageDown => picker.move_cursor(8),
            KeyCode::Home => picker.jump(true),
            KeyCode::End => picker.jump(false),
            KeyCode::Backspace => picker.pop(),
            KeyCode::Enter => self.activate_picker_row()?,
            KeyCode::Char(' ') if assign => self.activate_picker_row()?,
            KeyCode::Char(character) if !character.is_whitespace() => picker.push(character),
            _ => {}
        }
        Ok(())
    }

    fn activate_picker_row(&mut self) -> Result<()> {
        let Some(picker) = &self.tag_picker else {
            return Ok(());
        };
        let Some(row) = picker.current().cloned() else {
            return Ok(());
        };

        match picker.purpose {
            TagPurpose::Filter => {
                let tag = (row.kind == RowKind::Tag).then_some(row.name);
                self.close_tag_picker();
                self.set_tag_filter(tag)
            }
            TagPurpose::Assign { item_id } => self.toggle_item_tag(item_id, &row.name),
        }
    }

    /// Adds the tag to the item, or removes it when it is already there.
    fn toggle_item_tag(&mut self, item_id: i64, tag: &str) -> Result<()> {
        let tag = normalize_tag(tag);
        let Some(mut item) = self.db.get_item(item_id)? else {
            self.close_tag_picker();
            return Ok(());
        };

        let added = match item.tags.iter().position(|existing| *existing == tag) {
            Some(index) => {
                item.tags.remove(index);
                false
            }
            None => {
                item.tags.push(tag.clone());
                true
            }
        };
        self.db.update_item(&item)?;
        self.reload(Select::Id(item_id))?;

        let selected = self
            .db
            .get_item(item_id)?
            .map(|item| item.tags)
            .unwrap_or_default();
        if let Some(picker) = &mut self.tag_picker {
            picker.clear_query();
            picker.refresh(self.tags.clone(), selected);
        }

        let verb = if added { "Tagged" } else { "Untagged" };
        self.set_status(format!("{verb} · #{tag}"), Tone::Success);
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        self.reload(Select::Keep)?;
        self.set_status(format!("Refreshed · {}", self.items.len()), Tone::Success);
        Ok(())
    }

    fn change_section(&mut self, section: Kind) -> Result<()> {
        self.sidebar_row = Self::section_row(section);
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
            Pane::Sidebar => self.move_sidebar(-1),
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
            Pane::Sidebar => self.move_sidebar(1),
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

    /// Moves the sidebar cursor over sections and tags, wrapping at the ends.
    /// Sections switch immediately; tags wait for Enter.
    fn move_sidebar(&mut self, delta: isize) -> Result<()> {
        let rows = (Kind::ALL.len() + self.sidebar_tags().len()) as isize;
        let next = (self.sidebar_row as isize + delta).rem_euclid(rows) as usize;

        match Kind::ALL.get(next) {
            Some(section) => self.change_section(*section),
            None => {
                self.sidebar_row = next;
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
