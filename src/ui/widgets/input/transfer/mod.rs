use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SnapshotTransferItem,
    SystemEvent, WidgetId, WidgetTree,
};
use crate::widget;
// 引入组件局部状态与待发变化记录所需的单线程容器。
use std::cell::{Cell, RefCell};
// 引入稳定身份集合，确保同一 pane 的条目不会共享动态状态命名空间。
use std::collections::HashSet;
// 引入应用 renderer 与回调的单线程共享所有权句柄。
use std::rc::Rc;

// 声明 UIX 静态视觉契约与主题解析模块。
mod presentation;

use presentation::*;

// ════════════════════════════════════════════════════════════════════════════
// Transfer
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Eq)]
/// Transfer 源列表或目标列表中的单个数据项。
pub struct TransferItem {
    /// 标识跨列表移动和动态视图身份的稳定业务键。
    pub key: String,
    /// 默认条目绘制和快照使用的标题。
    pub title: String,
    /// 条目当前是否被选中以参与下一次移动。
    pub selected: bool,
}

impl TransferItem {
    /// 使用稳定业务键和标题创建未选中的条目。
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            selected: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 一次 Transfer 条目移动的方向。
pub enum MoveDirection {
    /// 将选中条目从源列表移动到目标列表。
    LeftToRight,
    /// 将选中条目从目标列表移回源列表。
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransferPane {
    Source,
    Target,
}

widget! {
    /// 在来源与目标双栏之间按稳定条目身份移动数据的穿梭框组件。
    pub struct Transfer {
        source: Vec<TransferItem>,
        target: Vec<TransferItem>,
        source_title: String,
        target_title: String,
        searchable: bool,
        search_query: String,
        item_renderer: Option<Rc<dyn Fn(&TransferItem) -> crate::ui::view::ViewNode>>,
        change_callback: Option<Rc<dyn Fn(&[TransferItem], &[TransferItem], MoveDirection)>>,
        last_frame: Cell<Option<Rect>>,
        focused: bool,
        active_pane: TransferPane,
        active_index: usize,
        pending_change: RefCell<Option<String>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static TransferVisual,
    }

    tab_index => (&self) -> i32 { i32::from(!self.source.is_empty() || !self.target.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.visual.layout.natural_width,
            self.visual.layout.natural_height,
        ))
    }

    accepts_text_input => (&self) -> bool { self.searchable }

