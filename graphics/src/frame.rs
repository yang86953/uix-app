//! 帧生命周期 —— CPU/GPU 引擎共用的 `begin_frame` / `end_frame` 逻辑。
//!
//! 将清除策略（全帧/逐矩形/跳过）和裁剪管理抽离为独立函数，
//! 引擎通过回调注入表面操作差异。

use uix_core::Rect;
use crate::types::DirtyRegion;
use crate::Color;

/// 清除操作指令（用于 `frame_begin_clear` 回调）。
#[derive(Debug, Clone, Copy)]
pub enum ClearOp {
    /// 清除整个表面
    All(Color),
    /// 清除指定像素矩形区域
    Rect(i32, i32, i32, i32, Color),
}

/// 将脏矩形（浮点坐标）转换为像素边界，返回 `(x, y, w, h)`。
///
/// 使用四舍五入取整（+0.5 → floor），与 fill_rect 保持一致，
/// 避免截断取整（as i32）与四舍五入之间的 1px 差异。
pub fn dirty_rect_to_pixels(r: Rect, max_w: i32, max_h: i32) -> Option<(i32, i32, i32, i32)> {
    let x0 = (r.x + 0.5).floor().max(0.0) as i32;
    let y0 = (r.y + 0.5).floor().max(0.0) as i32;
    let x1 = (r.x + r.w + 0.5).floor().max(0.0) as i32;
    let y1 = (r.y + r.h + 0.5).floor().max(0.0) as i32;
    let cw = (x1 - x0).min(max_w - x0).max(0);
    let ch = (y1 - y0).min(max_h - y0).max(0);
    if cw > 0 && ch > 0 { Some((x0, y0, cw, ch)) } else { None }
}

/// 帧开始阶段一：重置裁剪状态。返回旧裁剪矩形。
///
/// 引擎在 `begin_frame` 开头调用，存储返回值到 `pre_frame_clip`。
pub fn frame_begin_clip(
    dirty: &DirtyRegion,
    w: i32,
    h: i32,
    mut reset_clip: impl FnMut(Rect) -> Rect,
) -> Rect {
    let fw = w as f32;
    let fh = h as f32;
    let full = Rect::new(0.0, 0.0, fw, fh);

    if dirty.full_frame || !dirty.clear_required || dirty.rects().is_empty() {
        reset_clip(full)
    } else {
        let bounds = dirty.bounds();
        reset_clip(Rect::new(
            bounds.x.max(0.0),
            bounds.y.max(0.0),
            bounds.w.min(fw - bounds.x.max(0.0)),
            bounds.h.min(fh - bounds.y.max(0.0)),
        ))
    }
}

/// 帧开始阶段二：按脏区域策略清除表面。
///
/// 引擎在 `frame_begin_clip` 之后调用。
pub fn frame_begin_clear(
    dirty: &DirtyRegion,
    clear_color: Color,
    w: i32,
    h: i32,
    mut apply: impl FnMut(ClearOp),
) {
    if dirty.full_frame {
        apply(ClearOp::All(clear_color));
    } else if dirty.clear_required {
        for &r in dirty.rects() {
            if let Some((x, y, cw, ch)) = dirty_rect_to_pixels(r, w, h) {
                apply(ClearOp::Rect(x, y, cw, ch, clear_color));
            }
        }
    }
}

/// 帧结束：恢复裁剪状态。
pub fn frame_end(pre_clip: Rect, reset_clip: impl FnOnce(Rect)) {
    reset_clip(pre_clip);
}
