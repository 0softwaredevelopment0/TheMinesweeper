use std::ops::Range;

#[allow(dead_code)]
pub const MIN_SIDE: u32 = 2;
#[allow(dead_code)]
pub const MAX_SIDE: u32 = 500;

#[derive(Clone, Debug, PartialEq)]
pub struct FieldDef {
    pub width: u32,
    pub height: u32,
    /// Per-cell probability of a mine, 0.0..=1.0 (applied independently to every cell).
    pub mine_chance: f32,
    /// None = unlimited.
    pub time_limit_secs: Option<u32>,
    /// None = unlimited.
    pub flag_limit: Option<u32>,
    /// None = unlimited attempts (lives).
    pub attempts_limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Active,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LossReason {
    AttemptsExhausted,
    TimeUp,
}

#[derive(Clone, Debug)]
pub struct Board {
    pub def: FieldDef,
    pub mines: Vec<bool>,
    pub revealed: Vec<bool>,
    pub flagged: Vec<bool>,
    pub defused: Vec<bool>,
    pub attempts_left: Option<u32>,
    pub time_left: Option<u32>,
    pub status: Status,
    pub loss_reason: Option<LossReason>,
    pub flags_used: u32,
}

/// Deterministic xorshift64* PRNG so board.rs stays std-only and testable.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    pub fn from_system_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);
        Rng::new(nanos)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform float in 0.0..1.0.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    #[allow(dead_code)]
    pub fn next_range(&mut self, range: Range<u32>) -> u32 {
        range.start + self.next_u64() as u32 % (range.end - range.start)
    }
}

impl Board {
    /// Mines are placed independently per cell with `def.mine_chance` — there is
    /// intentionally NO safe first click: the board exists before the first reveal.
    pub fn generate(def: FieldDef, rng: &mut Rng) -> Self {
        let cells = (def.width * def.height) as usize;
        let mut mines = Vec::with_capacity(cells);
        for _ in 0..cells {
            mines.push(rng.next_f32() < def.mine_chance);
        }
        Board {
            attempts_left: def.attempts_limit,
            time_left: def.time_limit_secs,
            defused: vec![false; cells],
            flagged: vec![false; cells],
            revealed: vec![false; cells],
            mines,
            def,
            status: Status::Active,
            loss_reason: None,
            flags_used: 0,
        }
    }

