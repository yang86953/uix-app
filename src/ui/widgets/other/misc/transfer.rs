use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTransferItem, SystemEvent, WidgetTree,
};
// 引入 Transfer 私有排版值契约消费的主题接口。
use crate::ui::ThemeTokens;
// 引入组件局部状态与待发变化记录所需的单线程容器。
use std::cell::{Cell, RefCell};
// 引入稳定身份集合，确保同一 pane 的条目不会共享动态状态命名空间。
use std::collections::HashSet;
// 引入应用 renderer 与回调的单线程共享所有权句柄。
use std::rc::Rc;

// ════════════════════════════════════════════════════════════════════════════
// Transfer 面板布局常量（绘制、布局与命中测试共用，保持三处数值一致）。
// ════════════════════════════════════════════════════════════════════════════
// 中间操作按钮列宽（像素）。
const BTN_COL_W: f32 = 60.0;
// 面板头部高度（像素）。
const LIST_HEADER_H: f32 = 24.0;
// 搜索框高度（像素）。
const SEARCH_BAR_H: f32 = 24.0;
// 列表行高（像素）。
const ROW_H: f32 = 28.0;
// 面板最小半宽（像素），窄窗下保证按钮列可用。
const MIN_PANE_HALF_W: f32 = 40.0;
// 条目字号位于小号正文与正文 token 的中点。
const ITEM_FONT_MIDPOINT_WEIGHT: f32 = 0.5;

// 保存一次绘制内解析出的 Transfer 排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
struct TransferTypography {
    // 搜索说明与面板标题使用小号正文。
    caption: f32,
    // 列表条目使用小号正文与正文之间的紧凑字号。
    item: f32,
}

// 将主题排版 token 转换为组件私有绘制值。
impl TransferTypography {
    // 从当前组件主题作用域解析排版。
    fn resolve(tokens: &dyn ThemeTokens) -> Self {
        // 读取主题拥有的小号正文字号。
        let caption = tokens.font_size_sm();
        // 读取主题拥有的正文字号。
        let body = tokens.font_size();
        // 返回供当前绘制批次复用的稳定值。
        Self {
            // 搜索说明与标题直接使用小号正文 token。
            caption,
            // 默认主题下保持原 13px，同时跟随两个相邻 token 变化。
            item: caption + (body - caption) * ITEM_FONT_MIDPOINT_WEIGHT,
        }
    }
}

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

