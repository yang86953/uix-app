//! ImageGroup — a keyboard-friendly image gallery with an in-window preview.

use std::cell::{Cell, RefCell};
use std::fmt::Write;
use std::sync::OnceLock;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, OverlayEntry, OverlayKind, SnapshotFields, SystemEvent,
    WidgetId, WidgetTree,
};
use crate::widget;
// 引入同一 display Module 拥有的 UIX 图片视觉角色与解析结果。
use super::image::presentation::{
    ImageColorRole, ImageOverlayPalette, ImageOverlayVisual, ImageRadiusRole, image_overlay_visual,
};

// 保存由 UIX 声明的 ImageGroup 默认固有尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupDefaultsVisual {
    width: f32,
    height: f32,
}

// 保存由 UIX 声明的内嵌画廊布局与焦点圈几何。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupGalleryVisual {
    strip_height: f32,
    strip_min_height: f32,
    strip_min_width: f32,
    strip_max_ratio: f32,
    navigation_width: f32,
    navigation_width_ratio: f32,
    navigation_min_width: f32,
    border_width: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
}

// 保存由 UIX 声明的模态预览布局与覆盖层级。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupPreviewVisual {
    strip_height: f32,
    strip_min_height: f32,
    strip_min_width: f32,
    strip_max_ratio: f32,
    margin_ratio: f32,
    margin_min: f32,
    margin_max: f32,
    navigation_width: f32,
    navigation_width_ratio: f32,
    navigation_min_width: f32,
    close_size: f32,
    close_margin: f32,
    available_center_ratio: f32,
    panel_radius: f32,
    counter_y: f32,
    counter_height: f32,
    z_index: i32,
}

// 保存由 UIX 声明的缩略图条带、间距与选中边框。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupThumbnailVisual {
    inline_max_size: f32,
    preview_max_size: f32,
    padding: f32,
    padding_strip_ratio: f32,
    min_size: f32,
    gap: f32,
    gap_size_ratio: f32,
    center_ratio: f32,
    selected_stroke_width: f32,
    normal_stroke_width: f32,
}

// 保存由 UIX 声明的导航、空状态与预览控制图标及文案。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupControlsVisual {
    navigation_diameter: f32,
    navigation_icon_size: f32,
    navigation_icon_ratio: f32,
    close_icon_size: f32,
    close_icon_ratio: f32,
    empty_icon_size: f32,
    empty_icon_ratio: f32,
    previous_icon: &'static str,
    next_icon: &'static str,
    close_icon: &'static str,
    image_icon: &'static str,
    loading_label: &'static str,
    empty_label: &'static str,
    unavailable_label: &'static str,
}

// 保存由 UIX 声明的 ImageGroup 主题角色与圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupPaletteVisual {
    background: ImageColorRole,
    text_secondary: ImageColorRole,
    primary: ImageColorRole,
    fill: ImageColorRole,
    overlay: ImageOverlayVisual,
    frame_radius: ImageRadiusRole,
    thumbnail_radius: ImageRadiusRole,
}

// 完整视觉配置由全部 ImageGroup 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ImageGroupVisual {
    defaults: ImageGroupDefaultsVisual,
    gallery: ImageGroupGalleryVisual,
    preview: ImageGroupPreviewVisual,
    thumbnail: ImageGroupThumbnailVisual,
    controls: ImageGroupControlsVisual,
    palette: ImageGroupPaletteVisual,
}

// 保存 ImageGroup 每帧只解析一次的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedImageGroupVisual {
    background: Color,
    text_secondary: Color,
    primary: Color,
    fill: Color,
    overlay: ImageOverlayPalette,
    frame_radius: f32,
    thumbnail_radius: f32,
}

impl ImageGroupVisual {
    // 一次读取当前主题，避免每个缩略图和控制器重复解析 token。
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedImageGroupVisual {
        ResolvedImageGroupVisual {
            background: self.palette.background.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            fill: self.palette.fill.resolve(tokens),
            overlay: ImageOverlayPalette::resolve(tokens, self.palette.overlay),
            frame_radius: self.palette.frame_radius.resolve(tokens),
            thumbnail_radius: self.palette.thumbnail_radius.resolve(tokens),
        }
    }
}

