# TheMinesweeper

Portable classic Minesweeper in Rust with a [macroquad](https://macroquad.rs) GUI.
Run the exe from any folder — everything it stores lives next to it in a single SQLite
file (`minesweeper.db`), so the game is fully portable.

## Mechanics

- A field is created with custom parameters: width/height (2–500 each), per-cell mine
  chance (0.1–99%, fractional values allowed; placed independently — there is **no
  safe first click**), and optional limits: time, flags, attempts (lives; default 1,
  0/empty = unlimited).
- Classic presets prefill the form with equivalent densities: Beginner 9×9 · 12%,
  Intermediate 16×16 · 16%, Expert 30×16 · 21%.
- LMB opens a cell (number = active mines in the 8 neighbors). LMB on a revealed
  number **chords**: when the flagged neighbors match the number, all remaining
  neighbors open at once.
- RMB places a **permanent** flag — flags can never be removed. Flagging a cell that
  actually holds a mine **defuses** it instantly, for free; flagging a safe cell
  locks it forever, so flag only what you are sure about.
- Stepping on a mine (LMB) also defuses it — the cell becomes visible and harmless,
  but it burns one attempt (if limited). Out of attempts, or out of time — the field
  is lost and is deleted after you leave it.
- Win: every mine is defused (by flag or by stepping on it).
- Camera: mouse wheel zooms smoothly toward the cursor; middle mouse button / WASD
  pans; only visible cells render.
- Sounds (CC0 by Kenney): reveal click, flag, explosion, win, lose. `M` toggles mute.
- Progress autosaves after every action and every 5 minutes (so even `kill -9` loses
  almost nothing); finished fields are removed right after the "back" click.

## Build & run

Requires Rust 1.85+ (edition 2024).

```
cargo run          # run in dev mode
cargo build --release   # portable exe in target/release/
cargo test         # 23 unit tests: board logic + SQLite roundtrip
```

## Layout

```
src/
├── main.rs   — scenes (menu / create / game), HUD, camera, sounds, autosave
├── board.rs  — pure game logic (std-only, unit-tested)
├── db.rs     — SQLite persistence (rusqlite bundled)
└── ui.rs     — widgets + theme, Cyrillic-capable font (JetBrains Mono, OFL)
assets/
├── JetBrainsMono-Regular.ttf
└── sounds/   — CC0 sounds by Kenney (click, flag, explosion, win, lose)
```
