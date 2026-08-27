//! Image widget — 图片组件，Ant Design 风格。
//!
//! 支持占位图、fallback、描述、圆角与文件路径加载。

// 图片与图片组共享的主题绘制值跟随 Image widget 同目录维护。
pub(super) mod presentation;
// 单 Image 的加载生命周期状态保持在 widget 私有边界。
mod state;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::draw::renderer::Invalidation;
use crate::draw::resources::image::BitmapHandle;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::paint_scope::current_paint_widget;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, SystemEvent};
use crate::widget;
// 引入同一 widget 目录拥有的 UIX 图片视觉角色与解析结果。
use self::presentation::{
    ImageColorRole, ImageFontRole, ImageOverlayPalette, ImageOverlayVisual, image_overlay_visual,
};
// 引入 Image 私有加载生命周期状态。
use self::state::ImageLoadState;

// 保存由 UIX 声明的 Image 默认视觉与交互开关。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageDefaultsVisual {
    radius: f32,
    preview: bool,
    fit: bool,
    lazy: bool,
}

// 保存由 UIX 声明的缩略图占位、文字与图标几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageThumbnailVisual {
    padding: f32,
    radius_limit_ratio: f32,
    border_width: f32,
    icon_max_size: f32,
    icon_frame_ratio: f32,
    icon: &'static str,
    error_label: &'static str,
    label_font_size: f32,
    label_line_height_ratio: f32,
    center_ratio: f32,
}

// 保存由 UIX 声明的键盘焦点圈几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageFocusVisual {
    inset: f32,
    stroke_width: f32,
}

// 保存由 UIX 声明的缩放预览指示器几何与图标。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageIndicatorVisual {
    min_shortest: f32,
    badge_max_size: f32,
    outer_padding: f32,
    inset: f32,
    center_ratio: f32,
    icon_ratio: f32,
    icon: &'static str,
}

// 保存由 UIX 声明的模态预览布局、文案、图标与覆盖层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImagePreviewVisual {
    z_index: i32,
    margin_max: f32,
    margin_viewport_ratio: f32,
    margin_min: f32,
    min_extent: f32,
    missing_label: &'static str,
    missing_font: ImageFontRole,
    close_right_extent: f32,
    close_min_x: f32,
    close_y: f32,
    close_size: f32,
    center_ratio: f32,
    close_icon: &'static str,
    close_icon_size: f32,
}

// 保存由 UIX 声明的 Image 主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImagePaletteVisual {
    fill: ImageColorRole,
    text_secondary: ImageColorRole,
    primary: ImageColorRole,
    overlay: ImageOverlayVisual,
}

// 完整视觉配置由全部 Image 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ImageVisual {
    defaults: ImageDefaultsVisual,
    thumbnail: ImageThumbnailVisual,
    focus: ImageFocusVisual,
    indicator: ImageIndicatorVisual,
    preview: ImagePreviewVisual,
    palette: ImagePaletteVisual,
}

// 同目录 UIX 生成全部分组视觉、根视觉记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/image/image.uix");

// 保存 Image 每帧只解析一次的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedImageVisual {
    fill: Color,
    text_secondary: Color,
    primary: Color,
    overlay: ImageOverlayPalette,
    missing_font_size: f32,
}

impl ImageVisual {
    // 一次读取当前主题，避免缩略图、指示器、焦点圈和预览重复取 token。
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedImageVisual {
        ResolvedImageVisual {
            fill: self.palette.fill.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            overlay: ImageOverlayPalette::resolve(tokens, self.palette.overlay),
            missing_font_size: self.preview.missing_font.resolve(tokens),
        }
    }
}

