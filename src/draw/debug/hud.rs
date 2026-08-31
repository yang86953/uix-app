//! DebugHudState — 调试 HUD 的逐窗口交互状态。
//!
//! 单一职责：保存帧 HUD 的折叠、自定义位置与拖动会话，并把「上一帧布局矩形」
//! 作为输入命中与绘制布局的唯一几何事实。不持有平台事件类型；指针语义由
//! 宿主循环把原生事件翻译为 `pointer_down/move/up` 三个纯几何入口。

use std::cell::{Cell, RefCell};

use crate::core::{Point, Rect};

use super::overlay::DebugRenderService;

/// 折叠态药丸的固定宽度，容纳「UIX DEBUG」与紧凑帧耗时。
const COLLAPSED_PILL_W: f32 = 190.0;
/// 折叠态药丸的固定高度。
const COLLAPSED_PILL_H: f32 = 30.0;
/// 标题行折叠按钮的宽度。
const COLLAPSE_BUTTON_W: f32 = 15.0;
/// 标题行折叠按钮的高度。
const COLLAPSE_BUTTON_H: f32 = 13.0;
/// 折叠按钮距面板右缘的留白。
const COLLAPSE_BUTTON_RIGHT: f32 = 7.0;
/// HUD 面板上内边距；展开态高度公式与绘制侧共用同一常量。
pub(crate) const HUD_PAD_TOP: f32 = 10.0;

/// 一次 `layout` 计算出的几何事实，供绘制与命中共用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HudLayout {
    /// HUD 完整矩形（含标题行；折叠态即药丸本身）。
    pub bounds: Rect,
    /// 标题行折叠按钮矩形。
    pub collapse_button: Rect,
    /// 可发起拖动的标题条矩形（折叠态为药丸去掉按钮的剩余部分）。
    pub drag_handle: Rect,
    /// 当前是否处于折叠态。
    pub collapsed: bool,
}

/// 调试 HUD 的逐窗口交互状态。
///
/// 所有字段使用内部可变性：绘制侧只读布局、写入上一帧矩形；输入侧在
/// 共享引用上推进拖动会话。坐标与 HUD 绘制同处一个表面空间。
#[derive(Debug, Default)]
pub struct DebugHudState {
    /// 当前是否折叠为药丸。
    collapsed: Cell<bool>,
    /// 用户拖动后的自定义左上角；`None` 表示保持默认右上角锚点。
    position: Cell<Option<Point>>,
    /// 上一帧布局矩形；输入命中据此判定，未绘制过 HUD 时为空。
    bounds: Cell<Option<Rect>>,
    /// 进行中的拖动会话：记录指针相对面板左上角的抓取偏移。
    drag: RefCell<Option<Point>>,
}

impl DebugHudState {
    /// 创建处于默认锚点、展开态的 HUD 状态。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回当前是否折叠为药丸。
    pub fn collapsed(&self) -> bool {
        self.collapsed.get()
    }

    /// 设置折叠态，返回设置后的值。
    pub fn set_collapsed(&self, collapsed: bool) -> bool {
        self.collapsed.set(collapsed);
        collapsed
    }

    /// 翻转折叠态并返回新值。
    pub fn toggle_collapsed(&self) -> bool {
        let next = !self.collapsed.get();
        self.collapsed.set(next);
        next
    }

    /// 返回用户自定义的 HUD 左上角；未拖动过时为 `None`。
    pub fn position(&self) -> Option<Point> {
        self.position.get()
    }

    /// 返回上一帧布局矩形；从未绘制过 HUD 时为 `None`。
    pub fn bounds(&self) -> Option<Rect> {
        self.bounds.get()
    }

    /// 返回当前是否存在进行中的拖动会话。
    pub fn is_dragging(&self) -> bool {
        self.drag.borrow().is_some()
    }

