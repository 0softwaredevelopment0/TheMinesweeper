# TheMinesweeper — Development Plan

Portable Minesweeper in Rust (macroquad GUI, SQLite storage next to the exe).
Current status reflects the big mechanics milestone (v0.2.0-nightly.1).

## Architecture

```
src/
├── main.rs   — scenes: menu, create-field form, game; HUD; camera; autosave loop
├── board.rs  — pure game logic, std-only (xorshift Rng), unit-tested
├── db.rs     — rusqlite (bundled): schema + CRUD, boards as packed bitmaps
└── ui.rs     — minimal widgets (button, text field), theme, TTF font loader
```

Key rules encoded in `board.rs`:

- Mines are placed independently per cell (`mine_chance`) — no safe first click.
- Revealing a mine defuses it: cell becomes safe/visible, one attempt is burned;
  attempts exhausted → `Lost(AttemptsExhausted)`.
- Numbers count still-active (not defused) mines; defusing updates them.
- Win = every mine defused or flagged (wrong flags don't block).
- Time limit hitting zero → `Lost(TimeUp)`.
- Persistence: 4 packed bitmaps (mines/revealed/flagged/defused) as BLOBs.

## Done

- [x] Core logic + 17 unit tests (chance generation, flood-fill, defuse/lives,
      flag limits, win/lose edges, serialization roundtrip, SQLite roundtrip)
- [x] Menu: field list with status badges, delete with confirmation, scroll
- [x] Create form: name, width/height, mine chance %, time/flags/attempts limits
- [x] Game scene: grid rendering, LMB reveal, RMB flag, HUD (time/flags/attempts/
      defused counter), finish overlays with reason
- [x] Portable SQLite storage next to the exe; save on every action + 5 min autosave
- [x] Camera: wheel zoom, MMB/WASD pan, viewport-culled rendering (big fields OK)

## Next

### M-A — Feel & polish
- [ ] Chording (open neighbors when flags match the number) on MMB
- [ ] Right-click flag → question-mark cycle (optional setting)
- [ ] Reveal animation, timer color states, sounds (mute toggle)
- [ ] Window icon; remember camera/zoom per field
- [ ] Long-press / hold-to-reveal safety guard

### M-B — Persistence & UX upgrades
- [ ] Best-time / stats per field definition (wins, losses, best time)
- [ ] Preset difficulties (Beginner/Intermediate/Expert) in the create form
- [ ] Export/import a field as a file; DB backup on version upgrades
- [ ] Russian/English UI toggle (strings are already centralized enough to split)

### M-C — Release readiness
- [ ] GitHub Actions CI: build + test (windows-latest primary)
- [ ] Release profile tweaks (opt-level, LTO, strip), icon + version resource
- [ ] Tagged releases per workspace versioning (`major.minor.patch-channel.number`)
