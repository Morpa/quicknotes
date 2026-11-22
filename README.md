# quicknotes

Minimal terminal notes, todos and code snippets — local-first, no accounts, no
network. One `qn` binary, one SQLite file, Kanagawa colors.

```
▎ NOTES           2 │ NOTES · 2                              │ Groceries
  TODOS           2 │                                        │ NOTE ·  #home  · now
  SNIPPETS        1 │▎   Groceries                        now│ ────────────────────────────────────
                    │    Async runtime                    now│ milk, eggs
 TAGS               │                                        │
  #async         1  │                                        │
  #docs          1  │                                        │
  #home          1  │                                        │
  #rust          2  │                                        │
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  / search  t tags  s sort  ? help  q quit                                1/2
```

<details>
<summary>More screens</summary>

Todos with priority and completed items:

```
  NOTES           2 │ TODOS · 2 open · 1 done                │ Write the README
▎ TODOS           2 │                                        │ TODO · high priority ·  #docs  · now
  SNIPPETS        1 │▎   [ ] Write the README          !! now│ ────────────────────────────────────
                    │    [ ] Review PR                    now│ (no content yet)
 TAGS               │    [x] Ship phase 4                 now│
  #async         1  │                                        │
  #docs          1  │                                        │
  #home          1  │                                        │
  #rust          2  │                                        │
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  space done  / search  t tags  s sort  ? help  q quit                    1/3
```

Snippet preview (syntax colors are not visible in plain text):

```
  NOTES           2 │ SNIPPETS · 1                           │ Entry API
  TODOS           2 │                                        │ SNIPPET · rust ·  #rust  · now
▎ SNIPPETS        1 │▎   Entry API                        now│ ────────────────────────────────────
                    │                                        │ let count = map.entry(key).or_insert
 TAGS               │                                        │ *count += 1;
  #async         1  │                                        │
  #docs          1  │                                        │
  #home          1  │                                        │
  #rust          2  │                                        │
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  y copy  / search  t tags  s sort  ? help  q quit                        1/1
```

Command palette:

```
▎ NOTES           2 │ NOTES · 2                              │ Groceries
  TODOS           2 │                                        │ NOTE ·  #home  · now
  SNIPPETS        1 │▎   Groceries                        now│ ────────────────────────────────────
                    │    Async runtime                    now│ milk, eggs
 TAGS               │                                        │
  #async         1  │  ┌ COMMANDS ──────────────────────────────────────────┐
  #docs          1  │  │ › go                                               │
  #home          1  │  │ ────────────────────────────────────────────────── │
  #rust          2  │  │  Go to notes                                  tab  │
                    │  │  Go to todos                                  tab  │
                    │  │  Go to snippets                               tab  │
                    │  │  Cycle theme (wave, dragon, lotus)              T  │
                    │  │ enter run  esc close                               │
                    │  └────────────────────────────────────────────────────┘
────────────────────────────────────────────────────────────────────────────────────────────────────
 ↑↓ move  n new  e edit  / search  t tags  s sort  ? help  q quit                                1/2
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

Prebuilt binaries and an install script arrive in phase 7. Run `qn doctor`
after installing to check the terminal, the database and the clipboard.

For the icons, set your terminal font to a Nerd Font (Iosevka Nerd Font Propo
is the reference); with any other font use `icons = "ascii"`.

## Command line

| Command | What it does |
|---|---|
| `qn` | open the interface (prints a one-screen summary when piped) |
| `qn doctor` | check config, database, search index, terminal, font hint and clipboard; exits 1 when something is broken |
| `qn setup` | write the commented `config.toml` if it does not exist |
| `qn setup --print-config` | print the default configuration to stdout |
| `qn export [DIR \| -]` | write every item as Markdown with YAML frontmatter (`DIR` defaults to `quicknotes-export`, `-` prints to stdout) |
| `qn export --kind todo --tag work` | export only one kind and/or one tag |

Exported files are named `<kind>-<id>-<title-slug>.md`. Snippets are wrapped in
a fenced code block tagged with their language; todos carry `done`, `due` and
`priority` in the frontmatter.

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

`config.toml` is written with comments on first run (or by `qn setup`):

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
used when the terminal does not answer. `T` cycles Wave → Dragon → Lotus for the
current session.

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
| 4 | Todo interactions, snippet highlighting, tags ✅ |
| 5 | Incremental search and command palette ✅ |
| 6 | `qn doctor`, `setup`, `export`, themes, snapshots, polish ✅ |
| 7 | Release: release-please, CI, binaries and `install.sh` |