    /// 指针按下：命中折叠按钮时翻转折叠态，命中标题条时开始拖动。
    ///
    /// 返回是否消费该按下；落在展开面板正文区或面板之外时不消费，
    /// 面板之外的按下同时终结遗留的拖动会话，避免失去配对抬起后拖动卡死。
    pub fn pointer_down(&self, pos: Point) -> bool {
        let Some(bounds) = self.bounds.get() else {
            return false;
        };
        if !bounds.contains(pos) {
            self.drag.borrow_mut().take();
            return false;
        }
        let layout = self.layout_of(bounds);
        if layout.collapse_button.contains(pos) {
            self.toggle_collapsed();
            return true;
        }
        if layout.drag_handle.contains(pos) {
            *self.drag.borrow_mut() = Some(Point::new(pos.x - bounds.x, pos.y - bounds.y));
            return true;
        }
        false
    }

    /// 指针移动：仅存在拖动会话时更新自定义位置。
    ///
    /// 返回是否消费该移动；位置先落原值，出界约束由下一次 `layout` 收敛。
    pub fn pointer_move(&self, pos: Point) -> bool {
        let grab = *self.drag.borrow();
        let Some(grab) = grab else {
            return false;
        };
        self.position
            .set(Some(Point::new(pos.x - grab.x, pos.y - grab.y)));
        true
    }

    /// 指针抬起：结束拖动会话，返回本次是否确有拖动。
    pub fn pointer_up(&self, _pos: Point) -> bool {
        self.drag.borrow_mut().take().is_some()
    }

    /// 计算当前状态的 HUD 几何，并把结果登记为输入命中事实。
    ///
    /// 自定义位置会被钳制在表面内并写回；`line_count` 为展开态正文行数
    ///（含提示行），折叠态忽略该参数。
    pub fn layout(&self, line_count: usize, surface_w: i32, surface_h: i32) -> HudLayout {
        let laid = self.layout_of(self.target_rect(line_count, surface_w, surface_h));
        self.bounds.set(Some(laid.bounds));
        if self.position.get().is_some() {
            self.position
                .set(Some(Point::new(laid.bounds.x, laid.bounds.y)));
        }
        laid
    }

    // 布局几何只依赖折叠态与表面尺寸；命中复用同一套矩形规则。
    fn target_rect(&self, line_count: usize, surface_w: i32, surface_h: i32) -> Rect {
        let margin = DebugRenderService::HUD_MARGIN;
        let surface_w = surface_w as f32;
        let surface_h = surface_h as f32;
        let (w, h) = if self.collapsed.get() {
            (COLLAPSED_PILL_W, COLLAPSED_PILL_H)
        } else {
            let max_x = (surface_w - margin * 2.0).max(1.0);
            let w = DebugRenderService::HUD_PANEL_W.min(max_x);
            let h = 2.0 * HUD_PAD_TOP + DebugRenderService::HUD_LINE_H * (line_count as f32 + 1.0);
            (w, h)
        };
        let (mut x, mut y) = match self.position.get() {
            Some(pos) => (pos.x, pos.y),
            None => (surface_w - w - margin, margin),
        };
        x = x.clamp(margin, (surface_w - w - margin).max(margin));
        y = y.clamp(margin, (surface_h - h - margin).max(margin));
        Rect::new(x, y, w, h)
    }

    // 在给定面板矩形上推导按钮与拖动条；命中与布局共用。
    fn layout_of(&self, bounds: Rect) -> HudLayout {
        let title_h = if self.collapsed.get() {
            bounds.h
        } else {
            DebugRenderService::HUD_LINE_H
        };
        let button = Rect::new(
            bounds.x + bounds.w - COLLAPSE_BUTTON_W - COLLAPSE_BUTTON_RIGHT,
            bounds.y + ((title_h - COLLAPSE_BUTTON_H) / 2.0).max(0.0),
            COLLAPSE_BUTTON_W,
            COLLAPSE_BUTTON_H,
        );
        let handle = Rect::new(bounds.x, bounds.y, (button.x - bounds.x).max(0.0), title_h);
        HudLayout {
            bounds,
            collapse_button: button,
            drag_handle: handle,
            collapsed: self.collapsed.get(),
        }
    }
}
