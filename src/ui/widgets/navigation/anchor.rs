//! Anchor 锚点组件 — 页面内导航，滚动侦听高亮。
//!
//! 与 ScrollView 配合使用：监听滚动位置，自动高亮当前锚点。

use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{traits::GraphicsEngine, Color};
use crate::ui::{EventResult, SemanticEvent, SystemEvent, WidgetId, WidgetTree};
use std::cell::Cell;

define_widget! {
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
        pending_change: Cell<Option<usize>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let w = self.items.iter().map(|i| i.label.len() as f32 * 14.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(120.0)
            .max(120.0);
        Size::new(w, self.items.len() as f32 * 36.0)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let idx = (pos.y / 36.0) as usize;
                if idx < self.items.len() {
                    self.active_index = idx;
                    self.pending_change.set(Some(idx));
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter => EventResult::Handled,
            SystemEvent::PointerLeave => EventResult::Handled,
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let idx = self.pending_change.take()?;
        let href = self
            .items
            .get(idx)
            .map(|item| item.href.clone())
            .unwrap_or_default();
        Some(SemanticEvent::change(id, href))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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
            Rect::new(frame.x + frame.w - 1.0, frame.y, 1.0, frame.h),
            border_color, 1.0, None,
        );

        for (i, item) in self.items.iter().enumerate() {
            let y = frame.y + i as f32 * 36.0;
            let is_active = i == self.active_index;
            let color = if is_active { primary } else { text_secondary };

            // 激活态左侧指示条
            if is_active {
                ctx.fill_rect(Rect::new(frame.x, y, 3.0, 36.0), primary, None);
            }

            let label_x = frame.x + 16.0;
            let row_rect = Rect::new(frame.x, y, frame.w, 36.0);
            let label_y = ctx.visual_center_y(row_rect, 14.0);
            ctx.draw_text(&item.label, Point::new(label_x, label_y), color, 14.0);
        }
    }
}

/// 锚点项
#[derive(Debug, Clone)]
pub struct AnchorItem {
    /// 显示文本
    pub label: String,
    /// 锚点标识（对应目标 widget 的 ID 或 key）
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
    pub fn new(items: Vec<AnchorItem>) -> Self {
        let count = items.len();
        Self {
            items,
            active_index: 0,
            anchor_positions: vec![0.0; count],
            offset_top: 0.0,
            bg_color: None,
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
            if scroll_y < pos - self.offset_top - 10.0 {
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
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/navigation/anchor.rs"]
mod tests;