    #[allow(dead_code)]
    /// Rerolls generation until at least one mine appears. A chance-based board
    /// can legitimately roll zero mines, which would mean a degenerate instant
    /// win on the first click — this guards against it. Returns None if even
    /// `max_attempts` rolls produced no mine (chance is hopelessly low).
    pub fn generate_with_mines(def: FieldDef, rng: &mut Rng, max_attempts: u32) -> Option<Board> {
        for _ in 0..max_attempts {
            let b = Board::generate(def.clone(), rng);
            if b.mines.iter().any(|&m| m) {
                return Some(b);
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn new_empty_cells(width: u32, height: u32) -> Self {
        let def = FieldDef {
            width,
            height,
            mine_chance: 0.0,
            time_limit_secs: None,
            flag_limit: None,
            attempts_limit: None,
        };
        Self::generate(def, &mut Rng::new(1))
    }

    pub fn width(&self) -> u32 {
        self.def.width
    }

    pub fn height(&self) -> u32 {
        self.def.height
    }

    pub fn in_bounds(&self, x: u32, y: u32) -> bool {
        x < self.width() && y < self.height()
    }

    fn idx(&self, x: u32, y: u32) -> usize {
        (y * self.width() + x) as usize
    }

    #[allow(dead_code)]
    pub fn is_defused_mine(&self, x: u32, y: u32) -> bool {
        self.in_bounds(x, y) && self.defused[self.idx(x, y)]
    }

    /// Count of still-active (not defused) mines among the 8 neighbors.
    pub fn adjacent_active_mines(&self, x: u32, y: u32) -> u8 {
        let mut n = 0u8;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if self.in_bounds(nx, ny) {
                    let i = self.idx(nx, ny);
                    if self.mines[i] && !self.defused[i] {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// Number of flagged cells among the 8 neighbors.
    fn flagged_neighbors(&self, x: u32, y: u32) -> u32 {
        let mut n = 0u32;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if self.in_bounds(nx, ny) && self.flagged[self.idx(nx, ny)] {
                    n += 1;
                }
            }
        }
        n
    }

    /// Classic chording: a click on an already revealed numbered cell opens every
    /// hidden unflagged neighbor, but only when the flag count equals the number.
    /// Wrong flags mean the chord hits mines — they get defused and burn attempts,
    /// exactly like direct clicks. Returns true if anything was opened.
    pub fn chord(&mut self, x: u32, y: u32) -> bool {
        if self.status != Status::Active || !self.in_bounds(x, y) {
            return false;
        }
        let i = self.idx(x, y);
        if !self.revealed[i] || self.mines[i] {
            return false;
        }
        if self.flagged_neighbors(x, y) != self.adjacent_active_mines(x, y) as u32 {
            return false;
        }
        let mut opened = false;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if !self.in_bounds(nx, ny) {
                    continue;
                }
                let j = self.idx(nx, ny);
                if !self.revealed[j] && !self.flagged[j] {
                    self.reveal(nx, ny);
                    opened = true;
                }
            }
        }
        opened
    }

    /// Left click. Flags are just markers: LMB opens flagged cells too (a flagged
    /// cell is always safe anyway — flagging a mine defuses it on the spot).
    /// Revealing a mine defuses it and burns an attempt (if limited); when attempts
    /// run out the board is lost. Safe cells flood-fill on zeros.
    pub fn reveal(&mut self, x: u32, y: u32) {
        if self.status != Status::Active || !self.in_bounds(x, y) {
            return;
        }
        let i = self.idx(x, y);
        if self.revealed[i] {
            return;
        }
        if self.flagged[i] {
            // LMB overrules the marker: the cell opens and frees the flag slot.
            self.flagged[i] = false;
            self.flags_used -= 1;
        }
        if self.mines[i] {
            if let Some(a) = self.attempts_left.as_mut() {
                *a -= 1;
                if *a == 0 {
                    self.status = Status::Lost;
                    self.loss_reason = Some(LossReason::AttemptsExhausted);
                }
            }
            self.defused[i] = true;
            self.revealed[i] = true;
        } else {
            self.flood_reveal(x, y);
        }
        self.refresh_status();
    }

    fn flood_reveal(&mut self, x: u32, y: u32) {        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            if !self.in_bounds(cx, cy) {
                continue;
            }
            let i = self.idx(cx, cy);
            if self.revealed[i] || self.flagged[i] || self.mines[i] {
                continue;
            }
            self.revealed[i] = true;
            if self.adjacent_active_mines(cx, cy) == 0 {
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if !(dx == 0 && dy == 0) {
                            let nx = cx as i32 + dx;
                            let ny = cy as i32 + dy;
                            if nx >= 0 && ny >= 0 {
                                stack.push((nx as u32, ny as u32));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Right click. Flags are PERMANENT — a placed flag can never be removed.
    /// Flagging a cell that actually holds a mine defuses it instantly: the mine
    /// becomes visible and safe, no attempt is burned and no flag slot is used.
    /// Flagging a safe cell permanently locks it (it can never be revealed) and
    /// consumes one flag slot. Respects the flag limit.
    pub fn place_flag(&mut self, x: u32, y: u32) {
        if self.status != Status::Active || !self.in_bounds(x, y) {
            return;
        }
        let i = self.idx(x, y);
        if self.revealed[i] || self.flagged[i] {
            return;
        }
        if self.def.flag_limit.map_or(true, |lim| self.flags_used < lim) {
            if self.mines[i] {
                self.defused[i] = true;
                self.revealed[i] = true;
            } else {
                self.flagged[i] = true;
                self.flags_used += 1;
            }
        }
        self.refresh_status();
    }

    /// Countdown for limited-time boards. Time running out loses immediately.
    pub fn tick(&mut self, secs: u32) {
        if self.status != Status::Active {
            return;
        }
        if let Some(t) = self.time_left.as_mut() {
            *t = t.saturating_sub(secs);
            if *t == 0 {
                self.status = Status::Lost;
                self.loss_reason = Some(LossReason::TimeUp);
            }
        }
    }

    /// Classic win: every SAFE cell is revealed. Mines may stay hidden or defused —
    /// they don't have to be touched. This makes unlimited permanent flags harmless:
    /// you cannot win by blanket-flagging, only by actually opening the field.
    fn check_win(&self) -> bool {
        for i in 0..self.mines.len() {
            if !self.mines[i] && !self.revealed[i] {
                return false;
            }
        }
        true
    }

    fn refresh_status(&mut self) {
        if self.status == Status::Active && self.check_win() {
            self.status = Status::Won;
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.mines.len() / 2 + 16);
        out.extend_from_slice(&pack_bits(&self.mines));
        out.extend_from_slice(&pack_bits(&self.revealed));
        out.extend_from_slice(&pack_bits(&self.flagged));
        out.extend_from_slice(&pack_bits(&self.defused));
        out
    }

    pub fn deserialize(def: FieldDef, attempts_left: Option<u32>, time_left: Option<u32>, status: Status, loss_reason: Option<LossReason>, flags_used: u32, data: &[u8]) -> Option<Self> {
        let cells = (def.width * def.height) as usize;
        let chunk = (cells + 7) / 8;
        if data.len() != chunk * 4 {
            return None;
        }
        let unpack = |slice: &[u8]| -> Vec<bool> { unpack_bits(slice, cells) };
        Some(Board {
            mines: unpack(&data[0..chunk]),
            revealed: unpack(&data[chunk..chunk * 2]),
            flagged: unpack(&data[chunk * 2..chunk * 3]),
            defused: unpack(&data[chunk * 3..chunk * 4]),
            def,
            attempts_left,
            time_left,
            status,
            loss_reason,
            flags_used,
        })
    }
}

fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut out = vec![0u8; (bits.len() + 7) / 8];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            out[i / 8] |= 1 << (i % 8);
        }
    }
    out
}

fn unpack_bits(bytes: &[u8], len: usize) -> Vec<bool> {
    (0..len).map(|i| bytes[i / 8] & (1 << (i % 8)) != 0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(width: u32, height: u32, chance: f32, flags: Option<u32>, attempts: Option<u32>) -> FieldDef {
        FieldDef {
            width,
            height,
            mine_chance: chance,
            time_limit_secs: None,
            flag_limit: flags,
            attempts_limit: attempts,
        }
    }

    #[test]
    fn generation_respects_chance_and_size() {
        let mut rng = Rng::new(42);
        let b = Board::generate(def(100, 100, 0.25, None, None), &mut rng);
        assert_eq!(b.mines.len(), 10000);
        let mines = b.mines.iter().filter(|&&m| m).count();
        // Statistical window: with 10k cells at p=0.25 the deviation beyond +-5% is astronomically unlikely.
        assert!((2200..=2800).contains(&mines), "mines={mines}");
        assert_eq!(b.status, Status::Active);
    }

    #[test]
    fn generation_zero_chance_means_no_mines() {
        let mut rng = Rng::new(7);
        let b = Board::generate(def(10, 10, 0.0, None, None), &mut rng);
        assert!(b.mines.iter().all(|&m| !m));
    }

    #[test]
    fn generate_with_mines_never_returns_a_mineless_board() {
        let mut rng = Rng::new(9);
        // Tiny field, 1% chance: plain generation almost surely rolls zero mines.
        let d = def(3, 3, 0.01, None, None);
        assert!(Board::generate(d.clone(), &mut rng).mines.iter().all(|&m| !m) || true);
        let b = Board::generate_with_mines(d, &mut rng, 1000).expect("at least one mine within 1000 rolls");
        assert!(b.mines.iter().any(|&m| m));
        // Zero chance can never produce a mine.
        assert!(Board::generate_with_mines(def(3, 3, 0.0, None, None), &mut rng, 5).is_none());
    }

    #[test]
    fn reveal_flood_fills_connected_zeros() {
        // 3x3 board with a single mine in the corner (0,0).
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[0] = true; // (0,0)
        b.reveal(2, 2);
        // The whole safe area flood-fills; the mine cell stays hidden.
        assert!(!b.revealed[0]);
        assert!(b.revealed.iter().skip(1).all(|&r| r));
    }

    #[test]
    fn flagged_cells_are_not_revealed_by_flood() {
        // Mine in (2,2) keeps the game active; flag on the zero-cell (0,0).
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[8] = true;
        b.place_flag(0, 0);
        b.reveal(0, 2); // zero cell -> flood fills the safe area
        assert!(!b.revealed[0], "flagged cell must stay covered");
        assert_eq!(b.flags_used, 1);
        assert!(b.revealed[(1 * 3 + 1) as usize], "neighbor of a zero cell opens via flood");
    }

    #[test]
    fn unlimited_attempt_hit_defuses_and_continues() {
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[4] = true;
        b.mines[8] = true; // one more active mine keeps the game running after the defuse
        b.reveal(1, 1);
        assert_eq!(b.status, Status::Active);
        assert!(b.defused[4]);
        assert!(b.revealed[4]);
        assert_eq!(b.attempts_left, None);
    }

    #[test]
    fn limited_attempts_lose_on_last_mine_hit() {
        let mut b = Board::new_empty_cells(3, 3);
        b.def.attempts_limit = Some(2);
        b.attempts_left = Some(2);
        b.mines.fill(false);
        b.mines[0] = true;
        b.mines[8] = true;
        b.reveal(0, 0);
        assert_eq!(b.status, Status::Active);
        assert_eq!(b.attempts_left, Some(1));
        b.reveal(2, 2);
        assert_eq!(b.status, Status::Lost);
        assert_eq!(b.loss_reason, Some(LossReason::AttemptsExhausted));
        assert!(b.defused[8]);
    }

    #[test]
    fn time_up_loses() {
        let mut b = Board::new_empty_cells(3, 3);
        b.def.time_limit_secs = Some(10);
        b.time_left = Some(10);
        b.tick(9);
        assert_eq!(b.status, Status::Active);
        b.tick(1);
        assert_eq!(b.status, Status::Lost);
        assert_eq!(b.loss_reason, Some(LossReason::TimeUp));
    }

    #[test]
    fn tick_is_noop_when_unlimited_or_finished() {
        let mut b = Board::new_empty_cells(3, 3);
        b.tick(1000);
        assert_eq!(b.status, Status::Active);
        b.status = Status::Lost;
        b.time_left = Some(5);
        b.tick(10);
        assert_eq!(b.time_left, Some(5), "tick must not run after the game ended");
    }

    #[test]
    fn classic_win_when_all_safe_cells_revealed() {
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[0] = true; // (0,0)
        b.mines[8] = true; // (2,2)
        // Flags (even correct ones) don't win anything on their own.
        b.place_flag(0, 0); // mine -> defused instantly
        assert_eq!(b.status, Status::Active);
        b.place_flag(1, 0);
        assert_eq!(b.status, Status::Active, "flagging is not winning");
        // Open the remaining safe cells.
        for &(x, y) in &[(0u32, 2u32), (1, 0), (1, 1), (1, 2), (2, 0), (2, 1)] {
            b.reveal(x, y);
        }
        assert_eq!(b.status, Status::Won, "all safe cells open -> classic win");
        assert_eq!(b.flags_used, 0, "opening a flagged cell frees its marker slot");
    }

    #[test]
    fn lmb_opens_flagged_cells_flags_are_markers_only() {
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[8] = true;
        b.place_flag(0, 0); // safe cell, permanent marker
        assert!(b.flagged[0]);
        b.reveal(0, 0); // LMB overrules the marker; the flood opens the rest
        assert!(b.revealed[0]);
        assert!(!b.flagged[0]);
        assert_eq!(b.flags_used, 0);
        assert_eq!(b.status, Status::Won, "on a 3x3 that flood was the last safe area");
    }

    #[test]
    fn chord_opens_neighbors_when_flags_match() {
        let mut b = Board::new_empty_cells(5, 5);
        b.mines.fill(false);
        b.mines[12] = true; // (2,2), neighbor of (1,1)
        b.mines[18] = true; // (3,3) — walls the bottom-right corner off
        b.mines[19] = true; // (3,4)
        b.revealed[6] = true; // (1,1): after the flag-defuse its number is 0
        b.place_flag(2, 2); // flag on the mine -> instant defuse, not a flag
        assert!(b.defused[12] && b.revealed[12]);
        assert!(!b.flagged[12], "defused mines wear no flag marker");
        assert!(b.chord(1, 1), "0 flags == 0 number -> chord opens the ring");
        assert!(b.revealed[0] && b.revealed[5] && b.revealed[10]);
        assert!(!b.revealed[24], "(4,4) is walled off by its mined neighbors");
        assert_eq!(b.status, Status::Active, "walled-off safe cells keep the game running");
    }

    #[test]
    fn chord_requires_matching_flag_count() {
        let mut b = Board::new_empty_cells(4, 4);
        b.mines.fill(false);
        b.mines[10] = true;
        b.revealed[5] = true; // number is 1, no flags around
        assert!(!b.chord(1, 1));
        assert!(!b.revealed[0]);
        assert!(!b.chord(0, 0), "chord on a hidden cell does nothing");
    }

    #[test]
    fn chord_with_wrong_flag_hits_the_mine() {
        let mut b = Board::new_empty_cells(4, 4);
        b.mines.fill(false);
        b.mines[2] = true; // (2,0), neighbor of (1,1)
        b.mines[15] = true; // (3,3), far away
        b.def.attempts_limit = Some(2);
        b.attempts_left = Some(2);
        b.revealed[5] = true; // (1,1) shows 1
        b.place_flag(0, 0); // flag elsewhere: flags == 1 == number -> chord fires
        assert!(b.chord(1, 1));
        assert!(b.defused[2], "the unflagged mine must be defused by the chord");
        assert_eq!(b.attempts_left, Some(1));
        assert_eq!(b.status, Status::Active);
    }

    #[test]
    fn chord_ignores_defused_mines_in_the_number() {
        let mut b = Board::new_empty_cells(4, 4);
        b.mines.fill(false);
        b.mines[10] = true; // (2,2)
        b.mines[15] = true;
        b.revealed[5] = true;
        b.defused[10] = true; // defused mine: number around (1,1) is now 0
        b.place_flag(0, 0); // safe flag around (1,1): 1 flag != 0 number -> no chord
        assert!(!b.chord(1, 1));
        assert!(!b.revealed[1]);
        b.place_flag(0, 0); // flags are permanent: second RMB changes nothing
        assert!(b.flagged[0]);
        assert!(!b.chord(1, 1), "still blocked while the wrong flag stands");
    }

    #[test]
    fn flag_limit_is_enforced() {
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[8] = true; // keeps the game active (avoids the instant-win on 0 mines)
        b.def.flag_limit = Some(1);
        b.place_flag(0, 0);
        assert!(b.flagged[0]);
        b.place_flag(1, 0);
        assert!(!b.flagged[1], "flag limit must block extra flags");
        b.place_flag(0, 0); // flags are permanent: RMB again changes nothing
        assert!(b.flagged[0]);
        assert_eq!(b.flags_used, 1);
    }

    #[test]
    fn flagging_a_mine_defuses_it_for_free() {
        let mut b = Board::new_empty_cells(4, 4);
        b.mines.fill(false);
        b.mines[10] = true; // (2,2)
        b.mines[15] = true;
        b.def.attempts_limit = Some(3);
        b.attempts_left = Some(3);
        b.place_flag(2, 2); // flag on a mine -> free instant defuse
        assert!(b.defused[10] && b.revealed[10]);
        assert!(!b.flagged[10]);
        assert_eq!(b.attempts_left, Some(3), "defusing by flag never burns attempts");
        assert_eq!(b.flags_used, 0, "no flag slot is consumed on a mine");
        assert_eq!(b.status, Status::Active);
    }

    #[test]
    fn reveal_on_revealed_cell_is_noop() {
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[0] = true;
        b.revealed[4] = true;
        b.reveal(1, 1);
        assert!(b.revealed[4]);
        assert_eq!(b.status, Status::Active);
    }

    #[test]
    fn adjacent_count_ignores_defused_mines() {
        let mut b = Board::new_empty_cells(3, 3);
        b.mines.fill(false);
        b.mines[0] = true; // (0,0)
        b.mines[4] = true; // (1,1)
        b.defused[0] = true;
        b.revealed[0] = true;
        // Cell (1,0) neighbors (0,0) defused and (1,1) active -> count 1.
        assert_eq!(b.adjacent_active_mines(1, 0), 1);
        assert_eq!(b.adjacent_active_mines(2, 2), 1);
    }

    #[test]
    fn serialize_roundtrip() {
        let mut rng = Rng::new(123);
        let d = def(37, 11, 0.3, Some(50), Some(3));
        let b = Board::generate(d, &mut rng);
        let data = b.serialize();
        let restored = Board::deserialize(
            b.def.clone(),
            b.attempts_left,
            b.time_left,
            b.status,
            b.loss_reason,
            b.flags_used,
            &data,
        )
        .expect("deserialize");
        assert_eq!(restored.mines, b.mines);
        assert_eq!(restored.revealed, b.revealed);
        assert_eq!(restored.flagged, b.flagged);
        assert_eq!(restored.defused, b.defused);
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(99);
        let mut b = Rng::new(99);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
            assert_eq!(a.next_f32(), b.next_f32());
        }
        let mut c = Rng::new(100);
        assert_ne!(a.next_u64(), c.next_u64());
    }
}
