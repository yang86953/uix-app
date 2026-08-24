//! Tag widget — 彩色标签/徽标，支持关闭与勾选。

use std::cell::Cell;

use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{
    EventResult, KeyCode, MouseButton, PrimaryHue, SemanticEvent, SystemEvent, ThemeTokens,
    WidgetId,
};
use crate::widget;

const DEFAULT_TAG_FONT_SIZE: f32 = 12.0;

fn normalized_tag_font_size(size: f32) -> f32 {
    if size.is_finite() && size > 0.0 {
        size
    } else {
        DEFAULT_TAG_FONT_SIZE
    }
}

/// 预设标签类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TagColor {
    /// 使用主题中性填充与正文颜色。
    Default,
    /// 使用主题成功状态色对。
    Success,
    /// 使用主题信息状态色对。
    Info,
    /// 使用主题警告状态色对。
    Warning,
    /// 使用主题错误状态色对。
    Error,
    /// 使用当前主题品牌主色对。
    Blue,
    /// 使用青色调色板的低强调色对。
    Cyan,
    /// 使用极客蓝调色板的低强调色对。
    Geekblue,
    /// 使用紫色调色板的低强调色对。
    Purple,
    /// 使用品红调色板的低强调色对。
    Magenta,
    /// 使用红色调色板的低强调色对。
    Red,
    /// 使用橙色调色板的低强调色对。
    Orange,
    /// 使用主题警告色对呈现金色标签。
    Gold,
    /// 使用青柠调色板的低强调色对。
    Lime,
    /// 使用主题成功色对呈现绿色标签。
    Green,
}

