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

Phase 2 complete: the TUI shell with a three-pane layout, section sidebar,
navigable list, preview and status line. Items are still read-only — creating
and editing arrives in phase 3.

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
| `r` | reload from the database |
| `q`, `Esc`, `Ctrl+C` | quit |

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
| 3 | Create/edit/delete with editor overlay and markdown preview |
| 4 | Todo interactions, snippet highlighting, tags |
| 5 | Incremental search and command palette |
| 6 | `qn doctor`, CLI, markdown export, polish |
| 7 | Release: release-please, CI, binaries and `install.sh` |

Font note: quicknotes uses Nerd Font glyphs but cannot change your terminal
font. Configure **Iosevka Nerd Font Propo** in your terminal; `qn doctor`
(phase 6) will verify it.
