# quicknotes

Minimal terminal notes, todos and code snippets — local-first, no accounts, no network.

```
quicknotes 0.1.0
  config    ~/Library/Application Support/dev.andremorpa.quicknotes/config.toml
  database  ~/Library/Application Support/dev.andremorpa.quicknotes/quicknotes.db
  theme     Kanagawa Wave · terminal · crystalBlue
  items     0 notes · 0 open todos · 0 done · 0 snippets
```

## Status

Phase 5 complete: incremental full-text search, a fuzzy command palette and a
help overlay on top of phase 4's todos, highlighting and tags.

Phase 4 added: todos can be completed, prioritized and given a due date;
snippets are syntax-highlighted with the active Kanagawa palette and copied to
the clipboard; items carry tags, with a picker, chips and a sidebar filter.


## Build and run

```sh
cargo run          # opens the TUI (or prints a summary when piped)
cargo test
```

The binary is named `qn`:

```sh
cargo build --release
./target/release/qn
```

## Keys

| Key | Action |
|---|---|
| `↑` `↓` | move in the focused pane (scroll the preview) |
| `←` `→` | cycle focus between visible panes |
| `Tab` / `Shift+Tab` | next / previous section |
| `Home` / `End` | first / last item |
| `PageUp` / `PageDown` | jump a page |
| `Enter`, `e` | edit the selected item |
| `n` | new item in the active section |
| `Delete` | delete the selected item (asks for confirmation) |
| `Space`, `x` | complete / reopen the selected todo |
| `p` | cycle the todo priority (none → medium → high) |
| `s` | cycle the sort order (smart, updated, created, title, due, priority) |
| `y` | copy the selected item's body to the clipboard |
| `t` | tag picker for the selected item |
| `#` | filter the lists by a tag |
| `/` | search titles and bodies as you type (`Enter` keeps it, `Esc` clears it) |
| `Ctrl+K`, `:` | command palette (fuzzy matching) |
| `?` | keyboard help |
| `r` | reload from the database |
| `Esc` | clear the search, then the tag filter, then quit |
| `q`, `Ctrl+C` | quit |

In the sidebar, `↑` `↓` walk the sections and then the tags; `Enter` on a tag
turns its filter on or off. In the tag picker, type to search, `Space` toggles
a tag on the item (a new name offers to create it) and `Esc` closes it.

Inside the editor:

| Key | Action |
|---|---|
| `Ctrl+S` | save and close |
| `Esc` | cancel (confirmation when there are unsaved changes) |
| `Tab` / `Shift+Tab` | next / previous field (`Tab` indents inside the body) |
| `Enter` | next field from title/language; newline in the body |
| `Ctrl+P` | toggle the live markdown preview |
| `Ctrl+T` | jump to the tags field |

Todos also have a due date (`2026-12-31`, `today`, `tomorrow`, `+3d`, `+2w` or
empty) and a priority (`←` `→`). Tags are comma- or space-separated.

The editor autosaves after 1.2 seconds of inactivity once the title is not
empty, so a crash or a closed terminal does not lose the draft. Cancelling an
autosaved edit restores the previous version; cancelling an autosaved new item
removes it.

Notes and todos are rendered as markdown in the preview; snippets are
highlighted by language (syntect's default grammars, so TypeScript and TOML
fall back to plain text; `ts`/`tsx`/`js` use the JavaScript grammar).

The sidebar hides below 56 columns and the preview below 78, so narrow
terminals stay usable.

## Storage

- SQLite database: `~/Library/Application Support/dev.andremorpa.quicknotes/quicknotes.db`
  (macOS), `~/.local/share/quicknotes/quicknotes.db` (Linux).
- Config: same application directory, or `$XDG_CONFIG_HOME/quicknotes/config.toml` on Linux.
- Overrides: `QUICKNOTES_DB`, `QUICKNOTES_CONFIG`, `QUICKNOTES_LOG` (`debug`, `trace`, …).
- Logs go to `quicknotes.log` in the data directory — never to the terminal.

Search, section and tag filters combine: with a tag active, `/` only looks
inside items that carry it, in the current section.

Notes, todos and snippets share one table. Tags are normalized (lowercased,
leading `#` dropped) and full-text search uses FTS5 with prefix matching.

## Configuration

`config.toml` is written with comments on first run:

```toml
theme       = "kanagawa-wave"   # kanagawa-wave | kanagawa-dragon | kanagawa-lotus
background  = "terminal"        # terminal | sumi
accent      = "crystalBlue"     # crystalBlue | springBlue | carpYellow | sakuraPink
                                # waveAqua1 | springGreen | surimiOrange
icons       = "nerd"            # nerd | ascii
color_mode  = "truecolor"       # truecolor | 256
# db_path   = "~/notes/quicknotes.db"
```

`background = "terminal"` uses `Color::Reset`, so quicknotes blends with your
terminal's own Kanagawa colors. `color_mode = "256"` quantizes the palette for
terminals without truecolor.

## Roadmap

| Phase | Scope |
|---|---|
| 1 | Config, SQLite + FTS5, Kanagawa theme ✅ |
| 2 | TUI shell: event loop, three-pane layout, navigation ✅ |
| 3 | Create/edit/delete, editor overlay, markdown preview, autosave ✅ |
| 4 | Todo interactions, snippet highlighting, tags ✅ |
| 5 | Incremental search and command palette ✅ |
| 6 | `qn doctor`, CLI, markdown export, polish |
| 7 | Release: release-please, CI, binaries and `install.sh` |

Font note: quicknotes uses Nerd Font glyphs but cannot change your terminal
font. Configure **Iosevka Nerd Font Propo** in your terminal; `qn doctor`
(phase 6) will verify it.
