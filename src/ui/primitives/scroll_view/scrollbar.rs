//! ScrollBar — classic gutter scrollbar for one axis (vertical or horizontal).
//!
//! Used by [`ScrollView`](super::ScrollView). Track sits in a reserved gutter so
//! content is not painted underneath the thumb (not overlay-on-content).

use crate::core::{Point, Rect};
use crate::draw::Radius;
use crate::ui::widget_runtime::paint_context::PaintContext;
// 滚动条的全部静态几何与主题角色由父组件 UIX 生成。
use super::ScrollbarVisual;
/// Which axis this scrollbar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    /// 控制纵向滚动位置。
    Vertical,
    /// 控制横向滚动位置。
    Horizontal,
}

/// State and rendering for a single-axis scrollbar.
#[derive(Debug, Clone)]
pub struct ScrollBar {
    /// Whether the scrollbar is visible at all.
    pub show: bool,
    /// Whether the thumb is currently being dragged.
    pub dragging: bool,
    /// Whether the mouse is hovering over the thumb.
    pub hover: bool,
    /// Which axis this scrollbar controls.
    pub orientation: ScrollbarOrientation,
    /// Pointer offset from thumb leading edge along the scroll axis (relative coords).
    /// Kept while [`Self::dragging`] so the thumb follows the grab point, not jumps.
    drag_grab: f32,
}

impl ScrollBar {
    /// Layout gutter reserved for this axis when the bar is shown (track + edge pad).
    pub fn gutter() -> f32 {
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        visual.thickness + visual.edge_padding
    }

    /// Create a new scrollbar for the given orientation (visible by default).
    pub fn new(orientation: ScrollbarOrientation) -> Self {
        Self {
            show: super::SCROLL_VIEW_VISUAL_REF.default_scrollbar_visible,
            dragging: false,
            hover: false,
            orientation,
            drag_grab: 0.0,
        }
    }

    // ── Track geometry ──────────────────────────────────────────────────────

