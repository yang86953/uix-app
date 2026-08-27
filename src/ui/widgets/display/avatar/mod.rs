//! Avatar — circular avatar with initials/text.

use std::cell::Cell;

use crate::core::{Constraints, Rect, Size};
use crate::widget;
// 图片编解码 capability 启用时才需要在路径加载后触发重绘。
use crate::draw::Color;
#[cfg(feature = "image-codecs")]
// 该导入仅服务于头像路径解码入口。
use crate::draw::renderer::invalidate_paint_handle;
use crate::draw::resources::image::BitmapHandle;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
// 图片编解码 capability 启用时才追踪当前头像组件。
use crate::ui::SnapshotFields;
#[cfg(feature = "image-codecs")]
// 该导入仅服务于头像路径解码后的局部失效。
use crate::ui::widget_runtime::paint_scope::current_paint_widget;
use crate::ui::widget_runtime::widget::WidgetTree;

// 保存由 UIX 声明、由 Rust 资源与后备文字内核消费的静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AvatarVisual {
    default_extent: f32,
    text_scale: f32,
    square_text_fit: f32,
    circle_text_fit: f32,
    corner_radius_limit: f32,
    background_color: ColorValue,
    text_color: ColorValue,
    corner_radius: AvatarRadiusRole,
}

// 同目录 UIX 生成唯一视觉值及静态借用。
crate::uix_items!("src/ui/widgets/display/avatar/avatar.uix");

// 头像方形圆角使用的主题令牌角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AvatarRadiusRole {
    Small,
}

impl AvatarRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

widget! {
    /// 展示图片或自适应后备文字的头像组件。
    pub struct Avatar {
        text: String,
        size: f32,
        #[snapshot(skip)]
        size_authored: bool,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        square: bool,
        src: String,
        cached: Cell<Option<BitmapHandle>>,
        #[snapshot(skip)]
        visual: &'static AvatarVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let side = frame.w.min(frame.h);
        if side <= 0.0 {
            return;
        }
        let control = Rect::new(
            frame.x + (frame.w - side) * 0.5,
            frame.y + (frame.h - side) * 0.5,
            side,
            side,
        );
        let bg = self
            .bg_color
            .unwrap_or_else(|| self.visual.background_color.resolve(ctx.tokens()));
        let tc = self
            .text_color
            .unwrap_or_else(|| self.visual.text_color.resolve(ctx.tokens()));
        let corner_radius = self
            .visual
            .corner_radius
            .resolve(ctx.tokens())
            .min(side * self.visual.corner_radius_limit.clamp(0.0, 0.5));
        let r = if self.square {
            Some(crate::draw::Radius::uniform(corner_radius))
        } else {
            None
        };

        ctx.push_clip(frame);
        if self.square {
            ctx.fill_rect(control, bg, r);
        } else {
            let cx = control.x + side * 0.5;
            let cy = control.y + side * 0.5;
            let cr = side * 0.5;
            ctx.fill_circle(cx, cy, cr, bg);
        }

        // 图片编解码 capability 启用时解析头像文件路径。
        #[cfg(feature = "image-codecs")]
        // 路径能力开启时复用原有缓存与失效契约。
        let handle = self.resolve_handle(ctx, tree, control);
        // 图片编解码 capability 关闭时头像只保留文字回退。
        #[cfg(not(feature = "image-codecs"))]
        // 显式类型保证后续位图绘制分支保持同一契约。
        let handle: Option<BitmapHandle> = None;
        // 图片编解码 capability 关闭时不需要触发组件局部失效。
        #[cfg(not(feature = "image-codecs"))]
        // 显式消费组件树参数以保持无默认构建零新增警告。
        let _ = tree;
        let drew_image = if let Some(handle) = handle {
            let device_scale = ctx.device_pixel_ratio().max(f32::EPSILON);
            let target_side = (side * device_scale).ceil().clamp(1.0, 4096.0) as u32;
            let drawable = if self.square {
                ctx.image_service().rounded_square_crop_sized(
                    handle,
                    target_side,
                    corner_radius * device_scale,
                )
            } else {
                ctx.image_service()
                    .circular_crop_sized(handle, target_side)
            };
            if let Some(drawable) = drawable {
                ctx.draw_image_fill(drawable, control);
                true
            } else {
                false
            }
        } else {
            false
        };

        if !drew_image && !self.text.is_empty() {
            let base_font_size = side * self.visual.text_scale.max(0.0);
            let measured = ctx.measure_text(&self.text, base_font_size);
            let inner_side = side
                * if self.square {
                    self.visual.square_text_fit
                } else {
                    self.visual.circle_text_fit
                }
                .clamp(0.0, 1.0);
            let width_scale = if measured.w > 0.0 {
                inner_side / measured.w
            } else {
                1.0
            };
            let height_scale = if measured.h > 0.0 {
                inner_side / measured.h
            } else {
                1.0
            };
            let font_size = base_font_size * width_scale.min(height_scale).clamp(0.0, 1.0);
            if font_size > 0.0 {
                ctx.push_clip(control);
                ctx.text_center(&self.text, control, tc, font_size);
                ctx.pop_clip();
            }
        }
        ctx.pop_clip();
    }
}

