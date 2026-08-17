use crate::core::Point;
use crate::platform::windowing::{KeyMod, MouseButton};
use crate::ui::ComponentId;

/// Result of a drag target operation.
/// 原公开面遗留（SMC-04 模块收口后无内部消费方；保留供外部集成）。
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragEventResult {
    Accepted,
    Rejected,
    Ignored,
}

/// Manages drag-and-drop for a component.
pub struct DragManager {
    dragging: bool,
    drag_start_pos: Point,
    drag_offset: Point,
    potential: bool,
    last_pos: Point,
    button: MouseButton,
    mods: KeyMod,
    target: Option<ComponentId>,
}

impl Default for DragManager {
    fn default() -> Self {
        Self {
            dragging: false,
            drag_start_pos: Point::zero(),
            drag_offset: Point::zero(),
            potential: false,
            last_pos: Point::zero(),
            button: MouseButton::None,
            mods: KeyMod::NONE,
            target: None,
        }
    }
}

impl DragManager {
    /// 创建没有活动或候选拖拽手势的管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从给定位置立即开始拖拽并重置累计偏移。
    pub fn start_drag(&mut self, pos: Point) {
        self.dragging = true;
        self.potential = false;
        self.drag_start_pos = pos;
        self.last_pos = pos;
        self.drag_offset = Point::zero();
    }

    /// 记录尚未越过激活阈值的候选拖拽手势及其输入上下文。
    pub fn begin_gesture(
        &mut self,
        target: Option<ComponentId>,
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    ) {
        self.potential = true;
        self.dragging = false;
        self.drag_start_pos = pos;
        self.last_pos = pos;
        self.drag_offset = Point::zero();
        self.button = button;
        self.mods = mods;
        self.target = target;
    }

    /// 将当前候选手势转换为活动拖拽。
    pub fn activate_gesture(&mut self) {
        self.potential = false;
        self.dragging = true;
    }

    /// 更新候选或活动拖拽的当前位置和起点累计偏移。
    pub fn update_drag(&mut self, pos: Point) {
        if self.dragging || self.potential {
            self.drag_offset =
                Point::new(pos.x - self.drag_start_pos.x, pos.y - self.drag_start_pos.y);
            self.last_pos = pos;
        }
    }

    /// 无条件更新最近一次指针位置。
    pub fn update_last_pos(&mut self, pos: Point) {
        self.last_pos = pos;
    }

    /// 结束当前手势并清除拖拽输入上下文和累计偏移。
    pub fn end_drag(&mut self) {
        self.dragging = false;
        self.potential = false;
        self.drag_offset = Point::zero();
        self.button = MouseButton::None;
        self.mods = KeyMod::NONE;
        self.target = None;
    }

    /// 返回当前手势是否已经激活为拖拽。
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// 返回当前是否存在尚未激活的候选拖拽手势。
    pub fn is_potential(&self) -> bool {
        self.potential
    }

    /// 返回当前手势关联的目标组件身份。
    pub fn target(&self) -> Option<ComponentId> {
        self.target
    }

    /// 返回当前手势的起始位置。
    pub fn start_pos(&self) -> Point {
        self.drag_start_pos
    }

    /// 返回最近记录的指针位置。
    pub fn last_pos(&self) -> Point {
        self.last_pos
    }

    /// 返回开始当前手势的鼠标按钮。
    pub fn button(&self) -> MouseButton {
        self.button
    }

    /// 返回给定按钮是否属于当前候选或活动手势。
    pub fn is_gesture_button(&self, button: MouseButton) -> bool {
        (self.dragging || self.potential) && self.button == button
    }

    /// 返回开始当前手势时记录的键盘修饰状态。
    pub fn mods(&self) -> KeyMod {
        self.mods
    }

    /// 返回当前位置相对手势起点的累计偏移。
    pub fn drag_offset(&self) -> Point {
        self.drag_offset
    }

    /// 当被注销组件是当前目标时结束拖拽手势。
    pub fn unregister_component(&mut self, component_id: ComponentId) {
        if self.target == Some(component_id) {
            self.end_drag();
        }
    }

    /// 在组件树清理时终止任何候选或活动拖拽。
    pub fn clear_tree_drag(&mut self) {
        self.end_drag();
    }
}
