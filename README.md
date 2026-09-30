# TheMinesweeper

Portable classic Minesweeper in Rust with a [macroquad](https://macroquad.rs) GUI.
Run the exe from any folder — everything it stores lives next to it in a single SQLite
file (`minesweeper.db`), so the game is fully portable.

## Mechanics

- A field is created with custom parameters: width/height (2–500 each), per-cell mine
  chance (1–99%, placed independently — there is **no safe first click**), and optional
  limits: time, flags, attempts (lives; default 1, 0/empty = unlimited).
- LMB opens a cell (number = active mines in the 8 neighbors), RMB toggles a flag.
- Clicking a mine **defuses** it: the cell becomes visible and harmless and burns one
  attempt (if limited). Out of attempts, or out of time — the field is lost and is
  deleted after you leave it.
- Win: every mine is either defused by a click or covered by a flag (wrong flags on
  safe cells don't block the win).
- Camera: mouse wheel zooms, middle mouse button / WASD pans; only visible cells render.
- Progress autosaves after every action and every 5 minutes (so even `kill -9` loses
  almost nothing); finished fields are removed right after the "back" click.

## Build & run

Requires Rust 1.85+ (edition 2024).

```
cargo run          # run in dev mode
cargo build --release   # portable exe in target/release/
cargo test         # 17 unit tests: board logic + SQLite roundtrip
```

## Layout

```
src/
├── main.rs   — scenes (menu / create / game), HUD, camera, autosave
├── board.rs  — pure game logic (std-only, unit-tested)
├── db.rs     — SQLite persistence (rusqlite bundled)
└── ui.rs     — widgets + theme, Cyrillic-capable font (JetBrains Mono, OFL)
```
