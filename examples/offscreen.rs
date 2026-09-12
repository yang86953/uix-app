//! CPU rasterization without UI, GPU adapters or a native application.
use uix_app::core::Rect;
use uix_app::draw::{Color, RenderBackend, backend::CpuBackend};

fn main() {
    let mut backend = CpuBackend::new();
    backend.resize(16, 16).unwrap();
    backend.surface().canvas().fill_rect(
        Rect::new(4.0, 4.0, 8.0, 8.0),
        Color::from_rgb(255, 0, 0),
        None,
    );
    assert_eq!(backend.pixels()[8 * 16 + 8], 0xffff0000);
    assert_ne!(backend.pixels()[0], 0xffff0000);
    println!("standalone CPU pixel rendering passed");
}