// 将标签预设映射为当前主题下的背景与前景。
fn resolve_tag_colors(color: TagColor, tokens: &dyn ThemeTokens) -> (Color, Color) {
    // 功能色与品牌色优先服从当前主题的可定制 token。
    match color {
        // 默认标签使用中性填充与正文色。
        TagColor::Default => (tokens.color_fill_tertiary(), tokens.color_text()),
        // 成功标签使用主题成功色对。
        TagColor::Success => (tokens.color_success_bg(), tokens.color_success()),
        // 信息标签使用主题信息色对。
        TagColor::Info => (tokens.color_info_bg(), tokens.color_info()),
        // 警告标签使用主题警告色对。
        TagColor::Warning => (tokens.color_warning_bg(), tokens.color_warning()),
        // 错误标签使用主题错误色对。
        TagColor::Error => (tokens.color_error_bg(), tokens.color_error()),
        // 蓝色预设与当前主题品牌主色保持一致。
        TagColor::Blue => (tokens.color_primary_bg(), tokens.color_primary()),
        // 青色预设从 theme 层色阶解析明暗模式。
        TagColor::Cyan => PrimaryHue::Cyan.palette().subtle_pair(tokens.is_dark()),
        // 极客蓝预设从 theme 层色阶解析明暗模式。
        TagColor::Geekblue => PrimaryHue::Geekblue.palette().subtle_pair(tokens.is_dark()),
        // 紫色预设从 theme 层色阶解析明暗模式。
        TagColor::Purple => PrimaryHue::Purple.palette().subtle_pair(tokens.is_dark()),
        // 洋红预设从 theme 层色阶解析明暗模式。
        TagColor::Magenta => PrimaryHue::Magenta.palette().subtle_pair(tokens.is_dark()),
        // 红色预设从 theme 层色阶解析明暗模式。
        TagColor::Red => PrimaryHue::Red.palette().subtle_pair(tokens.is_dark()),
        // 橙色预设从 theme 层色阶解析明暗模式。
        TagColor::Orange => PrimaryHue::Orange.palette().subtle_pair(tokens.is_dark()),
        // 金色预设与当前主题警告色保持一致。
        TagColor::Gold => (tokens.color_warning_bg(), tokens.color_warning()),
        // 青柠预设从 theme 层色阶解析明暗模式。
        TagColor::Lime => PrimaryHue::Lime.palette().subtle_pair(tokens.is_dark()),
        // 绿色预设与当前主题成功色保持一致。
        TagColor::Green => (tokens.color_success_bg(), tokens.color_success()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagAction {
    Closed,
    Checked,
    Unchecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagTarget {
    Body,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TagPress {
    Pointer(TagTarget),
    Key(KeyCode),
}

#[derive(Debug, Clone, Copy)]
struct TagGeometry {
    frame: Rect,
    body: Rect,
    close: Option<Rect>,
    check: Option<Rect>,
    text: Rect,
}

widget! {
    /// 展示可关闭或可勾选短文本状态的标签组件。
    pub struct Tag {
        text: String,
        color: TagColor,
        closable: bool,
        font_size: f32,
        custom_color: Option<Color>,
        checkable: bool,
        checked: bool,
        visible: bool,
        focused: bool,
        /// Lucide 图标名称，绘制在文字之前。
        icon: String,
        last_size: Cell<Size>,
        layout_requested: Cell<bool>,
        pending_action: Cell<Option<TagAction>>,
        hovered_target: Cell<Option<TagTarget>>,
        pressed: Cell<Option<TagPress>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        if self.visible {
            constraints.clamp(self.intrinsic_size())
        } else {
            Size::zero()
        }
    }

    visible => (&self) -> bool { self.visible }

    tab_index => (&self) -> i32 {
        i32::from(self.visible && (self.closable || self.checkable))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible || (!self.closable && !self.checkable) {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(target) = self.target_at(*pos) {
                    self.hovered_target.set(Some(target));
                    self.pressed.set(Some(TagPress::Pointer(target)));
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(TagPress::Pointer(pressed)) = self.pressed.replace(None) else {
                    return EventResult::NotHandled;
                };
                let released = self.target_at(*pos);
                self.hovered_target.set(released);
                if released == Some(pressed) {
                    self.commit_target(pressed);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next = self.target_at(*pos);
                if self.hovered_target.replace(next) != next {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let pointer_pressed = matches!(self.pressed.get(), Some(TagPress::Pointer(_)));
                if pointer_pressed {
                    self.pressed.set(None);
                }
                let changed = self.hovered_target.replace(None).is_some() | pointer_pressed;
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                if self.pressed.get().is_none() {
                    self.pressed.set(Some(TagPress::Key(*key)));
                }
                EventResult::Handled
            }
            SystemEvent::KeyUp {
                key: key @ (KeyCode::Enter | KeyCode::Space),
                ..
            } => {
                if self.pressed.replace(None) == Some(TagPress::Key(*key)) {
                    self.commit_target(if self.checkable {
                        TagTarget::Body
                    } else {
                        TagTarget::Close
                    });
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyDown {
                key: KeyCode::Delete | KeyCode::Escape,
                ..
            } if self.closable => {
                self.dismiss();
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed.set(None);
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_action.replace(None).map(|action| {
            let payload = match action {
                TagAction::Closed => "closed",
                TagAction::Checked => "checked",
                TagAction::Unchecked => "unchecked",
            };
            SemanticEvent::change(id, payload)
        })
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let geometry = Self::geometry(frame, self.checkable, self.closable);
        let frame = geometry.frame;
        self.last_size.set(Size::new(frame.w, frame.h));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let font_size = normalized_tag_font_size(self.font_size);
        let (bg, fg) = if let Some(cc) = self.custom_color {
            // 自定义色对比文字：按亮度取黑白 token。
            (cc, if cc.is_light() { ctx.tokens().color_black() } else { ctx.tokens().color_white() })
        } else {
            // 非自定义标签在绘制时解析当前主题与明暗模式。
            resolve_tag_colors(self.color, ctx.tokens())
        };
        let radius = ctx
            .tokens()
            .border_radius_sm()
            .min(frame.w.min(frame.h) * 0.5);
        let r = Some(Radius::uniform(radius));
        ctx.push_clip(frame);
        ctx.fill_rect(frame, bg, r);
        let visual_target = match self.pressed.get() {
            Some(TagPress::Pointer(target)) if self.hovered_target.get() == Some(target) => {
                Some((target, true))
            }
            Some(TagPress::Key(_)) => Some((
                if self.checkable {
                    TagTarget::Body
                } else {
                    TagTarget::Close
                },
                true,
            )),
            _ => self.hovered_target.get().map(|target| (target, false)),
        };
        if let Some((target, pressed)) = visual_target {
            let target_frame = match target {
                TagTarget::Body => geometry.body,
                TagTarget::Close => geometry.close.unwrap_or(geometry.frame),
            };
            // 悬停/按压叠加：按底色亮度取黑白 token + 原 alpha（保持视觉等价，色相随主题可换）。
            let overlay = if bg.is_light() {
                ctx.tokens().color_black().with_alpha(if pressed { 28 } else { 14 })
            } else {
                ctx.tokens().color_white().with_alpha(if pressed { 32 } else { 16 })
            };
            ctx.fill_rect(target_frame, overlay, None);
        }
        if self.checked {
            let inset = 0.75_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            ctx.stroke_rect(
                Rect::new(
                    frame.x + inset,
                    frame.y + inset,
                    (frame.w - inset * 2.0).max(0.0),
                    (frame.h - inset * 2.0).max(0.0),
                ),
                fg,
                1.5,
                Some(Radius::uniform(radius)),
            );
        }
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
        if self.checked {
            if let Some(check) = geometry.check {
                crate::ui::widgets::icon::Icon::paint_in_frame(
                    ctx,
                    "check",
                    check,
                    fg,
                    10.0,
                );
            }
        }
        // 图标绘制（在文字之前）
        if !self.icon.is_empty() {
            let icon_size = font_size * 0.85;
            let icon_rect = Rect::new(
                geometry.text.x,
                geometry.text.y,
                icon_size.min(geometry.text.w),
                geometry.text.h,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                &self.icon,
                icon_rect,
                fg,
                icon_size,
            );
            let remaining = Rect::new(
                icon_rect.x + icon_rect.w + 4.0,
                geometry.text.y,
                (geometry.text.x + geometry.text.w - icon_rect.x - icon_rect.w - 4.0).max(0.0),
                geometry.text.h,
            );
            Self::paint_single_line(ctx, &self.text, remaining, fg, font_size);
        } else {
            Self::paint_single_line(ctx, &self.text, geometry.text, fg, font_size);
        }
        if let Some(close) = geometry.close {
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "x",
                close,
                fg,
                10.0,
            );
        }
        ctx.pop_clip();
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self::new("")
    }
}

// 把标签 Rust 交互与绘制内核融合为 UIX 声明的单一叶节点。
fn build_tag_view(kernel: Tag) -> ViewNode {
    ViewNode::leaf(kernel)
}

impl View for Tag {
    fn build(self) -> ViewNode {
        // UIX 拥有公开组件根，Rust 保留关闭、勾选、布局和绘制机制。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/tag/tag.uix")
    }
}

// 验证标签预设色服从主题 token 与明暗模式。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/tag__palette_tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod palette_tests;

// 验证 UIX 声明壳与 Rust 内核的单节点契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/display/tag__uix_tests.rs"]
mod uix_tests;

impl Tag {
    /// 使用文本创建默认中性色、不可关闭且不可勾选的标签。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: TagColor::Default,
            closable: false,
            font_size: DEFAULT_TAG_FONT_SIZE,
            custom_color: None,
            checkable: false,
            checked: false,
            visible: true,
            focused: false,
            icon: String::new(),
            last_size: Cell::new(Size::zero()),
            layout_requested: Cell::new(false),
            pending_action: Cell::new(None),
            hovered_target: Cell::new(None),
            pressed: Cell::new(None),
        }
    }
    /// 设置由当前主题解析的预设标签颜色。
    pub fn color(mut self, c: TagColor) -> Self {
        self.color = c;
        self
    }
    /// 设置固定背景色，并根据背景亮度选择黑色或白色前景。
    pub fn custom_color(mut self, c: Color) -> Self {
        self.custom_color = Some(c);
        self
    }
    /// 启用标签关闭入口与对应交互。
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    /// 设置标签是否允许勾选；关闭时同时清除勾选状态。
    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        if !v {
            self.checked = false;
        }
        self
    }
    /// 设置可勾选 Tag 的初始状态；用户交互后的状态在 reconcile 中保留。
    pub fn default_checked(mut self, checked: bool) -> Self {
        self.checkable = true;
        self.checked = checked;
        self
    }
    /// 设置标签字号；非法或非正值回退为默认字号。
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = normalized_tag_font_size(s);
        self
    }

    /// 在文字前显示 Lucide 图标。
    pub fn icon(mut self, name: impl Into<String>) -> Self {
        self.icon = name.into();
        self
    }

    fn intrinsic_size(&self) -> Size {
        let font_size = normalized_tag_font_size(self.font_size);
        let text_width = crate::draw::resources::font::text_backend::estimate_text_metrics(
            &self.text,
            f32::INFINITY,
            font_size,
        )
        .max_line_width;
        let icon_width = if !self.icon.is_empty() {
            font_size + 4.0
        } else {
            0.0
        };
        let w = text_width
            + icon_width
            + 16.0
            + if self.checkable { 14.0 } else { 0.0 }
            + if self.closable { 20.0 } else { 0.0 };
        Size::new(w, font_size + 8.0)
    }

    fn geometry(frame: Rect, checkable: bool, closable: bool) -> TagGeometry {
        let frame = Rect::new(
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
        );
        let close_width = if closable && frame.w > 0.0 {
            20.0_f32.min(frame.w)
        } else {
            0.0
        };
        let close = (close_width > 0.0).then(|| {
            Rect::new(
                frame.x + frame.w - close_width,
                frame.y,
                close_width,
                frame.h,
            )
        });
        let body = Rect::new(frame.x, frame.y, (frame.w - close_width).max(0.0), frame.h);
        let horizontal_padding = 8.0_f32.min(body.w * 0.25);
        let check_width = if checkable {
            14.0_f32.min((body.w - horizontal_padding * 2.0).max(0.0))
        } else {
            0.0
        };
        let check = (checkable && check_width > 0.0)
            .then(|| Rect::new(body.x + horizontal_padding, body.y, check_width, body.h));
        let reserved_check = if check.is_some() { check_width } else { 0.0 };
        let text_x = body.x + horizontal_padding + reserved_check;
        let text = Rect::new(
            text_x,
            body.y,
            (body.x + body.w - horizontal_padding - text_x).max(0.0),
            body.h,
        );
        TagGeometry {
            frame,
            body,
            close,
            check,
            text,
        }
    }

    fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line(value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn target_at(&self, point: crate::core::Point) -> Option<TagTarget> {
        let size = self.last_size.get();
        let size = if size.w > 0.0 && size.h > 0.0 {
            size
        } else {
            self.intrinsic_size()
        };
        let geometry = Self::geometry(
            Rect::new(0.0, 0.0, size.w, size.h),
            self.checkable,
            self.closable,
        );
        if self.closable && geometry.close.is_some_and(|close| close.contains(point)) {
            Some(TagTarget::Close)
        } else if self.checkable && geometry.body.contains(point) {
            Some(TagTarget::Body)
        } else {
            None
        }
    }

    fn commit_target(&mut self, target: TagTarget) {
        match target {
            TagTarget::Body => self.toggle_checked(),
            TagTarget::Close => self.dismiss(),
        }
    }

    /// 返回标签当前是否参与布局与绘制。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 返回标签当前的勾选状态。
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// 在标签可勾选时更新勾选状态，否则忽略请求。
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable {
            self.checked = checked;
        }
    }

    /// 隐藏可见标签并清理焦点与指针交互状态。
    pub fn close(&mut self) {
        if self.visible {
            self.visible = false;
            self.focused = false;
            self.hovered_target.set(None);
            self.pressed.set(None);
            self.layout_requested.set(true);
        }
    }

    /// 重新显示已隐藏的标签并请求布局。
    pub fn open(&mut self) {
        if !self.visible {
            self.visible = true;
            self.layout_requested.set(true);
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tag {
            text: self.text.clone(),
            color: self.color,
            closable: self.closable,
            font_size: self.font_size,
            custom_color: self.custom_color,
            checkable: self.checkable,
            checked: self.checked,
            visible: self.visible,
            icon: self.icon.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let was_checkable = self.checkable;
        let runtime_checked = self.checked;
        self.text = next.text;
        self.color = next.color;
        self.closable = next.closable;
        self.font_size = next.font_size;
        self.custom_color = next.custom_color;
        self.checkable = next.checkable;
        self.icon = next.icon;
        self.checked = if !self.checkable {
            false
        } else if was_checkable {
            runtime_checked
        } else {
            next.checked
        };
        self.hovered_target.set(None);
        self.pressed.set(None);
    }

    fn dismiss(&mut self) {
        self.close();
        self.pending_action.set(Some(TagAction::Closed));
    }

    fn toggle_checked(&mut self) {
        self.checked = !self.checked;
        self.pending_action.set(Some(if self.checked {
            TagAction::Checked
        } else {
            TagAction::Unchecked
        }));
    }
}
