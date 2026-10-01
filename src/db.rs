use crate::board::{Board, FieldDef, LossReason, Status};
use rusqlite::{params, Connection, OptionalExtension};

#[allow(dead_code)]
pub struct FieldRow {
    pub id: i64,
    pub name: String,
    pub def: FieldDef,
    pub status: Status,
    pub attempts_left: Option<u32>,
    pub time_left: Option<u32>,
    pub flags_used: u32,
    pub is_record: bool,
}

#[allow(dead_code)]
pub struct RecordRow {
    pub id: i64,
    pub name: String,
    pub def: FieldDef,
    /// Seconds spent until the win (for limited fields: limit - remaining).
    pub time_played: u32,
    pub attempts_left: Option<u32>,
    pub flags_used: u32,
}

/// Portable layout: the database lives next to the executable.
pub fn db_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    exe.parent().unwrap_or(&exe).join("minesweeper.db")
}

pub fn open(path: &std::path::Path) -> Connection {
    let conn = Connection::open(path).expect("open minesweeper.db");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS fields (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            name           TEXT    NOT NULL,
            width          INTEGER NOT NULL,
            height         INTEGER NOT NULL,
            mine_chance    REAL    NOT NULL,
            time_limit     INTEGER NOT NULL,
            flag_limit     INTEGER NOT NULL,
            attempts_limit INTEGER NOT NULL,
            status         TEXT    NOT NULL DEFAULT 'active',
            attempts_left  INTEGER NOT NULL,
            time_left      INTEGER NOT NULL,
            flags_used     INTEGER NOT NULL DEFAULT 0,
            loss_reason    INTEGER NOT NULL DEFAULT 0,
            is_record      INTEGER NOT NULL DEFAULT 0,
            mines          BLOB    NOT NULL,
            revealed       BLOB    NOT NULL,
            flagged        BLOB    NOT NULL,
            defused        BLOB    NOT NULL,
            created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
         );
         CREATE TABLE IF NOT EXISTS records (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            name           TEXT    NOT NULL,
            width          INTEGER NOT NULL,
            height         INTEGER NOT NULL,
            mine_chance    REAL    NOT NULL,
            time_limit     INTEGER NOT NULL,
            flag_limit     INTEGER NOT NULL,
            attempts_limit INTEGER NOT NULL,
            time_played    INTEGER NOT NULL,
            attempts_left  INTEGER NOT NULL,
            flags_used     INTEGER NOT NULL DEFAULT 0,
            mines          BLOB    NOT NULL,
            revealed       BLOB    NOT NULL,
            flagged        BLOB    NOT NULL,
            defused        BLOB    NOT NULL,
            created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
         );",
    )
    .expect("create schema");
    // Migration for databases created before is_record existed.
    let _ = conn.execute_batch("ALTER TABLE fields ADD COLUMN is_record INTEGER NOT NULL DEFAULT 0;");
    conn
}

pub fn list_fields(conn: &Connection) -> Vec<FieldRow> {
    let mut stmt = conn
        .prepare("SELECT id, name, width, height, mine_chance, time_limit, flag_limit, attempts_limit, status, attempts_left, time_left, flags_used, is_record FROM fields ORDER BY id DESC")
        .expect("list query");
    let rows = stmt
        .query_map([], |r| {
            Ok(FieldRow {
                id: r.get(0)?,
                name: r.get(1)?,
                def: FieldDef {
                    width: r.get::<_, i64>(2)? as u32,
                    height: r.get::<_, i64>(3)? as u32,
                    mine_chance: r.get(4)?,
                    time_limit_secs: nonzero(r.get::<_, i64>(5)?),
                    flag_limit: nonzero(r.get::<_, i64>(6)?),
                    attempts_limit: nonzero(r.get::<_, i64>(7)?),
                },
                status: parse_status(&r.get::<_, String>(8)?),
                attempts_left: opt_nonneg(r.get::<_, i64>(9)?),
                time_left: opt_nonneg(r.get::<_, i64>(10)?),
                flags_used: r.get::<_, i64>(11)? as u32,
                is_record: r.get::<_, i64>(12)? != 0,
            })
        })
        .expect("list map");
    rows.filter_map(Result::ok).collect()
}