// 向 UIX 提供主题角色。
const fn image_large_font() -> ImageFontRole {
    ImageFontRole::Large
}
const fn image_fill_color() -> ImageColorRole {
    ImageColorRole::FillTertiary
}
const fn image_secondary_text_color() -> ImageColorRole {
    ImageColorRole::TextSecondary
}
const fn image_primary_color() -> ImageColorRole {
    ImageColorRole::Primary
}
const fn image_mask_color() -> ImageColorRole {
    ImageColorRole::Mask
}
const fn image_overlay_color() -> ImageColorRole {
    ImageColorRole::Overlay
}
const fn image_text_color() -> ImageColorRole {
    ImageColorRole::Text
}
const fn image_border_color() -> ImageColorRole {
    ImageColorRole::BorderSecondary
}

// Image — 图片显示组件。
widget! {
    /// 拥有资源加载、占位、错误内容与模态预览生命周期的图片组件。
    pub struct Image {
        src: String,
        alt: String,
        fallback: String,
        width: f32,
        height: f32,
        radius: f32,
        #[snapshot(skip)]
        radius_authored: bool,
        preview: bool,
        #[snapshot(skip)]
        preview_authored: bool,
        /// 预加载位图句柄（优先于 src 懒加载）。
        slot: Option<BitmapHandle>,
        /// src 首次加载成功后的句柄缓存，避免每帧查表。
        cached: Cell<Option<BitmapHandle>>,
        /// fit 模式：true=保持比例居中，false=拉伸填满。
        fit: bool,
        #[snapshot(skip)]
        fit_authored: bool,
        /// 延迟到组件进入可见绘制路径后加载；Image 的绘制本身已是按需加载。
        lazy: bool,
        #[snapshot(skip)]
        lazy_authored: bool,
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
        #[snapshot(skip)]
        visual: &'static ImageVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
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

    // 在错误子树真实离开直接子节点集合后同步清理物化派生状态。
    on_children_changed => (&mut self, child_count: usize) {
        // Image 的直接子节点只由可选 placeholder 与可选 error 子树组成。
        let placeholder_count = usize::from(self.placeholder_enabled);
        // 只有实际子节点数量回到 placeholder 基线时错误子树才已完成移除。
        if child_count == placeholder_count {
            // 此时 State、Effect 与动画源已由树 teardown 释放，可安全允许下次重新捕获。
            self.error_child_materialized.set(false);
        }
    }

    child_visible => (&self, index: usize) -> bool {
        let state = self.load_state.borrow();
        if self.placeholder_enabled && index == 0 {
            return state.shows_placeholder();
        }
        // handler 关闭或 Error 恢复后，正在 leave 的已物化错误子树仍必须保持可见。
        if (self.error_handler_enabled || self.error_child_materialized.get())
            && index == usize::from(self.placeholder_enabled)
        {
            // 当前 Error 或尚未真实移除的物化子树任一成立都继续参与离场绘制。
            return state.error().is_some() || self.error_child_materialized.get();
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

    overlay_entry => (&self, id: crate::ui::WidgetId, frame: Rect) -> Option<OverlayEntry> {
        self.preview_open.then(|| {
            OverlayEntry::new(id, OverlayKind::Modal)
                .bounds(self.surface_rect(frame))
                .z_index(self.visual.preview.z_index)
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

        let surface_size = ctx.surface_size();
        let (surface_w, surface_h) = (surface_size.w, surface_size.h);
        self.last_surface_w.set(surface_w);
        self.last_surface_h.set(surface_h);
        let frame = Self::normalized_frame(frame);
        let resolved = self.visual.resolve(ctx.tokens());
        let radius = self
            .radius
            .min(frame.w.min(frame.h) * self.visual.thumbnail.radius_limit_ratio);
        let handle = if pass == PaintPass::Content {
            let handle = self.resolve_handle(ctx, tree, frame);
            self.render_thumbnail(frame, radius, handle, &resolved, ctx);
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
            &resolved,
            ctx,
            tree,
        );
    }
}

// 把图片资源、状态机与 UIX 视觉表融合为单一根节点。
fn build_image_view(mut kernel: Image, visual: &'static ImageVisual) -> ViewNode {
    if !kernel.radius_authored {
        kernel.radius = visual.defaults.radius;
    }
    if !kernel.preview_authored {
        kernel.preview = visual.defaults.preview;
    }
    if !kernel.fit_authored {
        kernel.fit = visual.defaults.fit;
    }
    if !kernel.lazy_authored {
        kernel.lazy = visual.defaults.lazy;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Image {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占加载、动态子树、预览与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/image/image.uix")
    }
}

impl Image {
    const PLACEHOLDER_CHILD_KEY: &'static str = "uix:image:placeholder";
    // 错误动态子树的运行时 key 同时作为组件状态捕获的稳定实例身份。
    pub(crate) const ERROR_CHILD_KEY: &'static str = "uix:image:error";

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

    /// 创建指定固有尺寸、启用等比适配与预览且立即加载的图片组件。
    pub fn new(w: f32, h: f32) -> Self {
        Self {
            src: String::new(),
            alt: String::new(),
            fallback: String::new(),
            width: Self::finite_non_negative(w),
            height: Self::finite_non_negative(h),
            radius: IMAGE_VISUAL.defaults.radius,
            radius_authored: false,
            preview: IMAGE_VISUAL.defaults.preview,
            preview_authored: false,
            slot: None,
            cached: Cell::new(None),
            fit: IMAGE_VISUAL.defaults.fit,
            fit_authored: false,
            lazy: IMAGE_VISUAL.defaults.lazy,
            lazy_authored: false,
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
            visual: IMAGE_VISUAL_REF,
        }
    }

    /// 设置图片文件路径（渲染时懒加载）。
    // 图片编解码 capability 启用时才公开文件路径入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后仍可通过 slot 使用框架内部 RGBA 位图。
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

    /// 设置图片的替代描述文本，并在没有后备文本时用于加载失败显示。
    pub fn alt(mut self, a: &str) -> Self {
        self.alt = a.to_string();
        self
    }
    /// 设置资源不可用时优先显示的后备文本。
    pub fn fallback(mut self, f: &str) -> Self {
        self.fallback = f.to_string();
        self
    }
    /// 设置非负圆角半径；负数或非有限值归一化为零。
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = Self::finite_non_negative(r);
        self.radius_authored = true;
        self
    }
    /// 设置是否允许交互式模态预览；禁用时立即关闭已有预览。
    pub fn preview(mut self, v: bool) -> Self {
        self.preview = v;
        self.preview_authored = true;
        if !v {
            self.preview_open = false;
        }
        self
    }
    /// 保持宽高比居中（默认 true）；false 则拉伸填满。
    pub fn fit(mut self, v: bool) -> Self {
        self.fit = v;
        self.fit_authored = true;
        self
    }

    /// 延迟到图片与当前绘制视口相交时才解析资源。
    pub fn lazy(mut self, v: bool) -> Self {
        self.lazy = v;
        self.lazy_authored = true;
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

    /// 返回模态图片预览当前是否处于逻辑打开状态。
    pub fn is_preview_open(&self) -> bool {
        self.preview_open
    }

    /// 打开当前窗口内的模态图片预览；禁用 preview 时无操作。
    pub fn open_preview(&mut self) {
        if self.preview {
            self.preview_open = true;
        }
    }

    /// 关闭当前窗口内的模态图片预览。
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

    fn frame_is_visible(ctx: &mut PaintContext, frame: Rect) -> bool {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return false;
        }
        ctx.is_rect_visible(frame)
    }

    fn valid_handle(
        &self,
        image_service: &crate::draw::resources::image::ImageService,
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
        // 在借用期内提取后续判断值，避免复制 Error 携带的错误字符串。
        let (unchanged, previous_has_error, placeholder_visibility_changed) = {
            let previous = self.load_state.borrow();
            (
                *previous == next,
                previous.error().is_some(),
                self.placeholder_enabled
                    && previous.shows_placeholder() != next.shows_placeholder(),
            )
        };
        if unchanged {
            return;
        }
        // 记录新状态是否需要错误子树，避免只处理首次失败而遗漏恢复路径。
        let next_has_error = next.error().is_some();
        // 加载阶段、占位可见性或错误存在性变化都需要一次后继布局。
        let needs_follow_up = matches!(&next, ImageLoadState::Loading)
            || placeholder_visibility_changed
            || previous_has_error != next_has_error;
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
        resolved: &ResolvedImageVisual,
        ctx: &mut PaintContext,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }

        let rounded = Some(Radius::uniform(radius));
        ctx.push_clip(frame);
        if let Some(handle) = handle {
            ctx.fill_rect(frame, resolved.fill, rounded);
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
            ctx.fill_rect(frame, resolved.fill, rounded);
            ctx.stroke_rect(
                frame,
                resolved.overlay.border,
                self.visual.thumbnail.border_width,
                rounded,
            );
            if !self.custom_content_active() {
                let state = self.load_state.borrow();
                let label = if state.error().is_some() && !self.fallback.is_empty() {
                    self.fallback.as_str()
                } else if state.error().is_some() && self.error_handler_enabled {
                    self.visual.thumbnail.error_label
                } else if !self.alt.is_empty() {
                    self.alt.as_str()
                } else {
                    ""
                };
                if label.is_empty() {
                    let icon_size = self
                        .visual
                        .thumbnail
                        .icon_max_size
                        .min(frame.w.min(frame.h) * self.visual.thumbnail.icon_frame_ratio);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        self.visual.thumbnail.icon,
                        frame,
                        resolved.text_secondary,
                        icon_size,
                    );
                } else {
                    self.paint_centered_label(
                        ctx,
                        label,
                        frame,
                        resolved.text_secondary,
                        self.visual.thumbnail.label_font_size,
                    );
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
        resolved: &ResolvedImageVisual,
        ctx: &mut PaintContext,
        tree: &WidgetTree,
    ) {
        if frame.w > 0.0 && frame.h > 0.0 {
            ctx.push_clip(frame);
            if self.preview && handle.is_some() {
                self.paint_preview_indicator(ctx, frame, resolved);
            }
            if self.focused && tree.keyboard_focus_visible() && self.preview {
                let focus = Self::inset(frame, self.visual.focus.inset);
                ctx.stroke_rect(
                    focus,
                    resolved.primary,
                    self.visual.focus.stroke_width,
                    Some(Radius::uniform(radius.min(
                        focus.w.min(focus.h) * self.visual.thumbnail.radius_limit_ratio,
                    ))),
                );
            }
            ctx.pop_clip();
        }

        if self.preview_open {
            self.render_preview(ctx, handle, surface_w, surface_h, resolved);
            self.preview_painted.set(true);
        } else {
            self.preview_painted.set(false);
        }
    }

    fn paint_centered_label(
        &self,
        ctx: &mut PaintContext,
        label: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        let content = Self::inset(frame, self.visual.thumbnail.padding);
        if content.w <= 0.0 || content.h <= 0.0 {
            return;
        }
        let metrics = crate::draw::resources::font::text_backend::estimate_text_metrics(
            label, content.w, font_size,
        );
        let line_height = font_size * self.visual.thumbnail.label_line_height_ratio;
        let label_height = (metrics.line_count.max(1) as f32 * line_height).min(content.h);
        let label_frame = Rect::new(
            content.x,
            content.y + (content.h - label_height) * self.visual.thumbnail.center_ratio,
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

    fn paint_preview_indicator(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        resolved: &ResolvedImageVisual,
    ) {
        let shortest = frame.w.min(frame.h);
        if shortest < self.visual.indicator.min_shortest {
            return;
        }
        let badge_size = self
            .visual
            .indicator
            .badge_max_size
            .min((shortest - self.visual.indicator.outer_padding).max(0.0));
        let inset = self
            .visual
            .indicator
            .inset
            .min((shortest - badge_size).max(0.0) * self.visual.indicator.center_ratio);
        let badge = Rect::new(
            frame.x + frame.w - badge_size - inset,
            frame.y + inset,
            badge_size,
            badge_size,
        );
        ctx.fill_circle(
            badge.x + badge.w * self.visual.indicator.center_ratio,
            badge.y + badge.h * self.visual.indicator.center_ratio,
            badge_size * self.visual.indicator.center_ratio,
            // 缩放徽标使用主题浮层表面。
            resolved.overlay.surface,
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.indicator.icon,
            badge,
            // 缩放图标使用主题浮层前景。
            resolved.overlay.foreground,
            badge_size * self.visual.indicator.icon_ratio,
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
        self.radius_authored = next.radius_authored;
        self.preview = next.preview;
        self.preview_authored = next.preview_authored;
        if !self.preview {
            self.preview_open = false;
            self.focused = false;
        }
        self.slot = next.slot;
        self.fit = next.fit;
        self.fit_authored = next.fit_authored;
        self.lazy = next.lazy;
        self.lazy_authored = next.lazy_authored;
        self.placeholder_enabled = next.placeholder_enabled;
        self.error_handler_enabled = next.error_handler_enabled;
        self.placeholder_view
            .replace(next.placeholder_view.into_inner());
        self.error_view_factory = next.error_view_factory;
        self.visual = next.visual;
        if image_source_changed {
            self.cached.set(None);
            self.load_state.replace(next_load_state);
        } else {
            if lazy_changed
                && matches!(
                    &*self.load_state.borrow(),
                    ImageLoadState::Deferred | ImageLoadState::Pending
                )
            {
                self.load_state.replace(next_load_state);
            }
        }
    }

    fn resolve_handle(
        &self,
        ctx: &mut PaintContext,
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

        // 图片编解码 capability 启用时才尝试文件路径加载。
        #[cfg(feature = "image-codecs")]
        // 关闭 capability 后不会保留运行时失败的路径型伪入口。
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
        // 接收由活跃 Image owner 所属树签发的窄捕获能力。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        current_child_count: usize,
    ) -> Option<crate::ui::view::ViewNode> {
        // 运行时树负责判断同 key 子节点是否已存在，组件只判断当前业务需求。
        if !self.error_handler_enabled {
            return None;
        }
        let error = self.load_state.borrow().error()?.to_owned();
        let placeholder_count = usize::from(self.placeholder_enabled);
        if current_child_count != placeholder_count {
            return None;
        }
        // 用固定错误子树 key 保持状态身份，直到真实移除错误子树才释放其状态。
        self.capture_error_view(capture_context, error)
    }

    // 为父级声明协调重新捕获当前错误子树，保留已物化实例的私有状态。
    pub(crate) fn error_view_for_reconcile(
        &self,
        // 接收由活跃 Image owner 所属树签发的窄捕获能力。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
    ) -> Option<crate::ui::view::ViewNode> {
        // 仅在当前加载状态确实失败且错误 View 仍启用时重建声明输出。
        if !self.error_handler_enabled {
            // 关闭错误处理器必须让父级协调移除既有错误子树。
            return None;
        }
        // 复制错误文本，避免用户工厂在捕获期间借用 Image 内部状态。
        let error = self.load_state.borrow().error()?.to_owned();
        // 用固定错误子树 key 保持状态身份，直到真实移除错误子树才释放其状态。
        self.capture_error_view(capture_context, error)
    }

    // 在树签发的捕获边界内执行错误 View 工厂并附加稳定的运行时 key。
    fn capture_error_view(
        &self,
        // 接收由活跃 Image owner 所属树签发的窄捕获能力。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        // 接收当前失败实例的拥有型错误文本。
        error: String,
    ) -> Option<crate::ui::view::ViewNode> {
        // 读取当前声明提供的错误工厂，缺失时保持无子树语义。
        let factory = self.error_view_factory.as_ref()?;
        // 在 Image 专属槽位与稳定错误身份内完整捕获 State、Effect、动画与回执。
        Some(capture_context.capture(
            // 隔离同一 Image 的错误工厂与其他延迟 View 工厂。
            "image-error",
            // 运行时 key 与组件状态命名空间必须共享同一固定子树身份。
            Self::ERROR_CHILD_KEY,
            // 用户工厂只在树已验证且捕获已安装的边界内执行一次。
            || factory(&error).key(Self::ERROR_CHILD_KEY),
        ))
    }

    pub(crate) fn mark_error_view_materialized(&self) {
        self.error_child_materialized.set(true);
    }

    // 清除错误子树物化标记，让真实移除后的下一次失败可以重新捕获。
    pub(crate) fn clear_error_view_materialized(&self) {
        // 只更新 Image 私有派生状态，不直接触碰运行时树结构。
        self.error_child_materialized.set(false);
    }

    // 返回当前运行时加载状态是否要求保留错误动态子树。
    pub(crate) fn needs_error_view(&self) -> bool {
        // handler 启用与实际 Error 状态必须同时成立。
        self.error_handler_enabled && self.load_state.borrow().error().is_some()
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
        ctx: &mut PaintContext,
        handle: Option<BitmapHandle>,
        surface_w: f32,
        surface_h: f32,
        resolved: &ResolvedImageVisual,
    ) {
        if !surface_w.is_finite() || !surface_h.is_finite() || surface_w <= 0.0 || surface_h <= 0.0
        {
            return;
        }
        let surface = Rect::new(0.0, 0.0, surface_w, surface_h);
        ctx.push_clip(surface);
        // 全屏预览背景使用主题语义遮罩。
        ctx.fill_rect(surface, resolved.overlay.mask, None);

        let margin = self
            .visual
            .preview
            .margin_max
            .min(
                (surface_w * self.visual.preview.margin_viewport_ratio)
                    .max(self.visual.preview.margin_min),
            )
            .min(
                (surface_h * self.visual.preview.margin_viewport_ratio)
                    .max(self.visual.preview.margin_min),
            );
        let preview_rect = Rect::new(
            margin,
            margin,
            (surface_w - margin * 2.0).max(self.visual.preview.min_extent),
            (surface_h - margin * 2.0).max(self.visual.preview.min_extent),
        );
        // 预览占位区域使用主题浮层表面。
        ctx.fill_rect(preview_rect, resolved.overlay.surface, None);
        if let Some(handle) = handle {
            ctx.draw_image(handle, preview_rect);
        } else {
            let label = if !self.fallback.is_empty() {
                &self.fallback
            } else if !self.alt.is_empty() {
                &self.alt
            } else {
                self.visual.preview.missing_label
            };
            self.paint_centered_label(
                ctx,
                label,
                preview_rect,
                resolved.overlay.foreground,
                resolved.missing_font_size,
            );
        }

        let close = Rect::new(
            (surface_w - self.visual.preview.close_right_extent)
                .max(self.visual.preview.close_min_x),
            self.visual.preview.close_y,
            self.visual.preview.close_size,
            self.visual.preview.close_size,
        );
        ctx.fill_circle(
            close.x + close.w * self.visual.preview.center_ratio,
            close.y + close.h * self.visual.preview.center_ratio,
            close.w * self.visual.preview.center_ratio,
            // 关闭按钮使用主题浮层表面。
            resolved.overlay.surface,
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.preview.close_icon,
            close,
            resolved.overlay.foreground,
            self.visual.preview.close_icon_size,
        );
        ctx.pop_clip();
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, i32) {
        (
            self.visual.defaults.radius,
            self.visual.thumbnail.padding,
            self.visual.indicator.badge_max_size,
            self.visual.preview.margin_max,
            self.visual.preview.z_index,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
