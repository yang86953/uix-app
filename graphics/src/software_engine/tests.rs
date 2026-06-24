use crate::software_engine::core::RenderTarget;
use crate::Color;

fn make_rt(w: i32, h: i32) -> RenderTarget {
    let mut rt = RenderTarget::new();
    rt.initialize(w, h);
    rt
}

fn unpack(p: u32) -> (u8, u8, u8, u8) {
    let b = (p & 0xFF) as u8;
    let g = ((p >> 8) & 0xFF) as u8;
    let r = ((p >> 16) & 0xFF) as u8;
    let a = ((p >> 24) & 0xFF) as u8;
    (r, g, b, a)
}

fn unpack_premul(p: u32) -> (u32, u32, u32, u32) {
    let a = (p >> 24) & 0xFF;
    let r = (p >> 16) & 0xFF;
    let g = (p >> 8) & 0xFF;
    let b = p & 0xFF;
    (r, g, b, a)
}

fn blue() -> Color {
    Color::from_rgb(22, 119, 255)
}

fn red_half() -> Color {
    Color::from_rgba(255, 0, 0, 128)
}

mod tests_basic;
mod tests_advanced;