pub fn field_name(conn: &Connection, id: i64) -> Option<String> {
    conn.query_row("SELECT name FROM fields WHERE id = ?1", params![id], |r| r.get(0))
        .optional()
        .expect("field name query")
}

pub fn insert_board(conn: &Connection, name: &str, board: &Board, is_record: bool) -> i64 {
    let data = board.serialize();
    conn.execute(
        "INSERT INTO fields (name, width, height, mine_chance, time_limit, flag_limit, attempts_limit, status, attempts_left, time_left, flags_used, is_record, mines, revealed, flagged, defused)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'active', ?8, ?9, 0, ?10, ?11, ?12, ?13, ?14)",
        params![
            name,
            board.def.width as i64,
            board.def.height as i64,
            board.def.mine_chance,
            board.def.time_limit_secs.unwrap_or(0) as i64,
            board.def.flag_limit.unwrap_or(0) as i64,
            board.def.attempts_limit.unwrap_or(0) as i64,
            board.attempts_left.unwrap_or(0) as i64,
            board.time_left.unwrap_or(0) as i64,
            is_record as i64,
            &data[0..data.len() / 4],
            &data[data.len() / 4..data.len() / 2],
            &data[data.len() / 2..data.len() * 3 / 4],
            &data[data.len() * 3 / 4..],
        ],
    )
    .expect("insert board");
    conn.last_insert_rowid()
}

pub fn save_board(conn: &Connection, id: i64, board: &Board) {
    let data = board.serialize();
    let chunk = data.len() / 4;
    conn.execute(
        "UPDATE fields SET status = ?2, attempts_left = ?3, time_left = ?4, flags_used = ?5, loss_reason = ?6,
            mines = ?7, revealed = ?8, flagged = ?9, defused = ?10 WHERE id = ?1",
        params![
            id,
            status_str(board.status),
            board.attempts_left.unwrap_or(0) as i64,
            board.time_left.unwrap_or(0) as i64,
            board.flags_used as i64,
            loss_code(board.loss_reason),
            &data[0..chunk],
            &data[chunk..chunk * 2],
            &data[chunk * 2..chunk * 3],
            &data[chunk * 3..],
        ],
    )
    .expect("save board");
}

pub fn load_board(conn: &Connection, id: i64) -> Option<Board> {
    conn.query_row(
        "SELECT name, width, height, mine_chance, time_limit, flag_limit, attempts_limit, status, attempts_left, time_left, flags_used, loss_reason, mines, revealed, flagged, defused FROM fields WHERE id = ?1",
        params![id],
        |r| {
            let def = FieldDef {
                width: r.get::<_, i64>(1)? as u32,
                height: r.get::<_, i64>(2)? as u32,
                mine_chance: r.get(3)?,
                time_limit_secs: nonzero(r.get::<_, i64>(4)?),
                flag_limit: nonzero(r.get::<_, i64>(5)?),
                attempts_limit: nonzero(r.get::<_, i64>(6)?),
            };
            let status = parse_status(&r.get::<_, String>(7)?);
            let loss_reason = parse_loss(r.get::<_, i64>(11)?);
            let mut data = Vec::new();
            data.extend_from_slice(&r.get::<_, Vec<u8>>(12)?);
            data.extend_from_slice(&r.get::<_, Vec<u8>>(13)?);
            data.extend_from_slice(&r.get::<_, Vec<u8>>(14)?);
            data.extend_from_slice(&r.get::<_, Vec<u8>>(15)?);
            Ok(Board::deserialize(
                def,
                opt_nonneg(r.get::<_, i64>(8)?),
                opt_nonneg(r.get::<_, i64>(9)?),
                status,
                loss_reason,
                r.get::<_, i64>(10)? as u32,
                &data,
            ))
        },
    )
    .optional()
    .expect("load board")
    .flatten()
}

/// Removes the field completely (used for explicit deletion and for leaving a finished board).
pub fn delete_field(conn: &Connection, id: i64) {
    conn.execute("DELETE FROM fields WHERE id = ?1", params![id])
        .expect("delete field");
}

