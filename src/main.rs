use macroquad::miniquad::conf::Conf;
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: format!("TheMinesweeper {}", env!("CARGO_PKG_VERSION")),
        window_width: 640,
        window_height: 480,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(Color::from_rgba(192, 192, 192, 255));

        draw_text(
            "TheMinesweeper — skeleton, see PLAN.md",
            20.0,
            40.0,
            26.0,
            BLACK,
        );

        next_frame().await
    }
}
