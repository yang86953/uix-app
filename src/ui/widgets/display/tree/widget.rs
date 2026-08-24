use crate::core::{Constraints, Rect, Size};
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetId, WidgetTree,
};
use crate::widget;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{DropPosition, TreeNode, TreePointerAction, TreeVisual};

/// 树节点的扁平化行数据：标题、键、图标、层级深度与交互状态。
pub(crate) struct FlatNode {
    pub(crate) title: String,
    pub(crate) key: String,
    pub(crate) icon: String,
    /// 缩进层级（0 为根级）。
    pub(crate) depth: usize,
    pub(crate) has_children: bool,
    pub(crate) expanded: bool,
    pub(crate) disabled: bool,
    pub(crate) checkable: bool,
    pub(crate) checked: bool,
}

widget! {
    /// 树形列表组件：支持多选、搜索、拖拽、展开/折叠与键盘导航。
    pub struct Tree {
        pub(crate) nodes: Vec<TreeNode>,
        pub(crate) flat: Vec<FlatNode>,
        pub(crate) selected_key: String,
        pub(crate) selected_keys: Vec<String>,
        pub(crate) expanded_keys: Vec<String>,
        pub(crate) multiple: bool,
        pub(crate) searchable: bool,
        pub(crate) search_query: String,
        pub(crate) draggable: bool,
        pub(crate) drop_callback: Option<Rc<dyn Fn(&str, &str, DropPosition)>>,
        pub(crate) expand_callback: Option<Rc<dyn Fn(&str)>>,
        pub(crate) focused: bool,
        pub(crate) hovered_action: Option<TreePointerAction>,
        pub(crate) pressed_action: Option<TreePointerAction>,
        pub(crate) dragged_key: Option<String>,
        pub(crate) pending_change: RefCell<Option<String>>,
        pub(crate) body_scroll: VirtualListScroll,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) layout_requested: Cell<bool>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        // 全部实例共享 UIX 声明固化后的只读视觉配置。
        #[snapshot(skip)]
        pub(crate) visual: &'static TreeVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    // 可搜索时接收文本输入。
    accepts_text_input => (&self) -> bool { self.searchable }

    // 文本输入光标位置：随搜索词宽度右移。
    text_input_cursor_rect => (&self) -> Rect {
        let frame = self.local_frame();
        // 估算文本宽度并限制在框内。
        let chrome = self.visual.chrome;
        let width = (self.search_query.chars().count() as f32 * chrome.search_cursor_char_width
            + chrome.search_cursor_base_width)
            .clamp(
                chrome.search_cursor_base_width,
                (frame.w - chrome.search_horizontal_padding * 2.0)
                    .max(chrome.search_cursor_base_width),
            );
        Rect::new(
            frame.x + chrome.search_horizontal_padding + width,
            frame.y + chrome.search_cursor_y,
            chrome.search_cursor_width,
            chrome.search_cursor_height,
        )
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    // 滚动增量（累积后返回并清零）。
    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    // 视口滚动偏移：仅垂直方向由虚拟列表驱动。
    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    // 取出布局请求标记（一次性）。
    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    // 事件入口：滚轮、指针交互、键盘导航与文本搜索。
    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            // 滚轮：仅在列表区域内（非搜索栏）滚动虚拟列表。
            SystemEvent::Wheel { pos, delta } => {
                let frame = self.local_frame();
                if !frame.contains(*pos)
                    || self.searchable
                        && pos.y < frame.y + self.search_height_for_intrinsic()
                {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.body_viewport_height();
                let dy = self.body_scroll.scroll_by_wheel(
                    delta.y,
                    self.flat.len(),
                    self.visual.geometry.row_height,
                    viewport_h,
                );
                // 实际滚动发生才上报脏区。
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            // 左键按下：命中操作区则记录按压态；可拖拽时记下拖拽源。
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(action) = self.action_at_point(*pos) {
                    if self.draggable {
                        if let TreePointerAction::Select(key) = &action {
                            self.dragged_key = Some(key.clone());
                        }
                    }
                    self.hovered_action = Some(action.clone());
                    self.pressed_action = Some(action);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            // 左键抬起：命中同位置则提交操作；拖拽时触发放置回调。
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(pressed) = self.pressed_action.take() else {
                    return EventResult::NotHandled;
                };
                let released = self.action_at_point(*pos);
                self.hovered_action.clone_from(&released);
                // 拖拽放置：源键与目标键不同且命中目标行时回调。
                if let Some(source) = self.dragged_key.take() {
                    if let Some(TreePointerAction::Select(target)) = released.as_ref() {
                        if source != *target {
                            if let Some(callback) = self.drop_callback.as_ref() {
                                callback(&source, target, self.drop_position_at(*pos));
                            }
                            return EventResult::Handled;
                        }
                    }
                }
                // 按下与抬起命中同一操作时提交（点击生效）。
                if released.as_ref() == Some(&pressed) {
                    self.commit_pointer_action(pressed);
                }
                EventResult::Handled
            }
            // 指针移动：更新悬浮高亮。
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.action_at_point(*pos);
                if hovered != self.hovered_action {
                    self.hovered_action = hovered;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            // 指针离开：清理悬浮/按压/拖拽状态。
            SystemEvent::PointerLeave => {
                let changed = self.hovered_action.take().is_some()
                    | self.pressed_action.take().is_some()
                    | self.dragged_key.take().is_some();
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                // 失焦同时清理全部指针状态。
                self.hovered_action = None;
                self.pressed_action = None;
                self.dragged_key = None;
                EventResult::Handled
            }
            // 键盘导航。
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Backspace if self.searchable && !self.search_query.is_empty() => {
                    // 退格删除搜索词末字符。
                    self.search_query.pop();
                    self.refresh_search();
                    EventResult::Handled
                }
                KeyCode::Escape if self.searchable && !self.search_query.is_empty() => {
                    // 清空搜索词。
                    self.search_query.clear();
                    self.refresh_search();
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.move_selection(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_selection(false);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.expand_or_descend();
                    EventResult::Handled
                }
                KeyCode::Left => {
                    self.collapse_or_ascend();
                    EventResult::Handled
                }
                // 空格切换勾选，回车激活。
                KeyCode::Space | KeyCode::Enter => {
                    self.activate_current(*key == KeyCode::Space);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            // 文本输入/粘贴：可搜索时追加到搜索词并过滤。
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.searchable && !text.is_empty() && !text.chars().any(char::is_control) =>
            {
                self.search_query.push_str(text);
                self.refresh_search();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    // 语义事件：取出变更待发键并上报 change 事件。
    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    // 渲染：搜索栏、虚拟化行列表（高亮/勾选/展开/图标/标题）与焦点框。
    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        // 空尺寸直接记录帧并退出。
        if frame.w <= 0.0 || frame.h <= 0.0 {
            self.last_frame.set(Some(frame));
            return;
        }
        self.last_frame.set(Some(frame));
        let resolved = self.visual.resolve(ctx.tokens());
        let chrome = self.visual.chrome;
        let row_height = self.visual.geometry.row_height;

        let search_h = self.search_height_for_intrinsic();
        // 绘制顶部搜索栏（框线 + 提示词或查询词）。
        if self.searchable {
            let search_rect = Rect::new(frame.x, frame.y, frame.w, search_h);
            ctx.fill_rect(search_rect, resolved.search_background, None);
            ctx.stroke_rect(
                search_rect,
                resolved.search_border,
                chrome.search_stroke_width,
                None,
            );
            let query = if self.search_query.is_empty() {
                chrome.search_placeholder
            } else {
                &self.search_query
            };
            let query_color = if self.search_query.is_empty() {
                resolved.text_secondary
            } else {
                resolved.text
            };
            self.paint_single_line(
                ctx,
                query,
                Rect::new(
                    frame.x + chrome.search_horizontal_padding,
                    frame.y,
                    (frame.w - chrome.search_horizontal_padding * 2.0).max(0.0),
                    search_h,
                ),
                query_color,
                chrome.search_font_size,
            );
        }

        let viewport_h = self.body_viewport_height();
        // 按虚拟滚动范围仅绘制可见行。
        let (start, end) = self
            .body_scroll
            .scroll_range(self.flat.len(), row_height, viewport_h);
        ctx.push_clip(frame);

        for i in start..end {
            let node = &self.flat[i];
            let y = frame.y + search_h + i as f32 * row_height
                - self.body_scroll.scroll_offset();
            // 剔除视口外行。
            if y + row_height < frame.y + search_h
                || y > frame.y + search_h + viewport_h
            {
                continue;
            }
            let is_selected = self.multiple && self.selected_keys.contains(&node.key)
                || (!self.multiple && node.key == self.selected_key);
            let geometry = self.row_geometry(frame, i, y);
            let action_matches_key = |action: &TreePointerAction| match action {
                TreePointerAction::Check(key)
                | TreePointerAction::Toggle(key)
                | TreePointerAction::Select(key) => key == &node.key,
            };

            // 选中/悬浮/按压行背景。
            if is_selected {
                ctx.fill_rect(geometry.row, resolved.selected_fill, None);
            }
            if self
                .hovered_action
                .as_ref()
                .is_some_and(action_matches_key)
            {
                ctx.fill_rect(geometry.row, resolved.hover_fill, None);
            }
            if self
                .pressed_action
                .as_ref()
                .is_some_and(action_matches_key)
            {
                ctx.fill_rect(geometry.row, resolved.pressed_fill, None);
            }

            // 多选模式下绘制当前行焦点框。
            if self.focused
                && tree.keyboard_focus_visible()
                && self.multiple
                && node.key == self.selected_key
            {
                let inset = chrome
                    .row_focus_inset
                    .min(geometry.row.w * chrome.center_ratio)
                    .min(geometry.row.h * chrome.center_ratio);
                let focus = Rect::new(
                    geometry.row.x + inset,
                    geometry.row.y + inset,
                    (geometry.row.w - inset * 2.0).max(0.0),
                    (geometry.row.h - inset * 2.0).max(0.0),
                );
                if focus.w > 0.0 && focus.h > 0.0 {
                    ctx.stroke_rect(focus, resolved.primary, chrome.row_focus_stroke, None);
                }
            }

            // 勾选图标。
            if let Some(check) = geometry.check {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if node.checked {
                        chrome.checked_icon
                    } else {
                        chrome.unchecked_icon
                    },
                    check,
                    if node.checked {
                        resolved.primary
                    } else {
                        resolved.text_secondary
                    },
                    chrome
                        .check_icon_size
                        .min(check.h * chrome.check_icon_height_ratio),
                );
            }

            // 展开/折叠箭头。
            if let Some(toggle) = geometry.toggle {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if node.expanded {
                        chrome.expanded_icon
                    } else {
                        chrome.collapsed_icon
                    },
                    toggle,
                    resolved.text_secondary,
                    chrome
                        .branch_icon_size
                        .min(toggle.h * chrome.branch_icon_height_ratio),
                );
            }

            // 节点自定义图标。
            if let Some(icon) = geometry.icon {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    &node.icon,
                    icon,
                    resolved.text_secondary,
                    chrome
                        .node_icon_size
                        .min(icon.h * chrome.node_icon_height_ratio),
                );
            }

            // 标题颜色：禁用置灰，选中用主题色。
            let tc = if node.disabled {
                resolved.text_secondary
            } else if is_selected {
                resolved.primary
            } else {
                resolved.text
            };
            self.paint_single_line(
                ctx,
                &node.title,
                geometry.title,
                tc,
                chrome.title_font_size,
            );
        }

        ctx.pop_clip();
        // 组件整体焦点框。
        if self.focused && tree.keyboard_focus_visible() {
            let inset = chrome
                .focus_inset
                .min(frame.w * chrome.center_ratio)
                .min(frame.h * chrome.center_ratio);
            let focus = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            if focus.w > 0.0 && focus.h > 0.0 {
                ctx.stroke_rect(focus, resolved.primary, chrome.focus_stroke, None);
            }
        }
    }
}