/// Saves the winning snapshot of a record-flagged field.
pub fn insert_record(conn: &Connection, name: &str, board: &Board, time_played: u32) -> i64 {
    let data = board.serialize();
    conn.execute(
        "INSERT INTO records (name, width, height, mine_chance, time_limit, flag_limit, attempts_limit, time_played, attempts_left, flags_used, mines, revealed, flagged, defused)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            name,
            board.def.width as i64,
            board.def.height as i64,
            board.def.mine_chance,
            board.def.time_limit_secs.unwrap_or(0) as i64,
            board.def.flag_limit.unwrap_or(0) as i64,
            board.def.attempts_limit.unwrap_or(0) as i64,
            time_played as i64,
            board.attempts_left.unwrap_or(0) as i64,
            board.flags_used as i64,
            &data[0..data.len() / 4],
            &data[data.len() / 4..data.len() / 2],
            &data[data.len() / 2..data.len() * 3 / 4],
            &data[data.len() * 3 / 4..],
        ],
    )
    .expect("insert record");
    conn.last_insert_rowid()
}

pub fn list_records(conn: &Connection) -> Vec<RecordRow> {
    let mut stmt = conn
        .prepare("SELECT id, name, width, height, mine_chance, time_limit, flag_limit, attempts_limit, time_played, attempts_left, flags_used FROM records ORDER BY time_played ASC, id DESC")
        .expect("records list query");
    let rows = stmt
        .query_map([], |r| {
            Ok(RecordRow {
                id: r.get(0)?,
                name: r.get(1)?,
                def: FieldDef {
                    width: r.get::<_, i64>(2)? as u32,
                    height: r.get::<_, i64>(3)? as u32,
                    mine_chance: r.get(4)?,
                    time_limit_secs: nonzero(r.get::<_, i64>(5)?),
                    flag_limit: nonzero(r.get::<_, i64>(6)?),
                    attempts_limit: nonzero(r.get::<_, i64>(7)?),
                },
                time_played: r.get::<_, i64>(8)? as u32,
                attempts_left: opt_nonneg(r.get::<_, i64>(9)?),
                flags_used: r.get::<_, i64>(10)? as u32,
            })
        })
        .expect("records list map");
    rows.filter_map(Result::ok).collect()
}

/// Loads the winning board snapshot for the record viewer.
pub fn load_record_board(conn: &Connection, id: i64) -> Option<Board> {
    conn.query_row(
        "SELECT width, height, mine_chance, time_limit, flag_limit, attempts_limit, flags_used, mines, revealed, flagged, defused FROM records WHERE id = ?1",
        params![id],
        |r| {
            let def = FieldDef {
                width: r.get::<_, i64>(0)? as u32,
                height: r.get::<_, i64>(1)? as u32,
                mine_chance: r.get(2)?,
                time_limit_secs: nonzero(r.get::<_, i64>(3)?),
                flag_limit: nonzero(r.get::<_, i64>(4)?),
                attempts_limit: nonzero(r.get::<_, i64>(5)?),
            };
            let mut data = Vec::new();
            data.extend_from_slice(&r.get::<_, Vec<u8>>(7)?);
            data.extend_from_slice(&r.get::<_, Vec<u8>>(8)?);
            data.extend_from_slice(&r.get::<_, Vec<u8>>(9)?);
            data.extend_from_slice(&r.get::<_, Vec<u8>>(10)?);
            Ok(Board::deserialize(
                def,
                None,
                None,
                Status::Won,
                None,
                r.get::<_, i64>(6)? as u32,
                &data,
            ))
        },
    )
    .optional()
    .expect("load record board")
    .flatten()
}

pub fn delete_record(conn: &Connection, id: i64) {
    conn.execute("DELETE FROM records WHERE id = ?1", params![id])
        .expect("delete record");
}

fn status_str(s: Status) -> &'static str {
    match s {
        Status::Active => "active",
        Status::Won => "won",
        Status::Lost => "lost",
    }
}