// 缓存预览计数文案，索引未变时不再逐帧 format! 分配。
#[derive(Debug)]
struct ImageGroupCounterCache {
    current: usize,
    count: usize,
    text: String,
}

impl Default for ImageGroupCounterCache {
    fn default() -> Self {
        Self {
            current: usize::MAX,
            count: usize::MAX,
            text: String::new(),
        }
    }
}

// 组合 UIX 声明的 ImageGroup 默认尺寸。
const fn image_group_defaults(width: f32, height: f32) -> ImageGroupDefaultsVisual {
    ImageGroupDefaultsVisual { width, height }
}

// 组合 UIX 声明的内嵌画廊视觉。
#[allow(clippy::too_many_arguments)]
const fn image_group_gallery(
    strip_height: f32,
    strip_min_height: f32,
    strip_min_width: f32,
    strip_max_ratio: f32,
    navigation_width: f32,
    navigation_width_ratio: f32,
    navigation_min_width: f32,
    border_width: f32,
    focus_inset: f32,
    focus_stroke_width: f32,
) -> ImageGroupGalleryVisual {
    ImageGroupGalleryVisual {
        strip_height,
        strip_min_height,
        strip_min_width,
        strip_max_ratio,
        navigation_width,
        navigation_width_ratio,
        navigation_min_width,
        border_width,
        focus_inset,
        focus_stroke_width,
    }
}

// 组合 UIX 声明的模态预览视觉。
#[allow(clippy::too_many_arguments)]
const fn image_group_preview(
    strip_height: f32,
    strip_min_height: f32,
    strip_min_width: f32,
    strip_max_ratio: f32,
    margin_ratio: f32,
    margin_min: f32,
    margin_max: f32,
    navigation_width: f32,
    navigation_width_ratio: f32,
    navigation_min_width: f32,
    close_size: f32,
    close_margin: f32,
    available_center_ratio: f32,
    panel_radius: f32,
    counter_y: f32,
    counter_height: f32,
    z_index: f32,
) -> ImageGroupPreviewVisual {
    ImageGroupPreviewVisual {
        strip_height,
        strip_min_height,
        strip_min_width,
        strip_max_ratio,
        margin_ratio,
        margin_min,
        margin_max,
        navigation_width,
        navigation_width_ratio,
        navigation_min_width,
        close_size,
        close_margin,
        available_center_ratio,
        panel_radius,
        counter_y,
        counter_height,
        z_index: z_index as i32,
    }
}

// 组合 UIX 声明的缩略图视觉。
#[allow(clippy::too_many_arguments)]
const fn image_group_thumbnail(
    inline_max_size: f32,
    preview_max_size: f32,
    padding: f32,
    padding_strip_ratio: f32,
    min_size: f32,
    gap: f32,
    gap_size_ratio: f32,
    center_ratio: f32,
    selected_stroke_width: f32,
    normal_stroke_width: f32,
) -> ImageGroupThumbnailVisual {
    ImageGroupThumbnailVisual {
        inline_max_size,
        preview_max_size,
        padding,
        padding_strip_ratio,
        min_size,
        gap,
        gap_size_ratio,
        center_ratio,
        selected_stroke_width,
        normal_stroke_width,
    }
}

// 组合 UIX 声明的图标、控制器与静态文案。
#[allow(clippy::too_many_arguments)]
const fn image_group_controls(
    navigation_diameter: f32,
    navigation_icon_size: f32,
    navigation_icon_ratio: f32,
    close_icon_size: f32,
    close_icon_ratio: f32,
    empty_icon_size: f32,
    empty_icon_ratio: f32,
    previous_icon: &'static str,
    next_icon: &'static str,
    close_icon: &'static str,
    image_icon: &'static str,
    loading_label: &'static str,
    empty_label: &'static str,
    unavailable_label: &'static str,
) -> ImageGroupControlsVisual {
    ImageGroupControlsVisual {
        navigation_diameter,
        navigation_icon_size,
        navigation_icon_ratio,
        close_icon_size,
        close_icon_ratio,
        empty_icon_size,
        empty_icon_ratio,
        previous_icon,
        next_icon,
        close_icon,
        image_icon,
        loading_label,
        empty_label,
        unavailable_label,
    }
}