impl Default for Avatar {
    fn default() -> Self {
        Self::new("")
    }
}

// 向 UIX 提供头像默认背景主题角色。
const fn avatar_primary_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}

// 向 UIX 提供头像默认文字主题角色。
const fn avatar_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

// 向 UIX 提供方形头像的小号圆角令牌角色。
const fn avatar_radius_sm() -> AvatarRadiusRole {
    AvatarRadiusRole::Small
}

// 把 UIX 声明的视觉配置融合进头像资源与后备文字内核。
fn build_avatar_view(mut kernel: Avatar, visual: &'static AvatarVisual) -> ViewNode {
    // 未显式设置尺寸时采用 UIX 声明的默认边长。
    if !kernel.size_authored {
        kernel.size = visual.default_extent;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Avatar {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 内核继续独占资源生命周期、测量与绘制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/avatar/avatar.uix")
    }
}

impl Avatar {
    /// 创建使用主题颜色、默认 32 像素圆形和给定后备文字的头像。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: AVATAR_VISUAL.default_extent,
            size_authored: false,
            bg_color: None,
            text_color: None,
            square: false,
            src: String::new(),
            cached: Cell::new(None),
            visual: AVATAR_VISUAL_REF,
        }
    }
    /// 设置头像边长；非正数或非有限值回退为 32 像素。
    pub fn size(mut self, s: f32) -> Self {
        self.size_authored = s.is_finite() && s > 0.0;
        self.size = if self.size_authored {
            s
        } else {
            self.visual.default_extent
        };
        self
    }
    /// 设置头像背景色，覆盖主题默认值。
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    /// 设置后备文字颜色，覆盖主题默认值。
    pub fn text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }
    /// 设置头像是否使用方形而非圆形裁剪。
    pub fn square(mut self, v: bool) -> Self {
        self.square = v;
        self
    }
    // 图片编解码 capability 启用时才公开头像路径入口。
    #[cfg(feature = "image-codecs")]
    // 关闭 capability 后使用方无法设置需要解码的图片来源。
    /// 设置头像图片路径并清除旧位图缓存；加载不可用时显示后备文字。
    pub fn src(mut self, s: &str) -> Self {
        self.src = s.to_string();
        self.cached.set(None);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.size, self.size)
    }

    // 图片编解码 capability 启用时才编译头像路径解析逻辑。
    #[cfg(feature = "image-codecs")]
    // 路径解析继续复用应用级 ImageService 缓存。
    fn resolve_handle(
        &self,
        ctx: &PaintContext,
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

    // 测试目标保留已加载位图句柄观测入口，供头像资源测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn loaded_handle_for_test(&self) -> Option<BitmapHandle> {
        self.cached.get()
    }
}

impl Avatar {
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.size = next.size;
        self.size_authored = next.size_authored;
        self.bg_color = next.bg_color;
        self.text_color = next.text_color;
        self.square = next.square;
        self.visual = next.visual;
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
