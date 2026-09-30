use macroquad::prelude::*;
use std::cell::RefCell;

pub const FONT_BYTES: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");

thread_local! {
    static FONT: RefCell<Option<Font>> = const { RefCell::new(None) };
}

pub fn init_font() {
    match load_ttf_font_from_bytes(FONT_BYTES) {
        Ok(f) => FONT.with(|slot| *slot.borrow_mut() = Some(f)),
        Err(e) => eprintln!("font load failed, falling back to default: {e:?}"),
    }
}

pub fn txt(label: &str, pos: Vec2, size: f32, color: Color) {
    FONT.with(|slot| {
        let slot = slot.borrow_mut();
        draw_text_ex(
            label,
            pos.x,
            pos.y,
            TextParams {
                font: slot.as_ref(),
                font_size: size as u16,
                color,
                ..Default::default()
            },
        );
    });
}

pub fn txt_w(label: &str, size: f32) -> f32 {
    FONT.with(|slot| {
        let slot = slot.borrow();
        measure_text(label, slot.as_ref(), size as u16, 1.0).width
    })
}

/// macroquad lacks Rect-based rectangle helpers; these mirror the classic API.
pub fn draw_rectangle_rec(r: Rect, color: Color) {
    draw_rectangle(r.x, r.y, r.w, r.h, color);
}

pub fn draw_rectangle_lines_ex(r: Rect, thickness: f32, color: Color) {
    draw_rectangle_lines(r.x, r.y, r.w, r.h, thickness, color);
}

pub fn txt_centered(label: &str, center: Vec2, size: f32, color: Color) {
    txt(label, center - vec2(txt_w(label, size) / 2.0, -size * 0.35), size, color);
}

// ---------- theme ----------

pub const COL_BG: Color = Color::new(0.10, 0.11, 0.13, 1.0);
pub const COL_PANEL: Color = Color::new(0.17, 0.18, 0.21, 1.0);
pub const COL_PANEL_DARK: Color = Color::new(0.13, 0.14, 0.16, 1.0);
pub const COL_BTN: Color = Color::new(0.26, 0.28, 0.32, 1.0);
pub const COL_BTN_HOVER: Color = Color::new(0.34, 0.36, 0.41, 1.0);
pub const COL_BTN_DISABLED: Color = Color::new(0.20, 0.21, 0.23, 1.0);
pub const COL_BORDER: Color = Color::new(0.08, 0.08, 0.09, 1.0);
#[allow(dead_code)]
pub const COL_ACCENT: Color = Color::new(0.35, 0.63, 1.0, 1.0);
pub const COL_FOCUS: Color = Color::new(0.45, 0.75, 1.0, 1.0);
pub const COL_TEXT: Color = Color::new(0.90, 0.90, 0.92, 1.0);
pub const COL_TEXT_DIM: Color = Color::new(0.58, 0.59, 0.62, 1.0);
pub const COL_DANGER: Color = Color::new(0.86, 0.25, 0.22, 1.0);
pub const COL_DANGER_HOVER: Color = Color::new(1.0, 0.32, 0.29, 1.0);
pub const COL_OK: Color = Color::new(0.30, 0.78, 0.42, 1.0);
pub const COL_GOLD: Color = Color::new(0.95, 0.78, 0.25, 1.0);

pub const COL_CELL_HIDDEN: Color = Color::new(0.53, 0.56, 0.60, 1.0);
pub const COL_CELL_HIDDEN_B: Color = Color::new(0.50, 0.53, 0.57, 1.0);
pub const COL_CELL_OPEN: Color = Color::new(0.78, 0.79, 0.81, 1.0);
pub const COL_CELL_OPEN_B: Color = Color::new(0.75, 0.76, 0.78, 1.0);
pub const COL_CELL_GRID: Color = Color::new(0.42, 0.44, 0.47, 1.0);
pub const COL_MINE_BG: Color = Color::new(0.24, 0.24, 0.26, 1.0);
pub const COL_MINE_DOT: Color = Color::new(0.07, 0.07, 0.08, 1.0);

pub const NUM_COLORS: [Color; 8] = [
    Color::new(0.15, 0.35, 0.86, 1.0),
    Color::new(0.15, 0.59, 0.23, 1.0),
    Color::new(0.86, 0.23, 0.20, 1.0),
    Color::new(0.10, 0.18, 0.63, 1.0),
    Color::new(0.55, 0.10, 0.18, 1.0),
    Color::new(0.12, 0.55, 0.55, 1.0),
    Color::new(0.16, 0.16, 0.18, 1.0),
    Color::new(0.47, 0.47, 0.49, 1.0),
];

// ---------- widgets ----------

pub fn mouse() -> Vec2 {
    let mp = mouse_position();
    vec2(mp.0, mp.1)
}

pub fn panel(rect: Rect, color: Color) {
    draw_rectangle_rec(rect, color);
    draw_rectangle_lines_ex(rect, 2.0, COL_BORDER);
}

pub fn button(rect: Rect, label: &str, size: f32, enabled: bool) -> bool {
    let m = mouse();
    let hovered = enabled && rect.contains(m);
    let bg = if !enabled {
        COL_BTN_DISABLED
    } else if hovered {
        COL_BTN_HOVER
    } else {
        COL_BTN
    };
    draw_rectangle_rec(rect, bg);
    draw_rectangle_lines_ex(rect, 2.0, COL_BORDER);
    txt_centered(label, rect.center(), size, if enabled { COL_TEXT } else { COL_TEXT_DIM });
    hovered && is_mouse_button_released(MouseButton::Left)
}

