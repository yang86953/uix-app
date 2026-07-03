//! Phase 0 渲染回归基线测试。
//!
//! 未达标项标 `#[ignore]`，待后续 Phase 关闭（见 docs/rendering-engine-optimization.md §7.4）。
//!
//! 运行方式：
//! ```bash
//! cargo test --features test-harness -p uix-ui render_baseline
//! cargo test -p uix-graphics render_baseline
//! ```

#![cfg(feature = "test-harness")]

use uix_graphics::pipeline::RenderMetrics;

#[test]
#[ignore = "TODO Phase 2: 静止窗口无交互时 present_calls 应为 0"]
fn baseline_idle_zero_present() {
    let _ = RenderMetrics::default();
}

#[test]
#[ignore = "TODO Phase 2: hover 应仅产生 button 局部 damage"]
fn baseline_hover_partial_damage() {
    let _ = RenderMetrics::default();
}

#[test]
#[ignore = "TODO Phase 5: scroll 应近似 strip 重绘而非视口全量"]
fn baseline_scroll_strip_repaint() {
    let _ = RenderMetrics::default();
}

#[test]
#[ignore = "TODO Phase 2: 10 个独立动画应 update/paint 各 10 节点"]
fn baseline_ten_animations_ten_nodes() {
    let _ = RenderMetrics::default();
}
