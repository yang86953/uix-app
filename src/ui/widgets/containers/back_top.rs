//! BackTop 回到顶部 — 滚动超过阈值时显示返回顶部按钮。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, KeyCode, SystemEvent, WidgetTree};

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
            } => EventResult::Handled,
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
            focused: false,
        }
    }

    /// 手动模式下更新当前滚动位置；返回可见性是否发生变化。
    pub fn update_visibility(&mut self, scroll_y: f32) -> bool {
        self.controlled_scroll_y = None;
        self.set_scroll_y(scroll_y)
    }

    /// 声明式设置当前滚动位置，适合从 `State<f32>` 读取后随 reconcile 更新。
    pub fn scroll_y(mut self, scroll_y: f32) -> Self {
        let scroll_y = Self::normalize_scroll_y(scroll_y);
        self.controlled_scroll_y = Some(scroll_y);
        self.set_scroll_y(scroll_y);
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
        self.controlled_scroll_y = next.controlled_scroll_y;
        if let Some(scroll_y) = self.controlled_scroll_y {
            self.set_scroll_y(scroll_y);
        }
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