// 组合 UIX 声明的主题角色。
#[allow(clippy::too_many_arguments)]
const fn image_group_palette(
    background: ImageColorRole,
    text_secondary: ImageColorRole,
    primary: ImageColorRole,
    fill: ImageColorRole,
    overlay: ImageOverlayVisual,
    frame_radius: ImageRadiusRole,
    thumbnail_radius: ImageRadiusRole,
) -> ImageGroupPaletteVisual {
    ImageGroupPaletteVisual {
        background,
        text_secondary,
        primary,
        fill,
        overlay,
        frame_radius,
        thumbnail_radius,
    }
}

// 组合 UIX 声明的完整 ImageGroup 视觉配置。
const fn image_group_visual(
    defaults: ImageGroupDefaultsVisual,
    gallery: ImageGroupGalleryVisual,
    preview: ImageGroupPreviewVisual,
    thumbnail: ImageGroupThumbnailVisual,
    controls: ImageGroupControlsVisual,
    palette: ImageGroupPaletteVisual,
) -> ImageGroupVisual {
    ImageGroupVisual {
        defaults,
        gallery,
        preview,
        thumbnail,
        controls,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接写入的文案、图标与主题角色。
const fn image_group_previous_icon() -> &'static str {
    "chevron-left"
}
const fn image_group_next_icon() -> &'static str {
    "chevron-right"
}
const fn image_group_close_icon() -> &'static str {
    "x"
}
const fn image_group_image_icon() -> &'static str {
    "image"
}
const fn image_group_loading_label() -> &'static str {
    "图片加载中…"
}
const fn image_group_empty_label() -> &'static str {
    "暂无图片"
}
const fn image_group_unavailable_label() -> &'static str {
    "图片不可用"
}
const fn image_group_container_color() -> ImageColorRole {
    ImageColorRole::Container
}
const fn image_group_secondary_text_color() -> ImageColorRole {
    ImageColorRole::TextSecondary
}
const fn image_group_primary_color() -> ImageColorRole {
    ImageColorRole::Primary
}
const fn image_group_fill_color() -> ImageColorRole {
    ImageColorRole::FillTertiary
}
const fn image_group_mask_color() -> ImageColorRole {
    ImageColorRole::Mask
}
const fn image_group_overlay_color() -> ImageColorRole {
    ImageColorRole::Overlay
}
const fn image_group_text_color() -> ImageColorRole {
    ImageColorRole::Text
}
const fn image_group_border_color() -> ImageColorRole {
    ImageColorRole::BorderSecondary
}
const fn image_group_body_radius() -> ImageRadiusRole {
    ImageRadiusRole::Body
}
const fn image_group_small_radius() -> ImageRadiusRole {
    ImageRadiusRole::Small
}

// Rust 直接构造或绕过 View 声明根时保持既有视觉；正常 View 构建会改用 UIX 静态配置。
static DEFAULT_IMAGE_GROUP_VISUAL: ImageGroupVisual = image_group_visual(
    image_group_defaults(320.0, 220.0),
    image_group_gallery(56.0, 72.0, 24.0, 0.4, 36.0, 1.0 / 3.0, 4.0, 1.0, 1.0, 2.0),
    image_group_preview(
        72.0,
        120.0,
        32.0,
        0.25,
        0.06,
        4.0,
        40.0,
        48.0,
        1.0 / 3.0,
        4.0,
        40.0,
        12.0,
        0.5,
        4.0,
        8.0,
        24.0,
        1100.0,
    ),
    image_group_thumbnail(44.0, 52.0, 6.0, 0.2, 4.0, 6.0, 0.25, 0.5, 2.0, 1.0),
    image_group_controls(
        30.0,
        16.0,
        0.6,
        20.0,
        0.6,
        16.0,
        0.5,
        "chevron-left",
        "chevron-right",
        "x",
        "image",
        "图片加载中…",
        "暂无图片",
        "图片不可用",
    ),
    image_group_palette(
        ImageColorRole::Container,
        ImageColorRole::TextSecondary,
        ImageColorRole::Primary,
        ImageColorRole::FillTertiary,
        image_overlay_visual(
            ImageColorRole::Mask,
            ImageColorRole::Overlay,
            ImageColorRole::Text,
            ImageColorRole::BorderSecondary,
            0.5,
        ),
        ImageRadiusRole::Body,
        ImageRadiusRole::Small,
    ),
);

