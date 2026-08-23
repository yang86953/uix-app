use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入稳定节点 key 的受控状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SnapshotTreeNode,
    SystemEvent, WidgetId, WidgetTree,
};
use std::cell::{Cell, RefCell};

// 将表面约束与坐标转换隔离到私有几何模块。
mod geometry;
// 将弹层缓存与实际视口方法隔离到私有实现模块。
mod methods;

// 复用所有 TreeSelect 消费端共享的最终几何函数。
use geometry::{
    // 将相对弹层转换为窗口绝对坐标。
    absolute_tree_select_popup_rect,
    // 合并触发器、弹层与当前表面。
    tree_select_surface_rect,
};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;
const DROPDOWN_TRIGGER_HEIGHT: f32 = 32.0;
const MAX_DROPDOWN_VIEWPORT_HEIGHT: f32 = 280.0;
const MIN_DROPDOWN_WIDTH: f32 = 200.0;

widget! {
    /// 通过窗口内树形弹层选择并绑定稳定节点键的组件。
    pub struct TreeSelect {
        placeholder: String,
        value: String,
        value_key: String,
        // 保存外部稳定节点 key 的受控绑定。
        value_binding: Option<State<String>>,
        nodes: Vec<TreeNode>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        hovered_option: Option<String>,
        highlighted_option: Option<String>,
        pending_change: RefCell<Option<String>>,
        pub(crate) dropdown_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        last_frame: Cell<Option<Rect>>,
        // 缓存相对触发器原点的最终弹层矩形。
        dropdown_rect: Cell<Rect>,
        // 记录当前弹层缓存对应的显示行数。
        dropdown_row_count: Cell<usize>,
        // 缓存显式登记或绘制取得的当前逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        // 缓存最终弹层对应的绝对触发器锚点。
        popup_anchor_frame: Cell<Option<Rect>>,
        // 累积当前呈现周期内需要清理的绝对弹层区域。
        dropdown_damage_rect: Cell<Rect>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.focused = true;
                let frame = self.interaction_frame();
                if point_in_half_open_rect(frame, *pos) {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                // 事件命中复用当前表面解析后的实际弹层。
                let popup = self.interaction_popup_rect(frame, self.flatten_nodes().len());
                // 只在打开状态与最终弹层内处理行选择。
                if self.open && point_in_half_open_rect(popup, *pos) {
                    if let Some(idx) = self.dropdown_row_at_y(pos.y) {
                        let flat = self.flatten_nodes();
                        if flat.get(idx).is_some_and(|(_, _, _, disabled)| *disabled) {
                            return EventResult::Handled;
                        }
                        if self.select_flat_index(idx) {
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }
                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if !self.open {
                    return EventResult::NotHandled;
                }
                // 悬停命中复用当前表面解析后的实际弹层。
                let popup = self.interaction_popup_rect(
                    // 使用组件本地触发器 frame。
                    self.interaction_frame(),
                    // 使用当前扁平行数。
                    self.flatten_nodes().len(),
                );
                // 只在最终弹层内解析悬停行。
                let next = if point_in_half_open_rect(popup, *pos) {
                    let idx = self.dropdown_row_at_y(pos.y);
                    let flat = self.flatten_nodes();
                    idx.and_then(|i| {
                        flat.get(i).and_then(|(key, _, _, disabled)| {
                            (!disabled).then(|| key.clone())
                        })
                    })
                } else {
                    None
                };
                if self.hovered_option != next {
                    self.hovered_option = next;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                if self.hovered_option.take().is_some() {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Wheel { delta, pos, .. } => {
                // 先读取当前扁平行数供几何与滚动共用。
                let row_count = self.flatten_nodes().len();
                // 滚轮命中与视口高度复用最终弹层。
                let popup = self.interaction_popup_rect(self.interaction_frame(), row_count);
                // 只在打开状态与最终弹层内处理滚轮。
                if self.open && point_in_half_open_rect(popup, *pos) {
                    // 使用受当前表面缩高后的实际视口。
                    let viewport_h = popup.h;
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        row_count,
                        DROPDOWN_ROW_HEIGHT,
                        viewport_h,
                    );
                    if dy.abs() > 0.01 {
                        self.push_scroll_delta(0.0, dy);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if !self.open {
                    return match key {
                        KeyCode::Down | KeyCode::Enter | KeyCode::Space => {
                            self.open();
                            EventResult::Handled
                        }
                        _ => EventResult::NotHandled,
                    };
                }
                match key {
                    KeyCode::Down => {
                        self.move_highlight(true);
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        self.move_highlight(false);
                        EventResult::Handled
                    }
                    KeyCode::Home => {
                        self.move_to_edge(true);
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        self.move_to_edge(false);
                        EventResult::Handled
                    }
                    KeyCode::Enter | KeyCode::Space => {
                        self.select_highlighted();
                        EventResult::Handled
                    }
                    KeyCode::Escape => {
                        self.close();
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            // 读取最近登记或绘制记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame, self.flatten_nodes().len());
            // 命中只使用当前实际行数解析弹层。
            let popup = self.remember_popup_rect(frame, surface, self.flatten_nodes().len());
            // 将相对弹层转换为窗口绝对坐标。
            let popup = absolute_tree_select_popup_rect(frame, popup);
            // 触发器与实际弹层命中框共同收敛到当前表面。
            tree_select_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w.max(0.0), frame.h.max(0.0))));
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_quaternary();
        let fill = ctx.tokens().color_fill_tertiary();
        let scale = (frame.h / DROPDOWN_TRIGGER_HEIGHT).clamp(0.0, 1.0);
        let font_size = 13.0 * scale;
        let left_padding = 10.0 * scale;
        let arrow_slot = 28.0 * scale;
        let radius = Some(Radius::uniform(
            (ctx.tokens().border_radius_sm() * scale).min(frame.h.max(0.0) * 0.5),
        ));
        let input_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let bc = if self.open || self.focused { primary } else { border };
        ctx.push_clip(input_rect);
        ctx.fill_rect(input_rect, bg, radius);
        ctx.stroke_rect(
            input_rect,
            bc,
            if self.open || self.focused { 2.0 } else { 1.0 },
            radius,
        );
        if font_size > 0.0 && input_rect.w > 0.0 {
            let arrow_rect = Rect::new(
                input_rect.x + (input_rect.w - arrow_slot).max(0.0),
                input_rect.y,
                arrow_slot.min(input_rect.w),
                input_rect.h,
            );
            let text_area = Rect::new(
                input_rect.x + left_padding,
                input_rect.y,
                (arrow_rect.x - input_rect.x - left_padding).max(0.0),
                input_rect.h,
            );
            if text_area.w > 0.0 {
                let display = if self.value.is_empty() {
                    &self.placeholder
                } else {
                    &self.value
                };
                let display_color = if self.value.is_empty() {
                    text_tertiary
                } else {
                    text
                };
                let input_y = ctx.visual_center_y(text_area, font_size);
                ctx.push_clip(text_area);
                ctx.draw_text(
                    display,
                    Point::new(text_area.x, input_y),
                    display_color,
                    font_size,
                );
                ctx.pop_clip();
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if self.is_present() {
                    "chevron-up"
                } else {
                    "chevron-down"
                },
                arrow_rect,
                text_secondary,
                12.0 * scale,
            );
        }
        ctx.pop_clip();

        if !self.is_present() {
            return;
        }

        // 将逻辑表面映射到当前组件坐标，兼容被提升的滚动浮层。
        let surface = ctx.logical_surface_rect();
        // 先读取当前扁平节点列表供几何与绘制共用。
        let flat = self.flatten_nodes();
        // 在绘制弹层前解析并缓存同帧最终几何。
        let popup = self.remember_popup_rect(frame, surface, flat.len());
        // 将相对触发器缓存转换为窗口绝对弹层矩形。
        let list_rect = absolute_tree_select_popup_rect(frame, popup);
        // 空表面不生成可见树选择弹层。
        if list_rect.w <= 0.0 || list_rect.h <= 0.0 {
            // 保留触发器绘制结果并跳过弹层。
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(bg, opacity);
        let border = fade_color(border, opacity);
        let primary = fade_color(primary, opacity);
        let text = fade_color(text, opacity);
        let fill = fade_color(fill, opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let text_tertiary = fade_color(text_tertiary, opacity);
        let display_row_count = flat.len().max(1);
        let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        // 将整个树选择弹层裁剪到当前逻辑表面。
        ctx.push_clip(surface);
        // 再按最终弹层矩形裁剪行与边框。
        ctx.push_clip(list_rect);
        ctx.fill_rect(list_rect, bg, panel_radius);
        ctx.stroke_rect(list_rect, border, 1.0, panel_radius);

        if flat.is_empty() {
            let text_area = Rect::new(
                list_rect.x + 10.0,
                list_rect.y,
                (list_rect.w - 20.0).max(0.0),
                list_rect.h,
            );
            let row_y = ctx.visual_center_y(text_area, 13.0);
            ctx.push_clip(text_area);
            ctx.draw_text(
                crate::ui::widget_runtime::locale::use_locale().no_data,
                Point::new(text_area.x, row_y),
                text_tertiary,
                13.0,
            );
            ctx.pop_clip();
            ctx.pop_clip();
            // 恢复弹层外层的逻辑表面裁剪。
            ctx.pop_clip();
            return;
        }

        let scroll_offset = self.dropdown_scroll.scroll_offset();
        let (start, end) = self.dropdown_scroll.scroll_range(
            display_row_count,
            DROPDOWN_ROW_HEIGHT,
            list_rect.h,
        );

        for (i, (key, title, depth, disabled)) in flat.iter().enumerate().take(end).skip(start) {
            let item_y = list_rect.y + i as f32 * DROPDOWN_ROW_HEIGHT - scroll_offset;
            if item_y + DROPDOWN_ROW_HEIGHT <= list_rect.y
                || item_y >= list_rect.y + list_rect.h
            {
                continue;
            }
            let item_rect = Rect::new(
                list_rect.x,
                item_y,
                list_rect.w,
                DROPDOWN_ROW_HEIGHT,
            );
            let indent = (*depth as f32 * 20.0 + 8.0)
                .min((item_rect.w - 34.0).max(8.0));
            let is_hovered = !disabled && self.hovered_option.as_ref() == Some(key);
            let is_highlighted = !disabled && self.highlighted_option.as_ref() == Some(key);
            let is_selected = *key == self.value_key;

            if is_hovered || is_highlighted {
                ctx.fill_rect(item_rect, fill, None);
            }
            if is_selected {
                ctx.fill_rect(item_rect, primary_bg, None);
            }

            let row_y = ctx.visual_center_y(item_rect, 13.0);
            let tc = if *disabled {
                text_tertiary
            } else if is_selected {
                primary
            } else {
                text
            };
            let text_area = Rect::new(
                item_rect.x + indent,
                item_rect.y,
                (item_rect.w - indent - 10.0).max(0.0),
                item_rect.h,
            );
            if text_area.w > 0.0 {
                ctx.push_clip(text_area);
                ctx.draw_text(title, Point::new(text_area.x, row_y), tc, 13.0);
                ctx.pop_clip();
            }
        }

        ctx.pop_clip();
        // 恢复弹层外层的逻辑表面裁剪。
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取当前行数供表面回退与脏区解析共用。
        let row_count = self.flatten_nodes().len();
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame, row_count);
        // 合并当前与本次呈现周期历史弹层脏区。
        let popup = self.damage_popup_rect(frame, surface, row_count);
        // 将触发器和弹层脏区限制在当前表面内。
        tree_select_surface_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            // 读取当前行数供表面回退与弹层解析共用。
            let row_count = self.flatten_nodes().len();
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame, row_count);
            // 解析并缓存当前实际弹层。
            let popup = self.remember_popup_rect(frame, surface, row_count);
            // 将相对弹层转换为窗口绝对坐标。
            let bounds = absolute_tree_select_popup_rect(frame, popup);
            // 创建只覆盖实际弹层的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记边界与绘制、命中共用同一矩形。
                .bounds(bounds)
                .z_index(900)
        })
    }

    // 使用组件树提供的同帧表面创建树选择弹层登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际弹层缓存。
        self.remember_popup_rect(frame, surface, self.flatten_nodes().len());
        // 复用统一的树选择弹层登记逻辑。
        self.overlay_entry(id, frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            return false;
        }

        if self.transition.finished {
            if self.closing {
                self.closing = false;
                self.hovered_option = None;
                self.highlighted_option = None;
                self.dropdown_scroll.set_scroll_offset(0.0);
            }
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.closing = false;
            self.hovered_option = None;
            self.highlighted_option = None;
            self.dropdown_scroll.set_scroll_offset(0.0);
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            // 读取当前行数供表面回退与脏区解析共用。
            let row_count = self.flatten_nodes().len();
            // 动画脏区使用最近记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame, row_count);
            // 动画期间同时覆盖当前与历史最终弹层。
            let popup = self.damage_popup_rect(frame, surface, row_count);
            // 将动画脏区限制在当前表面。
            tree_select_surface_rect(frame, popup, surface)
        } else {
            Rect::zero()
        }
    }
}

impl TreeSelect {
    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, DROPDOWN_TRIGGER_HEIGHT)
    }

    pub(crate) fn dropdown_row_at_y(&self, pos_y: f32) -> Option<usize> {
        // 行命中复用当前表面约束后的实际弹层。
        let popup = self.dropdown_local_rect();
        if pos_y < popup.y || pos_y >= popup.y + popup.h {
            return None;
        }
        let local_y = pos_y - popup.y + self.dropdown_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let idx = (local_y / DROPDOWN_ROW_HEIGHT) as usize;
        let flat_len = self.flatten_nodes().len();
        if idx < flat_len { Some(idx) } else { None }
    }

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        })
    }

    fn dropdown_local_rect(&self) -> Rect {
        // 读取事件路径使用的本地触发器 frame。
        let frame = self.interaction_frame();
        // 返回当前表面解析后的最终本地弹层。
        self.interaction_popup_rect(frame, self.flatten_nodes().len())
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn flatten_nodes(&self) -> Vec<(String, String, usize, bool)> {
        let mut result = Vec::new();
        self.flatten(&self.nodes, 0, &mut result);
        result
    }

    fn flatten(
        &self,
        nodes: &[TreeNode],
        depth: usize,
        result: &mut Vec<(String, String, usize, bool)>,
    ) {
        for node in nodes {
            result.push((node.key.clone(), node.title.clone(), depth, node.disabled));
            if !node.children.is_empty() {
                self.flatten(&node.children, depth + 1, result);
            }
        }
    }

    // 从外部稳定 key 同步运行时展示标题。
    fn sync_bound_value(&mut self) {
        // 未绑定时保留组件内部选择状态。
        let Some(key) = self.value_binding.as_ref().map(State::get) else {
            return;
        };
        // 绑定值始终保存稳定节点 key。
        self.value_key.clone_from(&key);
        // 展示值由当前树结构中的节点标题派生。
        self.value = self
            .flatten_nodes()
            .into_iter()
            .find_map(|(candidate, title, _, _)| (candidate == key).then_some(title))
            .unwrap_or_default();
    }

    // 将用户选择发布回外部稳定 key 状态。
    fn write_bound_value(&self, key: &str) {
        // 非受控模式不产生外部写入。
        let Some(state) = self.value_binding.as_ref() else {
            return;
        };
        // 避免向状态系统重复发布相同值。
        if state.get() != key {
            state.set(key.to_owned());
        }
    }

    /// 创建一个使用默认占位文本、空节点树且未展开的树选择器。
    pub fn new() -> Self {
        Self {
            placeholder: "Please select".into(),
            value: String::new(),
            value_key: String::new(),
            // 默认保持非受控选择模式。
            value_binding: None,
            nodes: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            hovered_option: None,
            highlighted_option: None,
            pending_change: RefCell::new(None),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            // 首次表面解析前弹层缓存为空。
            dropdown_rect: Cell::new(Rect::zero()),
            // 零行标记尚未生成有效弹层缓存。
            dropdown_row_count: Cell::new(0),
            // 首次登记或绘制前尚未取得当前逻辑表面。
            surface_rect: Cell::new(None),
            // 首次登记前尚未取得绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 首次呈现前没有历史弹层脏区。
            dropdown_damage_rect: Cell::new(Rect::zero()),
        }
    }
    /// 设置尚未选中节点时显示的占位文本。
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_string();
        self
    }
    /// 替换候选节点树，并按当前稳定节点键重新解析显示标题。
    pub fn nodes(mut self, n: Vec<TreeNode>) -> Self {
        self.nodes = n;
        // 支持先绑定 value 再设置树节点的构造顺序。
        self.sync_bound_value();
        self
    }
    /// 将稳定节点键双向绑定到外部字符串状态。
    pub fn bind_value(mut self, state: &State<String>) -> Self {
        // 克隆轻量状态句柄供交互提交使用。
        self.value_binding = Some(state.clone());
        // 构造时立即读取状态并登记响应式依赖。
        self.sync_bound_value();
        self
    }
    /// 返回当前选中节点用于展示的标题。
    pub fn value(&self) -> &str {
        &self.value
    }
    /// 返回当前选中节点的稳定键。
    pub fn value_key(&self) -> &str {
        &self.value_key
    }

    /// 返回弹层当前是否处于逻辑展开状态。
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 返回弹层当前是否仍需呈现，包括退出过渡阶段。
    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    /// 展开弹层、重置其表面缓存，并高亮当前选择或首个可用节点。
    pub fn open(&mut self) {
        // 新呈现周期重新收集弹层脏区。
        self.dropdown_damage_rect.set(Rect::zero());
        // 新呈现周期等待当前帧重新解析弹层。
        self.dropdown_row_count.set(0);
        // 丢弃上一呈现周期的相对弹层缓存。
        self.dropdown_rect.set(Rect::zero());
        // 等待当前帧取得最新逻辑表面。
        self.surface_rect.set(None);
        // 等待当前帧取得最新绝对锚点。
        self.popup_anchor_frame.set(None);
        self.open = true;
        self.closing = false;
        self.dropdown_scroll.set_scroll_offset(0.0);
        let flat = self.flatten_nodes();
        self.hovered_option = None;
        let highlighted_index = flat
            .iter()
            .position(|(key, _, _, disabled)| key == &self.value_key && !disabled)
            .or_else(|| flat.iter().position(|(_, _, _, disabled)| !disabled));
        self.highlighted_option = highlighted_index.map(|index| flat[index].0.clone());
        if let Some(index) = highlighted_index {
            self.reveal_index(index, flat.len());
        }
        self.scroll_delta_strip.set((0.0, 0.0));
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    /// 关闭弹层并在已呈现时启动退出过渡。
    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.hovered_option = None;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::TreeSelect {
            placeholder: self.placeholder.clone(),
            nodes: self
                .nodes
                .iter()
                .map(SnapshotTreeNode::from_tree_node)
                .collect(),
            value: self.value.clone(),
            value_key: self.value_key.clone(),
            open: self.open,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 受控实例以新声明树中的状态值为权威。
        let controlled_value = next
            .value_binding
            .as_ref()
            .map(|_| (next.value.clone(), next.value_key.clone()));
        let nodes_changed = self.nodes != next.nodes;
        self.placeholder = next.placeholder;
        self.nodes = next.nodes;
        // 同步声明式重建携带的状态句柄。
        self.value_binding = next.value_binding;
        // 非受控实例保留内部选择，受控实例接受最新状态。
        if let Some((value, value_key)) = controlled_value {
            self.value = value;
            self.value_key = value_key;
        }
        let row_count = self.flatten_nodes().len();
        // 在可变借用虚拟滚动器前计算当前实际视口。
        let viewport_height = self.effective_dropdown_viewport_height(row_count);
        self.dropdown_scroll.clamp_to_content(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            // 动态节点变化使用当前实际视口收敛滚动状态。
            viewport_height,
        );
        if self.is_present() && nodes_changed {
            let flat = self.flatten_nodes();
            self.hovered_option = None;
            self.highlighted_option = flat
                .iter()
                .find(|(key, _, _, disabled)| key == &self.value_key && !disabled)
                .or_else(|| flat.iter().find(|(_, _, _, disabled)| !disabled))
                .map(|(key, _, _, _)| key.clone());
        }
    }

    fn select_highlighted(&mut self) {
        let Some(key) = self.highlighted_option.as_deref() else {
            return;
        };
        let flat = self.flatten_nodes();
        if let Some(index) = flat
            .iter()
            .position(|(candidate, _, _, _)| candidate == key)
        {
            self.select_flat_index(index);
        }
    }

    fn select_flat_index(&mut self, index: usize) -> bool {
        let flat = self.flatten_nodes();
        let Some((key, title, _, disabled)) = flat.get(index) else {
            return false;
        };
        if *disabled {
            return false;
        }
        self.value.clone_from(title);
        self.value_key.clone_from(key);
        // 先更新组件内部状态，再发布稳定节点 key。
        self.write_bound_value(key);
        self.pending_change.replace(Some(key.clone()));
        self.close();
        true
    }

    fn move_highlight(&mut self, forward: bool) {
        let flat = self.flatten_nodes();
        let enabled: Vec<usize> = flat
            .iter()
            .enumerate()
            .filter_map(|(index, (_, _, _, disabled))| (!disabled).then_some(index))
            .collect();
        if enabled.is_empty() {
            return;
        }
        let current = self
            .highlighted_option
            .as_ref()
            .and_then(|key| enabled.iter().position(|index| flat[*index].0 == *key));
        let position = match (current, forward) {
            (Some(position), true) => (position + 1) % enabled.len(),
            (Some(position), false) => (position + enabled.len() - 1) % enabled.len(),
            (None, true) => 0,
            (None, false) => enabled.len() - 1,
        };
        let index = enabled[position];
        self.highlighted_option = Some(flat[index].0.clone());
        self.reveal_index(index, flat.len());
    }

    fn move_to_edge(&mut self, first: bool) {
        let flat = self.flatten_nodes();
        let index = if first {
            flat.iter().position(|(_, _, _, disabled)| !disabled)
        } else {
            flat.iter().rposition(|(_, _, _, disabled)| !disabled)
        };
        let Some(index) = index else {
            return;
        };
        self.highlighted_option = Some(flat[index].0.clone());
        self.reveal_index(index, flat.len());
    }

    fn reveal_index(&mut self, index: usize, row_count: usize) {
        // 键盘显露使用受当前表面缩高后的实际视口。
        let viewport_height = self.effective_dropdown_viewport_height(row_count);
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = index as f32 * DROPDOWN_ROW_HEIGHT;
        let row_bottom = row_top + DROPDOWN_ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll
            .clamp_to_content(row_count, DROPDOWN_ROW_HEIGHT, viewport_height);
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }
}

fn point_in_half_open_rect(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

impl Default for TreeSelect {
    fn default() -> Self {
        Self::new()
    }
}

// 将 TreeSelect 表面约束契约放在独立测试文件中。
