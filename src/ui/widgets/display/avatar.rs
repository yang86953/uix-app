//! Avatar — circular avatar with initials/text.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::image::BitmapHandle;
use crate::draw::painting::PaintContext;
use crate::draw::pipeline::invalidate_paint_handle;
use crate::draw::Color;
use crate::ui::core::paint_scope::current_paint_widget;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

component! {
    pub struct Avatar {
        text: String,
        size: f32,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        square: bool,
        src: String,
        cached: Cell<Option<BitmapHandle>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let primary_bg = ctx.tokens().color_primary_bg();
        let primary = ctx.tokens().color_primary();
        let bg = self.bg_color.unwrap_or(primary_bg);
        let tc = self.text_color.unwrap_or(primary);
        let r = if self.square { Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_sm())) } else { None };

        if self.square {
            ctx.fill_rect(frame, bg, r);
        } else {
            let cx = frame.x + frame.w * 0.5;
            let cy = frame.y + frame.h * 0.5;
            let cr = frame.w.min(frame.h) * 0.5;
            ctx.fill_circle(cx, cy, cr, bg);
        }

        let handle = self.resolve_handle(ctx, tree, frame);
        let drew_image = if let Some(handle) = handle {
            let drawable = if self.square {
                Some(handle)
            } else {
                ctx.image_service().circular_crop(handle)
            };
            if let Some(drawable) = drawable {
                ctx.draw_image_fill(drawable, frame);
                true
            } else {
                false
            }
        } else {
            false
        };

        if !drew_image && !self.text.is_empty() {
            let font_size = self.size * 0.45;
            ctx.text_center(&self.text, frame, tc, font_size);
        }
    }
}

impl Default for Avatar {
    fn default() -> Self {
        Self::new("")
    }
}

impl Avatar {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: 32.0,
            bg_color: None,
            text_color: None,
            square: false,
            src: String::new(),
            cached: Cell::new(None),
        }
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = if s.is_finite() && s > 0.0 { s } else { 32.0 };
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }
    pub fn square(mut self, v: bool) -> Self {
        self.square = v;
        self
    }
    pub fn src(mut self, s: &str) -> Self {
        self.src = s.to_string();
        self.cached.set(None);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.size, self.size)
    }

    fn resolve_handle(
        &self,
        ctx: &PaintContext<'_>,
        tree: &WidgetTree,
        frame: Rect,
    ) -> Option<BitmapHandle> {
        let images = ctx.image_service();
        if let Some(handle) = self.cached.get() {
            if images.is_valid(handle) {
                return Some(handle);
            }
            self.cached.set(None);
        }
        let handle = images.ensure_loaded(&self.src)?;
        self.cached.set(Some(handle));
        if let Some(id) = current_paint_widget() {
            invalidate_paint_handle(&tree.invalidation_handle(), id, Some(frame));
        }
        Some(handle)
    }

    #[cfg(test)]
    pub(crate) fn loaded_handle_for_test(&self) -> Option<BitmapHandle> {
        self.cached.get()
    }
}

impl Avatar {
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.size = next.size;
        self.bg_color = next.bg_color;
        self.text_color = next.text_color;
        self.square = next.square;
        if self.src != next.src {
            self.src = next.src;
            self.cached.set(None);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Avatar {
            text: self.text.clone(),
            size: self.size,
            bg_color: self.bg_color,
            text_color: self.text_color,
            square: self.square,
            src: self.src.clone(),
        }
    }
}