    text_input_cursor_rect => (&self) -> Rect {
        let layout = self.visual.layout;
        let width = (self.search_query.chars().count() as f32 * layout.cursor_char_width
            + layout.cursor_padding)
            .clamp(layout.cursor_min_width, layout.cursor_max_width);
        Rect::new(
            layout.cursor_x_inset + width,
            layout.cursor_y,
            layout.cursor_width,
            layout.cursor_height,
        )
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        let layout = self.visual.layout;
        let half = ((frame.w - layout.button_column_width) * 0.5)
            .max(layout.min_pane_half_width);
        let row_h = layout.row_height;
        let header_h = layout.header_height
            + if self.searchable { layout.search_height } else { 0.0 };
        let mut layouts = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            let (pane, raw_index) = if index < self.source.len() {
                (TransferPane::Source, index)
            } else {
                (TransferPane::Target, index - self.source.len())
            };
            let visible = self.visible_indices(pane).into_iter().position(|item| item == raw_index);
            let rect = if let Some(visible_index) = visible {
                let x = if pane == TransferPane::Source {
                    frame.x
                } else {
                    frame.x + half + layout.button_column_width
                };
                Rect::new(x, frame.y + header_h + visible_index as f32 * row_h, half, row_h)
            } else {
                Rect::zero()
            };
            layouts.push((child.id, rect));
        }
        layouts
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => self.pointer_down(*pos),
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Backspace, .. }
                if self.searchable && !self.search_query.is_empty() =>
            {
                self.search_query.pop();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key: KeyCode::Escape, .. }
                if self.searchable && !self.search_query.is_empty() =>
            {
                self.search_query.clear();
                EventResult::Handled
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.searchable && !text.is_empty() && !text.chars().any(char::is_control) =>
            {
                self.search_query.push_str(text);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => self.key_down(*key),
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|keys| SemanticEvent::change(id, keys))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        // 双栏、搜索框、条目与按钮共享同一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = self.visual.layout;
        let search_h = if self.searchable { layout.search_height } else { 0.0 };
        let content_header_h = layout.header_height + search_h;
        let half = ((frame.w - layout.button_column_width) * 0.5)
            .max(layout.min_pane_half_width);
        let item_h = layout.row_height;
        let r = Some(Radius::uniform(visual.radius));
        let left_rect = Rect::new(frame.x, frame.y, half, frame.h);
        ctx.fill_rect(left_rect, visual.background, r);
        let left_border = if self.focused
            && tree.keyboard_focus_visible()
            && self.active_pane == TransferPane::Source
        {
            visual.primary
        } else {
            visual.border
        };
        ctx.stroke_rect(
            left_rect,
            left_border,
            if left_border == visual.primary {
                self.visual.chrome.focus_border_width
            } else {
                self.visual.chrome.border_width
            },
            r,
        );
        let loc = crate::ui::widget_runtime::locale::use_locale();
        let source_title = if self.source_title.is_empty() { loc.transfer_source } else { &self.source_title };
        if self.searchable {
            ctx.stroke_rect(
                Rect::new(
                    frame.x + layout.search_horizontal_inset,
                    frame.y + layout.search_vertical_inset,
                    (half - layout.search_horizontal_inset * 2.0).max(0.0),
                    layout.search_box_height,
                ),
                visual.border,
                self.visual.chrome.border_width,
                None,
            );
            ctx.draw_text(
                &format!("搜索: {}", self.search_query),
                Point::new(
                    frame.x + layout.text_horizontal_inset,
                    frame.y + layout.text_top_inset,
                ),
                visual.text_quaternary,
                visual.typography.caption,
            );
        }
        ctx.draw_text(
            &format!("{} ({}项)", source_title, self.source.len()),
            Point::new(
                frame.x + layout.text_horizontal_inset,
                frame.y + search_h + layout.text_top_inset,
            ),
            visual.text_quaternary,
            visual.typography.caption,
        );
        for (i, raw_index) in self.visible_indices(TransferPane::Source).into_iter().enumerate() {
            let item = &self.source[raw_index];
            let y = frame.y + content_header_h + i as f32 * item_h;
            let row_rect = Rect::new(frame.x, y, half, item_h);
            // 文字垂直定位与实际绘制共享同一个主题派生字号。
            let row_y = ctx.visual_center_y(row_rect, visual.typography.item);
            if item.selected { ctx.fill_rect(row_rect, visual.fill_tertiary, None); }
            if self.focused
                && tree.keyboard_focus_visible()
                && self.active_pane == TransferPane::Source
                && self.active_index == raw_index
            {
                ctx.stroke_rect(row_rect, visual.primary, self.visual.chrome.border_width, None);
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if item.selected {
                    self.visual.icons.selected
                } else {
                    self.visual.icons.unselected
                },
                Rect::new(
                    frame.x + layout.row_icon_inset,
                    y,
                    layout.row_icon_width,
                    item_h,
                ),
                visual.text,
                visual.checkbox_icon_size,
            );
            if self.item_renderer.is_none() {
                // 默认条目文本使用主题派生的紧凑字号。
                ctx.draw_text(
                    // 绘制当前源条目标题。
                    &item.title,
                    // 保持既有条目文本起点。
                    Point::new(frame.x + layout.row_text_inset, row_y),
                    // 使用当前主题正文颜色。
                    visual.text,
                    // 使用与垂直定位一致的派生字号。
                    visual.typography.item,
                );
            }
        }
        let btn_y = frame.y + frame.h * 0.5 - layout.button_group_half_height;
        let rbtn_rect = Rect::new(
            frame.x + half + layout.button_horizontal_inset,
            btn_y,
            layout.button_width,
            layout.button_height,
        );
        let lbtn_rect = Rect::new(
            frame.x + half + layout.button_horizontal_inset,
            btn_y + layout.button_height + layout.button_vertical_gap,
            layout.button_width,
            layout.button_height,
        );
        ctx.fill_rect(
            rbtn_rect,
            visual.primary,
            Some(Radius::uniform(self.visual.chrome.button_radius)),
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.icons.move_right,
            rbtn_rect,
            visual.white,
            visual.arrow_icon_size,
        );
        ctx.fill_rect(
            lbtn_rect,
            visual.border,
            Some(Radius::uniform(self.visual.chrome.button_radius)),
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            self.visual.icons.move_left,
            lbtn_rect,
            visual.text,
            visual.arrow_icon_size,
        );
        let right_x = frame.x + half + layout.button_column_width;
        let right_rect = Rect::new(right_x, frame.y, half, frame.h);
        ctx.fill_rect(right_rect, visual.background, r);
        let right_border = if self.focused
            && tree.keyboard_focus_visible()
            && self.active_pane == TransferPane::Target
        {
            visual.primary
        } else {
            visual.border
        };
        ctx.stroke_rect(
            right_rect,
            right_border,
            if right_border == visual.primary {
                self.visual.chrome.focus_border_width
            } else {
                self.visual.chrome.border_width
            },
            r,
        );
        let target_title = if self.target_title.is_empty() { loc.transfer_target } else { &self.target_title };
        if self.searchable {
            ctx.stroke_rect(
                Rect::new(
                    right_x + layout.search_horizontal_inset,
                    frame.y + layout.search_vertical_inset,
                    (half - layout.search_horizontal_inset * 2.0).max(0.0),
                    layout.search_box_height,
                ),
                visual.border,
                self.visual.chrome.border_width,
                None,
            );
            ctx.draw_text(
                &format!("搜索: {}", self.search_query),
                Point::new(
                    right_x + layout.text_horizontal_inset,
                    frame.y + layout.text_top_inset,
                ),
                visual.text_quaternary,
                visual.typography.caption,
            );
        }
        ctx.draw_text(
            &format!("{} ({}项)", target_title, self.target.len()),
            Point::new(
                right_x + layout.text_horizontal_inset,
                frame.y + search_h + layout.text_top_inset,
            ),
            visual.text_quaternary,
            visual.typography.caption,
        );
        for (i, raw_index) in self.visible_indices(TransferPane::Target).into_iter().enumerate() {
            let item = &self.target[raw_index];
            let y = frame.y + content_header_h + i as f32 * item_h;
            let row_rect = Rect::new(right_x, y, half, item_h);
            // 文字垂直定位与实际绘制共享同一个主题派生字号。
            let row_y = ctx.visual_center_y(row_rect, visual.typography.item);
            if item.selected { ctx.fill_rect(row_rect, visual.fill_tertiary, None); }
            if self.focused
                && tree.keyboard_focus_visible()
                && self.active_pane == TransferPane::Target
                && self.active_index == raw_index
            {
                ctx.stroke_rect(row_rect, visual.primary, self.visual.chrome.border_width, None);
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if item.selected {
                    self.visual.icons.selected
                } else {
                    self.visual.icons.unselected
                },
                Rect::new(
                    right_x + layout.row_icon_inset,
                    y,
                    layout.row_icon_width,
                    item_h,
                ),
                visual.text,
                visual.checkbox_icon_size,
            );
            if self.item_renderer.is_none() {
                // 默认条目文本使用主题派生的紧凑字号。
                ctx.draw_text(
                    // 绘制当前目标条目标题。
                    &item.title,
                    // 保持既有条目文本起点。
                    Point::new(right_x + layout.row_text_inset, row_y),
                    // 使用当前主题正文颜色。
                    visual.text,
                    // 使用与垂直定位一致的派生字号。
                    visual.typography.item,
                );
            }
        }
    }
}
impl Transfer {
    /// 创建空的、不可搜索且活动 pane 为源列表的 Transfer。
    pub fn new() -> Self {
        Self {
            source: Vec::new(),
            target: Vec::new(),
            source_title: String::new(),
            target_title: String::new(),
            searchable: false,
            search_query: String::new(),
            item_renderer: None,
            change_callback: None,
            last_frame: Cell::new(None),
            focused: false,
            active_pane: TransferPane::Source,
            active_index: 0,
            pending_change: RefCell::new(None),
            visual: TRANSFER_VISUAL_REF,
        }
    }
    /// 替换源列表中的全部条目。
    pub fn source(mut self, items: Vec<TransferItem>) -> Self {
        self.source = items;
        self
    }
    /// 替换目标列表中的全部条目。
    pub fn target(mut self, items: Vec<TransferItem>) -> Self {
        self.target = items;
        self
    }

