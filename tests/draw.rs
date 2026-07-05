//! draw 域集成测试。

mod common;

#[path = "draw/integration.rs"]
mod integration;

#[path = "draw/engine_cpu.rs"]
mod engine_cpu;

#[path = "draw/bitmap_font.rs"]
mod bitmap_font;

#[path = "draw/font_service.rs"]
mod font_service;

#[path = "draw/rasterizer_ext.rs"]
mod rasterizer_ext;

#[path = "draw/spatial_integration.rs"]
mod spatial_integration;

#[path = "draw/pipeline_session.rs"]
mod pipeline_session;

#[path = "draw/invalidation_phase2.rs"]
mod invalidation_phase2;

#[path = "draw/render_object.rs"]
mod render_object;

#[path = "draw/repaint_boundary.rs"]
mod repaint_boundary;

#[path = "draw/scroll_viewport.rs"]
mod scroll_viewport;

#[path = "draw/render_baseline.rs"]
mod render_baseline;
