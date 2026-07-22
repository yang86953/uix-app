//! Image widget — 图片组件，Ant Design 风格。
//!
//! 支持占位图、fallback、描述、圆角与文件路径加载。

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::image::BitmapHandle;
use crate::draw::painting::{PaintContext, PaintPass};
use crate::draw::pipeline::Invalidation;
use crate::draw::{Color, Radius};
use crate::ui::core::paint_scope::current_paint_widget;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, SystemEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImageLoadState {
    Empty,
    Deferred,
    Pending,
    Loading,
    Ready,
    Error(String),
}

impl ImageLoadState {
    fn shows_placeholder(&self) -> bool {
        matches!(
            self,
            Self::Empty | Self::Deferred | Self::Pending | Self::Loading
        )
    }

    fn error(&self) -> Option<&str> {
        match self {
            Self::Error(error) => Some(error),
            _ => None,
        }
    }
}

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
        /// 延迟到组件进入可见绘制路径后加载；Image 的绘制本身已是按需加载。
        lazy: bool,
        placeholder_enabled: bool,
        error_handler_enabled: bool,
        #[snapshot(skip)]
        placeholder_view: RefCell<Option<crate::ui::view::ViewNode>>,
        #[snapshot(skip)]
        error_view_factory: Option<Rc<dyn Fn(&str) -> crate::ui::view::ViewNode>>,
        #[snapshot(skip)]
        load_state: RefCell<ImageLoadState>,
        #[snapshot(skip)]
        error_child_materialized: Cell<bool>,
        preview_open: bool,
        focused: bool,
        last_surface_w: Cell<f32>,
        last_surface_h: Cell<f32>,
        /// 关闭预览后的首帧仍需清除上一帧覆盖的整个 surface。
        preview_painted: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> { Some(frame) }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        self.placeholder_view
            .borrow_mut()
            .take()
            .map(|view| view.key(Self::PLACEHOLDER_CHILD_KEY))
            .into_iter()
            .collect()
    }

    child_visible => (&self, index: usize) -> bool {
        let state = self.load_state.borrow();
        if self.placeholder_enabled && index == 0 {
            return state.shows_placeholder();
        }
        if self.error_handler_enabled && index == usize::from(self.placeholder_enabled) {
            return state.error().is_some();
        }
        true
    }

    tab_index => (&self) -> i32 { i32::from(self.preview) }

    hit_test_frame => (&self, actual_frame: Rect) -> Rect {
        if self.preview_open {
            self.surface_rect(actual_frame)
        } else {
            actual_frame
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.preview {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::PointerDown { button: MouseButton::Left, .. } => {
                if self.preview_open {
                    self.close_preview();
                } else {
                    self.open_preview();
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Enter | KeyCode::Space, .. } => {
                if self.preview_open {
                    self.close_preview();
                } else {
                    self.open_preview();
                }
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. } if self.preview_open => {
                self.close_preview();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        if self.preview_open || self.preview_painted.get() {
            self.surface_rect(frame)
        } else {
            frame
        }
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<OverlayEntry> {
        self.preview_open.then(|| {
            OverlayEntry::new(id, OverlayKind::Modal)
                .bounds(self.surface_rect(frame))
                .z_index(1100)
                .dismiss_on_outside(false)
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let has_custom_content = self.placeholder_enabled || self.error_handler_enabled;
        let pass = ctx.paint_pass();
        if pass != PaintPass::Content
            && !(has_custom_content && pass == PaintPass::AfterChildren)
        {
            return;
        }

        let (surface_w, surface_h) = {
            let canvas = ctx.canvas_2d();
            (canvas.width() as f32, canvas.height() as f32)
        };
        self.last_surface_w.set(surface_w);
        self.last_surface_h.set(surface_h);
        let frame = Self::normalized_frame(frame);
        let radius = self.radius.min(frame.w.min(frame.h) * 0.5);
        let handle = if pass == PaintPass::Content {
            let handle = self.resolve_handle(ctx, tree, frame);
            self.render_thumbnail(frame, radius, handle, ctx);
            handle
        } else {
            self.valid_handle(ctx.image_service())
        };

        if pass == PaintPass::Content && has_custom_content {
            return;
        }

        self.render_adornments(
            frame,
            radius,
            handle,
            surface_w,
            surface_h,
            ctx,
            tree,
        );
    }
}

impl Image {
    const PLACEHOLDER_PADDING: f32 = 8.0;
    const PLACEHOLDER_CHILD_KEY: &'static str = "uix:image:placeholder";
    const ERROR_CHILD_KEY: &'static str = "uix:image:error";

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
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

    pub fn new(w: f32, h: f32) -> Self {
        Self {
            src: String::new(),
            alt: String::new(),
            fallback: String::new(),
            width: Self::finite_non_negative(w),
            height: Self::finite_non_negative(h),
            radius: 6.0,
            preview: true,
            slot: None,
            cached: Cell::new(None),
            fit: true,
            lazy: false,
            placeholder_enabled: false,
            error_handler_enabled: false,
            placeholder_view: RefCell::new(None),
            error_view_factory: None,
            load_state: RefCell::new(ImageLoadState::Empty),
            error_child_materialized: Cell::new(false),
            preview_open: false,
            focused: false,
            last_surface_w: Cell::new(0.0),
            last_surface_h: Cell::new(0.0),
            preview_painted: Cell::new(false),
        }
    }

    /// 设置图片文件路径（渲染时懒加载）。
    pub fn src(mut self, path: impl Into<String>) -> Self {
        self.src = path.into();
        self.cached.set(None);
        self.reset_load_state();
        self
    }

    /// 绑定已预加载的位图句柄。
    pub fn slot(mut self, handle: BitmapHandle) -> Self {
        self.slot = Some(handle);
        self.cached.set(None);
        self.reset_load_state();
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
        self.radius = Self::finite_non_negative(r);
        self
    }
    pub fn preview(mut self, v: bool) -> Self {
        self.preview = v;
        if !v {
            self.preview_open = false;
        }
        self
    }
    /// 保持宽高比居中（默认 true）；false 则拉伸填满。
    pub fn fit(mut self, v: bool) -> Self {
        self.fit = v;
        self
    }

    /// 延迟到图片与当前绘制视口相交时才解析资源。
    pub fn lazy(mut self, v: bool) -> Self {
        self.lazy = v;
        self.reset_load_state();
        self
    }

    /// 设置图片完成加载前显示的占位 View。
    pub fn placeholder<V: crate::ui::view::View>(mut self, view: V) -> Self {
        self.placeholder_enabled = true;
        self.placeholder_view
            .replace(Some(crate::ui::view::View::build(view)));
        self
    }

    /// 设置加载失败后的 View 工厂。工厂接收图片服务返回的具体失败原因。
    pub fn on_error<F, V>(mut self, factory: F) -> Self
    where
        F: Fn(&str) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.error_handler_enabled = true;
        self.error_view_factory = Some(Rc::new(move |error| {
            crate::ui::view::View::build(factory(error))
        }));
        self
    }

    pub fn is_preview_open(&self) -> bool {
        self.preview_open
    }

    /// 打开当前窗口内的模态图片预览；禁用 preview 时无操作。
    pub fn open_preview(&mut self) {
        if self.preview {
            self.preview_open = true;
        }
    }

    pub fn close_preview(&mut self) {
        self.preview_open = false;
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    fn finite_non_negative(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn inset(frame: Rect, amount: f32) -> Rect {
        let amount = amount.min(frame.w * 0.5).min(frame.h * 0.5).max(0.0);
        Rect::new(
            frame.x + amount,
            frame.y + amount,
            (frame.w - amount * 2.0).max(0.0),
            (frame.h - amount * 2.0).max(0.0),
        )
    }

    fn initial_load_state(&self) -> ImageLoadState {
        if self.src.is_empty() && self.slot.is_none() {
            ImageLoadState::Empty
        } else if self.lazy {
            ImageLoadState::Deferred
        } else {
            ImageLoadState::Pending
        }
    }

    fn reset_load_state(&mut self) {
        self.load_state.replace(self.initial_load_state());
        self.error_child_materialized.set(false);
    }

    fn frame_is_visible(ctx: &mut PaintContext<'_>, frame: Rect) -> bool {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return false;
        }
        let canvas = ctx.canvas_2d();
        let (offset_x, offset_y) = canvas.offset();
        let surface_frame = canvas.current_transform().transform_rect(Rect::new(
            frame.x + offset_x,
            frame.y + offset_y,
            frame.w,
            frame.h,
        ));
        surface_frame
            .intersect(&canvas.current_clip())
            .is_some_and(|visible| visible.w > 0.0 && visible.h > 0.0)
    }

    fn valid_handle(
        &self,
        image_service: &crate::draw::image::ImageService,
    ) -> Option<BitmapHandle> {
        if let Some(handle) = self.slot {
            if image_service.is_valid(handle) {
                return Some(handle);
            }
        }
        if let Some(handle) = self.cached.get() {
            if image_service.is_valid(handle) {
                return Some(handle);
            }
            self.cached.set(None);
        }
        None
    }

    fn set_load_state(&self, next: ImageLoadState, tree: &WidgetTree, frame: Rect) {
        let previous = self.load_state.borrow().clone();
        if previous == next {
            return;
        }
        let needs_follow_up = matches!(&next, ImageLoadState::Loading)
            || (self.placeholder_enabled
                && previous.shows_placeholder() != next.shows_placeholder())
            || (self.error_handler_enabled
                && !self.error_child_materialized.get()
                && matches!(&next, ImageLoadState::Error(_)));
        self.load_state.replace(next);
        if !needs_follow_up {
            return;
        }
        if let Some(id) = current_paint_widget() {
            let invalidation = tree.invalidation_handle();
            if let Ok(mut queue) = invalidation.lock() {
                queue.extend([
                    Invalidation::Layout(id),
                    Invalidation::Paint {
                        id,
                        rect: Some(frame),
                    },
                ]);
            };
        }
    }

    fn custom_content_active(&self) -> bool {
        match &*self.load_state.borrow() {
            ImageLoadState::Ready => false,
            ImageLoadState::Error(_) => {
                self.error_handler_enabled && self.error_child_materialized.get()
            }
            ImageLoadState::Empty
            | ImageLoadState::Deferred
            | ImageLoadState::Pending
            | ImageLoadState::Loading => self.placeholder_enabled,
        }
    }

    fn render_thumbnail(
        &self,
        frame: Rect,
        radius: f32,
        handle: Option<BitmapHandle>,
        ctx: &mut PaintContext<'_>,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }

        let fill = ctx.tokens().color_fill_tertiary();
        let text_secondary = ctx.tokens().color_text_secondary();
        let rounded = Some(Radius::uniform(radius));
        ctx.push_clip(frame);
        if let Some(handle) = handle {
            ctx.fill_rect(frame, fill, rounded);
            let device_scale = ctx.device_pixel_ratio().max(f32::EPSILON);
            let target_width = (frame.w * device_scale).ceil().clamp(1.0, 4096.0) as u32;
            let target_height = (frame.h * device_scale).ceil().clamp(1.0, 4096.0) as u32;
            if let Some(drawable) = ctx.image_service().rounded_rect_sized(
                handle,
                target_width,
                target_height,
                radius * device_scale,
                self.fit,
            ) {
                ctx.draw_image_fill(drawable, frame);
            } else if self.fit {
                ctx.draw_image(handle, frame);
            } else {
                ctx.draw_image_fill(handle, frame);
            }
        } else {
            ctx.fill_rect(frame, fill, rounded);
            ctx.stroke_rect(frame, ctx.tokens().color_border_secondary(), 1.0, rounded);
            if !self.custom_content_active() {
                let state = self.load_state.borrow();
                let label = if state.error().is_some() && !self.fallback.is_empty() {
                    self.fallback.as_str()
                } else if state.error().is_some() && self.error_handler_enabled {
                    "加载失败"
                } else if !self.alt.is_empty() {
                    self.alt.as_str()
                } else {
                    ""
                };
                if label.is_empty() {
                    let icon_size = 24.0_f32.min(frame.w.min(frame.h) * 0.45);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        "image",
                        frame,
                        text_secondary,
                        icon_size,
                    );
                } else {
                    Self::paint_centered_label(ctx, label, frame, text_secondary, 13.0);
                }
            }
        }
        ctx.pop_clip();
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "paint state is kept explicit at the Image render boundary"
    )]
    fn render_adornments(
        &self,
        frame: Rect,
        radius: f32,
        handle: Option<BitmapHandle>,
        surface_w: f32,
        surface_h: f32,
        ctx: &mut PaintContext<'_>,
        tree: &WidgetTree,
    ) {
        if frame.w > 0.0 && frame.h > 0.0 {
            ctx.push_clip(frame);
            if self.preview && handle.is_some() {
                Self::paint_preview_indicator(ctx, frame);
            }
            if self.focused && tree.keyboard_focus_visible() && self.preview {
                let focus = Self::inset(frame, 1.0);
                ctx.stroke_rect(
                    focus,
                    ctx.tokens().color_primary(),
                    2.0,
                    Some(Radius::uniform(radius.min(focus.w.min(focus.h) * 0.5))),
                );
            }
            ctx.pop_clip();
        }

        if self.preview_open {
            self.render_preview(ctx, handle, surface_w, surface_h);
            self.preview_painted.set(true);
        } else {
            self.preview_painted.set(false);
        }
    }

    fn paint_centered_label(
        ctx: &mut PaintContext<'_>,
        label: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        let content = Self::inset(frame, Self::PLACEHOLDER_PADDING);
        if content.w <= 0.0 || content.h <= 0.0 {
            return;
        }
        let metrics =
            crate::draw::font::text_backend::estimate_text_metrics(label, content.w, font_size);
        let line_height = font_size * 1.5;
        let label_height = (metrics.line_count.max(1) as f32 * line_height).min(content.h);
        let label_frame = Rect::new(
            content.x,
            content.y + (content.h - label_height) * 0.5,
            content.w,
            label_height,
        );
        ctx.push_clip(label_frame);
        if metrics.line_count <= 1 {
            ctx.text_center(label, label_frame, color, font_size);
        } else {
            ctx.draw_text_wrapped(label, label_frame, color, font_size);
        }
        ctx.pop_clip();
    }

    fn paint_preview_indicator(ctx: &mut PaintContext<'_>, frame: Rect) {
        let shortest = frame.w.min(frame.h);
        if shortest < 20.0 {
            return;
        }
        let badge_size = 20.0_f32.min((shortest - 8.0).max(0.0));
        let inset = 6.0_f32.min((shortest - badge_size).max(0.0) * 0.5);
        let badge = Rect::new(
            frame.x + frame.w - badge_size - inset,
            frame.y + inset,
            badge_size,
            badge_size,
        );
        ctx.fill_circle(
            badge.x + badge.w * 0.5,
            badge.y + badge.h * 0.5,
            badge_size * 0.5,
            Color::from_rgba(0, 0, 0, 140),
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "zoom-in",
            badge,
            Color::white(),
            badge_size * 0.58,
        );
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
            preview_open: self.preview_open,
            fit: self.fit,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let image_source_changed = self.src != next.src || self.slot != next.slot;
        let lazy_changed = self.lazy != next.lazy;
        let next_load_state = next.load_state.into_inner();
        self.src = next.src;
        self.alt = next.alt;
        self.fallback = next.fallback;
        self.width = next.width;
        self.height = next.height;
        self.radius = next.radius;
        self.preview = next.preview;
        if !self.preview {
            self.preview_open = false;
            self.focused = false;
        }
        self.slot = next.slot;
        self.fit = next.fit;
        self.lazy = next.lazy;
        self.placeholder_enabled = next.placeholder_enabled;
        self.error_handler_enabled = next.error_handler_enabled;
        self.placeholder_view
            .replace(next.placeholder_view.into_inner());
        self.error_view_factory = next.error_view_factory;
        if image_source_changed {
            self.cached.set(None);
            self.load_state.replace(next_load_state);
            self.error_child_materialized.set(false);
        } else {
            if lazy_changed
                && matches!(
                    &*self.load_state.borrow(),
                    ImageLoadState::Deferred | ImageLoadState::Pending
                )
            {
                self.load_state.replace(next_load_state);
            }
            if self.load_state.borrow().error().is_some() || !self.error_handler_enabled {
                // Reconcile removes component-generated error children because the
                // next declarative Image has not observed this runtime failure yet.
                self.error_child_materialized.set(false);
            }
        }
    }

    fn resolve_handle(
        &self,
        ctx: &mut PaintContext<'_>,
        tree: &WidgetTree,
        frame: Rect,
    ) -> Option<BitmapHandle> {
        let svc = ctx.image_service();
        if let Some(handle) = self.valid_handle(svc) {
            self.set_load_state(ImageLoadState::Ready, tree, frame);
            return Some(handle);
        }

        if self.load_state.borrow().error().is_some() {
            return None;
        }
        if self.src.is_empty() && self.slot.is_none() {
            self.set_load_state(ImageLoadState::Empty, tree, frame);
            return None;
        }
        if self.lazy && !Self::frame_is_visible(ctx, frame) {
            self.set_load_state(ImageLoadState::Deferred, tree, frame);
            return None;
        }

        if self.placeholder_enabled
            && matches!(
                &*self.load_state.borrow(),
                ImageLoadState::Deferred | ImageLoadState::Pending
            )
        {
            // Keep the first visible frame free of filesystem IO/decode so a
            // custom placeholder is actually presented before synchronous load.
            self.set_load_state(ImageLoadState::Loading, tree, frame);
            return None;
        }

        if !self.src.is_empty() {
            return match ctx.image_service().load_from_path(&self.src) {
                Ok(handle) => {
                    self.cached.set(Some(handle));
                    self.set_load_state(ImageLoadState::Ready, tree, frame);
                    Some(handle)
                }
                Err(error) => {
                    self.set_load_state(ImageLoadState::Error(error.to_string()), tree, frame);
                    None
                }
            };
        }

        self.set_load_state(
            ImageLoadState::Error("预加载图片句柄已失效".to_owned()),
            tree,
            frame,
        );
        None
    }

    pub(crate) fn error_view_for_refresh(
        &self,
        current_child_count: usize,
    ) -> Option<crate::ui::view::ViewNode> {
        if self.error_child_materialized.get() || !self.error_handler_enabled {
            return None;
        }
        let error = self.load_state.borrow().error()?.to_owned();
        let placeholder_count = usize::from(self.placeholder_enabled);
        if current_child_count != placeholder_count {
            return None;
        }
        self.error_view_factory
            .as_ref()
            .map(|factory| factory(&error).key(Self::ERROR_CHILD_KEY))
    }

    pub(crate) fn mark_error_view_materialized(&self) {
        self.error_child_materialized.set(true);
    }

    fn surface_rect(&self, fallback: Rect) -> Rect {
        let w = self.last_surface_w.get();
        let h = self.last_surface_h.get();
        if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
            Rect::new(0.0, 0.0, w, h)
        } else {
            fallback
        }
    }

    fn render_preview(
        &self,
        ctx: &mut PaintContext<'_>,
        handle: Option<BitmapHandle>,
        surface_w: f32,
        surface_h: f32,
    ) {
        if !surface_w.is_finite() || !surface_h.is_finite() || surface_w <= 0.0 || surface_h <= 0.0
        {
            return;
        }
        let surface = Rect::new(0.0, 0.0, surface_w, surface_h);
        ctx.push_clip(surface);
        ctx.fill_rect(surface, Color::from_rgba(0, 0, 0, 204), None);

        let margin = 48.0_f32
            .min((surface_w * 0.1).max(16.0))
            .min((surface_h * 0.1).max(16.0));
        let preview_rect = Rect::new(
            margin,
            margin,
            (surface_w - margin * 2.0).max(1.0),
            (surface_h - margin * 2.0).max(1.0),
        );
        ctx.fill_rect(preview_rect, Color::from_rgba(18, 18, 18, 255), None);
        if let Some(handle) = handle {
            ctx.draw_image(handle, preview_rect);
        } else {
            let label = if !self.fallback.is_empty() {
                &self.fallback
            } else if !self.alt.is_empty() {
                &self.alt
            } else {
                "图片不可用"
            };
            Self::paint_centered_label(ctx, label, preview_rect, Color::white(), 16.0);
        }

        let close = Rect::new((surface_w - 52.0).max(4.0), 12.0, 40.0, 40.0);
        ctx.fill_circle(
            close.x + close.w * 0.5,
            close.y + close.h * 0.5,
            close.w * 0.5,
            Color::from_rgba(0, 0, 0, 180),
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(ctx, "x", close, Color::white(), 20.0);
        ctx.pop_clip();
    }
}
