//! 损坏区域计算辅助。

use super::*;

// 脏矩形达到该数量后才评估单次包围盒重绘，避免小规模更新增加填充量。
const DENSE_DIRTY_RECT_THRESHOLD: usize = 4;
// 包围盒面积不超过离散矩形总面积两倍时，优先一次遍历，限制额外重绘范围。
const DENSE_DIRTY_MAX_OVERDRAW: f64 = 2.0;

impl Default for ScenePipeline {
    fn default() -> Self {
        Self::new()
    }
}

pub(super) fn draw_debug_overlay(
    engine: &mut dyn RenderTarget,
    scene: &impl ScenePaint,
    hover_pos: Option<Point>,
    frame: Option<&crate::draw::debug::DebugFrameSnapshot>,
    metrics: Option<&RenderMetrics>,
    font: FontHandle,
    font_service: &FontService,
) {
    let canvas = engine.canvas_2d();
    DebugRenderService::new(true).draw_final_overlay(
        canvas,
        scene,
        hover_pos,
        frame,
        metrics,
        font,
        font_service,
    );
}

pub(super) fn normalize_end_outcome(
    end_outcome: RenderOutcome,
    caps: crate::draw::GraphicsCapabilities,
    damage: DamageRegion,
) -> RenderOutcome {
    match end_outcome {
        RenderOutcome::Present(_) if caps.uses_external_presenter() => {
            RenderOutcome::PresentPending(damage)
        }
        RenderOutcome::Present(_) => RenderOutcome::Present(damage),
        RenderOutcome::PresentPending(_) if caps.uses_external_presenter() => {
            RenderOutcome::PresentPending(damage)
        }
        RenderOutcome::PresentPending(_) => RenderOutcome::Failed(
            crate::draw::renderer::GraphicsFailure::from_error(Error::new(
                Errc::InvalidState,
                "backend-managed backend returned an external presentation pending result",
            )),
        ),
        RenderOutcome::FrameReady(_) => RenderOutcome::Failed(
            crate::draw::renderer::GraphicsFailure::from_error(Error::new(
                Errc::InvalidState,
                "end_frame returned FrameReady instead of final presentation",
            )),
        ),
        RenderOutcome::Idle => RenderOutcome::Idle,
        RenderOutcome::Failed(error) => RenderOutcome::Failed(error),
    }
}

pub(super) fn compute_present_damage(
    dirty: &DirtyRegion,
    full_frame: bool,
    rendered_first: bool,
) -> DamageRegion {
    if !rendered_first || full_frame {
        return DamageRegion::full();
    }
    // scroll 视口已并入 dirty（见 render_frame 的 dirty_with_scroll 构造）。
    let rects: Vec<Rect> = dirty
        .rects()
        .iter()
        .filter(|r| r.w > 0.0 && r.h > 0.0)
        .map(pad_damage_rect)
        .collect();
    if rects.is_empty() {
        DamageRegion::full()
    } else {
        DamageRegion::partial(rects)
    }
}

/// 消费实际绘制区，在原矩形分配内建立最终呈现损伤。
pub(super) fn compute_present_damage_owned(
    dirty: DirtyRegion,
    rendered_first: bool,
) -> DamageRegion {
    if !rendered_first || dirty.full_frame {
        return DamageRegion::full();
    }
    let mut rects = dirty.into_rects();
    rects.retain(|rect| rect.w > 0.0 && rect.h > 0.0);
    for rect in &mut rects {
        *rect = pad_damage_rect(rect);
    }
    if rects.is_empty() {
        DamageRegion::full()
    } else {
        DamageRegion::partial(rects)
    }
}

/// 将高密度局部脏区收敛为一次包围盒重绘。
///
/// 多矩形逐块绘制会为每个矩形重复遍历并编码整棵场景；当包围盒额外面积受控时，
/// 扩大实际清理与 present damage 比重复提交更便宜。稀疏区域仍保留离散矩形。
pub(super) fn coalesce_dense_dirty_region(region: DirtyRegion) -> DirtyRegion {
    if region.full_frame || region.rects().len() < DENSE_DIRTY_RECT_THRESHOLD {
        return region;
    }
    if !region.rects().iter().copied().all(valid_frame_rect) {
        return region;
    }

    let bounds = region.bounds();
    if !valid_frame_rect(bounds) {
        return region;
    }
    let dirty_area = region
        .rects()
        .iter()
        .map(|rect| f64::from(rect.w) * f64::from(rect.h))
        .sum::<f64>();
    let bounds_area = f64::from(bounds.w) * f64::from(bounds.h);
    if dirty_area.is_finite()
        && bounds_area.is_finite()
        && bounds_area <= dirty_area * DENSE_DIRTY_MAX_OVERDRAW
    {
        DirtyRegion::area(bounds)
    } else {
        region
    }
}

