//! ScrollBar — classic gutter scrollbar for one axis (vertical or horizontal).
//!
//! Used by [`ScrollView`](super::ScrollView). Track sits in a reserved gutter so
//! content is not painted underneath the thumb (not overlay-on-content).

use crate::core::{Point, Rect};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
/// Which axis this scrollbar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    Vertical,
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
    /// Thumb / track thickness.
    pub const SB_W: f32 = 6.0;
    /// Margin between track and viewport outer edge.
    const EDGE_PAD: f32 = 2.0;
    const THUMB_MIN: f32 = 18.0;

    /// Layout gutter reserved for this axis when the bar is shown (track + edge pad).
    pub const fn gutter() -> f32 {
        Self::SB_W + Self::EDGE_PAD
    }

    /// Create a new scrollbar for the given orientation (visible by default).
    pub fn new(orientation: ScrollbarOrientation) -> Self {
        Self {
            show: true,
            dragging: false,
            hover: false,
            orientation,
            drag_grab: 0.0,
        }
    }

    // ── Track geometry ──────────────────────────────────────────────────────

    /// Track rectangle in **absolute** coordinates (for rendering).
    pub fn track_rect_abs(&self, abs_frame: Rect) -> Rect {
        match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(
                abs_frame.x + abs_frame.w - Self::gutter(),
                abs_frame.y + Self::EDGE_PAD,
                Self::SB_W,
                (abs_frame.h - Self::EDGE_PAD * 2.0).max(0.0),
            ),
            ScrollbarOrientation::Horizontal => Rect::new(
                abs_frame.x + Self::EDGE_PAD,
                abs_frame.y + abs_frame.h - Self::gutter(),
                (abs_frame.w - Self::EDGE_PAD * 2.0).max(0.0),
                Self::SB_W,
            ),
        }
    }

    /// Track rectangle in **relative** coordinates (for hit-testing with
    /// mouse positions that are relative to the viewport frame).
    pub fn track_rect_rel(&self, frame: Rect) -> Rect {
        match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(
                frame.w - Self::gutter(),
                Self::EDGE_PAD,
                Self::SB_W,
                (frame.h - Self::EDGE_PAD * 2.0).max(0.0),
            ),
            ScrollbarOrientation::Horizontal => Rect::new(
                Self::EDGE_PAD,
                frame.h - Self::gutter(),
                (frame.w - Self::EDGE_PAD * 2.0).max(0.0),
                Self::SB_W,
            ),
        }
    }

    // ── Thumb geometry ──────────────────────────────────────────────────────

    /// Thumb rectangle in **absolute** coordinates (for rendering).
    pub fn thumb_rect_abs(&self, abs_frame: Rect, scroll: f32, max_scroll: f32) -> Option<Rect> {
        if max_scroll <= 0.0 {
            return None;
        }
        let track = self.track_rect_abs(abs_frame);
        let (pos, size) = self.compute_thumb_on_track(track, abs_frame, scroll, max_scroll);
        Some(match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(track.x, pos, Self::SB_W, size),
            ScrollbarOrientation::Horizontal => Rect::new(pos, track.y, size, Self::SB_W),
        })
    }

    /// Thumb rectangle in **relative** coordinates (for hit-testing).
    pub fn thumb_rect_rel(&self, frame: Rect, scroll: f32, max_scroll: f32) -> Option<Rect> {
        if max_scroll <= 0.0 {
            return None;
        }
        let track = self.track_rect_rel(frame);
        let (pos, size) = self.compute_thumb_on_track(track, frame, scroll, max_scroll);
        Some(match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(track.x, pos, Self::SB_W, size),
            ScrollbarOrientation::Horizontal => Rect::new(pos, track.y, size, Self::SB_W),
        })
    }

    /// Compute the thumb's position offset and size along the track.
    fn compute_thumb_on_track(
        &self,
        track: Rect,
        view: Rect,
        scroll: f32,
        max_scroll: f32,
    ) -> (f32, f32) {
        match self.orientation {
            ScrollbarOrientation::Vertical => {
                let ratio = view.h / (view.h + max_scroll);
                let thumb_h = (ratio * track.h).max(Self::THUMB_MIN).min(track.h);
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
                let thumb_w = (ratio * track.w).max(Self::THUMB_MIN).min(track.w);
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

    /// End an active thumb drag.
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.drag_grab = 0.0;
    }

    /// Compute the new scroll value from a drag position in **relative**
    /// coordinates.
    ///
    /// `axis_pos` is the mouse coordinate along the scroll axis
    /// (e.g. `pos.y` for vertical, `pos.x` for horizontal).
    /// Uses [`Self::drag_grab`] from [`Self::begin_drag`] so the thumb tracks
    /// the grab point instead of snapping its leading edge to the pointer.
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
        let track = self.track_rect_rel(frame);
        let (_, thumb_size) = self.compute_thumb_on_track(track, frame, scroll, max_scroll);

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
        let corner = Radius::uniform(3.0);

        // Track background
        let track = self.track_rect_abs(abs_frame);
        ctx.fill_rect(track, ctx.tokens().color_fill_tertiary(), Some(corner));

        // Thumb
        if let Some(thumb) = self.thumb_rect_abs(abs_frame, scroll, max_scroll) {
            let color = if self.hover || self.dragging {
                ctx.tokens().color_fill()
            } else {
                ctx.tokens().color_fill_secondary()
            };
            ctx.fill_rect(thumb, color, Some(corner));
        }
    }
}
