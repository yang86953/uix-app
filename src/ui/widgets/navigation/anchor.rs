//! Anchor 锚点组件 — 页面内导航，滚动侦听高亮。
//!
//! 与 ScrollView 配合使用：监听滚动位置，自动高亮当前锚点。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;
use std::rc::Rc;

component! {
    /// Anchor — 锚点导航条。
    ///
    /// 传入 LinkItem 列表，点击跳转到对应锚点，滚动时高亮当前锚点。
    pub struct Anchor {
        /// 锚点链接列表
        items: Vec<AnchorItem>,
        /// 当前高亮索引
        active_index: usize,
        /// 每个锚点对应的滚动 Y 位置（由外部注入或 on_update 计算）
        anchor_positions: Vec<f32>,
        /// 容器顶部偏移（Header 高度等）
        offset_top: f32,
        /// 背景色
        bg_color: Option<Color>,
        show_ink: bool,
        bounds: f32,
        container_enabled: bool,
        #[snapshot(skip)]
        container_view: Option<Rc<dyn Fn() -> crate::ui::view::ViewNode>>,
        last_frame: Cell<Option<Rect>>,
        focused: bool,
        pending_change: Cell<Option<usize>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.items.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        if !self.container_enabled {
            return Vec::new();
        }
        let content = self.container_rect(frame);
        children.iter().map(|child| (child.id, content)).collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(if self.container_enabled { self.container_rect(frame) } else { frame })
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        self.container_view
            .as_ref()
            .map(|factory| vec![factory()])
            .unwrap_or_default()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if !self.local_navigation_rect().contains(*pos) || pos.y < 0.0 {
                    return EventResult::NotHandled;
                }
                let idx = (pos.y / 36.0) as usize;
                if idx < self.items.len() {
                    self.select(idx, true);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter => EventResult::Handled,
            SystemEvent::PointerLeave => EventResult::Handled,
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    self.move_active(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select(0, true);
                    EventResult::Handled
                }
                KeyCode::End if !self.items.is_empty() => {
                    self.select(self.items.len() - 1, true);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.items.is_empty() => {
                    self.pending_change.set(Some(self.active_index));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let idx = self.pending_change.take()?;
        let href = self
            .items
            .get(idx)
            .map(|item| item.href.clone())
            .unwrap_or_default();
        Some(SemanticEvent::change(id, href))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        self.last_frame.set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let navigation = self.navigation_rect(frame);
        // 背景
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
        let primary = ctx.tokens().color_primary();
        let _text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let border_color = ctx.tokens().color_border_secondary();

        // 分割线
        ctx.stroke_rect(
            Rect::new(navigation.x + navigation.w - 1.0, navigation.y, 1.0, navigation.h),
            border_color, 1.0, None,
        );

        ctx.push_clip(navigation);
        for (i, item) in self.items.iter().enumerate() {
            let y = navigation.y + i as f32 * 36.0;
            let is_active = i == self.active_index;
            let color = if is_active { primary } else { text_secondary };

            // 激活态左侧指示条
            if is_active && self.show_ink {
                ctx.fill_rect(Rect::new(navigation.x, y, 3.0, 36.0), primary, None);
            }

            let label_x = navigation.x + 16.0;
            let row_rect = Rect::new(navigation.x, y, navigation.w, 36.0);
            let label_y = ctx.visual_center_y(row_rect, 14.0);
            ctx.draw_text(&item.label, Point::new(label_x, label_y), color, 14.0);
        }
        ctx.pop_clip();

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                navigation,
                primary,
                1.5,
                Some(crate::draw::Radius::uniform(
                    ctx.tokens().border_radius_sm(),
                )),
            );
        }
    }
}

/// 锚点项
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorItem {
    /// 显示文本
    pub label: String,
    /// 锚点标识（对应目标 component 的 ID 或 key）
    pub href: String,
}

impl AnchorItem {
    pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
        }
    }
}

impl Anchor {
    fn intrinsic_size(&self) -> Size {
        let navigation = self.intrinsic_navigation_size();
        if self.container_enabled {
            Size::new(navigation.w + 320.0, navigation.h.max(240.0))
        } else {
            navigation
        }
    }

