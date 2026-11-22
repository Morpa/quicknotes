# quicknotes

Minimal terminal notes, todos and code snippets — local-first, no accounts, no
network. One `qn` binary, one SQLite file, Kanagawa colors.

```
▎ NOTES           2 │ NOTES · 2                              │ Groceries
  TODOS           2 │                                        │ NOTE · now
  SNIPPETS        1 │▎   Groceries                        now│ ────────────────────────────────────
                    │    Async runtime                    now│ milk, eggs
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  / search  s sort  ? help  q quit                                        1/2
```

<details>
<summary>More screens</summary>

Todos with priority and completed items:

```
  NOTES           2 │ TODOS · 2 open · 1 done                │ Write the README
▎ TODOS           2 │                                        │ TODO · high priority · now
  SNIPPETS        1 │▎   [ ] Write the README          !! now│ ────────────────────────────────────
                    │    [ ] Review PR                    now│ (no content yet)
                    │    [x] Ship phase 4                 now│
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  space done  / search  s sort  ? help  q quit                            1/3
```

Snippet preview (syntax colors are not visible in plain text):

```
  NOTES           2 │ SNIPPETS · 1                           │ Entry API
  TODOS           2 │                                        │ SNIPPET · rust · now
▎ SNIPPETS        1 │▎   Entry API                        now│ ────────────────────────────────────
                    │                                        │ let count = map.entry(key).or_insert
                    │                                        │ *count += 1;
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  y copy  / search  s sort  ? help  q quit                                1/1
```

Command palette:

```
▎ NOTES           2 │ NOTES · 2                              │ Groceries
  TODOS           2 │                                        │ NOTE · now
  SNIPPETS        1 │▎   Groceries                        now│ ────────────────────────────────────
                    │    Async runtime                    now│ milk, eggs
                    │  ┌ COMMANDS ──────────────────────────────────────────┐
                    │  │ › go                                               │
                    │  │ ────────────────────────────────────────────────── │
                    │  │  Go to notes                                  tab  │
                    │  │  Go to todos                                  tab  │
                    │  │  Go to snippets                               tab  │
                    │  │  Cycle theme (wave, dragon, lotus)              T  │
                    │  │ enter run  esc close                               │
                    │  └────────────────────────────────────────────────────┘
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  / search  s sort  ? help  q quit                                        1/2
```

</details>

These screens are rendered by the test suite (`tests/snapshots.rs`, ratatui's
`TestBackend`) with ASCII icons, so they are exact but carry no colors. They are
regenerated with `UPDATE_SNAPSHOTS=1 cargo test --test snapshots`.

## Install

Requires Rust 1.88 or newer.

```sh
git clone <repository-url> quicknotes && cd quicknotes
cargo install --path . --locked      # installs `qn` into ~/.cargo/bin
```

or build in place:

```sh
cargo build --release
./target/release/qn
```

Prebuilt binaries and an install script arrive in phase 7. `qn` takes no
arguments: it opens the interface (or prints a one-screen summary when piped).

For the icons, set your terminal font to a Nerd Font (Iosevka Nerd Font Propo
is the reference); with any other font use `icons = "ascii"`.

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
| `*` | pin / unpin the selected item (pinned items sort first) |
| `u` | undo the last delete (up to 20, newest first) |
| `y` | copy the selected item's body to the clipboard |
| `E` | edit the body in `$VISUAL` / `$EDITOR` (falls back to `vi`) |
| `T` | cycle the theme Wave → Dragon → Lotus and save it to `config.toml` |
| `/` | search titles and bodies as you type (`Enter` keeps it, `Esc` clears it) |
| `Ctrl+K`, `:` | command palette (fuzzy matching) |
| `?` | keyboard help (two columns on wide terminals, scrollable on narrow ones) |
| `r` | reload from the database |
| `Esc` | clear the search, then quit |
| `q`, `Ctrl+C` | quit |

`E` suspends the interface, opens a temporary file (`.md`, or the snippet's
language extension so your editor picks a syntax) and applies the result when
the editor exits. An editor that returns immediately, such as `code` without
`--wait`, needs its blocking flag: `EDITOR="code --wait"`. Arguments are split
on spaces; shell quoting is not supported. Nothing changes if the editor fails
or the text is the same.

Inside the editor:

| Key | Action |
|---|---|
| `Ctrl+S` | save and close |
| `Esc` | cancel (confirmation when there are unsaved changes) |
| `Tab` / `Shift+Tab` | next / previous field (`Tab` indents inside the body) |
| `Enter` | next field from title/language; newline in the body |
| `Ctrl+P` | toggle the live markdown preview |

Todos also have a due date (`2026-12-31`, `today`, `tomorrow`, `+3d`, `+2w` or
empty) and a priority (`←` `→`).

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

Notes, todos and snippets share one table, and full-text search uses FTS5 with
prefix matching.

## Configuration

`config.toml` is written with comments on first run:

```toml
theme       = "kanagawa-wave"   # kanagawa-wave | kanagawa-dragon | kanagawa-lotus | auto
background  = "terminal"        # terminal | sumi
accent      = "crystalBlue"     # crystalBlue | springBlue | carpYellow | sakuraPink
                                # waveAqua1 | springGreen | surimiOrange
icons       = "nerd"            # nerd | ascii
color_mode  = "truecolor"       # truecolor | 256
# db_path   = "~/notes/quicknotes.db"
```

`theme = "auto"` asks the terminal for its background color (OSC 11, then
`COLORFGBG`) and picks Wave for dark terminals or Lotus for light ones; Wave is
used when the terminal does not answer. `T` cycles Wave → Dragon → Lotus and
rewrites only the `theme` line of `config.toml`, keeping your comments; it
replaces `auto` with the variant you land on.

`background = "terminal"` uses `Color::Reset`, so quicknotes blends with your
terminal's own Kanagawa colors. `color_mode = "256"` quantizes the palette for
terminals without truecolor.

## Development

```sh
cargo test                                 # unit, integration and snapshot tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

## Roadmap

| Phase | Scope |
|---|---|
| 1 | Config, SQLite + FTS5, Kanagawa theme ✅ |
| 2 | TUI shell: event loop, three-pane layout, navigation ✅ |
| 3 | Create/edit/delete, editor overlay, markdown preview, autosave ✅ |
| 4 | Todo interactions, snippet highlighting ✅ |
| 5 | Incremental search and command palette ✅ |
| 6 | Auto theme, UI snapshots, polish ✅ |
| – | Pin, undo delete, `$EDITOR`, saved theme ✅ |
| 7 | Release: release-please, CI, binaries and `install.sh` |