// 首次 UIX 构建固化声明值，后续实例共享同一份只读视觉配置。
static UIX_IMAGE_GROUP_VISUAL: OnceLock<ImageGroupVisual> = OnceLock::new();

#[derive(Debug)]
struct GalleryGeometry {
    main: Rect,
    previous: Option<Rect>,
    next: Option<Rect>,
    thumbnails: ThumbnailLayout,
}

#[derive(Debug)]
struct PreviewGeometry {
    preview: Rect,
    previous: Option<Rect>,
    next: Option<Rect>,
    close: Rect,
    thumbnails: ThumbnailLayout,
}

// 保存等距缩略图的紧凑范围描述，迭代时按需生成矩形而不分配 Vec。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct ThumbnailLayout {
    start: usize,
    count: usize,
    x: f32,
    y: f32,
    size: f32,
    stride: f32,
}

impl ThumbnailLayout {
    fn iter(self) -> impl Iterator<Item = (usize, Rect)> {
        (0..self.count).map(move |offset| {
            (
                self.start + offset,
                Rect::new(
                    self.x + offset as f32 * self.stride,
                    self.y,
                    self.size,
                    self.size,
                ),
            )
        })
    }
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
        #[snapshot(skip)]
        counter_cache: RefCell<ImageGroupCounterCache>,
        #[snapshot(skip)]
        visual: &'static ImageGroupVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.visual.defaults.width,
            self.visual.defaults.height,
        ))
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
                .z_index(self.visual.preview.z_index)
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
        // 整个画廊与预览共享一次主题解析，缩略图数量不再放大 token 读取成本。
        let resolved = self.visual.resolve(ctx.tokens());
        self.paint_gallery(frame, ctx, tree, &resolved);

        if self.preview_open {
            self.paint_preview(
                ctx,
                Rect::new(0.0, 0.0, surface_w, surface_h),
                &resolved,
            );
            self.preview_painted.set(true);
        } else {
            self.preview_painted.set(false);
        }
    }
}

