# TheMinesweeper

![Latest release](https://img.shields.io/github/v/release/0softwaredevelopment0/TheMinesweeper)

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
- RMB places a flag; RMB on a flagged cell removes it. Flags are **blind**: they never
  reveal or defuse anything — a flagged mine looks exactly like a flagged safe cell
  (no board hint, no counter change, numbers untouched). Flagged cells are
  click-locked: LMB does nothing on them until you remove the flag.
- Stepping on a mine (LMB) **defuses** it — the cell shows the neutralized mine and
  burns one attempt (if limited). That is the only thing that moves the neighbor
  numbers down. Out of attempts, or out of time — the field is lost and is deleted
  after you leave it.
- Win: every mine is either flagged or defused, AND every safe cell is revealed —
  blanket-flagging or step-farming alone never wins.
- **Records**: tick "count as record" in the create form — a WIN on such a field
  saves a snapshot of the winning position plus time spent, attempts left and flags
  used into the Records tab (main screen). The record view is read-only: pan with
  WASD/arrows, zoom with the wheel (built for large fields); records can be deleted
  with confirmation. Losses are never recorded.
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
