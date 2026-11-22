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

Phase 3 complete: create, edit and delete notes, todos and snippets through an
editor overlay, with markdown preview, a split live preview and idle autosave.
Todo interactions, snippet highlighting and tags arrive in phase 4.

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
| `r` | reload from the database |
| `q`, `Esc`, `Ctrl+C` | quit |

Inside the editor:

| Key | Action |
|---|---|
| `Ctrl+S` | save and close |
| `Esc` | cancel (confirmation when there are unsaved changes) |
| `Tab` / `Shift+Tab` | next / previous field (`Tab` indents inside the body) |
| `Enter` | next field from title/language; newline in the body |
| `Ctrl+P` | toggle the live markdown preview |

The editor autosaves after 1.2 seconds of inactivity once the title is not
empty, so a crash or a closed terminal does not lose the draft. Cancelling an
autosaved edit restores the previous version; cancelling an autosaved new item
removes it.

Notes and todos are rendered as markdown in the preview; snippets stay
verbatim until the Kanagawa syntax theme lands in phase 4.

The sidebar hides below 56 columns and the preview below 78, so narrow
terminals stay usable.

## Storage

- SQLite database: `~/Library/Application Support/dev.andremorpa.quicknotes/quicknotes.db`
  (macOS), `~/.local/share/quicknotes/quicknotes.db` (Linux).
- Config: same application directory, or `$XDG_CONFIG_HOME/quicknotes/config.toml` on Linux.
- Overrides: `QUICKNOTES_DB`, `QUICKNOTES_CONFIG`, `QUICKNOTES_LOG` (`debug`, `trace`, …).
- Logs go to `quicknotes.log` in the data directory — never to the terminal.

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
| 4 | Todo interactions, snippet highlighting, tags |
| 5 | Incremental search and command palette |
| 6 | `qn doctor`, CLI, markdown export, polish |
| 7 | Release: release-please, CI, binaries and `install.sh` |

Font note: quicknotes uses Nerd Font glyphs but cannot change your terminal
font. Configure **Iosevka Nerd Font Propo** in your terminal; `qn doctor`
(phase 6) will verify it.