    /// Track rectangle in **absolute** coordinates (for rendering).
    pub fn track_rect_abs(&self, abs_frame: Rect) -> Rect {
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(
                abs_frame.x + abs_frame.w - Self::gutter(),
                abs_frame.y + visual.edge_padding,
                visual.thickness,
                (abs_frame.h - visual.edge_padding * 2.0).max(0.0),
            ),
            ScrollbarOrientation::Horizontal => Rect::new(
                abs_frame.x + visual.edge_padding,
                abs_frame.y + abs_frame.h - Self::gutter(),
                (abs_frame.w - visual.edge_padding * 2.0).max(0.0),
                visual.thickness,
            ),
        }
    }

    /// Track rectangle in **relative** coordinates (for hit-testing with
    /// mouse positions that are relative to the viewport frame).
    pub fn track_rect_rel(&self, frame: Rect) -> Rect {
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(
                frame.w - Self::gutter(),
                visual.edge_padding,
                visual.thickness,
                (frame.h - visual.edge_padding * 2.0).max(0.0),
            ),
            ScrollbarOrientation::Horizontal => Rect::new(
                visual.edge_padding,
                frame.h - Self::gutter(),
                (frame.w - visual.edge_padding * 2.0).max(0.0),
                visual.thickness,
            ),
        }
    }

    // ── Thumb geometry ──────────────────────────────────────────────────────

    /// Thumb rectangle in **absolute** coordinates (for rendering).
    pub fn thumb_rect_abs(&self, abs_frame: Rect, scroll: f32, max_scroll: f32) -> Option<Rect> {
        if max_scroll <= 0.0 {
            return None;
        }
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        let track = self.track_rect_abs(abs_frame);
        let (pos, size) = self.compute_thumb_on_track(visual, track, abs_frame, scroll, max_scroll);
        Some(match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(track.x, pos, visual.thickness, size),
            ScrollbarOrientation::Horizontal => Rect::new(pos, track.y, size, visual.thickness),
        })
    }

    /// Thumb rectangle in **relative** coordinates (for hit-testing).
    pub fn thumb_rect_rel(&self, frame: Rect, scroll: f32, max_scroll: f32) -> Option<Rect> {
        if max_scroll <= 0.0 {
            return None;
        }
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        let track = self.track_rect_rel(frame);
        let (pos, size) = self.compute_thumb_on_track(visual, track, frame, scroll, max_scroll);
        Some(match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(track.x, pos, visual.thickness, size),
            ScrollbarOrientation::Horizontal => Rect::new(pos, track.y, size, visual.thickness),
        })
    }

    /// Compute the thumb's position offset and size along the track.
    fn compute_thumb_on_track(
        &self,
        visual: &ScrollbarVisual,
        track: Rect,
        view: Rect,
        scroll: f32,
        max_scroll: f32,
    ) -> (f32, f32) {
        match self.orientation {
            ScrollbarOrientation::Vertical => {
                let ratio = view.h / (view.h + max_scroll);
                let thumb_h = (ratio * track.h).max(visual.thumb_min_extent).min(track.h);
                let usable = track.h - thumb_h;
                let pos = if usable > 0.0 {
                    track.y + (scroll / max_scroll) * usable
                } else {
                    track.y
                };
                (pos, thumb_h)
            }
            ScrollbarOrientation::Horizontal => {
                let ratio = view.w / (view.w + max_scroll);
                let thumb_w = (ratio * track.w).max(visual.thumb_min_extent).min(track.w);
                let usable = track.w - thumb_w;
                let pos = if usable > 0.0 {
                    track.x + (scroll / max_scroll) * usable
                } else {
                    track.x
                };
                (pos, thumb_w)
            }
        }
    }

    // ── Hit-testing ─────────────────────────────────────────────────────────

    /// Check whether a **relative** mouse position hits the thumb.
    pub fn hit_test_thumb(&self, frame: Rect, pos: Point, scroll: f32, max_scroll: f32) -> bool {
        self.thumb_rect_rel(frame, scroll, max_scroll)
            .is_some_and(|r| r.contains(pos))
    }

    // ── Drag calculation ────────────────────────────────────────────────────

    /// Start a thumb drag at `pos` (relative to viewport), remembering the grab
    /// offset inside the thumb so subsequent moves do not jump.
    pub fn begin_drag(&mut self, frame: Rect, pos: Point, scroll: f32, max_scroll: f32) {
        self.dragging = true;
        self.drag_grab = self
            .thumb_rect_rel(frame, scroll, max_scroll)
            .map(|thumb| match self.orientation {
                ScrollbarOrientation::Vertical => pos.y - thumb.y,
                ScrollbarOrientation::Horizontal => pos.x - thumb.x,
            })
            .unwrap_or(0.0);
    }

    /// 结束当前滑块拖动。
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_grab = 0.0;
    }

    /// 根据**相对坐标**中的拖动位置计算新滚动值。
    ///
    /// `axis_pos` 是鼠标在滚动轴上的坐标（垂直时为 `pos.y`，水平时为 `pos.x`）。
    /// 使用 [`Self::begin_drag`] 记录的 `drag_grab`，使滑块跟随抓取点，
    /// 而不是把滑块前缘瞬移到指针位置。
    pub fn scroll_from_drag(
        &self,
        frame: Rect,
        axis_pos: f32,
        scroll: f32,
        max_scroll: f32,
    ) -> f32 {
        if max_scroll <= 0.0 {
            return scroll;
        }
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        let track = self.track_rect_rel(frame);
        let (_, thumb_size) = self.compute_thumb_on_track(visual, track, frame, scroll, max_scroll);

        let (track_extent, track_start) = match self.orientation {
            ScrollbarOrientation::Vertical => (track.h, track.y),
            ScrollbarOrientation::Horizontal => (track.w, track.x),
        };
        let usable = track_extent - thumb_size;
        if usable <= 0.0 {
            return scroll;
        }
        let thumb_leading = axis_pos - self.drag_grab;
        ((thumb_leading - track_start) / usable).clamp(0.0, 1.0) * max_scroll
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    /// Render the scrollbar track and thumb.
    ///
    /// `abs_frame` is the viewport frame in absolute (screen) coordinates.
    /// `scroll` and `max_scroll` define the current scroll position.
    pub fn render(&self, abs_frame: Rect, ctx: &mut PaintContext, scroll: f32, max_scroll: f32) {
        if !self.show || max_scroll <= 0.0 {
            return;
        }
        let visual = &super::SCROLL_VIEW_VISUAL_REF.scrollbar;
        let corner = Radius::uniform(visual.corner_radius);

        // Track background
        let track = self.track_rect_abs(abs_frame);
        ctx.fill_rect(
            track,
            visual.track_color.resolve(ctx.tokens()),
            Some(corner),
        );

        // Thumb
        if let Some(thumb) = self.thumb_rect_abs(abs_frame, scroll, max_scroll) {
            let color_role = if self.hover || self.dragging {
                visual.active_thumb_color
            } else {
                visual.thumb_color
            };
            ctx.fill_rect(thumb, color_role.resolve(ctx.tokens()), Some(corner));
        }
    }
}
