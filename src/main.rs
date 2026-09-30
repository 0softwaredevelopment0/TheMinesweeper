mod board;
mod db;
mod ui;

use board::{Board, FieldDef, LossReason, Rng, Status};
use db::FieldRow;
use macroquad::prelude::*;
use ui::{draw_rectangle_lines_ex, draw_rectangle_rec, TextField};

const HUD_H: f32 = 56.0;
const CELL_MIN: f32 = 10.0;
const CELL_MAX: f32 = 56.0;
const AUTOSAVE_SECS: f32 = 300.0;

enum Scene {
    Menu,
    Create,
    Game,
}

struct GameView {
    id: i64,
    board: Board,
    cell: f32,
    cam: Vec2,
    sec_accum: f32,
    autosave_accum: f32,
    session_elapsed: u32,
    pan_grab: Option<Vec2>,
}

struct CreateForm {
    name: TextField,
    width: TextField,
    height: TextField,
    chance: TextField,
    time: TextField,
    flags: TextField,
    attempts: TextField,
    active: usize,
    error: Option<String>,
}

impl CreateForm {
    fn new() -> Self {
        CreateForm {
            name: ui::TextField::new(false, 32),
            width: ui::TextField::with("16", true, 3),
            height: ui::TextField::with("16", true, 3),
            chance: ui::TextField::with("10", true, 2),
            time: ui::TextField::new(true, 7),
            flags: ui::TextField::new(true, 5),
            attempts: ui::TextField::with("1", true, 4),
            active: 0,
            error: None,
        }
    }

    fn fields_mut(&mut self) -> [&mut TextField; 7] {
        [
            &mut self.name,
            &mut self.width,
            &mut self.height,
            &mut self.chance,
            &mut self.time,
            &mut self.flags,
            &mut self.attempts,
        ]
    }

    fn build_def(&self) -> Result<FieldDef, String> {
        let width = self.width.parse_u32().ok_or("Width: enter a number from 2 to 500")?;
        let height = self.height.parse_u32().ok_or("Height: enter a number from 2 to 500")?;
        if !(2..=500).contains(&width) || !(2..=500).contains(&height) {
            return Err("Field size: 2 to 500 per side".into());
        }
        let chance_pct = self.chance.parse_u32().ok_or("Mine chance: enter a percentage from 1 to 99")?;
        if !(1..=99).contains(&chance_pct) {
            return Err("Mine chance: 1 to 99%".into());
        }
        let time = if self.time.value.trim().is_empty() {
            None
        } else {
            let t = self.time.parse_u32().ok_or("Time: enter seconds or leave empty")?;
            if t == 0 || t > 2_592_000 {
                return Err("Time: 1 second to 30 days".into());
            }
            Some(t)
        };
        let flags = if self.flags.value.trim().is_empty() {
            None
        } else {
            let f = self.flags.parse_u32().ok_or("Flags: enter a number or leave empty")?;
            Some(f.min(width * height))
        };
        let attempts = if self.attempts.value.trim().is_empty() {
            None
        } else {
            let a = self.attempts.parse_u32().ok_or("Attempts: enter a number or leave empty")?;
            if a > 9999 {
                return Err("Attempts: at most 9999".into());
            }
            (a > 0).then_some(a)
        };
        Ok(FieldDef {
            width,
            height,
            mine_chance: chance_pct as f32 / 100.0,
            time_limit_secs: time,
            flag_limit: flags,
            attempts_limit: attempts,
        })
    }
}

fn clamp_cam(cam: Vec2, board: &Board, cell: f32) -> Vec2 {
    let sw = screen_width();
    let sh = screen_height();
    let bw = board.width() as f32 * cell;
    let bh = board.height() as f32 * cell;
    let margin = 120.0;
    let cx = if bw + margin * 2.0 <= sw {
        bw / 2.0
    } else {
        cam.x.clamp(sw / 2.0 - margin, bw + margin - sw / 2.0)
    };
    let cy = if bh + margin * 2.0 + HUD_H <= sh {
        bh / 2.0
    } else {
        cam.y.clamp(HUD_H + sh / 2.0 - margin, bh + margin - sh / 2.0)
    };
    vec2(cx, cy)
}