    fn intrinsic_navigation_size(&self) -> Size {
        let w = self
            .items
            .iter()
            .map(|i| i.label.chars().count() as f32 * 14.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(120.0)
            .max(120.0);
        Size::new(w, self.items.len() as f32 * 36.0)
    }

    pub fn new(items: Vec<AnchorItem>) -> Self {
        let count = items.len();
        Self {
            items,
            active_index: 0,
            anchor_positions: vec![0.0; count],
            offset_top: 0.0,
            bg_color: None,
            show_ink: true,
            bounds: 10.0,
            container_enabled: false,
            container_view: None,
            last_frame: Cell::new(None),
            focused: false,
            pending_change: Cell::new(None),
        }
    }

    /// 设置锚点 Y 位置列表（由外部根据内容布局计算后注入）
    pub fn set_positions(&mut self, positions: Vec<f32>) {
        self.anchor_positions = positions;
    }

    /// 根据当前滚动 Y 更新高亮
    pub fn update_active(&mut self, scroll_y: f32) {
        let mut idx = self.items.len().saturating_sub(1);
        for (i, &pos) in self.anchor_positions.iter().enumerate() {
            if scroll_y < pos - self.offset_top - self.bounds {
                idx = i.saturating_sub(1);
                break;
            }
        }
        self.active_index = idx;
    }

    pub fn set_offset_top(mut self, v: f32) -> Self {
        self.offset_top = v;
        self
    }

    pub fn target_offset(self, offset: f32) -> Self {
        self.set_offset_top(offset)
    }

    pub fn show_ink(mut self, show: bool) -> Self {
        self.show_ink = show;
        self
    }

    pub fn bounds(mut self, bounds: f32) -> Self {
        self.bounds = if bounds.is_finite() {
            bounds.max(0.0)
        } else {
            10.0
        };
        self
    }

    pub fn container<F, V>(mut self, factory: F) -> Self
    where
        F: Fn() -> V + 'static,
        V: crate::ui::view::View,
    {
        self.container_enabled = true;
        self.container_view = Some(Rc::new(move || crate::ui::view::View::build(factory())));
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn active_index(&self) -> usize {
        self.active_index
    }
    pub fn active_href(&self) -> &str {
        self.items
            .get(self.active_index)
            .map(|i| i.href.as_str())
            .unwrap_or("")
    }
    pub fn items(&self) -> &[AnchorItem] {
        &self.items
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let old_position_count = self.anchor_positions.len();
        self.items = next.items;
        self.offset_top = next.offset_top;
        self.bg_color = next.bg_color;
        self.show_ink = next.show_ink;
        self.bounds = next.bounds;
        self.container_enabled = next.container_enabled;
        self.container_view = next.container_view;
        self.active_index = self.active_index.min(self.items.len().saturating_sub(1));
        if old_position_count != self.items.len() {
            self.anchor_positions = vec![0.0; self.items.len()];
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Anchor {
            items: self.items.clone(),
            active_index: self.active_index,
            offset_top: self.offset_top,
            bg_color: self.bg_color,
        }
    }

    fn select(&mut self, index: usize, emit: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index;
        self.active_index = index;
        if emit && changed {
            self.pending_change.set(Some(index));
        }
    }

    fn move_active(&mut self, forward: bool) {
        if self.items.is_empty() {
            return;
        }
        let next = if forward {
            (self.active_index + 1).min(self.items.len() - 1)
        } else {
            self.active_index.saturating_sub(1)
        };
        self.select(next, true);
    }

    fn navigation_rect(&self, frame: Rect) -> Rect {
        let width = self.intrinsic_navigation_size().w.min(frame.w).max(0.0);
        Rect::new(frame.x, frame.y, width, frame.h)
    }

    fn container_rect(&self, frame: Rect) -> Rect {
        let navigation = self.navigation_rect(frame);
        Rect::new(
            navigation.x + navigation.w,
            frame.y,
            (frame.w - navigation.w).max(0.0),
            frame.h,
        )
    }

    fn local_navigation_rect(&self) -> Rect {
        self.navigation_rect(self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        }))
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
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
        )
    }
}