pub fn danger_button(rect: Rect, label: &str, size: f32, enabled: bool) -> bool {
    let m = mouse();
    let hovered = enabled && rect.contains(m);
    draw_rectangle_rec(rect, if hovered { COL_DANGER_HOVER } else { COL_DANGER });
    draw_rectangle_lines_ex(rect, 2.0, COL_BORDER);
    txt_centered(label, rect.center(), size, WHITE);
    hovered && is_mouse_button_released(MouseButton::Left)
}

#[derive(Clone, Default)]
pub struct TextField {
    pub value: String,
    pub numeric: bool,
    /// Numeric fields additionally accept a single '.' when this is set.
    pub decimal: bool,
    pub max_len: usize,
}

impl TextField {
    pub fn new(numeric: bool, max_len: usize) -> Self {
        TextField { value: String::new(), numeric, decimal: false, max_len }
    }

    pub fn with(value: &str, numeric: bool, max_len: usize) -> Self {
        TextField { value: value.to_string(), numeric, decimal: false, max_len }
    }

    /// Numeric field that accepts fractional input like "12.5".
    pub fn with_dec(value: &str, max_len: usize) -> Self {
        TextField { value: value.to_string(), numeric: true, decimal: true, max_len }
    }

    pub fn parse_u32(&self) -> Option<u32> {
        self.value.trim().parse::<u32>().ok()
    }

    pub fn parse_f32(&self) -> Option<f32> {
        let v: f32 = self.value.trim().parse().ok()?;
        v.is_finite().then_some(v)
    }
}

/// Draws the field; returns true when clicked (i.e. it wants focus).
pub fn text_field(field: &TextField, rect: Rect, focused: bool, placeholder: &str, size: f32) -> bool {
    draw_rectangle_rec(rect, COL_PANEL_DARK);
    draw_rectangle_lines_ex(rect, if focused { 3.0 } else { 2.0 }, if focused { COL_FOCUS } else { COL_BORDER });
    let empty = field.value.is_empty();
    let label = if empty { placeholder } else { &field.value };
    let color = if empty { COL_TEXT_DIM } else { COL_TEXT };
    let base = vec2(rect.x + 8.0, rect.y + rect.h / 2.0 + size * 0.35);
    txt(label, base, size, color);
    if focused && ((get_time() * 2.0) as i64) % 2 == 0 {
        let w = if empty { 0.0 } else { txt_w(&field.value, size) };
        draw_rectangle_rec(Rect::new(base.x + w + 2.0, rect.y + 6.0, 2.0, rect.h - 12.0), COL_FOCUS);
    }
    rect.contains(mouse()) && is_mouse_button_released(MouseButton::Left)
}

/// Feeds this frame's keyboard input to the focused field. Call once per frame.
pub fn handle_typing(field: &mut TextField) {
    while let Some(ch) = get_char_pressed() {
        if ch.is_control() {
            continue;
        }
        let dot_ok = field.decimal && ch == '.' && !field.value.contains('.');
        if field.numeric && !ch.is_ascii_digit() && !dot_ok {
            continue;
        }
        if field.value.chars().count() < field.max_len {
            field.value.push(ch);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        field.value.pop();
    }
}

/// Windows (miniquad) reports RAW wheel deltas of +-120 per notch, while
/// Linux/macOS report +-1.0. Normalize everything to whole notches, capped so
/// a fast scroll cannot jump more than 3 steps per frame.
pub fn normalize_wheel(delta: f32) -> f32 {
    if delta.abs() < f32::EPSILON {
        return 0.0;
    }
    delta.signum() * (delta.abs() / 120.0).ceil().min(3.0)
}

pub fn fmt_time(total_secs: u32) -> String {
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

#[allow(dead_code)]
pub fn fmt_plural_ru(n: u32, one: &str, few: &str, many: &str) -> String {
    let mod10 = n % 10;
    let mod100 = n % 100;
    let word = if mod10 == 1 && mod100 != 11 {
        one
    } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
        few
    } else {
        many
    };
    format!("{n} {word}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_normalizes_to_notches() {
        // Windows (miniquad) raw deltas: +-120 per notch.
        assert_eq!(normalize_wheel(120.0), 1.0);
        assert_eq!(normalize_wheel(-120.0), -1.0);
        assert_eq!(normalize_wheel(240.0), 2.0);
        assert_eq!(normalize_wheel(600.0), 3.0, "fast scroll is capped at 3");
        assert_eq!(normalize_wheel(0.0), 0.0);
        // Linux/macOS already report +-1.0 per notch.
        assert_eq!(normalize_wheel(1.0), 1.0);
        assert_eq!(normalize_wheel(-1.0), -1.0);
        assert_eq!(normalize_wheel(2.0), 1.0, "sub-notch deltas count as one notch");
    }

    #[test]
    fn fmt_time_zero_pads() {
        assert_eq!(fmt_time(0), "0:00");
        assert_eq!(fmt_time(59), "0:59");
        assert_eq!(fmt_time(300), "5:00");
        assert_eq!(fmt_time(3723), "62:03");
    }
}
