//! BackTop 回到顶部 — 滚动超过阈值时显示返回顶部按钮。

use crate::core::{Constraints, Rect, Size};
// 引入 UIX 声明壳物化叶节点所需的 View 契约。
use crate::ui::view::{View, ViewNode};
// 引入 UIX 静态颜色角色到当前主题的解析契约。
use crate::ui::theme::style::{ColorValue, PaletteColor};
// 引入浮层背景中性色角色。
use crate::ui::theme::NeutralRole;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::SnapshotFields;
use crate::widget;
// 引入事件、键盘、声明式状态与组件树公开契约。
use crate::ui::{EventResult, KeyCode, State, SystemEvent, WidgetTree};

const DEFAULT_VISIBILITY_HEIGHT: f32 = 400.0;

// 保存由 UIX 声明、由 Rust 绘制内核消费的紧凑静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
struct BackTopVisual {
    // 保存 Lucide 图标名称。
    icon_name: &'static str,
    // 保存图标绘制尺寸。
    icon_size: f32,
    // 保存组件方形固有边长。
    extent: f32,
    // 保存环形边框宽度。
    ring_width: f32,
    // 保存环形外半径相对当前最短边的比例。
    ring_radius_ratio: f32,
    // 保存环形边框的主题色角色。
    ring_color: ColorValue,
    // 保存环形内部的主题色角色。
    fill_color: ColorValue,
}

impl Default for BackTopVisual {
    fn default() -> Self {
        Self {
            // 直接 Rust 叶构造保留旧版默认图标。
            icon_name: "chevron-up",
            // 保留旧版图标尺寸。
            icon_size: 14.0,
            // 保留旧版固有边长。
            extent: 40.0,
            // 保留原有内外圆两像素差。
            ring_width: 2.0,
            // 保留原有外圆占当前最短边四成的缩放契约。
            ring_radius_ratio: 0.4,
            // 环形边框继续使用主色令牌。
            ring_color: ColorValue::Palette(PaletteColor::Primary),
            // 内部继续使用浮层背景令牌。
            fill_color: ColorValue::Neutral(NeutralRole::BgElevated),
        }
    }
}

widget! {
    /// BackTop — 回到顶部按钮。
    pub struct BackTop {
        /// 滚动超过此高度才显示
        visibility_height: f32,
        /// 是否可见
        visible: bool,
        /// 声明式滚动位置；None 表示由 update_visibility 维护运行态
        controlled_scroll_y: Option<f32>,
        /// 应用拥有的滚动状态句柄；组件只克隆句柄并在激活时写回顶部。
        scroll_binding: Option<State<f32>>,
        focused: bool,
        #[snapshot(skip)]
        /// UIX 声明的静态图标、几何与主题色角色。
        visual: BackTopVisual,
    }

    visible => (&self) -> bool { self.visible }

    tab_index => (&self) -> i32 { i32::from(self.visible) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if !self.visible {
            return EventResult::NotHandled;
        }
        match event {
            SystemEvent::PointerDown {
                button: crate::ui::MouseButton::Left,
                ..
            }
            | SystemEvent::KeyDown {
                key: KeyCode::Enter | KeyCode::Space,
                ..
            } => {
                // 激活事实同步写回绑定状态并立即隐藏按钮。
                self.request_top();
                // 阻止同一次激活继续冒泡。
                EventResult::Handled
            },
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        if !self.visible { return; }

        // 主题颜色选择由 UIX 声明，当前绘制边界只做令牌解析。
        let primary = self.visual.ring_color.resolve(ctx.tokens());
        // 解析环形内部背景色。
        let bg_elevated = self.visual.fill_color.resolve(ctx.tokens());

        // 按 UIX 声明的外环比例居中解析圆心。
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        // 使用当前 frame 最短边乘以声明比例，保持旧版伸缩质量。
        let r = frame.w.min(frame.h).max(0.0)
            * self.visual.ring_radius_ratio.clamp(0.0, 0.5);

        ctx.fill_circle(cx, cy, r, primary);
        ctx.fill_circle(
            cx,
            cy,
            (r - self.visual.ring_width).max(0.0),
            bg_elevated,
        );
        crate::ui::widgets::general::icon::Icon::paint_in_frame(
            ctx,
            self.visual.icon_name,
            frame,
            primary,
            self.visual.icon_size,
        );
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                primary,
                1.5,
                Some(crate::draw::Radius::uniform(frame.w.min(frame.h) * 0.5)),
            );
        }
    }
}

impl Default for BackTop {
    fn default() -> Self {
        Self::new()
    }
}

impl BackTop {
    /// 创建使用默认可见阈值且初始隐藏的返回顶部按钮。
    pub fn new() -> Self {
        Self {
            visibility_height: DEFAULT_VISIBILITY_HEIGHT,
            visible: false,
            controlled_scroll_y: None,
            // 默认手动模式不持有应用状态句柄。
            scroll_binding: None,
            focused: false,
            // 直接 Rust 叶路径保留与 UIX 声明相同的兼容默认。
            visual: BackTopVisual::default(),
        }
    }