// 把画廊交互、资源绘制与 UIX 视觉表融合为单一根节点。
fn build_image_group_view(mut kernel: ImageGroup, declared_visual: ImageGroupVisual) -> ViewNode {
    kernel.visual = UIX_IMAGE_GROUP_VISUAL.get_or_init(|| declared_visual);
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
            counter_cache: RefCell::new(ImageGroupCounterCache::default()),
            visual: &DEFAULT_IMAGE_GROUP_VISUAL,
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
            let geometry = self.preview_geometry(surface, self.images.len(), self.current_index());
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
                let _ = self.select_index(index);
                return EventResult::Handled;
            }
            if !geometry.preview.contains(surface_pos) {
                self.close_preview();
            }
            // 模态预览内的任意点击都由所属 ImageGroup 消费，避免穿透到底层。
            return EventResult::Handled;
        }

        let local_frame = Rect::new(0.0, 0.0, frame.w, frame.h);
        let geometry = self.gallery_geometry(local_frame, self.images.len(), self.current_index());
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
            let _ = self.select_index(index);
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

    fn gallery_geometry(&self, frame: Rect, image_count: usize, current: usize) -> GalleryGeometry {
        let gallery = self.visual.gallery;
        let strip_height = if image_count > 1
            && frame.h >= gallery.strip_min_height
            && frame.w >= gallery.strip_min_width
        {
            gallery.strip_height.min(frame.h * gallery.strip_max_ratio)
        } else {
            0.0
        };
        let main = Rect::new(frame.x, frame.y, frame.w, (frame.h - strip_height).max(0.0));
        let strip = Rect::new(frame.x, frame.y + main.h, frame.w, strip_height);
        let navigation_width = gallery
            .navigation_width
            .min(main.w * gallery.navigation_width_ratio)
            .min(main.h);
        let navigation = image_count > 1 && navigation_width >= gallery.navigation_min_width;
        let previous_slot = Rect::new(main.x, main.y, navigation_width, main.h);
        let next_slot = Rect::new(
            main.x + main.w - navigation_width,
            main.y,
            navigation_width,
            main.h,
        );
        GalleryGeometry {
            main,
            previous: navigation.then(|| self.navigation_control_rect(previous_slot)),
            next: navigation.then(|| self.navigation_control_rect(next_slot)),
            thumbnails: self.thumbnail_layout(
                strip,
                image_count,
                current,
                self.visual.thumbnail.inline_max_size,
            ),
        }
    }

    fn preview_geometry(
        &self,
        surface: Rect,
        image_count: usize,
        current: usize,
    ) -> PreviewGeometry {
        let preview_visual = self.visual.preview;
        let thumbnail_height = if image_count > 1
            && surface.h >= preview_visual.strip_min_height
            && surface.w >= preview_visual.strip_min_width
        {
            preview_visual
                .strip_height
                .min(surface.h * preview_visual.strip_max_ratio)
        } else {
            0.0
        };
        let content_height = (surface.h - thumbnail_height).max(0.0);
        let shortest = surface.w.min(content_height).max(0.0);
        let margin = (shortest * preview_visual.margin_ratio)
            .clamp(preview_visual.margin_min, preview_visual.margin_max);
        let preview = Rect::new(
            surface.x + margin,
            surface.y + margin,
            (surface.w - margin * 2.0).max(0.0),
            (content_height - margin * 2.0).max(0.0),
        );
        let navigation_width = preview_visual
            .navigation_width
            .min(surface.w * preview_visual.navigation_width_ratio)
            .min(content_height);
        let navigation = image_count > 1 && navigation_width >= preview_visual.navigation_min_width;
        let close_size = preview_visual
            .close_size
            .min(surface.w)
            .min(surface.h)
            .max(0.0);
        let close_margin = preview_visual
            .close_margin
            .min((surface.w - close_size).max(0.0) * preview_visual.available_center_ratio)
            .min((surface.h - close_size).max(0.0) * preview_visual.available_center_ratio);
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
            previous: navigation.then(|| self.navigation_control_rect(previous_slot)),
            next: navigation.then(|| self.navigation_control_rect(next_slot)),
            close: Rect::new(
                surface.x + surface.w - close_size - close_margin,
                surface.y + close_margin,
                close_size,
                close_size,
            ),
            thumbnails: self.thumbnail_layout(
                strip,
                image_count,
                current,
                self.visual.thumbnail.preview_max_size,
            ),
        }
    }

    fn thumbnail_layout(
        &self,
        strip: Rect,
        image_count: usize,
        current: usize,
        maximum_size: f32,
    ) -> ThumbnailLayout {
        if image_count == 0 || strip.w <= 0.0 || strip.h <= 0.0 {
            return ThumbnailLayout::default();
        }
        let thumbnail = self.visual.thumbnail;
        let padding = thumbnail
            .padding
            .min(strip.h * thumbnail.padding_strip_ratio);
        let size = maximum_size
            .min((strip.h - padding * 2.0).max(0.0))
            .min(strip.w);
        if size < thumbnail.min_size {
            return ThumbnailLayout::default();
        }
        let gap = thumbnail.gap.min(size * thumbnail.gap_size_ratio);
        let capacity = (((strip.w + gap) / (size + gap)).floor() as usize)
            .max(1)
            .min(image_count);
        let start = current
            .saturating_sub(capacity / 2)
            .min(image_count - capacity);
        let total_width = size * capacity as f32 + gap * capacity.saturating_sub(1) as f32;
        let x = strip.x + (strip.w - total_width).max(0.0) * self.visual.thumbnail.center_ratio;
        let y = strip.y + (strip.h - size).max(0.0) * self.visual.thumbnail.center_ratio;
        ThumbnailLayout {
            start,
            count: capacity,
            x,
            y,
            size,
            stride: size + gap,
        }
    }

    fn navigation_control_rect(&self, slot: Rect) -> Rect {
        let diameter = self
            .visual
            .controls
            .navigation_diameter
            .min(slot.w)
            .min(slot.h)
            .max(0.0);
        Rect::new(
            slot.x + (slot.w - diameter) * self.visual.thumbnail.center_ratio,
            slot.y + (slot.h - diameter) * self.visual.thumbnail.center_ratio,
            diameter,
            diameter,
        )
    }

    fn paint_gallery(
        &self,
        frame: Rect,
        ctx: &mut PaintContext,
        tree: &WidgetTree,
        resolved: &ResolvedImageGroupVisual,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let radius = resolved
            .frame_radius
            .min(frame.w.min(frame.h) * self.visual.thumbnail.center_ratio);
        ctx.push_clip(frame);
        ctx.fill_rect(frame, resolved.background, Some(Radius::uniform(radius)));
        ctx.stroke_rect(
            frame,
            resolved.overlay.border,
            self.visual.gallery.border_width,
            Some(Radius::uniform(radius)),
        );

        let geometry = self.gallery_geometry(frame, self.images.len(), self.current_index());
        if let Some(path) = self.images.get(self.current_index()) {
            self.paint_image(
                ctx,
                path,
                geometry.main,
                true,
                self.visual.controls.loading_label,
                resolved,
            );
        } else {
            ctx.text_center(
                self.visual.controls.empty_label,
                geometry.main,
                resolved.text_secondary,
                resolved.overlay.compact_font,
            );
        }
        self.paint_controls(ctx, &geometry, resolved);

        if self.focused && tree.keyboard_focus_visible() {
            let inset = self
                .visual
                .gallery
                .focus_inset
                .min(frame.w * self.visual.thumbnail.center_ratio)
                .min(frame.h * self.visual.thumbnail.center_ratio);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                resolved.primary,
                self.visual.gallery.focus_stroke_width,
                Some(Radius::uniform(radius)),
            );
        }
        ctx.pop_clip();
    }

    fn paint_controls(
        &self,
        ctx: &mut PaintContext,
        geometry: &GalleryGeometry,
        resolved: &ResolvedImageGroupVisual,
    ) {
        if let Some(previous) = geometry.previous {
            self.paint_navigation_control(
                ctx,
                previous,
                self.visual.controls.previous_icon,
                resolved.overlay.foreground,
                resolved,
            );
        }
        if let Some(next) = geometry.next {
            self.paint_navigation_control(
                ctx,
                next,
                self.visual.controls.next_icon,
                resolved.overlay.foreground,
                resolved,
            );
        }
        let current = self.current_index();
        for (index, rect) in geometry.thumbnails.iter() {
            if let Some(path) = self.images.get(index) {
                self.paint_image(ctx, path, rect, false, "", resolved);
            }
            ctx.stroke_rect(
                rect,
                if index == current {
                    resolved.primary
                } else {
                    resolved.overlay.border
                },
                if index == current {
                    self.visual.thumbnail.selected_stroke_width
                } else {
                    self.visual.thumbnail.normal_stroke_width
                },
                Some(Radius::uniform(resolved.thumbnail_radius)),
            );
        }
    }

    fn paint_preview(
        &self,
        ctx: &mut PaintContext,
        surface: Rect,
        resolved: &ResolvedImageGroupVisual,
    ) {
        let surface = Self::normalized_frame(surface);
        if surface.w <= 0.0 || surface.h <= 0.0 || self.images.is_empty() {
            return;
        }
        let geometry = self.preview_geometry(surface, self.images.len(), self.current_index());
        ctx.push_clip(surface);
        // 全窗口背景使用主题语义遮罩。
        ctx.fill_rect(surface, resolved.overlay.mask, None);
        if geometry.preview.w > 0.0 && geometry.preview.h > 0.0 {
            // 预览占位区域使用主题浮层表面。
            ctx.fill_rect(
                geometry.preview,
                resolved.overlay.surface,
                Some(Radius::uniform(self.visual.preview.panel_radius)),
            );
            if let Some(path) = self.images.get(self.current_index()) {
                self.paint_image(
                    ctx,
                    path,
                    geometry.preview,
                    true,
                    self.visual.controls.unavailable_label,
                    resolved,
                );
            }
        }

        if let Some(previous) = geometry.previous {
            // 导航箭头使用浮层正文前景。
            self.paint_navigation_control(
                ctx,
                previous,
                self.visual.controls.previous_icon,
                resolved.overlay.foreground,
                resolved,
            );
        }
        if let Some(next) = geometry.next {
            // 导航箭头使用浮层正文前景。
            self.paint_navigation_control(
                ctx,
                next,
                self.visual.controls.next_icon,
                resolved.overlay.foreground,
                resolved,
            );
        }
        let current = self.current_index();
        for (index, rect) in geometry.thumbnails.iter() {
            if let Some(path) = self.images.get(index) {
                self.paint_image(ctx, path, rect, false, "", resolved);
            }
            ctx.stroke_rect(
                rect,
                if index == current {
                    resolved.primary
                } else {
                    // 未选缩略图使用主题浮层弱边界。
                    resolved.overlay.border
                },
                if index == current {
                    self.visual.thumbnail.selected_stroke_width
                } else {
                    self.visual.thumbnail.normal_stroke_width
                },
                Some(Radius::uniform(resolved.thumbnail_radius)),
            );
        }

        if geometry.close.w > 0.0 && geometry.close.h > 0.0 {
            ctx.fill_circle(
                geometry.close.x + geometry.close.w * self.visual.preview.available_center_ratio,
                geometry.close.y + geometry.close.h * self.visual.preview.available_center_ratio,
                geometry.close.w.min(geometry.close.h) * self.visual.preview.available_center_ratio,
                // 关闭按钮使用主题浮层表面。
                resolved.overlay.surface,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.controls.close_icon,
                geometry.close,
                // 关闭图标使用主题正文前景。
                resolved.overlay.foreground,
                self.visual.controls.close_icon_size.min(
                    geometry.close.w.min(geometry.close.h) * self.visual.controls.close_icon_ratio,
                ),
            );
        }
        let counter = Rect::new(
            surface.x,
            surface.y + self.visual.preview.counter_y,
            surface.w,
            self.visual.preview.counter_height.min(surface.h),
        );
        // 只在索引或图片数量变化时改写计数文案，并复用 String 容量。
        let current = self.current_index();
        let count = self.images.len();
        self.refresh_counter_text(current, count);
        let counter_cache = self.counter_cache.borrow();
        ctx.text_center(
            &counter_cache.text,
            counter,
            // 计数文本使用主题正文前景。
            resolved.overlay.foreground,
            // 使用主题派生的紧凑字号。
            resolved.overlay.compact_font,
        );
        ctx.pop_clip();
    }

    // 只在索引事实变化时刷新预览计数，并保留已有 String 容量。
    fn refresh_counter_text(&self, current: usize, count: usize) {
        let mut counter_cache = self.counter_cache.borrow_mut();
        if counter_cache.current == current && counter_cache.count == count {
            return;
        }
        counter_cache.current = current;
        counter_cache.count = count;
        counter_cache.text.clear();
        let _ = write!(&mut counter_cache.text, "{} / {}", current + 1, count);
    }

    fn paint_image(
        &self,
        ctx: &mut PaintContext,
        path: &str,
        frame: Rect,
        fit: bool,
        unavailable_label: &str,
        resolved: &ResolvedImageGroupVisual,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        ctx.fill_rect(frame, resolved.fill, None);
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
            ctx.text_center(
                unavailable_label,
                frame,
                resolved.text_secondary,
                resolved.overlay.compact_font.min(frame.h.max(1.0)),
            );
        } else {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.controls.image_icon,
                frame,
                resolved.text_secondary,
                self.visual
                    .controls
                    .empty_icon_size
                    .min(frame.w.min(frame.h) * self.visual.controls.empty_icon_ratio),
            );
        }
    }

    fn paint_navigation_control(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        icon: &str,
        color: Color,
        resolved: &ResolvedImageGroupVisual,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let diameter = self
            .visual
            .controls
            .navigation_diameter
            .min(frame.w)
            .min(frame.h);
        let circle = Rect::new(
            frame.x + (frame.w - diameter) * self.visual.thumbnail.center_ratio,
            frame.y + (frame.h - diameter) * self.visual.thumbnail.center_ratio,
            diameter,
            diameter,
        );
        ctx.fill_circle(
            circle.x + circle.w * self.visual.thumbnail.center_ratio,
            circle.y + circle.h * self.visual.thumbnail.center_ratio,
            diameter * self.visual.thumbnail.center_ratio,
            // 导航按钮使用主题浮层表面。
            resolved.overlay.surface,
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            icon,
            circle,
            color,
            self.visual
                .controls
                .navigation_icon_size
                .min(diameter * self.visual.controls.navigation_icon_ratio),
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
        self.visual = next.visual;
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

    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, i32) {
        (
            self.visual.defaults.width,
            self.visual.defaults.height,
            self.visual.gallery.strip_height,
            self.visual.preview.strip_height,
            self.visual.controls.navigation_diameter,
            self.visual.preview.z_index,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