fn origin(cam: Vec2) -> Vec2 {
    vec2(screen_width() / 2.0 - cam.x, screen_height() / 2.0 - cam.y)
}

fn fmt_params_line(row: &FieldRow) -> String {
    let d = &row.def;
    let time = d.time_limit_secs.map_or_else(|| "∞".to_string(), ui::fmt_time);
    let flags = d.flag_limit.map_or_else(|| "∞".to_string(), |f| f.to_string());
    let attempts = d.attempts_limit.map_or_else(|| "∞".to_string(), |a| a.to_string());
    format!(
        "{}x{} · mines {}% · time {} · flags {} · attempts {}",
        d.width,
        d.height,
        (d.mine_chance * 100.0).round() as u32,
        time,
        flags,
        attempts
    )
}

fn window_conf() -> Conf {
    Conf {
        window_title: format!("TheMinesweeper {}", env!("CARGO_PKG_VERSION")),
        window_width: 1280,
        window_height: 800,
        window_resizable: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    ui::init_font();
    let conn = db::open(&db::db_path());

    let mut scene = Scene::Menu;
    let mut rows: Vec<FieldRow> = db::list_fields(&conn);
    let mut form = CreateForm::new();
    let mut game: Option<GameView> = None;
    let mut menu_scroll: f32 = 0.0;
    let mut confirm_delete: Option<i64> = None;

    loop {
        clear_background(ui::COL_BG);
        match scene {
            Scene::Menu => {
                if let Some(id) = confirm_delete {
                    let name = rows.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_default();
                    let sw = screen_width();
                    let sh = screen_height();
                    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
                    let p = Rect::new(sw / 2.0 - 250.0, sh / 2.0 - 100.0, 500.0, 200.0);
                    ui::panel(p, ui::COL_PANEL);
                    ui::txt("Delete field?", vec2(p.x + 24.0, p.y + 44.0), 22.0, ui::COL_TEXT);
                    ui::txt(
                        &format!("\"{name}\" — its progress will be lost permanently"),
                        vec2(p.x + 24.0, p.y + 72.0),
                        14.0,
                        ui::COL_TEXT_DIM,
                    );
                    if ui::danger_button(Rect::new(p.x + 24.0, p.y + 116.0, 150.0, 46.0), "Delete", 18.0, true) {
                        db::delete_field(&conn, id);
                        rows = db::list_fields(&conn);
                        confirm_delete = None;
                    }
                    if ui::button(Rect::new(p.x + p.w - 174.0, p.y + 116.0, 150.0, 46.0), "Cancel", 18.0, true) {
                        confirm_delete = None;
                    }
                } else {
                    ui::txt("MINESWEEPER", vec2(28.0, 54.0), 40.0, ui::COL_TEXT);
                    ui::txt(
                        "portable · fields are stored next to the exe",
                        vec2(30.0, 80.0),
                        14.0,
                        ui::COL_TEXT_DIM,
                    );
                    if ui::button(Rect::new(screen_width() - 268.0, 30.0, 240.0, 48.0), "+ New field", 18.0, true) {
                        form = CreateForm::new();
                        scene = Scene::Create;
                    }

                    let list_w = 900.0f32.min(screen_width() - 40.0);
                    let list_x = (screen_width() - list_w) / 2.0;
                    let top = 110.0;
                    let row_h = 92.0;
                    let gap = 10.0;

                    let visible = screen_height() - top - 20.0;
                    let total_h = rows.len() as f32 * (row_h + gap);
                    let max_scroll = (total_h - visible).max(0.0);
                    menu_scroll = (menu_scroll - mouse_wheel().1 * 44.0).clamp(0.0, max_scroll);

                    if rows.is_empty() {
                        ui::txt_centered(
                            "No fields yet — create your first one",
                            vec2(screen_width() / 2.0, screen_height() / 2.0),
                            20.0,
                            ui::COL_TEXT_DIM,
                        );
                    }

                    let mut open_id: Option<i64> = None;
                    let mut broken_id: Option<i64> = None;
                    for (i, row) in rows.iter().enumerate() {
                        let y = top + i as f32 * (row_h + gap) - menu_scroll;
                        if y + row_h < top || y > screen_height() {
                            continue;
                        }
                        let r = Rect::new(list_x, y, list_w, row_h);
                        ui::panel(r, ui::COL_PANEL);
                        ui::txt(&row.name, vec2(r.x + 18.0, r.y + 32.0), 20.0, ui::COL_TEXT);
                        ui::txt(&fmt_params_line(row), vec2(r.x + 18.0, r.y + 58.0), 14.0, ui::COL_TEXT_DIM);

                        let (status_label, status_color) = match row.status {
                            Status::Active => ("in progress", ui::COL_OK),
                            Status::Won => ("cleared", ui::COL_GOLD),
                            Status::Lost => ("failed", ui::COL_DANGER),
                        };
                        ui::txt(status_label, vec2(r.x + r.w - 268.0, r.y + 30.0), 14.0, status_color);

                        let open_label = if row.status == Status::Active { "Play" } else { "Open" };
                        if ui::button(Rect::new(r.x + r.w - 250.0, r.y + 18.0, 118.0, 42.0), open_label, 16.0, true) {
                            if db::load_board(&conn, row.id).is_some() {
                                open_id = Some(row.id);
                            } else {
                                broken_id = Some(row.id);
                            }
                        }
                        if ui::danger_button(Rect::new(r.x + r.w - 120.0, r.y + 18.0, 100.0, 42.0), "Delete", 16.0, true) {
                            confirm_delete = Some(row.id);
                        }
                    }
                    if let Some(id) = broken_id {
                        db::delete_field(&conn, id);
                        rows = db::list_fields(&conn);
                    }
                    if let Some(id) = open_id {
                        if let Some(b) = db::load_board(&conn, id) {
                            let fit = ((screen_width() - 120.0) / b.width() as f32)
                                .min((screen_height() - HUD_H - 120.0) / b.height() as f32);
                            let cell = fit.clamp(CELL_MIN, CELL_MAX);
                            let cam = clamp_cam(
                                vec2(b.width() as f32 * cell / 2.0, b.height() as f32 * cell / 2.0),
                                &b,
                                cell,
                            );
                            game = Some(GameView {
                                id,
                                board: b,
                                cell,
                                cam,
                                sec_accum: 0.0,
                                autosave_accum: 0.0,
                                session_elapsed: 0,
                                pan_grab: None,
                            });
                            scene = Scene::Game;
                        }
                    }
                }
            }
            Scene::Create => {
                let sw = screen_width();
                let sh = screen_height();
                let pw = 640.0f32.min(sw - 40.0);
                let ph = 760.0f32.min(sh - 40.0);
                let p = Rect::new((sw - pw) / 2.0, (sh - ph) / 2.0, pw, ph);
                ui::panel(p, ui::COL_PANEL);
                ui::txt("New field", vec2(p.x + 28.0, p.y + 46.0), 28.0, ui::COL_TEXT);
                ui::txt(
                    "mines are placed at random — every cell rolls its own chance",
                    vec2(p.x + 28.0, p.y + 72.0),
                    13.0,
                    ui::COL_TEXT_DIM,
                );

                // Live density preview: expected mine count from current form values.
                {
                    let fwc = form.width.parse_u32().unwrap_or(16);
                    let fhc = form.height.parse_u32().unwrap_or(16);
                    let fpc = form.chance.parse_u32().unwrap_or(10);
                    let expected = (fwc as u64 * fhc as u64 * fpc as u64 + 50) / 100;
                    ui::txt(
                        &format!(
                            "expect ~{expected} mines on {fwc}x{fhc} — the fewer mines, the more one click opens"
                        ),
                        vec2(p.x + 28.0, p.y + 94.0),
                        12.0,
                        ui::COL_ACCENT,
                    );
                }

                let lx = p.x + 28.0;
                let fw = pw - 56.0;

                // Classic difficulty presets. Our mines are chance-based, so the
                // percentages mirror the original densities (10/81, 40/256, 99/480).
                let presets: [(&str, u32, u32, u32); 3] = [
                    ("Beginner", 9, 9, 12),
                    ("Intermediate", 16, 16, 16),
                    ("Expert", 30, 16, 21),
                ];
                {
                    let pw_btn = (fw - 16.0) / 3.0;
                    for (i, (label, w, h, c)) in presets.into_iter().enumerate() {
                        let r = Rect::new(lx + i as f32 * (pw_btn + 8.0), p.y + 104.0, pw_btn, 44.0);
                        if ui::button(r, &format!("{label} {w}x{h} {c}%"), 13.0, true) {
                            form.width.value = w.to_string();
                            form.height.value = h.to_string();
                            form.chance.value = c.to_string();
                            form.time.value.clear();
                            form.flags.value.clear();
                            form.attempts.value = "1".to_string();
                            form.error = None;
                        }
                    }
                }

                let row0 = p.y + 160.0;
                let field_rects: [Rect; 7] = [
                    Rect::new(lx, row0 + 24.0, fw, 42.0),
                    Rect::new(lx, row0 + 98.0, fw / 2.0 - 8.0, 42.0),
                    Rect::new(lx + fw / 2.0 + 8.0, row0 + 98.0, fw / 2.0 - 8.0, 42.0),
                    Rect::new(lx, row0 + 172.0, fw, 42.0),
                    Rect::new(lx, row0 + 246.0, fw, 42.0),
                    Rect::new(lx, row0 + 320.0, fw, 42.0),
                    Rect::new(lx, row0 + 394.0, fw, 42.0),
                ];
                let labels: [&str; 7] = [
                    "Name",
                    "Width (2–500)",
                    "Height (2–500)",
                    "Mine chance, % (1–99)",
                    "Time, sec",
                    "Flags",
                    "Attempts",
                ];
                let placeholders: [&str; 7] = [
                    "unnamed",
                    "",
                    "",
                    "",
                    "empty or 0 - unlimited",
                    "empty or 0 - unlimited",
                    "empty or 0 - unlimited",
                ];

                let active = form.active;
                let mut clicked_focus: Option<usize> = None;
                {
                    let fields = form.fields_mut();
                    for i in 0..7 {
                        ui::txt(
                            labels[i],
                            vec2(field_rects[i].x, field_rects[i].y - 8.0),
                            14.0,
                            ui::COL_TEXT_DIM,
                        );
                        if ui::text_field(fields[i], field_rects[i], active == i, placeholders[i], 17.0) {
                            clicked_focus = Some(i);
                        }
                    }
                }
                if let Some(f) = clicked_focus {
                    form.active = f;
                }
                let fa = form.active;
                {
                    let fields = form.fields_mut();
                    ui::handle_typing(fields[fa]);
                }

                if let Some(e) = &form.error {
                    ui::txt(e, vec2(p.x + 28.0, p.y + ph - 150.0), 14.0, ui::COL_DANGER);
                }

                let by = p.y + ph - 64.0;
                if ui::button(Rect::new(p.x + 28.0, by, 220.0, 46.0), "Create field", 17.0, true) {
                    match form.build_def() {
                        Ok(def) => match Board::generate_with_mines(def, &mut Rng::from_system_time(), 16) {
                            Some(b) => {
                                let name = if form.name.value.trim().is_empty() {
                                    format!("Field {}", rows.len() + 1)
                                } else {
                                    form.name.value.trim().to_string()
                                };
                                let id = db::insert_board(&conn, &name, &b);
                                let fit = ((screen_width() - 120.0) / b.width() as f32)
                                    .min((screen_height() - HUD_H - 120.0) / b.height() as f32);
                                let cell = fit.clamp(CELL_MIN, CELL_MAX);
                                let cam = clamp_cam(
                                    vec2(b.width() as f32 * cell / 2.0, b.height() as f32 * cell / 2.0),
                                    &b,
                                    cell,
                                );
                                game = Some(GameView {
                                    id,
                                    board: b,
                                    cell,
                                    cam,
                                    sec_accum: 0.0,
                                    autosave_accum: 0.0,
                                    session_elapsed: 0,
                                    pan_grab: None,
                                });
                                scene = Scene::Game;
                            }
                            None => {
                                form.error =
                                    Some("Mine chance too low: not a single mine was rolled — raise it".into());
                            }
                        },
                        Err(e) => form.error = Some(e),
                    }
                }
                if ui::button(Rect::new(p.x + pw - 168.0, by, 140.0, 46.0), "Cancel", 17.0, true)
                    || is_key_pressed(KeyCode::Escape)
                {
                    scene = Scene::Menu;
                }
            }
            Scene::Game => {
                let Some(g) = &mut game else {
                    scene = Scene::Menu;
                    continue;
                };
                let dt = get_frame_time();

                // Timer + autosave scheduling.
                g.sec_accum += dt;
                g.autosave_accum += dt;
                let mut need_save = g.autosave_accum >= AUTOSAVE_SECS;
                while g.sec_accum >= 1.0 {
                    g.sec_accum -= 1.0;
                    g.session_elapsed += 1;
                    let before = g.board.status;
                    g.board.tick(1);
                    if g.board.status != before {
                        need_save = true;
                    }
                }
                if need_save {
                    g.autosave_accum = 0.0;
                    db::save_board(&conn, g.id, &g.board);
                }

                let active = g.board.status == Status::Active;

                // Camera.
                if active {
                    let wheel = mouse_wheel().1;
                    if wheel != 0.0 {
                        g.cell = (g.cell * (1.1f32).powf(wheel)).clamp(CELL_MIN, CELL_MAX);
                    }
                    if is_mouse_button_pressed(MouseButton::Middle) {
                        g.pan_grab = Some(ui::mouse());
                    } else if is_mouse_button_down(MouseButton::Middle) {
                        if let Some(grab) = g.pan_grab.take() {
                            let m = ui::mouse();
                            g.cam -= m - grab;
                            g.pan_grab = Some(m);
                        }
                    } else {
                        g.pan_grab = None;
                    }
                    let pan_speed = 700.0 * dt;
                    if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
                        g.cam.x -= pan_speed;
                    }
                    if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
                        g.cam.x += pan_speed;
                    }
                    if is_key_down(KeyCode::Up) || is_key_down(KeyCode::W) {
                        g.cam.y -= pan_speed;
                    }
                    if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
                        g.cam.y += pan_speed;
                    }
                }
                g.cam = clamp_cam(g.cam, &g.board, g.cell);

                // Board rendering (screen-space, no camera matrices).
                let o = origin(g.cam);
                let bw = g.board.width();
                let bh = g.board.height();
                let cell = g.cell;
                let x0 = ((-o.x) / cell).floor().max(0.0) as u32;
                let y0 = ((HUD_H - o.y) / cell).floor().max(0.0) as u32;
                let x1 = (((screen_width() - o.x) / cell).ceil() as u32 + 1).min(bw);
                let y1 = (((screen_height() - o.y) / cell).ceil() as u32 + 1).min(bh);

                let hover_cell: Option<(u32, u32)> = {
                    let m = ui::mouse();
                    let cx = ((m.x - o.x) / cell).floor();
                    let cy = ((m.y - o.y) / cell).floor();
                    if cx >= 0.0 && cy >= 0.0 && cx < bw as f32 && cy < bh as f32 && m.y > HUD_H {
                        Some((cx as u32, cy as u32))
                    } else {
                        None
                    }
                };

                let finished = g.board.status != Status::Active;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = (y * bw + x) as usize;
                        let r = Rect::new(o.x + x as f32 * cell, o.y + y as f32 * cell, cell, cell);
                        let checker = (x + y) % 2 == 0;
                        let revealed = g.board.revealed[i];
                        let flagged = g.board.flagged[i];
                        let is_mine = g.board.mines[i];
                        let defused = g.board.defused[i];
                        let show_mine = defused || (finished && is_mine && !flagged);
                        let bg = if revealed {
                            if checker { ui::COL_CELL_OPEN } else { ui::COL_CELL_OPEN_B }
                        } else if checker {
                            ui::COL_CELL_HIDDEN
                        } else {
                            ui::COL_CELL_HIDDEN_B
                        };
                        draw_rectangle_rec(r, bg);
                        if show_mine {
                            draw_rectangle_rec(r, ui::COL_MINE_BG);
                            let c = vec2(r.x + cell / 2.0, r.y + cell / 2.0);
                            draw_circle(c.x, c.y, cell * 0.28, ui::COL_MINE_DOT);
                            for k in 0..8 {
                                let a = k as f32 * std::f32::consts::TAU / 8.0;
                                draw_line(
                                    c.x + a.cos() * cell * 0.28,
                                    c.y + a.sin() * cell * 0.28,
                                    c.x + a.cos() * cell * 0.40,
                                    c.y + a.sin() * cell * 0.40,
                                    (cell * 0.06).max(1.0),
                                    ui::COL_MINE_DOT,
                                );
                            }
                        } else if revealed {
                            let n = g.board.adjacent_active_mines(x, y);
                            if n > 0 {
                                ui::txt_centered(
                                    &n.to_string(),
                                    vec2(r.x + cell / 2.0, r.y + cell / 2.0),
                                    cell * 0.55,
                                    ui::NUM_COLORS[(n - 1) as usize],
                                );
                            }
                        }
                        if flagged && !show_mine {
                            let px = r.x + cell * 0.36;
                            let pole_top = r.y + cell * 0.2;
                            let pole_bot = r.y + cell * 0.72;
                            draw_line(px, pole_top, px, pole_bot, 2.0f32.max(cell * 0.05), ui::COL_TEXT);
                            draw_triangle(
                                vec2(px, r.y + cell * 0.2),
                                vec2(px + cell * 0.36, (pole_top + pole_bot) / 2.0),
                                vec2(px, r.y + cell * 0.46),
                                ui::COL_DANGER,
                            );
                        }
                        draw_rectangle_lines_ex(r, 1.0, ui::COL_CELL_GRID);
                    }
                }
                if active {
                    if let Some((hx, hy)) = hover_cell {
                        let r = Rect::new(o.x + hx as f32 * cell, o.y + hy as f32 * cell, cell, cell);
                        draw_rectangle_rec(r, Color::new(1.0, 1.0, 1.0, 0.14));
                    }
                }

                // Board input.
                if active {
                    if let Some((x, y)) = hover_cell {
                        if is_mouse_button_pressed(MouseButton::Left) {
                            g.board.reveal(x, y);
                            db::save_board(&conn, g.id, &g.board);
                            g.autosave_accum = 0.0;
                        } else if is_mouse_button_pressed(MouseButton::Right) {
                            g.board.toggle_flag(x, y);
                            db::save_board(&conn, g.id, &g.board);
                            g.autosave_accum = 0.0;
                        }
                    }
                }

                // HUD.
                draw_rectangle(0.0, 0.0, screen_width(), HUD_H, ui::COL_PANEL_DARK);
                draw_line(0.0, HUD_H, screen_width(), HUD_H, 2.0, ui::COL_BORDER);
                let mines_total = g.board.mines.iter().filter(|&&m| m).count() as u32;
                let defused_cnt = g.board.defused.iter().filter(|&&d| d).count() as u32;
                let time_str = match g.board.time_left {
                    Some(t) => ui::fmt_time(t),
                    None => ui::fmt_time(g.session_elapsed),
                };
                let time_color = if g.board.time_left.is_some_and(|t| t <= 30) {
                    ui::COL_DANGER
                } else {
                    ui::COL_TEXT
                };
                let flags_str = match g.board.def.flag_limit {
                    Some(l) => format!("Flags {}/{}", g.board.flags_used, l),
                    None => format!("Flags {}/inf", g.board.flags_used),
                };
                let att_str = match (g.board.attempts_left, g.board.def.attempts_limit) {
                    (Some(a), Some(l)) => format!("Attempts {a}/{l}"),
                    _ => "Attempts inf".into(),
                };
                ui::txt(&format!("Time {time_str}"), vec2(16.0, 34.0), 16.0, time_color);
                ui::txt(&flags_str, vec2(190.0, 34.0), 16.0, ui::COL_TEXT);
                ui::txt(&att_str, vec2(360.0, 34.0), 16.0, ui::COL_TEXT);
                ui::txt(
                    &format!("Mines defused {defused_cnt}/{mines_total}"),
                    vec2(520.0, 34.0),
                    16.0,
                    ui::COL_TEXT,
                );
                ui::txt(
                    "LMB - reveal · RMB - flag · wheel - zoom · MMB/WASD - pan",
                    vec2(16.0, screen_height() - 12.0),
                    13.0,
                    ui::COL_TEXT_DIM,
                );
                let mut exit_requested = ui::button(Rect::new(screen_width() - 136.0, 10.0, 120.0, 36.0), "Menu", 16.0, true)
                    || is_key_pressed(KeyCode::Escape);

                // Finish overlay.
                if g.board.status != Status::Active {
                    let sw = screen_width();
                    let sh = screen_height();
                    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
                    let p = Rect::new(sw / 2.0 - 280.0, sh / 2.0 - 170.0, 560.0, 340.0);
                    ui::panel(p, ui::COL_PANEL);
                    let (title, color, reason) = match g.board.status {
                        Status::Won => (
                            "YOU WIN",
                            ui::COL_GOLD,
                            "Every mine is defused or flagged",
                        ),
                        Status::Lost => (
                            "YOU LOSE",
                            ui::COL_DANGER,
                            match g.board.loss_reason {
                                Some(LossReason::TimeUp) => "Time is up",
                                _ => "Out of attempts",
                            },
                        ),
                        Status::Active => unreachable!(),
                    };
                    ui::txt_centered(title, vec2(sw / 2.0, p.y + 90.0), 42.0, color);
                    ui::txt_centered(reason, vec2(sw / 2.0, p.y + 140.0), 16.0, ui::COL_TEXT_DIM);
                    ui::txt_centered(
                        &format!("Mines defused: {defused_cnt} of {mines_total}"),
                        vec2(sw / 2.0, p.y + 172.0),
                        16.0,
                        ui::COL_TEXT,
                    );
                    if ui::button(Rect::new(sw / 2.0 - 120.0, p.y + 230.0, 240.0, 52.0), "Back to menu", 20.0, true) {
                        exit_requested = true;
                    }
                }

                if exit_requested {
                    db::save_board(&conn, g.id, &g.board);
                    if g.board.status != Status::Active {
                        // Finished boards leave the list right after the "back" click.
                        db::delete_field(&conn, g.id);
                    }
                    rows = db::list_fields(&conn);
                    game = None;
                    scene = Scene::Menu;
                }
            }
        }
        next_frame().await
    }
}
