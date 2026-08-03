use super::*;

pub trait EventExt: Into<ViewNode> + Sized {
    fn on_event(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_event(handler)
    }

    fn on_event_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_event_capture(handler)
    }

    fn on_pointer(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_pointer(handler)
    }

    fn on_pointer_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_pointer_capture(handler)
    }

    fn on_key(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_key(handler)
    }

    fn on_key_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_key_capture(handler)
    }

    fn on_focus(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_focus(handler)
    }

    fn on_scroll(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_scroll(handler)
    }

    fn tab_index(self, index: i32) -> ViewNode {
        self.into().tab_index(index)
    }

    fn focusable(self, focusable: bool) -> ViewNode {
        self.into().focusable(focusable)
    }

    fn focus_handle(self, handle: &FocusHandle) -> ViewNode {
        self.into().focus_handle(handle)
    }
}

impl<T: Into<ViewNode>> EventExt for T {}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供无障碍声明。
pub trait AccessibilityExt: Into<ViewNode> + Sized {
    fn accessibility(self, accessibility: AccessibilitySnapshot) -> ViewNode {
        self.into().accessibility(accessibility)
    }

    fn role(self, role: AccessibilityRole) -> ViewNode {
        self.into().role(role)
    }

    fn accessible_name(self, name: impl Into<String>) -> ViewNode {
        self.into().accessible_name(name)
    }

    fn accessibility_state(self, state: AccessibilityState) -> ViewNode {
        self.into().accessibility_state(state)
    }

    fn aria(self, name: &'static str, value: impl Into<String>) -> ViewNode {
        self.into().aria(name, value)
    }
}

impl<T: Into<ViewNode>> AccessibilityExt for T {}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供挂载过渡。
pub trait TransitionExt: Into<ViewNode> + Sized {
    fn enter_animation(self, animation: crate::ui::animation::AnimationConfig) -> ViewNode {
        self.into().enter_animation(animation)
    }

    fn leave_animation(self, animation: crate::ui::animation::AnimationConfig) -> ViewNode {
        self.into().leave_animation(animation)
    }

    fn stagger_enter(
        self,
        interval_secs: f64,
        animation: crate::ui::animation::AnimationConfig,
    ) -> ViewNode {
        self.into().stagger_enter(interval_secs, animation)
    }
}

impl<T: Into<ViewNode>> TransitionExt for T {}

/// 为所有 `Into<ViewNode>` 类型提供样式链（知识库：`C:\data\note\我的项目\软件\UIX App\使用.md`）。
///
/// 实现委托 [`ViewNode`] 同名方法，避免双份逻辑漂移。
/// **链式顺序**：先写 builder 专有方法（如 `button(…).primary().on_click(…)`），再写本 trait
/// （`bg` / `padding`…）——一旦进入 `ViewNode`，`primary` 等 builder 方法不可再调。
pub trait StyleExt: Into<ViewNode> + Sized {
    /// Assigns a stable selector for test automation. Put builder-specific
    /// methods before this call because it materializes a [`ViewNode`].
    fn automation_id(self, id: impl Into<String>) -> ViewNode {
        self.into().automation_id(id)
    }

    fn color(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().color(color)
    }

    fn font_size(self, size: impl Into<TypographyToken>) -> ViewNode {
        self.into().font_size(size)
    }

    fn bg(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg(color)
    }

    fn bg_animated(self, color: &crate::ui::animation::Animated<Color>) -> ViewNode {
        self.into().bg_animated(color)
    }

    /// 设置指针悬停时的背景色；未设置时沿用普通背景。
    fn bg_hover(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg_hover(color)
    }

    /// 设置焦点状态的背景色；未设置时沿用悬停或普通背景。
    fn bg_focus(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg_focus(color)
    }

    /// 设置按压或键盘激活期间的背景色；未设置时沿用普通背景。
    fn bg_active(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg_active(color)
    }

    fn padding(self, p: impl Into<EdgeInsets>) -> ViewNode {
        self.into().padding(p)
    }

    fn margin(self, m: impl Into<EdgeInsets>) -> ViewNode {
        self.into().margin(m)
    }

    fn width(self, w: f32) -> ViewNode {
        self.into().width(w)
    }

    fn width_animated(self, width: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().width_animated(width)
    }

    fn height(self, h: f32) -> ViewNode {
        self.into().height(h)
    }

    fn height_animated(self, height: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().height_animated(height)
    }

    fn offset(self, offset: Point) -> ViewNode {
        self.into().offset(offset)
    }

    fn offset_animated(self, offset: &crate::ui::animation::Animated<Point>) -> ViewNode {
        self.into().offset_animated(offset)
    }

    fn scale(self, scale: f32) -> ViewNode {
        self.into().scale(scale)
    }

    fn scale_animated(self, scale: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().scale_animated(scale)
    }

    fn flex_grow(self, g: f32) -> ViewNode {
        self.into().flex_grow(g)
    }

    fn flex_shrink(self, s: f32) -> ViewNode {
        self.into().flex_shrink(s)
    }

    fn align(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align(a)
    }

    fn justify(self, j: crate::ui::layout::JustifyContent) -> ViewNode {
        self.into().justify(j)
    }

    fn align_self(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align_self(a)
    }

    fn grid_cell(self, cell: usize) -> ViewNode {
        self.into().grid_cell(cell)
    }

    fn grid_span(self, columns: u32, rows: u32) -> ViewNode {
        self.into().grid_span(columns, rows)
    }

    fn gap(self, g: f32) -> ViewNode {
        self.into().gap(g)
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    fn overflow_content(self) -> ViewNode {
        self.into().overflow_content()
    }

    fn border(self, width: f32, color: impl Into<ColorValue>) -> ViewNode {
        self.into().border(width, color)
    }

    fn radius(self, r: f32) -> ViewNode {
        self.into().radius(r)
    }

    fn radius_animated(self, radius: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().radius_animated(radius)
    }

    fn opacity(self, o: f32) -> ViewNode {
        self.into().opacity(o)
    }

    fn opacity_animated(self, opacity: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().opacity_animated(opacity)
    }

    fn color_animated(self, color: &crate::ui::animation::Animated<Color>) -> ViewNode {
        self.into().color_animated(color)
    }

    /// 保留节点身份，但让整棵子树退出布局、绘制、命中与焦点候选。
    fn visible(self, visible: bool) -> ViewNode {
        self.into().visible(visible)
    }
}

impl<T: Into<ViewNode>> StyleExt for T {}