    /// 手动模式下更新当前滚动位置；返回可见性是否发生变化。
    pub fn update_visibility(&mut self, scroll_y: f32) -> bool {
        // 手动更新显式退出声明式状态绑定模式。
        self.scroll_binding = None;
        self.controlled_scroll_y = None;
        self.set_scroll_y(scroll_y)
    }

    /// 声明式设置当前滚动位置，适合从 `State<f32>` 读取后随 reconcile 更新。
    pub fn scroll_y(mut self, scroll_y: f32) -> Self {
        // 数值快照模式不保留旧 State 句柄。
        self.scroll_binding = None;
        let scroll_y = Self::normalize_scroll_y(scroll_y);
        self.controlled_scroll_y = Some(scroll_y);
        self.set_scroll_y(scroll_y);
        self
    }

    /// 双向绑定应用拥有的滚动位置；激活 BackTop 时写回零。
    pub fn scroll_state(mut self, state: &State<f32>) -> Self {
        // 克隆轻量状态句柄而不复制或夺取应用状态所有权。
        self.scroll_binding = Some(state.clone());
        // 读取当前快照供本轮可见性计算与 reconcile 使用。
        let scroll_y = Self::normalize_scroll_y(state.get());
        // 标记本轮声明式滚动值。
        self.controlled_scroll_y = Some(scroll_y);
        // 立即同步当前可见性。
        self.set_scroll_y(scroll_y);
        // 返回可继续配置的组件。
        self
    }

    /// 设置按钮开始显示所需的纵向滚动阈值。
    pub fn visibility_height(mut self, v: f32) -> Self {
        self.visibility_height = Self::normalize_visibility_height(v);
        if let Some(scroll_y) = self.controlled_scroll_y {
            self.set_scroll_y(scroll_y);
        }
        self
    }
    /// 返回按钮当前是否达到显示条件。
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn intrinsic_size(&self) -> Size {
        if self.visible {
            Size::new(self.visual.extent, self.visual.extent)
        } else {
            Size::zero()
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::BackTop {
            visibility_height: self.visibility_height,
            visible: self.visible,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.visibility_height = Self::normalize_visibility_height(next.visibility_height);
        // reconcile 接管新 View 中同一应用状态的轻量句柄。
        self.scroll_binding = next.scroll_binding;
        self.controlled_scroll_y = next.controlled_scroll_y;
        // 同步 UIX 声明的静态视觉配置，不覆盖运行交互状态。
        self.visual = next.visual;
        // 绑定模式优先读取当前应用事实，避免采用生成阶段后的过期快照。
        if let Some(scroll_y) = self.scroll_binding.as_ref().map(State::get) {
            // 同步绑定值派生的可见性。
            self.controlled_scroll_y = Some(scroll_y);
            // 应用规范化后的滚动值。
            self.set_scroll_y(scroll_y);
        } else if let Some(scroll_y) = self.controlled_scroll_y {
            self.set_scroll_y(scroll_y);
        }
    }

    // 处理指针或键盘激活请求。
    fn request_top(&mut self) {
        // 绑定存在时把应用滚动事实更新为顶部。
        if let Some(state) = &self.scroll_binding {
            // State 自己负责通知声明式依赖。
            state.set(0.0);
        }
        // 当前组件同步保存顶部快照。
        self.controlled_scroll_y = Some(0.0);
        // 立即隐藏组件并清理焦点状态。
        self.set_scroll_y(0.0);
    }

    fn set_scroll_y(&mut self, scroll_y: f32) -> bool {
        let visible = Self::normalize_scroll_y(scroll_y) > self.visibility_height;
        let changed = self.visible != visible;
        self.visible = visible;
        if !visible {
            self.focused = false;
        }
        changed
    }

    fn normalize_visibility_height(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            DEFAULT_VISIBILITY_HEIGHT
        }
    }

    fn normalize_scroll_y(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

// 向 UIX 静态模板提供零分配 Lucide 图标角色。
const fn back_top_chevron_up() -> &'static str {
    "chevron-up"
}

// 向 UIX 静态模板提供零分配主色令牌。
const fn back_top_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

// 向 UIX 静态模板提供零分配浮层背景令牌。
const fn back_top_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

// 把 UIX 声明的静态配置融合进原有 BackTop 叶内核。
#[allow(clippy::too_many_arguments)]
fn build_back_top_view(
    mut kernel: BackTop,
    icon_name: &'static str,
    icon_size: f32,
    extent: f32,
    ring_width: f32,
    ring_radius_ratio: f32,
    ring_color: ColorValue,
    fill_color: ColorValue,
) -> ViewNode {
    // 一次性拷贝紧凑配置，不创建子节点或包装容器。
    kernel.visual = BackTopVisual {
        icon_name,
        icon_size,
        extent,
        ring_width,
        ring_radius_ratio,
        ring_color,
        fill_color,
    };
    // 保持原有单 Widget 树形和分配数量。
    ViewNode::leaf(kernel)
}

impl View for BackTop {
    fn build(self) -> ViewNode {
        // 使用局部名称作为 UIX 表达式的拥有型 Rust 内核。
        let kernel = self;
        // 静态视觉完全从 UIX 文件物化，Rust 只消费生成结果。
        crate::uix!("src/ui/widgets/containers/back_top/back_top.uix")
    }
}

// 只在本组件边界验证声明式滚动状态与激活回写。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/containers/back_top__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
