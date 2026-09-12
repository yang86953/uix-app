use super::*;

/// 为所有可转换为 [`ViewNode`] 的 builder 提供事件与焦点声明。
pub trait EventExt: Into<ViewNode> + Sized {
    /// 注册目标阶段的通用系统事件处理器。
    fn on_event(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_event(handler)
    }

    /// 注册捕获阶段的通用系统事件处理器。
    fn on_event_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_event_capture(handler)
    }

    /// 注册目标阶段的指针事件处理器。
    fn on_pointer(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_pointer(handler)
    }

    /// 注册捕获阶段的指针事件处理器。
    fn on_pointer_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_pointer_capture(handler)
    }

    /// 注册目标阶段的键盘事件处理器。
    fn on_key(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_key(handler)
    }

    /// 注册捕获阶段的键盘事件处理器。
    fn on_key_capture(
        self,
        handler: impl FnMut(&SystemEvent) -> EventResult + 'static,
    ) -> ViewNode {
        self.into().on_key_capture(handler)
    }

    /// 注册焦点事件处理器。
    fn on_focus(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_focus(handler)
    }

    /// 注册滚动事件处理器。
    fn on_scroll(self, handler: impl FnMut(&SystemEvent) -> EventResult + 'static) -> ViewNode {
        self.into().on_scroll(handler)
    }

    /// 设置节点的 Tab 导航索引。
    fn tab_index(self, index: i32) -> ViewNode {
        self.into().tab_index(index)
    }

    /// 显式设置节点是否可聚焦。
    fn focusable(self, focusable: bool) -> ViewNode {
        self.into().focusable(focusable)
    }

    /// 将外部焦点句柄绑定到节点。
    fn focus_handle(self, handle: &FocusHandle) -> ViewNode {
        self.into().focus_handle(handle)
    }
}

impl<T: Into<ViewNode>> EventExt for T {}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供无障碍声明。
pub trait AccessibilityExt: Into<ViewNode> + Sized {
    /// 设置完整无障碍快照覆盖。
    fn accessibility(self, accessibility: AccessibilitySnapshot) -> ViewNode {
        self.into().accessibility(accessibility)
    }

    /// 设置无障碍角色。
    fn role(self, role: AccessibilityRole) -> ViewNode {
        self.into().role(role)
    }

    /// 设置无障碍名称。
    fn accessible_name(self, name: impl Into<String>) -> ViewNode {
        self.into().accessible_name(name)
    }

    /// 设置结构化无障碍状态。
    fn accessibility_state(self, state: AccessibilityState) -> ViewNode {
        self.into().accessibility_state(state)
    }

    /// 添加一个稳定的 ARIA 属性覆盖。
    fn aria(self, name: &'static str, value: impl Into<String>) -> ViewNode {
        self.into().aria(name, value)
    }
}

impl<T: Into<ViewNode>> AccessibilityExt for T {}

/// 为所有可转换为 [`ViewNode`] 的 builder 提供挂载过渡。
pub trait TransitionExt: Into<ViewNode> + Sized {
    /// 设置节点挂载时的进入动画。
    fn enter_animation(self, animation: crate::ui::animation::AnimationConfig) -> ViewNode {
        self.into().enter_animation(animation)
    }

    /// 设置节点卸载前的离开动画。
    fn leave_animation(self, animation: crate::ui::animation::AnimationConfig) -> ViewNode {
        self.into().leave_animation(animation)
    }

    /// 设置直接子节点依次进入的间隔与动画。
    fn stagger_enter(
        self,
        interval_secs: f64,
        animation: crate::ui::animation::AnimationConfig,
    ) -> ViewNode {
        self.into().stagger_enter(interval_secs, animation)
    }
}

impl<T: Into<ViewNode>> TransitionExt for T {}