// 判断局部脏区是否应按原始离散矩形逐块遍历。
pub(super) fn should_split_dirty_rects(region: &DirtyRegion) -> bool {
    // 与绘制路径原判定一致：完整帧和单矩形不进入拆分遍历。
    !region.full_frame
        && region.rects().len() > 1
        // 至少一个正面积矩形时才进入拆分分支。
        && region.rects().iter().any(|rect| positive_dirty_rect(*rect))
}

// 保留拆分绘制原有的正面积筛选，不扩大几何合法性语义。
pub(super) fn positive_dirty_rect(rect: Rect) -> bool {
    rect.w > 0.0 && rect.h > 0.0
}

pub(super) fn valid_scroll_copy(viewport: Rect, dx: f32, dy: f32) -> bool {
    if !valid_frame_rect(viewport)
        || ![viewport.x, viewport.y, viewport.w, viewport.h]
            .into_iter()
            .all(|value| value == value.round())
        || !dx.is_finite()
        || !dy.is_finite()
    {
        return false;
    }
    let dx = dx.round();
    let dy = dy.round();
    (dx != 0.0 || dy != 0.0)
        && (dx == 0.0 || dx.abs() < viewport.w)
        && (dy == 0.0 || dy.abs() < viewport.h)
}

pub(super) fn scroll_exposed_rect(viewport: Rect, dx: f32, dy: f32) -> Option<Rect> {
    if !valid_frame_rect(viewport) || !dx.is_finite() || !dy.is_finite() {
        return None;
    }
    let dx = dx.round();
    let dy = dy.round();
    let horizontal = (dx != 0.0).then(|| {
        let width = dx.abs().min(viewport.w);
        let x = if dx > 0.0 {
            viewport.x + viewport.w - width
        } else {
            viewport.x
        };
        Rect::new(x, viewport.y, width, viewport.h)
    });
    let vertical = (dy != 0.0).then(|| {
        let height = dy.abs().min(viewport.h);
        let y = if dy > 0.0 {
            viewport.y + viewport.h - height
        } else {
            viewport.y
        };
        Rect::new(viewport.x, y, viewport.w, height)
    });
    match (horizontal, vertical) {
        (Some(horizontal), Some(vertical)) => Some(horizontal.union(&vertical)),
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}

/// 以 `begin_frame` 返回的实际清区为权威绘制区，并验证它覆盖请求区。
pub(super) fn resolve_frame_region(
    requested_full_frame: bool,
    requested_bounds: Rect,
    actual: DamageRegion,
) -> Result<DirtyRegion, Error> {
    if actual.full {
        return Ok(DirtyRegion::full());
    }
    if requested_full_frame {
        return Err(Error::new(
            Errc::InvalidState,
            "begin_frame returned partial damage for a full redraw request",
        ));
    }

    for rect in &actual.rects {
        if !valid_frame_rect(*rect) {
            return Err(Error::new(
                Errc::InvalidState,
                "begin_frame returned an invalid partial damage rectangle",
            ));
        }
    }
    // backend 返回的 Vec 继续成为绘制区 owner，避免逐项重建第二份分配。
    let actual_region = DirtyRegion::from_valid_rects(actual.rects);
    if actual_region.is_empty() || !rect_covers(actual_region.bounds(), requested_bounds) {
        return Err(Error::new(
            Errc::InvalidState,
            "begin_frame damage does not cover the requested paint region",
        ));
    }
    Ok(actual_region)
}

pub(super) fn valid_frame_rect(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && rect.w.is_finite()
        && rect.h.is_finite()
        && rect.w > 0.0
        && rect.h > 0.0
}

pub(super) fn rect_covers(outer: Rect, inner: Rect) -> bool {
    outer.x <= inner.x
        && outer.y <= inner.y
        && outer.x + outer.w >= inner.x + inner.w
        && outer.y + outer.h >= inner.y + inner.h
}

pub(super) fn pad_damage_rect(r: &Rect) -> Rect {
    Rect::new(
        (r.x - 1.0).max(0.0),
        (r.y - 1.0).max(0.0),
        r.w + 2.0,
        r.h + 2.0,
    )
}

pub(super) fn classify_invalidation(
    rendered_first: bool,
    dirty_region: &DirtyRegion,
    source_hint: InvalidationSource,
) -> InvalidationSource {
    if !rendered_first {
        return InvalidationSource::FirstFrame;
    }
    if !dirty_region.is_empty() {
        return match source_hint {
            InvalidationSource::AnimationPolling | InvalidationSource::LayoutEvent => source_hint,
            _ => InvalidationSource::DirtyRegion,
        };
    }
    InvalidationSource::None
}