    /// [`Self::source`] 的左右列表兼容别名。
    pub fn left_data(self, items: Vec<TransferItem>) -> Self {
        self.source(items)
    }

    /// [`Self::target`] 的左右列表兼容别名。
    pub fn right_data(self, items: Vec<TransferItem>) -> Self {
        self.target(items)
    }

    /// 设置源列表和目标列表的标题。
    pub fn titles(mut self, source: impl Into<String>, target: impl Into<String>) -> Self {
        self.source_title = source.into();
        self.target_title = target.into();
        self
    }

    /// 设置是否允许按条目标题筛选；关闭时清空查询文本。
    pub fn searchable(mut self, value: bool) -> Self {
        self.searchable = value;
        if !value {
            self.search_query.clear();
        }
        self
    }

    /// 返回当前搜索查询文本。
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    // 在所属 WidgetTree 签发的动态命名空间中构建全部条目声明子树。
    pub(crate) fn item_views_for_reconcile(
        // 借用固定树状态存储与当前 Transfer owner 的窄捕获能力。
        &self,
        // 接收只能由活跃宿主树创建的动态捕获上下文。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        // 返回完整捕获但尚未发布的条目声明集合。
    ) -> Vec<crate::ui::view::ViewNode> {
        // 没有自定义 renderer 时用空集合协调并释放旧动态条目。
        let Some(factory) = self.item_renderer.as_ref() else {
            // 默认文本绘制路径不需要额外 View 子树。
            return Vec::new();
        };
        // 在调用任何应用 renderer 前固定 pane、条目快照与结构身份。
        let entries = self
            // 先枚举源列表，保持既有 source-first 声明顺序。
            .source
            // 只借用当前 Transfer 运行时拥有的条目快照。
            .iter()
            // 为源条目生成与既有 keyed reconcile 兼容的稳定身份。
            .map(|item| (item, format!("transfer:source:{}", item.key)))
            // 再追加目标列表，保持布局索引与 pane 边界一致。
            .chain(
                // 枚举当前目标列表中的全部条目。
                self.target
                    // 只借用当前 Transfer 运行时拥有的目标条目快照。
                    .iter()
                    // 让跨 pane 移动形成新的动态实例身份。
                    .map(|item| (item, format!("transfer:target:{}", item.key))),
            )
            // 保存完整预检批次，避免部分工厂执行后才发现身份冲突。
            .collect::<Vec<_>>();
        // 建立本批次动态身份集合以拒绝同 pane 重复业务键。
        let mut unique_keys = HashSet::with_capacity(entries.len());
        // 在任何用户代码执行前验证全部结构与状态身份唯一。
        for (_, stable_key) in &entries {
            // 同一稳定身份只能对应一个动态条目实例。
            assert!(
                // 首次插入成功才允许后续捕获该条目。
                unique_keys.insert(stable_key.clone()),
                // 不回显业务 key，避免诊断泄露应用数据。
                "Transfer item renderer 收到重复的 pane 内稳定业务 key"
            );
        }
        // 预分配完整条目数量，保持一次动态事务的集合边界稳定。
        let mut children = Vec::with_capacity(entries.len());
        // 仅在全部身份预检成功后依次调用应用 renderer。
        for (item, stable_key) in entries {
            // 为 keyed reconcile 保留与状态命名空间相同的身份副本。
            let view_key = stable_key.clone();
            // 在所属窗口树的 Transfer 条目槽位捕获完整声明输出。
            let view = capture_context.capture(
                // 固定槽位隔离同一宿主下的其他延迟 View 工厂。
                "transfer-item",
                // pane 与业务 key 共同拥有私有状态和结构身份。
                stable_key,
                // 只在完整捕获边界中执行应用提供的条目 renderer。
                || factory(item),
            );
            // 应用 renderer 自设根 key 会形成状态与结构双身份。
            assert!(
                // 框架是 Transfer 动态条目根身份的唯一权威。
                view.key.is_none(),
                // 明确要求调用方使用 TransferItem.key 表达稳定身份。
                "Transfer item renderer 不得直接设置根 key；请使用 TransferItem.key"
            );
            // 强制结构协调与树私有捕获使用同一稳定身份。
            children.push(view.key(view_key));
        }
        // 将完整捕获但未发布的批次交回 WidgetTree 动态协调器。
        children
    }