fn parse_status(s: &str) -> Status {
    match s {
        "won" => Status::Won,
        "lost" => Status::Lost,
        _ => Status::Active,
    }
}

fn loss_code(r: Option<LossReason>) -> i64 {
    match r {
        None => 0,
        Some(LossReason::AttemptsExhausted) => 1,
        Some(LossReason::TimeUp) => 2,
    }
}

fn parse_loss(code: i64) -> Option<LossReason> {
    match code {
        1 => Some(LossReason::AttemptsExhausted),
        2 => Some(LossReason::TimeUp),
        _ => None,
    }
}

fn nonzero(v: i64) -> Option<u32> {
    u32::try_from(v).ok().filter(|&n| n > 0)
}

fn opt_nonneg(v: i64) -> Option<u32> {
    u32::try_from(v).ok().filter(|&n| n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Rng;

    fn temp_db() -> (Connection, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("minesweeper_test_{}.db", Rng::from_system_time().next_u64()));
        (open(&path), path)
    }

    #[test]
    fn record_roundtrip() {
        let (conn, path) = temp_db();
        let mut rng = Rng::new(11);
        let def = FieldDef {
            width: 8,
            height: 8,
            mine_chance: 0.15,
            time_limit_secs: Some(120),
            flag_limit: Some(10),
            attempts_limit: Some(2),
        };
        let b = Board::generate(def.clone(), &mut rng);
        let id = insert_record(&conn, "my record", &b, 77);
        let recs = list_records(&conn);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].name, "my record");
        assert_eq!(recs[0].def, def);
        assert_eq!(recs[0].time_played, 77);
        let snap = load_record_board(&conn, id).expect("snapshot");
        assert_eq!(snap.mines, b.mines);
        assert_eq!(snap.revealed, b.revealed);
        assert_eq!(snap.flagged, b.flagged);
        assert_eq!(snap.defused, b.defused);
        delete_record(&conn, id);
        assert!(list_records(&conn).is_empty());
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn is_record_persists() {
        let (conn, path) = temp_db();
        let mut rng = Rng::new(12);
        let def = FieldDef {
            width: 5,
            height: 5,
            mine_chance: 0.1,
            time_limit_secs: None,
            flag_limit: None,
            attempts_limit: None,
        };
        let b = Board::generate(def, &mut rng);
        let id = insert_board(&conn, "rec field", &b, true);
        let rows = list_fields(&conn);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_record);
        assert_eq!(field_name(&conn, id).as_deref(), Some("rec field"));
        drop(conn);
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn insert_load_save_roundtrip() {
        let (conn, path) = temp_db();
        let mut rng = Rng::new(5);
        let def = FieldDef {
            width: 9,
            height: 7,
            mine_chance: 0.2,
            time_limit_secs: Some(300),
            flag_limit: Some(15),
            attempts_limit: Some(2),
        };
        let b = Board::generate(def, &mut rng);
        let id = insert_board(&conn, "тест", &b, false);
        let loaded = load_board(&conn, id).expect("loaded");
        assert_eq!(loaded.def, b.def);
        assert_eq!(loaded.mines, b.mines);
        assert_eq!(loaded.status, Status::Active);

        let mut played = loaded;
        played.reveal(4, 3);
        save_board(&conn, id, &played);
        let again = load_board(&conn, id).expect("reloaded");
        assert_eq!(again.revealed, played.revealed);

        delete_field(&conn, id);
        assert!(load_board(&conn, id).is_none());
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn unlimited_zero_means_none() {
        let (conn, path) = temp_db();
        let mut rng = Rng::new(6);
        let def = FieldDef {
            width: 5,
            height: 5,
            mine_chance: 0.1,
            time_limit_secs: None,
            flag_limit: None,
            attempts_limit: None,
        };
        let b = Board::generate(def, &mut rng);
        let id = insert_board(&conn, "x", &b, false);
        let loaded = load_board(&conn, id).expect("loaded");
        assert_eq!(loaded.def.time_limit_secs, None);
        assert_eq!(loaded.def.flag_limit, None);
        assert_eq!(loaded.def.attempts_limit, None);
        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
