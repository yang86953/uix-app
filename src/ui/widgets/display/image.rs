//! Image widget — 图片组件，Ant Design 风格。
//!
//! 支持占位图、fallback、描述、圆角与文件路径加载。

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::image::BitmapHandle;
use crate::draw::painting::PaintContext;
use crate::draw::pipeline::invalidate_paint_handle;
use crate::draw::{Color, Radius};
use crate::ui::core::paint_scope::current_paint_widget;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

// Image — 图片显示组件。
component! {
    pub struct Image {
        src: String,
        alt: String,
        fallback: String,
        width: f32,
        height: f32,
        radius: f32,
        preview: bool,
        /// 预加载位图句柄（优先于 src 懒加载）。
        slot: Option<BitmapHandle>,
        /// src 首次加载成功后的句柄缓存，避免每帧查表。
        cached: Cell<Option<BitmapHandle>>,
        /// fit 模式：true=保持比例居中，false=拉伸填满。
        fit: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let fill = ctx.tokens().color_fill_tertiary();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(self.radius));

        let handle = self.resolve_handle(ctx, tree, frame);
        let drew = if let Some(h) = handle {
            ctx.fill_rect(frame, fill, r);
            if self.fit {
                ctx.draw_image(h, frame);
            } else {
                ctx.draw_image_fill(h, frame);
            }
            true
        } else {
            false
        };

        if !drew {
            // 占位/错误态：灰色背景 + 图标
            ctx.fill_rect(frame, fill, r);
            ctx.stroke_rect(frame, ctx.tokens().color_border_secondary(), 1.0, r);
            let load_failed = !self.src.is_empty() || self.slot.is_some();
            let placeholder = if load_failed && !self.fallback.is_empty() {
                &self.fallback
            } else if !self.alt.is_empty() {
                &self.alt
            } else {
                "🖼"
            };
            ctx.text_center(placeholder, frame, text_sec, if load_failed { 13.0 } else { 24.0 });
        }

        // 预览图标叠加
        if self.preview && drew {
            let preview_icon = "🔍";
            let icon_size = 20.0;
            let icon_x = frame.x + frame.w - icon_size - 8.0;
            let icon_y = frame.y + 8.0;
            ctx.fill_circle(icon_x + icon_size * 0.5, icon_y + icon_size * 0.5, icon_size * 0.5, Color::from_rgba(0, 0, 0, 120));
            ctx.draw_text(preview_icon, Point::new(icon_x + 2.0, icon_y + 2.0), Color::white(), 14.0);
        }

        // 描述文字（底部）
        if !self.alt.is_empty() && drew {
            ctx.draw_text(&self.alt, Point::new(frame.x + 4.0, frame.y + frame.h + 4.0), text_sec, 11.0);
        }
    }
}

impl Image {
    pub fn new(w: f32, h: f32) -> Self {
        Self {
            src: String::new(),
            alt: String::new(),
            fallback: String::new(),
            width: w,
            height: h,
            radius: 6.0,
            preview: true,
            slot: None,
            cached: Cell::new(None),
            fit: true,
        }
    }

    /// 设置图片文件路径（渲染时懒加载）。
    pub fn src(mut self, path: impl Into<String>) -> Self {
        self.src = path.into();
        self.cached.set(None);
        self
    }

    /// 绑定已预加载的位图句柄。
    pub fn slot(mut self, handle: BitmapHandle) -> Self {
        self.slot = Some(handle);
        self.cached.set(None);
        self
    }

    pub fn alt(mut self, a: &str) -> Self {
        self.alt = a.to_string();
        self
    }
    pub fn fallback(mut self, f: &str) -> Self {
        self.fallback = f.to_string();
        self
    }
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    pub fn preview(mut self, v: bool) -> Self {
        self.preview = v;
        self
    }
    /// 保持宽高比居中（默认 true）；false 则拉伸填满。
    pub fn fit(mut self, v: bool) -> Self {
        self.fit = v;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Image {
            src: self.src.clone(),
            alt: self.alt.clone(),
            fallback: self.fallback.clone(),
            width: self.width,
            height: self.height,
            radius: self.radius,
            preview: self.preview,
            fit: self.fit,
        }
    }

    fn resolve_handle(
        &self,
        ctx: &PaintContext<'_>,
        tree: &WidgetTree,
        frame: Rect,
    ) -> Option<BitmapHandle> {
        let svc = ctx.image_service();

        if let Some(h) = self.slot {
            if svc.is_valid(h) {
                return Some(h);
            }
        }

        if let Some(h) = self.cached.get() {
            if svc.is_valid(h) {
                return Some(h);
            }
            self.cached.set(None);
        }

        if !self.src.is_empty() {
            if let Some(h) = svc.ensure_loaded(&self.src) {
                let first_load = self.cached.get().is_none();
                self.cached.set(Some(h));
                if first_load {
                    if let Some(id) = current_paint_widget() {
                        invalidate_paint_handle(&tree.invalidation_handle(), id, Some(frame));
                    }
                }
                return Some(h);
            }
        }

        None
    }
}
