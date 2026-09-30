# TheMinesweeper — Development Plan

Classic Minesweeper written in Rust with [macroquad](https://macroquad.rs) for rendering and input.

## Goals

- Faithful classic rules: minefield, flags, question marks, first-click safety, chording.
- Simple, dependency-light GUI (macroquad only, no heavyweight framework).
- Headless game logic separated from rendering and covered by unit tests.

## Architecture

```
src/
├── main.rs      — macroquad app: window, game loop, input mapping, rendering
├── board.rs     — pure game logic (no rendering deps, fully unit-tested)
└── theme.rs     — colors, fonts, layout constants
```

Key pieces of `board.rs`:

- `Cell` — `is_mine: bool`, `state: CellState` where `CellState` is `Hidden | Revealed | Flagged | Question`.
- `Board::new(width, height, mine_count)` — empty board; mines are placed lazily on the first reveal so the first click is never a mine and never touches one.
- `Board::reveal(x, y)` — reveal a cell; flood-fill uncovers the connected zero-adjacency area.
- `Board::toggle_flag(x, y)` — cycles `Hidden → Flagged → Question → Hidden`.
- `Board::chord(x, y)` — reveal all unflagged neighbors when the flag count matches the adjacent-mine number.
- `Board::status()` — `InProgress | Won | Lost` (`Won` = all non-mine cells revealed, `Lost` = a mine was revealed).

Rendering never mutates logic directly; it translates mouse input into board calls and draws `Board` state.

## Milestones

### M1 — Core logic (headless, no graphics)
- [ ] `Board` + `Cell` types, lazy mine placement with first-click safety
- [ ] Flood-fill reveal, flag cycling, chording
- [ ] Win/lose detection
- [ ] Unit tests (`cargo test`): placement safety, flood-fill reachability, win/lose edges, chord rules

### M2 — Window, rendering, input
- [ ] macroquad window sized to the board, grid rendering with classic 1–8 number colors
- [ ] LMB reveal, RMB flag, MMB (or both-buttons) chord
- [ ] Mine counter, timer, restart button in the top bar
- [ ] Game over / victory overlay, board stays visible

### M3 — Game flow
- [ ] Difficulty presets: Beginner 9×9/10, Intermediate 16×16/40, Expert 30×16/99
- [ ] Main menu: difficulty select + custom size/mine count (with sane limits)
- [ ] Best times persisted to a local file per difficulty

### M4 — Polish
- [ ] Sounds (reveal, explosion, win) with a mute toggle
- [ ] Window icon, version in the window title
- [ ] Keyboard shortcuts: N — new game, Esc — menu

### M5 — Release readiness
- [ ] GitHub Actions CI: build + test on Windows (primary), Linux, macOS
- [ ] Tagged releases following the workspace versioning scheme (`major.minor.patch-channel.number`)

## Versioning

Single version for the whole project, workspace scheme: `version.subversion.patch-channel.number`.
Current: `0.1.0-nightly.1` (initial skeleton — essentially untested, hence Nightly).
First playable build with tested logic is expected to move to at least `alpha`.

## Verification

- `cargo test` — logic tests must pass before any commit that touches `board.rs`.
- `cargo run` — manual playtest of the current milestone's checklist.
