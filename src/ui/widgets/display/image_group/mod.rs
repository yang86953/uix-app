//! ImageGroup — a keyboard-friendly image gallery with an in-window preview.

use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, SnapshotFields, SystemEvent,
    WidgetId, WidgetTree,
};
use crate::widget;
// 引入同一 display Module 拥有的图片浮层调色板。
use super::image::presentation::ImageOverlayPalette;

#[derive(Debug)]
struct GalleryGeometry {
    main: Rect,
    previous: Option<Rect>,
    next: Option<Rect>,
    thumbnails: Vec<(usize, Rect)>,
}

#[derive(Debug)]
struct PreviewGeometry {
    preview: Rect,
    previous: Option<Rect>,
    next: Option<Rect>,
    close: Rect,
    thumbnails: Vec<(usize, Rect)>,
}

widget! {
    /// 图片组画廊：展示本地图片、缩略图列表及当前窗口内的模态预览。
    pub struct ImageGroup {
        images: Vec<String>,
        start_index: usize,
        current: Cell<usize>,
        preview_open: bool,
        focused: bool,
        pending_index: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        last_surface_w: Cell<f32>,
        last_surface_h: Cell<f32>,
        /// 关闭预览后的首帧仍需清除上一帧覆盖的整个 surface。
        preview_painted: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(320.0, 220.0))
    }

    tab_index => (&self) -> i32 { i32::from(!self.images.is_empty()) }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        if self.preview_open {
            self.surface_rect(actual_frame)
        } else {
            Self::normalized_frame(actual_frame)
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.pending_index.set(None);
        match event {
            SystemEvent::FocusIn if !self.images.is_empty() => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. } => {
                self.handle_pointer_down(*pos)
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.preview_open => {
                self.close_preview();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. }
                if !self.preview_open && !self.images.is_empty() =>
            {
                self.open_preview();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Left, .. } => {
                if self.select_previous() { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::KeyDown { key: KeyCode::Right, .. } => {
                if self.select_next() { EventResult::Handled } else { EventResult::NotHandled }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<crate::ui::SemanticEvent> {
        self.pending_index
            .take()
            .map(|index| crate::ui::SemanticEvent::change(id, index.to_string()))
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        if self.preview_open || self.preview_painted.get() {
            self.surface_rect(frame)
        } else {
            Self::normalized_frame(frame)
        }
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<OverlayEntry> {
        self.preview_open.then(|| {
            OverlayEntry::new(id, OverlayKind::Modal)
                .bounds(self.surface_rect(frame))
                .z_index(1100)
                .dismiss_on_outside(false)
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let surface_size = ctx.surface_size();
        let surface_w = surface_size.w;
        let surface_h = surface_size.h;
        self.last_surface_w.set(surface_w);
        self.last_surface_h.set(surface_h);

        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(frame));
        self.paint_gallery(frame, ctx, tree);

        if self.preview_open {
            self.paint_preview(ctx, Rect::new(0.0, 0.0, surface_w, surface_h));
            self.preview_painted.set(true);
        } else {
            self.preview_painted.set(false);
        }
    }
}

// 把画廊交互、资源绘制与预览内核融合为 UIX 声明的单一根节点。
fn build_image_group_view(kernel: ImageGroup) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for ImageGroup {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占索引、输入、预览与资源绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/image_group/image_group.uix")
    }
}

impl Default for ImageGroup {
    fn default() -> Self {
        Self::new()
    }
}

// 集中验证 UIX 声明根保持 ImageGroup 内核与公开配置。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/image_group_tests.rs"]
mod tests;

impl ImageGroup {
    const INLINE_THUMB_STRIP_HEIGHT: f32 = 56.0;
    const PREVIEW_THUMB_STRIP_HEIGHT: f32 = 72.0;

    /// 创建没有图片、起始索引为零且预览关闭的图片组。
    pub fn new() -> Self {
        Self {
            images: Vec::new(),
            start_index: 0,
            current: Cell::new(0),
            preview_open: false,
            focused: false,
            pending_index: Cell::new(None),
            last_frame: Cell::new(None),
            last_surface_w: Cell::new(0.0),
            last_surface_h: Cell::new(0.0),
            preview_painted: Cell::new(false),
        }
    }

    // 图片编解码 capability 启用时才公开路径集合入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后 ImageGroup 不接受无法解码的来源。
    /// 替换图片路径集合，并按起始索引夹紧当前项；空集合会关闭预览。
    pub fn images<I, S>(mut self, images: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.images = images.into_iter().map(Into::into).collect();
        self.current
            .set(self.start_index.min(self.images.len().saturating_sub(1)));
        if self.images.is_empty() {
            self.preview_open = false;
        }
        self
    }

    /// 设置初始图片索引，并按当前图片数量夹紧当前项。
    pub fn start_index(mut self, index: usize) -> Self {
        self.start_index = index;
        self.current
            .set(index.min(self.images.len().saturating_sub(1)));
        self
    }

    /// 返回夹紧到当前图片集合的索引；空集合返回零。
    pub fn current_index(&self) -> usize {
        self.current.get().min(self.images.len().saturating_sub(1))
    }

    /// 返回模态画廊预览当前是否处于逻辑打开状态。
    pub fn is_preview_open(&self) -> bool {
        self.preview_open
    }

    /// 打开当前窗口内的模态画廊预览；空图片组无操作。
    pub fn open_preview(&mut self) {
        if !self.images.is_empty() {
            self.preview_open = true;
        }
    }

    /// 关闭当前窗口内的模态画廊预览。
    pub fn close_preview(&mut self) {
        self.preview_open = false;
    }

    fn handle_pointer_down(&mut self, pos: Point) -> EventResult {
        let Some(frame) = self.last_frame.get() else {
            return EventResult::NotHandled;
        };
        if self.images.is_empty() {
            return EventResult::NotHandled;
        }

        if self.preview_open {
            let surface = self.surface_rect(frame);
            let geometry = Self::preview_geometry(surface, self.images.len(), self.current_index());
            let surface_pos = Point::new(pos.x + frame.x, pos.y + frame.y);

            if geometry.close.contains(surface_pos) {
                self.close_preview();
                return EventResult::Handled;
            }
            if geometry
                .previous
                .is_some_and(|rect| rect.contains(surface_pos))
            {
                let _ = self.select_previous();
                return EventResult::Handled;
            }
            if geometry.next.is_some_and(|rect| rect.contains(surface_pos)) {
                let _ = self.select_next();
                return EventResult::Handled;
            }
            if let Some((index, _)) = geometry
                .thumbnails
                .iter()
                .find(|(_, rect)| rect.contains(surface_pos))
            {
                let _ = self.select_index(*index);
                return EventResult::Handled;
            }
            if !geometry.preview.contains(surface_pos) {
                self.close_preview();
            }
            // 模态预览内的任意点击都由所属 ImageGroup 消费，避免穿透到底层。
            return EventResult::Handled;
        }

        let local_frame = Rect::new(0.0, 0.0, frame.w, frame.h);
        let geometry = Self::gallery_geometry(local_frame, self.images.len(), self.current_index());
        if geometry.previous.is_some_and(|rect| rect.contains(pos)) {
            let _ = self.select_previous();
            return EventResult::Handled;
        }
        if geometry.next.is_some_and(|rect| rect.contains(pos)) {
            let _ = self.select_next();
            return EventResult::Handled;
        }
        if let Some((index, _)) = geometry
            .thumbnails
            .iter()
            .find(|(_, rect)| rect.contains(pos))
        {
            let _ = self.select_index(*index);
            return EventResult::Handled;
        }
        if geometry.main.contains(pos) {
            self.open_preview();
            return EventResult::Handled;
        }
        EventResult::NotHandled
    }

    fn select_index(&self, index: usize) -> bool {
        if index >= self.images.len() || index == self.current_index() {
            return false;
        }
        self.current.set(index);
        self.pending_index.set(Some(index));
        true
    }

    fn select_previous(&self) -> bool {
        let len = self.images.len();
        if len <= 1 {
            return false;
        }
        let next = (self.current_index() + len - 1) % len;
        self.current.set(next);
        self.pending_index.set(Some(next));
        true
    }

    fn select_next(&self) -> bool {
        let len = self.images.len();
        if len <= 1 {
            return false;
        }
        let next = (self.current_index() + 1) % len;
        self.current.set(next);
        self.pending_index.set(Some(next));
        true
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            if frame.x.is_finite() { frame.x } else { 0.0 },
            if frame.y.is_finite() { frame.y } else { 0.0 },
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn surface_rect(&self, fallback: Rect) -> Rect {
        let w = self.last_surface_w.get();
        let h = self.last_surface_h.get();
        if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
            Rect::new(0.0, 0.0, w, h)
        } else {
            Self::normalized_frame(fallback)
        }
    }

    fn gallery_geometry(frame: Rect, image_count: usize, current: usize) -> GalleryGeometry {
        let strip_height = if image_count > 1 && frame.h >= 72.0 && frame.w >= 24.0 {
            Self::INLINE_THUMB_STRIP_HEIGHT.min(frame.h * 0.4)
        } else {
            0.0
        };
        let main = Rect::new(frame.x, frame.y, frame.w, (frame.h - strip_height).max(0.0));
        let strip = Rect::new(frame.x, frame.y + main.h, frame.w, strip_height);
        let navigation_width = 36.0_f32.min(main.w / 3.0).min(main.h);
        let navigation = image_count > 1 && navigation_width >= 4.0;
        let previous_slot = Rect::new(main.x, main.y, navigation_width, main.h);
        let next_slot = Rect::new(
            main.x + main.w - navigation_width,
            main.y,
            navigation_width,
            main.h,
        );
        GalleryGeometry {
            main,
            previous: navigation.then(|| Self::navigation_control_rect(previous_slot)),
            next: navigation.then(|| Self::navigation_control_rect(next_slot)),
            thumbnails: Self::thumbnail_rects(strip, image_count, current, 44.0),
        }
    }

    fn preview_geometry(surface: Rect, image_count: usize, current: usize) -> PreviewGeometry {
        let thumbnail_height = if image_count > 1 && surface.h >= 120.0 && surface.w >= 32.0 {
            Self::PREVIEW_THUMB_STRIP_HEIGHT.min(surface.h * 0.25)
        } else {
            0.0
        };
        let content_height = (surface.h - thumbnail_height).max(0.0);
        let shortest = surface.w.min(content_height).max(0.0);
        let margin = (shortest * 0.06).clamp(4.0, 40.0);
        let preview = Rect::new(
            surface.x + margin,
            surface.y + margin,
            (surface.w - margin * 2.0).max(0.0),
            (content_height - margin * 2.0).max(0.0),
        );
        let navigation_width = 48.0_f32.min(surface.w / 3.0).min(content_height);
        let navigation = image_count > 1 && navigation_width >= 4.0;
        let close_size = 40.0_f32.min(surface.w).min(surface.h).max(0.0);
        let close_margin = 12.0_f32
            .min((surface.w - close_size).max(0.0) * 0.5)
            .min((surface.h - close_size).max(0.0) * 0.5);
        let strip = Rect::new(
            surface.x,
            surface.y + content_height,
            surface.w,
            thumbnail_height,
        );
        let previous_slot = Rect::new(surface.x, surface.y, navigation_width, content_height);
        let next_slot = Rect::new(
            surface.x + surface.w - navigation_width,
            surface.y,
            navigation_width,
            content_height,
        );
        PreviewGeometry {
            preview,
            previous: navigation.then(|| Self::navigation_control_rect(previous_slot)),
            next: navigation.then(|| Self::navigation_control_rect(next_slot)),
            close: Rect::new(
                surface.x + surface.w - close_size - close_margin,
                surface.y + close_margin,
                close_size,
                close_size,
            ),
            thumbnails: Self::thumbnail_rects(strip, image_count, current, 52.0),
        }
    }

    fn thumbnail_rects(
        strip: Rect,
        image_count: usize,
        current: usize,
        maximum_size: f32,
    ) -> Vec<(usize, Rect)> {
        if image_count == 0 || strip.w <= 0.0 || strip.h <= 0.0 {
            return Vec::new();
        }
        let padding = 6.0_f32.min(strip.h * 0.2);
        let size = maximum_size
            .min((strip.h - padding * 2.0).max(0.0))
            .min(strip.w);
        if size < 4.0 {
            return Vec::new();
        }
        let gap = 6.0_f32.min(size * 0.25);
        let capacity = (((strip.w + gap) / (size + gap)).floor() as usize)
            .max(1)
            .min(image_count);
        let start = current
            .saturating_sub(capacity / 2)
            .min(image_count - capacity);
        let total_width = size * capacity as f32 + gap * capacity.saturating_sub(1) as f32;
        let x = strip.x + (strip.w - total_width).max(0.0) * 0.5;
        let y = strip.y + (strip.h - size).max(0.0) * 0.5;
        (0..capacity)
            .map(|offset| {
                let index = start + offset;
                (
                    index,
                    Rect::new(x + offset as f32 * (size + gap), y, size, size),
                )
            })
            .collect()
    }

    fn navigation_control_rect(slot: Rect) -> Rect {
        let diameter = 30.0_f32.min(slot.w).min(slot.h).max(0.0);
        Rect::new(
            slot.x + (slot.w - diameter) * 0.5,
            slot.y + (slot.h - diameter) * 0.5,
            diameter,
            diameter,
        )
    }

    fn paint_gallery(&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let radius = ctx.tokens().border_radius().min(frame.w.min(frame.h) * 0.5);
        ctx.push_clip(frame);
        ctx.fill_rect(
            frame,
            ctx.tokens().color_bg_container(),
            Some(Radius::uniform(radius)),
        );
        ctx.stroke_rect(
            frame,
            ctx.tokens().color_border_secondary(),
            1.0,
            Some(Radius::uniform(radius)),
        );

        let geometry = Self::gallery_geometry(frame, self.images.len(), self.current_index());
        if let Some(path) = self.images.get(self.current_index()) {
            self.paint_image(ctx, path, geometry.main, true, "图片加载中…");
        } else {
            // 空状态说明使用主题派生的紧凑字号。
            let compact_font = ImageOverlayPalette::resolve(ctx.tokens()).compact_font;
            ctx.text_center(
                "暂无图片",
                geometry.main,
                ctx.tokens().color_text_secondary(),
                compact_font,
            );
        }
        self.paint_controls(ctx, &geometry);

        if self.focused && tree.keyboard_focus_visible() {
            let inset = 1.0_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                ctx.tokens().color_primary(),
                2.0,
                Some(Radius::uniform(radius)),
            );
        }
        ctx.pop_clip();
    }

    fn paint_controls(&self, ctx: &mut PaintContext, geometry: &GalleryGeometry) {
        let text_color = ctx.tokens().color_text();
        if let Some(previous) = geometry.previous {
            Self::paint_navigation_control(ctx, previous, "chevron-left", text_color);
        }
        if let Some(next) = geometry.next {
            Self::paint_navigation_control(ctx, next, "chevron-right", text_color);
        }
        for &(index, rect) in &geometry.thumbnails {
            if let Some(path) = self.images.get(index) {
                self.paint_image(ctx, path, rect, false, "");
            }
            ctx.stroke_rect(
                rect,
                if index == self.current_index() {
                    ctx.tokens().color_primary()
                } else {
                    ctx.tokens().color_border_secondary()
                },
                if index == self.current_index() {
                    2.0
                } else {
                    1.0
                },
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
    }

    fn paint_preview(&self, ctx: &mut PaintContext, surface: Rect) {
        let surface = Self::normalized_frame(surface);
        if surface.w <= 0.0 || surface.h <= 0.0 || self.images.is_empty() {
            return;
        }
        let geometry = Self::preview_geometry(surface, self.images.len(), self.current_index());
        // 在组件主题作用域内解析一次预览调色板。
        let palette = ImageOverlayPalette::resolve(ctx.tokens());
        ctx.push_clip(surface);
        // 全窗口背景使用主题语义遮罩。
        ctx.fill_rect(surface, palette.mask, None);
        if geometry.preview.w > 0.0 && geometry.preview.h > 0.0 {
            // 预览占位区域使用主题浮层表面。
            ctx.fill_rect(
                geometry.preview,
                palette.surface,
                Some(Radius::uniform(4.0)),
            );
            if let Some(path) = self.images.get(self.current_index()) {
                self.paint_image(ctx, path, geometry.preview, true, "图片不可用");
            }
        }

        if let Some(previous) = geometry.previous {
            // 导航箭头使用浮层正文前景。
            Self::paint_navigation_control(ctx, previous, "chevron-left", palette.foreground);
        }
        if let Some(next) = geometry.next {
            // 导航箭头使用浮层正文前景。
            Self::paint_navigation_control(ctx, next, "chevron-right", palette.foreground);
        }
        for &(index, rect) in &geometry.thumbnails {
            if let Some(path) = self.images.get(index) {
                self.paint_image(ctx, path, rect, false, "");
            }
            ctx.stroke_rect(
                rect,
                if index == self.current_index() {
                    ctx.tokens().color_primary()
                } else {
                    // 未选缩略图使用主题浮层弱边界。
                    palette.border
                },
                if index == self.current_index() {
                    2.0
                } else {
                    1.0
                },
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }

        if geometry.close.w > 0.0 && geometry.close.h > 0.0 {
            ctx.fill_circle(
                geometry.close.x + geometry.close.w * 0.5,
                geometry.close.y + geometry.close.h * 0.5,
                geometry.close.w.min(geometry.close.h) * 0.5,
                // 关闭按钮使用主题浮层表面。
                palette.surface,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "x",
                geometry.close,
                // 关闭图标使用主题正文前景。
                palette.foreground,
                20.0_f32.min(geometry.close.w.min(geometry.close.h) * 0.6),
            );
        }
        let counter = Rect::new(
            surface.x,
            surface.y + 8.0,
            surface.w,
            24.0_f32.min(surface.h),
        );
        ctx.text_center(
            &format!("{} / {}", self.current_index() + 1, self.images.len()),
            counter,
            // 计数文本使用主题正文前景。
            palette.foreground,
            // 使用主题派生的紧凑字号。
            palette.compact_font,
        );
        ctx.pop_clip();
    }

    fn paint_image(
        &self,
        ctx: &mut PaintContext,
        path: &str,
        frame: Rect,
        fit: bool,
        unavailable_label: &str,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        ctx.fill_rect(frame, ctx.tokens().color_fill_tertiary(), None);
        // 图片编解码 capability 启用时才解析画廊中的文件路径。
        #[cfg(feature = "image-codecs")]
        // 能力开启时从应用级缓存取得位图句柄。
        let handle = ctx.image_service().ensure_loaded(path);
        // 图片编解码 capability 关闭时统一进入不可用占位分支。
        #[cfg(not(feature = "image-codecs"))]
        // 显式类型保持后续绘制分支的句柄契约。
        let handle: Option<crate::draw::resources::image::BitmapHandle> = None;
        // 图片编解码 capability 关闭时路径不会进入文件加载。
        #[cfg(not(feature = "image-codecs"))]
        // 显式消费路径参数以保持无默认构建零新增警告。
        let _ = path;
        // 有效句柄继续沿用原有 fit 与 fill 绘制行为。
        if let Some(handle) = handle {
            if fit {
                ctx.draw_image(handle, frame);
            } else {
                ctx.draw_image_fill(handle, frame);
            }
        } else if !unavailable_label.is_empty() {
            // 不可用说明复用主题派生的紧凑字号。
            let compact_font = ImageOverlayPalette::resolve(ctx.tokens()).compact_font;
            ctx.text_center(
                unavailable_label,
                frame,
                ctx.tokens().color_text_secondary(),
                compact_font.min(frame.h.max(1.0)),
            );
        } else {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "image",
                frame,
                ctx.tokens().color_text_secondary(),
                16.0_f32.min(frame.w.min(frame.h) * 0.5),
            );
        }
    }

    fn paint_navigation_control(ctx: &mut PaintContext, frame: Rect, icon: &str, color: Color) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let diameter = 30.0_f32.min(frame.w).min(frame.h);
        let circle = Rect::new(
            frame.x + (frame.w - diameter) * 0.5,
            frame.y + (frame.h - diameter) * 0.5,
            diameter,
            diameter,
        );
        ctx.fill_circle(
            circle.x + circle.w * 0.5,
            circle.y + circle.h * 0.5,
            diameter * 0.5,
            // 导航按钮使用主题浮层表面。
            ctx.tokens().color_bg_overlay(),
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            icon,
            circle,
            color,
            16.0_f32.min(diameter * 0.6),
        );
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ImageGroup {
            images: self.images.clone(),
            start_index: self.start_index,
            current: self.current_index(),
            preview_open: self.preview_open,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let previous_index = self.current_index();
        let selected_identity = self.images.get(previous_index).map(|selected| {
            let occurrence = self.images[..=previous_index]
                .iter()
                .filter(|candidate| *candidate == selected)
                .count()
                .saturating_sub(1);
            (selected.clone(), occurrence)
        });
        self.images = next.images;
        self.start_index = next.start_index;
        let reconciled_index = selected_identity
            .as_ref()
            .and_then(|(path, occurrence)| {
                self.images
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| *candidate == path)
                    .nth(*occurrence)
                    .map(|(index, _)| index)
            })
            .unwrap_or_else(|| previous_index.min(self.images.len().saturating_sub(1)));
        self.current.set(reconciled_index);
        if self.images.is_empty() {
            self.preview_open = false;
            self.focused = false;
        }
        self.pending_index.set(None);
    }
}