    /// 自定义条目视图工厂；默认绘制仍使用稳定文本快照。
    pub fn render_item<F, V>(mut self, factory: F) -> Self
    where
        F: Fn(&TransferItem) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.item_renderer = Some(Rc::new(move |item| {
            crate::ui::view::View::build(factory(item))
        }));
        self
    }

    /// 注册移动完成后接收源列表、目标列表与移动方向的回调。
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(&[TransferItem], &[TransferItem], MoveDirection) + 'static,
    {
        self.change_callback = Some(Rc::new(callback));
        self
    }

    /// 返回当前源列表条目的只读切片。
    pub fn source_items(&self) -> &[TransferItem] {
        &self.source
    }

    /// 返回当前目标列表条目的只读切片。
    pub fn target_items(&self) -> &[TransferItem] {
        &self.target
    }

    /// 返回当前活动 pane 内的零基条目索引。
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// 返回当前活动 pane 是否为目标列表。
    pub fn target_is_active(&self) -> bool {
        self.active_pane == TransferPane::Target
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.visual = next.visual;
        self.source_title = next.source_title;
        self.target_title = next.target_title;
        self.searchable = next.searchable;
        self.search_query = if next.searchable {
            next.search_query
        } else {
            String::new()
        };
        self.item_renderer = next.item_renderer;
        self.change_callback = next.change_callback;
    }

    // 测试目标保留传输组件 frame 注入入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
    }

    fn visible_indices(&self, pane: TransferPane) -> Vec<usize> {
        let query = self.search_query.trim().to_lowercase();
        let items = match pane {
            TransferPane::Source => &self.source,
            TransferPane::Target => &self.target,
        };
        items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                query.is_empty()
                    || item.title.to_lowercase().contains(&query)
                    || item.key.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Transfer {
            source: self
                .source
                .iter()
                .map(SnapshotTransferItem::from_transfer_item)
                .collect(),
            target: self
                .target
                .iter()
                .map(SnapshotTransferItem::from_transfer_item)
                .collect(),
        }
    }

    fn pointer_down(&mut self, pos: Point) -> EventResult {
        let layout = self.visual.layout;
        let header = layout.header_height
            + if self.searchable {
                layout.search_height
            } else {
                0.0
            };
        let Some(frame) = self.last_frame.get().filter(|frame| frame.contains(pos)) else {
            return EventResult::NotHandled;
        };
        let x = pos.x - frame.x;
        let y = pos.y - frame.y;
        let half = ((frame.w - layout.button_column_width) * 0.5).max(layout.min_pane_half_width);
        let row = (y >= header).then(|| ((y - header) / layout.row_height) as usize);
        if x < half {
            return self.toggle_visible_row(TransferPane::Source, row);
        }
        if x > half + layout.button_column_width {
            return self.toggle_visible_row(TransferPane::Target, row);
        }
        let button_y = frame.h * 0.5 - layout.button_group_half_height;
        if y >= button_y && y < button_y + layout.button_height {
            self.move_selected(TransferPane::Source);
            return EventResult::Handled;
        }
        let second_button_y = button_y + layout.button_height + layout.button_vertical_gap;
        if y >= second_button_y && y < second_button_y + layout.button_height {
            self.move_selected(TransferPane::Target);
            return EventResult::Handled;
        }
        EventResult::NotHandled
    }

    fn key_down(&mut self, key: KeyCode) -> EventResult {
        if self.source.is_empty() && self.target.is_empty() {
            return EventResult::NotHandled;
        }
        match key {
            KeyCode::Left => self.activate_pane(TransferPane::Source),
            KeyCode::Right => self.activate_pane(TransferPane::Target),
            KeyCode::Up => self.active_index = self.active_index.saturating_sub(1),
            KeyCode::Down => {
                self.active_index =
                    (self.active_index + 1).min(self.active_len().saturating_sub(1));
            }
            KeyCode::Home => self.active_index = 0,
            KeyCode::End => self.active_index = self.active_len().saturating_sub(1),
            KeyCode::Space => {
                self.toggle_active();
            }
            KeyCode::Enter => {
                if !self.active_items().iter().any(|item| item.selected) {
                    self.toggle_active();
                }
                self.move_selected(self.active_pane);
            }
            _ => return EventResult::NotHandled,
        }
        EventResult::Handled
    }

    fn toggle_row(&mut self, pane: TransferPane, row: Option<usize>) -> EventResult {
        let Some(index) = row else {
            return EventResult::NotHandled;
        };
        let items = match pane {
            TransferPane::Source => &mut self.source,
            TransferPane::Target => &mut self.target,
        };
        let Some(item) = items.get_mut(index) else {
            return EventResult::NotHandled;
        };
        item.selected = !item.selected;
        self.active_pane = pane;
        self.active_index = index;
        self.focused = true;
        EventResult::Handled
    }

    fn toggle_visible_row(&mut self, pane: TransferPane, row: Option<usize>) -> EventResult {
        let Some(visible_row) = row else {
            return EventResult::NotHandled;
        };
        let Some(raw_row) = self.visible_indices(pane).get(visible_row).copied() else {
            return EventResult::NotHandled;
        };
        self.toggle_row(pane, Some(raw_row))
    }

    fn toggle_active(&mut self) {
        let index = self.active_index;
        let items = match self.active_pane {
            TransferPane::Source => &mut self.source,
            TransferPane::Target => &mut self.target,
        };
        if let Some(item) = items.get_mut(index) {
            item.selected = !item.selected;
        }
    }

    fn move_selected(&mut self, from: TransferPane) -> bool {
        let (source, target) = match from {
            TransferPane::Source => (&mut self.source, &mut self.target),
            TransferPane::Target => (&mut self.target, &mut self.source),
        };
        let mut kept = Vec::with_capacity(source.len());
        let mut moved = Vec::new();
        for mut item in source.drain(..) {
            if item.selected {
                item.selected = false;
                moved.push(item);
            } else {
                kept.push(item);
            }
        }
        *source = kept;
        if moved.is_empty() {
            return false;
        }
        target.extend(moved);
        self.active_index = self.active_index.min(self.active_len().saturating_sub(1));
        self.pending_change
            .replace(Some(self.target_keys_payload()));
        if let Some(callback) = self.change_callback.as_ref() {
            let direction = match from {
                TransferPane::Source => MoveDirection::LeftToRight,
                TransferPane::Target => MoveDirection::RightToLeft,
            };
            callback(&self.source, &self.target, direction);
        }
        true
    }

    fn activate_pane(&mut self, pane: TransferPane) {
        self.active_pane = pane;
        self.active_index = self.active_index.min(self.active_len().saturating_sub(1));
    }

    fn active_items(&self) -> &[TransferItem] {
        match self.active_pane {
            TransferPane::Source => &self.source,
            TransferPane::Target => &self.target,
        }
    }

    fn active_len(&self) -> usize {
        self.active_items().len()
    }

    fn target_keys_payload(&self) -> String {
        self.target
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }
}

// 把 Transfer 的 Rust 数据移动内核与 UIX 静态视觉组合为单一组件节点。
fn build_transfer_view(mut kernel: Transfer, visual: &'static TransferVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Transfer {
    fn build(self) -> ViewNode {
        build_transfer_view(self, TRANSFER_VISUAL_REF)
    }
}

impl Default for Transfer {
    fn default() -> Self {
        Self::new()
    }
}

// 验证 Transfer 排版随主题 token 解析。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/other/misc/transfer__typography_tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod typography_tests;
