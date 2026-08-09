//! BackTop 回到顶部 — 滚动超过阈值时显示返回顶部按钮。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::SnapshotFields;
use crate::ui::component::paint_context::PaintContext;
// 引入事件、键盘、声明式状态与组件树公开契约。
use crate::ui::{EventResult, KeyCode, State, SystemEvent, WidgetTree};

const DEFAULT_VISIBILITY_HEIGHT: f32 = 400.0;

component! {
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

        let primary = ctx.tokens().color_primary();
        let bg_elevated = ctx.tokens().color_bg_elevated();

        // 圆形按钮
        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = frame.w.min(frame.h) * 0.4;

        ctx.fill_circle(cx, cy, r, primary);
        ctx.fill_circle(cx, cy, r - 2.0, bg_elevated);
        crate::ui::widgets::general::icon::Icon::paint_in_frame(
            ctx,
            "chevron-up",
            frame,
            primary,
            14.0,
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
    pub fn new() -> Self {
        Self {
            visibility_height: DEFAULT_VISIBILITY_HEIGHT,
            visible: false,
            controlled_scroll_y: None,
            // 默认手动模式不持有应用状态句柄。
            scroll_binding: None,
            focused: false,
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

    pub fn visibility_height(mut self, v: f32) -> Self {
        self.visibility_height = Self::normalize_visibility_height(v);
        if let Some(scroll_y) = self.controlled_scroll_y {
            self.set_scroll_y(scroll_y);
        }
        self
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn intrinsic_size(&self) -> Size {
        if self.visible {
            Size::new(40.0, 40.0)
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

// 只在本组件边界验证声明式滚动状态与激活回写。
#[cfg(test)]
mod tests {
    // 引入待验证组件与事件处理 trait。
    use super::*;
    // 引入构造指针事件所需的几何与修饰键类型。
    use crate::core::Point;
    // 引入事件处理 trait 以调用组件契约。
    use crate::ui::{EventHandler, KeyMod, MouseButton};

    // 验证绑定状态决定可见性且激活写回顶部。
    #[test]
    fn bound_state_activation_writes_top() {
        // 创建超过默认阈值的应用滚动状态。
        let scroll_y = State::new(450.0_f32);
        // 绑定公开 BackTop 组件。
        let mut back_top = BackTop::new().scroll_state(&scroll_y);
        // 初始滚动位置应使按钮可见。
        assert!(back_top.is_visible());
        // 构造主指针激活事件。
        let result = back_top.on_event(&SystemEvent::PointerDown {
            // 坐标不影响组件自身的激活语义。
            pos: Point::zero(),
            // 只允许主按钮激活。
            button: MouseButton::Left,
            // 本测试不使用修饰键。
            mods: KeyMod::NONE,
        });
        // 激活必须由组件消费。
        assert_eq!(result, EventResult::Handled);
        // 应用拥有的状态必须被写回顶部。
        assert_eq!(scroll_y.get(), 0.0);
        // 当前组件必须立即隐藏。
        assert!(!back_top.is_visible());
    }
}
