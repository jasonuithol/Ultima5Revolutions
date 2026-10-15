mod lzw;
mod dgroup;
mod platform;
mod data;
mod mem;
mod loader;
mod g;
mod game;
mod engine;
mod ovl;

use std::path::PathBuf;

fn game_dir() -> PathBuf {
    std::env::var("U5_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/home/jason/Projects/Ultima 5"))
}

/// Developer smoke test of the text/graphics foundation: fonts, windows, print_string, tiles.
fn smoke_test(gm: &mut game::Game) {
    use engine::{gfx, text};
    gm.set_g16(g::SOUND_ENABLED, 0);
    engine::util::init_system(gm);
    gfx::load_video_driver(gm);
    gfx::init_graphics_driver(gm, 1);
    gfx::driver_init_screen(gm);
    gm.with_str(b"IBM.CH", |gm, s| text::load_font(gm, 0, s));
    gm.with_str(b"RUNES.CH", |gm, s| text::load_font(gm, 1, s));
    text::select_font(gm, 0);
    text::define_text_window(gm, 24, 39, 0, 0, 0);
    text::select_text_window(gm, 0);
    gm.set_g16(g::CURSOR_ADVANCE, 1);
    gm.with_str(b"TILES.16", |gm, s| gfx::load_picture_file(gm, s));
    for t in 0..121u16 { gfx::draw_tile(gm, (t / 11) as i16, (t % 11) as i16, (t + 100) as i16); }
    text::define_text_window(gm, 23, 39, 1, 24, 2);
    text::select_text_window(gm, 2);
    text::set_text_color(gm, 15);
    // a DATA.OVL string (the first weapon/armour name table entry) and a wrapped sentence
    text::print_string(gm, 0x0042);
    text::put_char(gm, b'\n');
    text::print_str(gm, "The quick brown fox jumps over the lazy dog and keeps running\n");
    text::print_number(gm, b' ', 5, -1234);
    text::put_char(gm, b'\n');
    text::put_char(gm, 0xfd); text::print_str(gm, "inverse"); text::put_char(gm, 0xfd);
    text::put_char(gm, b'\n');
    for _ in 0..30 { text::print_str(gm, "scroll test line\n"); }
    gm.pf.pump();
}

fn main() {
    let dir = game_dir();
    let headless = std::env::var("U5_HEADLESS").is_ok();
    let mut gm = game::Game::new(dir, headless);
    if std::env::var("U5_TEST").is_ok() {
        smoke_test(&mut gm);
        if headless { gm.pf.screenshot_ppm("/tmp/claude-1000/u5_smoke.ppm").unwrap(); println!("screenshot written"); return; }
        loop { let k = gm.pf.wait_key(|_| {}); if k & 0xff == 27 || gm.pf.quit_requested { break; } }
        return;
    }
    // Real entry: the original C main() (translated in engine::rest).
    engine::rest::main_entry(&mut gm, 0, 0);
}