/// 为所有 `Into<ViewNode>` 类型提供样式链（公开用法见仓库 `docs/使用/样式与主题.md`）。
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

    /// 设置文本颜色。
    fn color(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().color(color)
    }

    /// 设置背景颜色。
    fn background_color(self, color: impl Into<ColorValue>) -> ViewNode {
        // 委托 ViewNode 保存背景色值。
        self.into().background_color(color)
    }

    /// 绑定动画背景颜色。
    fn background_color_animated(
        // 消费当前 builder 或节点。
        self,
        // 借用共享动画源。
        color: &crate::ui::animation::Animated<Color>,
    ) -> ViewNode {
        // 委托 ViewNode 读取当前帧背景色。
        self.into().background_color_animated(color)
    }

    /// 设置字体大小令牌。
    fn font_size(self, size: impl Into<TypographyToken>) -> ViewNode {
        self.into().font_size(size)
    }

    /// 通过受控闭包精确更新节点样式，避免覆盖未声明字段。
    fn map_style(self, update: impl FnOnce(&mut Style)) -> ViewNode {
        // 先物化节点，再委托唯一的样式更新实现。
        self.into().map_style(update)
    }

    /// 设置常态背景色。
    fn bg(self, color: impl Into<ColorValue>) -> ViewNode {
        self.into().bg(color)
    }

    /// 绑定动画背景色。
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

    /// 设置四边内边距。
    fn padding(self, p: impl Into<EdgeInsets>) -> ViewNode {
        self.into().padding(p)
    }

    /// 设置四边外边距。
    fn margin(self, m: impl Into<EdgeInsets>) -> ViewNode {
        self.into().margin(m)
    }

    /// 设置显式宽度。
    fn width(self, w: f32) -> ViewNode {
        self.into().width(w)
    }

    /// 绑定动画宽度。
    fn width_animated(self, width: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().width_animated(width)
    }

    /// 设置显式高度。
    fn height(self, h: f32) -> ViewNode {
        self.into().height(h)
    }

    /// 绑定动画高度。
    fn height_animated(self, height: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().height_animated(height)
    }

    /// 设置最小宽度；接受 px 数值或 `StyleLength`。
    fn min_width(self, length: impl Into<crate::ui::theme::style::StyleLength>) -> ViewNode {
        self.into().min_width(length)
    }

    /// 设置最大宽度；接受 px 数值或 `StyleLength`。
    fn max_width(self, length: impl Into<crate::ui::theme::style::StyleLength>) -> ViewNode {
        self.into().max_width(length)
    }

    /// 设置最小高度；接受 px 数值或 `StyleLength`。
    fn min_height(self, length: impl Into<crate::ui::theme::style::StyleLength>) -> ViewNode {
        self.into().min_height(length)
    }

    /// 设置最大高度；接受 px 数值或 `StyleLength`。
    fn max_height(self, length: impl Into<crate::ui::theme::style::StyleLength>) -> ViewNode {
        self.into().max_height(length)
    }

    /// 设置绘制与命中偏移。
    fn offset(self, offset: Point) -> ViewNode {
        self.into().offset(offset)
    }

    /// 绑定动画绘制与命中偏移。
    fn offset_animated(self, offset: &crate::ui::animation::Animated<Point>) -> ViewNode {
        self.into().offset_animated(offset)
    }

    /// 设置以节点中心为基准的缩放。
    fn scale(self, scale: f32) -> ViewNode {
        self.into().scale(scale)
    }

    /// 绑定动画缩放。
    fn scale_animated(self, scale: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().scale_animated(scale)
    }

    /// 设置围绕布局帧中心应用的二维仿射视觉变换。
    fn affine_transform(self, transform: crate::draw::Transform) -> ViewNode {
        // 先物化节点，再委托唯一的视觉变换实现。
        self.into().affine_transform(transform)
    }

    /// 设置布局帧确定后解析的二维视觉变换原点。
    fn transform_origin(self, origin: crate::ui::TransformOrigin) -> ViewNode {
        // 先物化节点，再委托唯一的视觉变换原点实现。
        self.into().transform_origin(origin)
    }

    /// 设置指针命中节点时请求的平台光标。
    fn cursor(self, cursor: crate::platform::windowing::CursorType) -> ViewNode {
        // 委托 ViewNode 保存可继承的光标声明。
        self.into().cursor(cursor)
    }

    /// 设置 Flex 扩展系数。
    fn flex_grow(self, g: f32) -> ViewNode {
        self.into().flex_grow(g)
    }

    /// 设置 Flex 收缩系数。
    fn flex_shrink(self, s: f32) -> ViewNode {
        self.into().flex_shrink(s)
    }

    /// 设置容器交叉轴对齐方式。
    fn align(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align(a)
    }

    /// 设置容器主轴对齐方式。
    fn justify(self, j: crate::ui::layout::JustifyContent) -> ViewNode {
        self.into().justify(j)
    }

    /// 设置当前子项的交叉轴覆盖对齐方式。
    fn align_self(self, a: crate::ui::layout::AlignItems) -> ViewNode {
        self.into().align_self(a)
    }

    /// 设置兼容的一维 Grid 单元索引。
    fn grid_cell(self, cell: usize) -> ViewNode {
        self.into().grid_cell(cell)
    }

    /// 设置 Grid 跨列数和跨行数。
    fn grid_span(self, columns: u32, rows: u32) -> ViewNode {
        self.into().grid_span(columns, rows)
    }

    /// 设置直接子项的统一间距。
    fn gap(self, g: f32) -> ViewNode {
        self.into().gap(g)
    }

    /// 保留子项的自然主轴尺寸，用于 ScrollView 的内容容器。
    fn overflow_content(self) -> ViewNode {
        self.into().overflow_content()
    }

    /// 显式设置直接子树是否裁剪到当前节点边界。
    fn clip_content(self, clip: bool) -> ViewNode {
        // 委托 ViewNode 保存显式裁剪声明。
        self.into().clip_content(clip)
    }

    /// 设置统一宽度和颜色的边框。
    fn border(self, width: f32, color: impl Into<ColorValue>) -> ViewNode {
        self.into().border(width, color)
    }

    /// 设置或清除保留完整偏移的盒阴影定义。
    fn box_shadow(self, shadow: Option<crate::ui::theme::style::BoxShadowDef>) -> ViewNode {
        // 通过受控样式更新入口保留其他声明字段。
        self.into().map_style(|style| style.box_shadow = shadow)
    }

    /// 设置单值圆角半径；显式覆盖任何既有四角声明。
    fn radius(self, r: f32) -> ViewNode {
        self.into().radius(r)
    }

    /// 设置四角圆角半径；覆盖任何既有单值或四角声明。
    fn radius_corners(self, corners: crate::ui::theme::style::CornerRadii) -> ViewNode {
        self.into().radius_corners(corners)
    }

    /// 设置背景图尺寸策略（cover/contain/auto/显式尺寸）。
    fn background_size(
        self,
        size: crate::ui::theme::style::BackgroundSize,
    ) -> ViewNode {
        self.into().background_size(size)
    }

    /// 绑定动画圆角半径。
    fn radius_animated(self, radius: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().radius_animated(radius)
    }

    /// 设置节点透明度。
    fn opacity(self, o: f32) -> ViewNode {
        self.into().opacity(o)
    }

    /// 绑定动画透明度。
    fn opacity_animated(self, opacity: &crate::ui::animation::Animated<f32>) -> ViewNode {
        self.into().opacity_animated(opacity)
    }

    /// 绑定动画文本颜色。
    fn color_animated(self, color: &crate::ui::animation::Animated<Color>) -> ViewNode {
        self.into().color_animated(color)
    }

    /// 保留节点身份，但让整棵子树退出布局、绘制、命中与焦点候选。
    fn visible(self, visible: bool) -> ViewNode {
        self.into().visible(visible)
    }
}

impl<T: Into<ViewNode>> StyleExt for T {}