component! {
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
    }

    tab_index => (&self) -> i32 { i32::from(!self.source.is_empty() || !self.target.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(500.0, 200.0))
    }

    accepts_text_input => (&self) -> bool { self.searchable }

    text_input_cursor_rect => (&self) -> Rect {
        let width = (self.search_query.chars().count() as f32 * 8.0 + 8.0).clamp(8.0, 280.0);
        Rect::new(8.0 + width, 4.0, 1.0, 20.0)
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        let half = ((frame.w - BTN_COL_W) * 0.5).max(MIN_PANE_HALF_W);
        let row_h = ROW_H;
        let header_h = LIST_HEADER_H + if self.searchable { SEARCH_BAR_H } else { 0.0 };
        let mut layouts = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            let (pane, raw_index) = if index < self.source.len() {
                (TransferPane::Source, index)
            } else {
                (TransferPane::Target, index - self.source.len())
            };
            let visible = self.visible_indices(pane).into_iter().position(|item| item == raw_index);
            let rect = if let Some(visible_index) = visible {
                let x = if pane == TransferPane::Source { frame.x } else { frame.x + half + BTN_COL_W };
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|keys| SemanticEvent::change(id, keys))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let fill = ctx.tokens().color_fill_tertiary();
        // 在组件主题作用域内只解析一次排版值。
        let typography = TransferTypography::resolve(ctx.tokens());
        let search_h = if self.searchable { SEARCH_BAR_H } else { 0.0 };
        let content_header_h = LIST_HEADER_H + search_h;
        let half = ((frame.w - BTN_COL_W) * 0.5).max(MIN_PANE_HALF_W);
        let item_h = ROW_H;
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        let left_rect = Rect::new(frame.x, frame.y, half, frame.h);
        ctx.fill_rect(left_rect, bg, r);
        let left_border = if self.focused
            && tree.keyboard_focus_visible()
            && self.active_pane == TransferPane::Source
        {
            primary
        } else {
            border
        };
        ctx.stroke_rect(left_rect, left_border, if left_border == primary { 1.5 } else { 1.0 }, r);
        let loc = crate::ui::component::locale::use_locale();
        let source_title = if self.source_title.is_empty() { loc.transfer_source } else { &self.source_title };
        if self.searchable {
            ctx.stroke_rect(
                Rect::new(frame.x + 4.0, frame.y + 2.0, (half - 8.0).max(0.0), 20.0),
                border,
                1.0,
                None,
            );
            ctx.draw_text(
                &format!("搜索: {}", self.search_query),
                Point::new(frame.x + 8.0, frame.y + 6.0),
                text_sec,
                typography.caption,
            );
        }
        ctx.draw_text(
            &format!("{} ({}项)", source_title, self.source.len()),
            Point::new(frame.x + 8.0, frame.y + search_h + 6.0),
            text_sec,
            typography.caption,
        );
        for (i, raw_index) in self.visible_indices(TransferPane::Source).into_iter().enumerate() {
            let item = &self.source[raw_index];
            let y = frame.y + content_header_h + i as f32 * item_h;
            let row_rect = Rect::new(frame.x, y, half, item_h);
            // 文字垂直定位与实际绘制共享同一个主题派生字号。
            let row_y = ctx.visual_center_y(row_rect, typography.item);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            if self.focused
                && tree.keyboard_focus_visible()
                && self.active_pane == TransferPane::Source
                && self.active_index == raw_index
            {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if item.selected { "check-square" } else { "square" },
                Rect::new(frame.x + 4.0, y, 20.0, item_h),
                text,
                12.0,
            );
            if self.item_renderer.is_none() {
                // 默认条目文本使用主题派生的紧凑字号。
                ctx.draw_text(
                    // 绘制当前源条目标题。
                    &item.title,
                    // 保持既有条目文本起点。
                    Point::new(frame.x + 26.0, row_y),
                    // 使用当前主题正文颜色。
                    text,
                    // 使用与垂直定位一致的派生字号。
                    typography.item,
                );
            }
        }
        let btn_y = frame.y + frame.h * 0.5 - 20.0;
        let rbtn_rect = Rect::new(frame.x + half + 8.0, btn_y, 44.0, 20.0);
        let lbtn_rect = Rect::new(frame.x + half + 8.0, btn_y + 24.0, 44.0, 20.0);
        ctx.fill_rect(rbtn_rect, primary, Some(Radius::uniform(3.0)));
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "arrow-right",
            rbtn_rect,
            // 移动箭头（主色按钮上反白）：白色 token。
            ctx.tokens().color_white(),
            14.0,
        );
        ctx.fill_rect(lbtn_rect, border, Some(Radius::uniform(3.0)));
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "arrow-left",
            lbtn_rect,
            text,
            14.0,
        );
        let right_x = frame.x + half + BTN_COL_W;
        let right_rect = Rect::new(right_x, frame.y, half, frame.h);
        ctx.fill_rect(right_rect, bg, r);
        let right_border = if self.focused
            && tree.keyboard_focus_visible()
            && self.active_pane == TransferPane::Target
        {
            primary
        } else {
            border
        };
        ctx.stroke_rect(right_rect, right_border, if right_border == primary { 1.5 } else { 1.0 }, r);
        let target_title = if self.target_title.is_empty() { loc.transfer_target } else { &self.target_title };
        if self.searchable {
            ctx.stroke_rect(
                Rect::new(right_x + 4.0, frame.y + 2.0, (half - 8.0).max(0.0), 20.0),
                border,
                1.0,
                None,
            );
            ctx.draw_text(
                &format!("搜索: {}", self.search_query),
                Point::new(right_x + 8.0, frame.y + 6.0),
                text_sec,
                typography.caption,
            );
        }
        ctx.draw_text(
            &format!("{} ({}项)", target_title, self.target.len()),
            Point::new(right_x + 8.0, frame.y + search_h + 6.0),
            text_sec,
            typography.caption,
        );
        for (i, raw_index) in self.visible_indices(TransferPane::Target).into_iter().enumerate() {
            let item = &self.target[raw_index];
            let y = frame.y + content_header_h + i as f32 * item_h;
            let row_rect = Rect::new(right_x, y, half, item_h);
            // 文字垂直定位与实际绘制共享同一个主题派生字号。
            let row_y = ctx.visual_center_y(row_rect, typography.item);
            if item.selected { ctx.fill_rect(row_rect, fill, None); }
            if self.focused
                && tree.keyboard_focus_visible()
                && self.active_pane == TransferPane::Target
                && self.active_index == raw_index
            {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if item.selected { "check-square" } else { "square" },
                Rect::new(right_x + 4.0, y, 20.0, item_h),
                text,
                12.0,
            );
            if self.item_renderer.is_none() {
                // 默认条目文本使用主题派生的紧凑字号。
                ctx.draw_text(
                    // 绘制当前目标条目标题。
                    &item.title,
                    // 保持既有条目文本起点。
                    Point::new(right_x + 26.0, row_y),
                    // 使用当前主题正文颜色。
                    text,
                    // 使用与垂直定位一致的派生字号。
                    typography.item,
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
        let header = LIST_HEADER_H + if self.searchable { SEARCH_BAR_H } else { 0.0 };
        let Some(frame) = self.last_frame.get().filter(|frame| frame.contains(pos)) else {
            return EventResult::NotHandled;
        };
        let x = pos.x - frame.x;
        let y = pos.y - frame.y;
        let half = ((frame.w - BTN_COL_W) * 0.5).max(MIN_PANE_HALF_W);
        let row = (y >= header).then(|| ((y - header) / ROW_H) as usize);
        if x < half {
            return self.toggle_visible_row(TransferPane::Source, row);
        }
        if x > half + BTN_COL_W {
            return self.toggle_visible_row(TransferPane::Target, row);
        }
        let button_y = frame.h * 0.5 - 20.0;
        if y >= button_y && y < button_y + 20.0 {
            self.move_selected(TransferPane::Source);
            return EventResult::Handled;
        }
        if y >= button_y + 24.0 && y < button_y + 44.0 {
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
impl Default for Transfer {
    fn default() -> Self {
        Self::new()
    }
}

// 验证 Transfer 排版随主题 token 解析。
#[cfg(test)]
mod typography_tests {
    // 引入被测私有排版值契约。
    use super::TransferTypography;
    // 引入可定制的主题 token 实现。
    use crate::ui::theme::DesignTokens;

    // 自定义排版 token 必须驱动标题与条目字号。
    #[test]
    fn transfer_typography_resolves_custom_theme_tokens() {
        // 从完整亮色主题建立测试 token。
        let mut tokens = DesignTokens::antd_light();
        // 覆写小号正文以证明组件没有保留固定 12px。
        tokens.font_size_sm = 10.0;
        // 覆写正文以证明紧凑条目字号由主题相邻 token 派生。
        tokens.font_size = 18.0;
        // 解析当前测试主题的组件排版。
        let typography = TransferTypography::resolve(&tokens);
        // 标题与搜索说明必须直接使用小号正文 token。
        assert_eq!(typography.caption, 10.0);
        // 条目字号必须是两个相邻 token 的中点。
        assert_eq!(typography.item, 14.0);
    }
}
