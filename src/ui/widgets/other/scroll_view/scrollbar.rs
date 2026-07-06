//! ScrollBar — overlay scrollbar for one axis (vertical or horizontal).
//!
//! Used by [`ScrollView`](super::ScrollView) to render draggable scrollbar
//! tracks and thumbs. Each axis gets its own [`ScrollBar`] instance.

use crate::core::{Point, Rect};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
/// Which axis this scrollbar controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    Vertical,
    Horizontal,
}

/// State and rendering for a single-axis overlay scrollbar.
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
}

impl ScrollBar {
    const SB_W: f32 = 6.0;
    const THUMB_MIN: f32 = 18.0;

    /// Create a new scrollbar for the given orientation (visible by default).
    pub fn new(orientation: ScrollbarOrientation) -> Self {
        Self {
            show: true,
            dragging: false,
            hover: false,
            orientation,
        }
    }

    // ── Track geometry ──────────────────────────────────────────────────────

    /// Track rectangle in **absolute** coordinates (for rendering).
    pub fn track_rect_abs(&self, abs_frame: Rect) -> Rect {
        match self.orientation {
            ScrollbarOrientation::Vertical => Rect::new(
                abs_frame.x + abs_frame.w - Self::SB_W - 2.0,
                abs_frame.y + 2.0,
                Self::SB_W,
                abs_frame.h - 4.0,
            ),
            ScrollbarOrientation::Horizontal => Rect::new(
                abs_frame.x + 2.0,
                abs_frame.y + abs_frame.h - Self::SB_W - 2.0,
                abs_frame.w - 4.0,
                Self::SB_W,
            ),
        }
    }

    /// Track rectangle in **relative** coordinates (for hit-testing with
    /// mouse positions that are relative to the viewport frame).
    pub fn track_rect_rel(&self, frame: Rect) -> Rect {
        match self.orientation {
            ScrollbarOrientation::Vertical => {
                Rect::new(frame.w - Self::SB_W - 2.0, 2.0, Self::SB_W, frame.h - 4.0)
            }
            ScrollbarOrientation::Horizontal => {
                Rect::new(2.0, frame.h - Self::SB_W - 2.0, frame.w - 4.0, Self::SB_W)
            }
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

    /// Compute the new scroll value from a drag position in **relative**
    /// coordinates.
    ///
    /// `axis_pos` is the mouse coordinate along the scroll axis
    /// (e.g. `pos.y` for vertical, `pos.x` for horizontal).
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
        ((axis_pos - track_start) / usable).clamp(0.0, 1.0) * max_scroll
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
