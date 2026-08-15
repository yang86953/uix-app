use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::platform::windowing::ControlSize;
use crate::ui::animation::TransitionPlayer;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SystemEvent,
    WidgetTree,
};
use std::cell::{Cell, RefCell};

use super::search::VisibleRow;
use super::{OptGroup, SelectOption, SelectValueBinding, select_dirty_rect, select_popup_rect};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;

component! {
    /// 支持分组、单选或多选、搜索与弹层导航的选择组件。
    pub struct Select {
        pub(crate) options: Vec<SelectOption>,
        pub(crate) optgroups: Vec<OptGroup>,
        pub(crate) selected: usize,
        pub(crate) selected_multi: Vec<usize>,
        pub(crate) value_binding: Option<SelectValueBinding>,
        pub(crate) open: bool,
        pub(crate) disabled: bool,
        pub(crate) loading: bool,
        pub(crate) loading_phase: f32,
        pub(crate) loading_dirty: bool,
        pub(crate) select_size: ControlSize,
        pub(crate) hovered: bool,
        pub(crate) focused: bool,
        pub(crate) transition: TransitionPlayer,
        pub(crate) closing: bool,
        pub(crate) transition_dirty: bool,
        pub(crate) placeholder: String,
        pub(crate) hovered_option: Option<usize>,
        pub(crate) highlighted_option: Option<usize>,
        pub(crate) pending_change: RefCell<Option<String>>,
        pub(crate) multiple: bool,
        pub(crate) search: bool,
        pub(crate) search_query: String,
        pub(crate) custom_option_views: bool,
        pub(crate) materialized_custom_options: RefCell<Vec<usize>>,
        pub(crate) search_cursor_rect: Cell<Rect>,
        pub(crate) control_rect: Cell<Rect>,
        pub(crate) dropdown_rect: Cell<Rect>,
        // 缓存当前逻辑表面，统一布局、绘制、命中与浮层登记。
        pub(crate) surface_rect: Cell<Option<Rect>>,
        pub(crate) multi_remove_rects: RefCell<Vec<(usize, Rect)>>,
        pub(crate) dropdown_scroll: VirtualListScroll,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.search && !self.disabled }

    text_input_cursor_rect => (&self) -> Rect { self.search_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        // 从当前组件树根节点读取同帧逻辑表面。
        let surface = self.surface_from_tree(frame, tree);
        // 在放置自定义选项前解析并缓存最终弹层矩形。
        let popup = self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
        // 读取当前可见行集合。
        let rows = self.visible_rows();
        // 将弹层相对纵坐标转换为绝对列表起点。
        let list_y = frame.y + popup.y;
        // 多选行需要为复选框预留更宽左槽。
        let text_left = if self.multiple { 32.0 } else { 10.0 };
        // 自定义选项宽度使用受表面约束后的实际弹层宽度。
        let content_width = (popup.w - text_left - 32.0).max(0.0);
        // 读取当前物化的自定义选项索引。
        let indices = self.materialized_custom_options.borrow();
        children
            .iter()
            .zip(indices.iter().copied())
            .filter_map(|(child, option_index)| {
                let row_index = rows.iter().position(
                    |row| matches!(row, VisibleRow::Option(index) if *index == option_index),
                )?;
                Some((
                    child.id,
                    Rect::new(
                        frame.x + popup.x + text_left,
                        list_y + row_index as f32 * DROPDOWN_ROW_HEIGHT
                            - self.dropdown_scroll.scroll_offset(),
                        content_width,
                        DROPDOWN_ROW_HEIGHT,
                    ),
                ))
            })
            .collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        (self.custom_option_views && self.is_present()).then(|| {
            // 子树裁剪直接复用最近解析的实际弹层矩形。
            let popup = self.dropdown_rect.get();
            // 转换为窗口绝对坐标。
            Rect::new(frame.x + popup.x, frame.y + popup.y, popup.w, popup.h)
        })
    }

    hit_test_children => (&self) -> bool { false }

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
        if self.disabled {
            return EventResult::NotHandled;
        }
        self.sync_bound_selection();

        let visible_options = self.visible_option_indices();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if self.control_rect.get().contains(*pos) {
                    if self.multiple {
                        let remove = self
                            .multi_remove_rects
                            .borrow()
                            .iter()
                            .find_map(|(index, rect)| rect.contains(*pos).then_some(*index));
                        if let Some(index) = remove {
                            if let Some(position) = self
                                .selected_multi
                                .iter()
                                .position(|selected| *selected == index)
                            {
                                self.selected_multi.remove(position);
                                self.publish_multi_change();
                            }
                            return EventResult::Handled;
                        }
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.hovered_option = None;
                    return EventResult::Handled;
                }

                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    if let Some(flat_idx) = self.dropdown_row_at_y(pos.y) {
                        if let Some(opt_idx) = self.flat_row_option_index(flat_idx) {
                            self.highlighted_option = Some(opt_idx);
                            if self.multiple {
                                if let Some(multi_idx) =
                                    self.selected_multi.iter().position(|&i| i == opt_idx)
                                {
                                    self.selected_multi.remove(multi_idx);
                                } else {
                                    self.selected_multi.push(opt_idx);
                                }
                                self.publish_multi_change();
                            } else {
                                self.select_single(opt_idx);
                                self.close();
                            }
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }

                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    self.hovered_option = self.dropdown_row_at_y(pos.y);
                } else {
                    self.hovered_option = None;
                }
                self.hovered = self.control_rect.get().contains(*pos);
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.hovered_option = None;
                EventResult::Handled
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
            SystemEvent::Wheel { delta, pos, .. } => {
                if self.is_present() && self.dropdown_rect.get().contains(*pos) {
                    let row_count = self.dropdown_row_count();
                    // 滚轮范围使用受表面缩高后的实际视口高度。
                    let viewport_h = self.effective_dropdown_viewport_height(row_count);
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
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    if self.open {
                        if self.search || self.multiple {
                            self.move_highlight(true);
                        } else {
                            let position = visible_options
                                .iter()
                                .position(|index| *index == self.selected);
                            let next = match position {
                                Some(position) => visible_options.get(position + 1),
                                None => visible_options.first(),
                            }
                            .copied();
                            if let Some(next) = next {
                                self.select_single(next);
                            }
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if self.open && !visible_options.is_empty() {
                        if self.search || self.multiple {
                            self.move_highlight(false);
                        } else {
                            let position = visible_options
                                .iter()
                                .position(|index| *index == self.selected);
                            let prev = match position {
                                Some(position) => {
                                    visible_options.get(position.saturating_sub(1))
                                }
                                None => visible_options.last(),
                            }
                            .copied()
                            .unwrap_or(self.selected);
                            self.select_single(prev);
                        }
                    }
                    EventResult::Handled
                }
                KeyCode::Enter => {
                    if self.open {
                        if self.multiple {
                            self.toggle_highlighted_multi();
                        } else if self.search {
                            let selection = self
                                .highlighted_option
                                .filter(|index| visible_options.contains(index))
                                .or_else(|| visible_options.first().copied());
                            if let Some(index) = selection {
                                self.select_single(index);
                            }
                            self.close();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Space if self.search => EventResult::NotHandled,
                KeyCode::Space => {
                    if self.open {
                        if self.multiple {
                            self.toggle_highlighted_multi();
                        } else {
                            self.close();
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    if self.search && !self.search_query.is_empty() {
                        self.search_query.pop();
                        self.refresh_search_results();
                    } else if self.multiple && !self.selected_multi.is_empty() {
                        self.selected_multi.pop();
                        self.publish_multi_change();
                    }
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            SystemEvent::TextInput { text } if self.search => {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return EventResult::NotHandled;
                }
                self.search_query.push_str(text);
                self.refresh_search_results();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            // 读取最近布局或绘制记录的逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 解析实际可交互弹层，而不是保守 damage 高度。
            let popup = self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
            // 命中框与最终弹层共享同一表面约束。
            select_dirty_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.render_select(frame, ctx);
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取最近布局或绘制记录的逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 使用保守行数解析受表面约束的重绘弹层。
        let popup = self.dropdown_damage_rect(frame, surface);
        // 控件、弹层与阴影脏区全部收敛到当前表面。
        select_dirty_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        if !self.is_present() {
            return None;
        }

        // 读取最近布局或绘制记录的逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 解析并缓存当前实际选项弹层矩形。
        let popup = self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
        // 创建与绘制和命中一致的浮层登记。
        Some(
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(select_popup_rect(frame, popup))
                .z_index(900),
        )
    }

    // 使用组件树提供的同帧表面创建选择弹层登记。
    overlay_entry_for_surface => (&self, id: ComponentId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在登记前更新表面与实际弹层缓存。
        self.remember_dropdown_rect(frame, surface, self.dropdown_row_count());
        // 复用统一的选择弹层登记逻辑。
        self.overlay_entry(id, frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            self.loading_dirty = false;
            return false;
        }

        let transition_active = if self.transition.finished {
            if self.closing {
                self.open = false;
                self.closing = false;
                self.search_query.clear();
                self.highlighted_option = None;
            }
            self.transition_dirty = false;
            false
        } else {
            self.transition.update(dt);
            self.transition_dirty = true;

            if self.closing && self.transition.finished {
                self.open = false;
                self.closing = false;
                self.search_query.clear();
                self.highlighted_option = None;
            }

            self.is_present() && !self.transition.finished
        };

        self.loading_dirty = false;
        let loading_active = self.loading && self.is_present();
        if loading_active {
            let before = self.loading_phase;
            self.loading_phase = (self.loading_phase
                + dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8)
                .rem_euclid(std::f32::consts::TAU);
            self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        }

        transition_active || loading_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty || self.loading_dirty {
            // 动画脏区使用最近记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 使用保守行数解析动画可能覆盖的弹层区域。
            let popup = self.dropdown_damage_rect(frame, surface);
            // 将动画脏区限制在当前表面。
            select_dirty_rect(frame, popup, surface)
        } else {
            Rect::zero()
        }
    }
}

// 验证选择弹层使用组件树提供的当前逻辑表面。
#[cfg(test)]
// 将打开状态与内部几何构造限制在当前模块测试中。
mod tests {
    // 复用被测选择组件和结构化选项模型。
    use super::{Select, SelectOption};
    // 引入布局断言所需的基础几何类型。
    use crate::core::{Rect, Size};
    // 引入根节点 frame 读写所需的组件核心 trait。
    use crate::ui::component::widget::WidgetCore;
    // 引入直接执行组件布局与设置根表面所需的树接口。
    use crate::ui::{ComponentId, LayoutChild, WidgetLayout, WidgetTree};

    // 验证显示文案、状态值和快照观测保持各自契约。
    #[test]
    // 覆盖结构化选项的读取与观测路径。
    fn structured_options_keep_labels_separate_from_bound_values() {
        // 使用不同的显示文案和稳定值构造两个选项。
        let mut select = Select::new().select_options([
            // 中文文案映射到稳定地区代码。
            SelectOption::new("中国", "cn"),
            // 另一项文案映射到另一个稳定地区代码。
            SelectOption::new("美国", "us"),
        ]);
        // 选择第二项以覆盖读取稳定值的路径。
        select.select_single(1);

        // 当前业务值必须是稳定值，而不是显示文案。
        assert_eq!(select.current_value().as_deref(), Some("us"));
        // 绘制和搜索入口必须仍然返回显示文案。
        assert_eq!(select.option_label(1), Some("美国"));
        // 快照必须保留既有的显示文案观测语义。
        let crate::ui::SnapshotFields::Select { options, .. } = select.snapshot_fields() else {
            // 选择器只能生成选择器快照分支。
            panic!("选择器应生成 Select 快照");
        };
        // 快照列表不应暴露内部稳定值。
        assert_eq!(options, vec!["中国".to_owned(), "美国".to_owned()]);
    }

    // 自定义选项必须在绘制前的布局阶段直接使用组件树根表面。
    #[test]
    // 测试名称说明布局阶段的同帧表面约束职责。
    fn custom_option_layout_uses_current_tree_surface() {
        // 创建三行自定义选项以产生八十四像素自然弹层。
        let mut select = Select::new().options(["一", "二", "三"]);
        // 打开选择弹层参与子项布局。
        select.open();
        // 模拟声明了自定义选项渲染器的组件状态。
        select.custom_option_views = true;
        // 模拟当前仅物化首个可见自定义选项。
        select.mark_custom_options_materialized(vec![0]);
        // 将控件放在一百二十像素高表面的底部附近。
        let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
        // 创建组件树作为布局阶段的表面来源。
        let mut tree = WidgetTree::new();
        // 放入一个最小根组件以承载窗口表面 frame。
        tree.set_root(Box::new(Select::new()));
        // 取得刚创建的根节点并设置当前逻辑表面。
        tree.root_mut()
            // 测试树必须包含刚设置的根节点。
            .expect("测试组件树应包含根节点")
            // 将根节点布局结果设置为当前窗口表面。
            .set_frame(Rect::new(0.0, 0.0, 200.0, 120.0));
        // 创建与物化索引一一对应的自定义选项布局描述。
        let child = LayoutChild::new(ComponentId::new(7), Size::new(40.0, 20.0));
        // 在尚未执行绘制的情况下直接运行选择组件子项布局。
        let positions = select.layout_children(frame, &[child], &tree);
        // 当前物化的一项必须获得唯一布局结果。
        assert_eq!(positions.len(), 1);
        // 读取首项最终绝对布局矩形。
        let option = positions[0].1;

        // 底边空间不足时首项应随弹层翻转到控件上方。
        assert!(option.y < frame.y);
        // 受约束后的首项不得越出根表面顶边。
        assert!(option.y >= 0.0);
        // 布局阶段应已经缓存缩高到八十像素的实际弹层。
        assert_eq!(select.dropdown_rect.get().h, 80.0);
    }

    // 过滤后实际弹层与保守脏区翻转方向不同时必须同时覆盖上下两侧。
    #[test]
    // 测试名称说明过滤状态切换时的重绘覆盖职责。
    fn dirty_popup_covers_current_and_conservative_directions() {
        // 创建十行可搜索选项使保守弹层高度超过任一侧空间。
        let mut select = Select::searchable().options([
            // 唯一匹配项用于形成一行实际弹层。
            "匹配", // 其余九项用于扩大过滤前保守高度。
            "二",   // 保留第三个不匹配选项。
            "三",   // 保留第四个不匹配选项。
            "四",   // 保留第五个不匹配选项。
            "五",   // 保留第六个不匹配选项。
            "六",   // 保留第七个不匹配选项。
            "七",   // 保留第八个不匹配选项。
            "八",   // 保留第九个不匹配选项。
            "九",   // 保留第十个不匹配选项。
            "十",
        ]);
        // 打开选择弹层参与脏区解析。
        select.open();
        // 输入只匹配首项的搜索词。
        select.search_query = "匹配".to_owned();
        // 将控件放在表面中部偏下，使短弹层向下而保守弹层向上。
        let frame = Rect::new(20.0, 150.0, 120.0, 32.0);
        // 使用三百像素高表面制造两个不同放置方向。
        let surface = Rect::new(0.0, 0.0, 240.0, 300.0);
        // 解析同时服务当前状态与状态切换的局部脏区。
        let damage = select.dropdown_damage_rect(frame, surface);

        // 保守弹层必须覆盖控件上方区域。
        assert!(damage.y < 0.0);
        // 当前一行弹层必须仍缓存为控件下方二十八像素视口。
        assert_eq!(
            select.dropdown_rect.get(),
            Rect::new(0.0, 32.0, 120.0, 28.0)
        );
        // 合并脏区必须覆盖当前向下弹层的完整底边。
        assert!(damage.y + damage.h >= 60.0);
    }

    // 靠近表面底边时，弹层必须翻转或缩高后完整留在表面内。
    #[test]
    // 测试名称说明纵向表面约束职责。
    fn overlay_entry_constrains_popup_near_surface_bottom() {
        // 创建三行选项以产生八十四像素自然弹层。
        let mut select = Select::new().options(["一", "二", "三"]);
        // 打开选择弹层参与登记。
        select.open();
        // 将控件放在一百二十像素表面的底部附近。
        let frame = Rect::new(20.0, 80.0, 120.0, 32.0);
        // 构造比自然弹层更矮的当前逻辑表面。
        let surface = Rect::new(0.0, 0.0, 200.0, 120.0);
        // 通过组件树使用的显式表面入口创建登记。
        let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测选择组件。
            &select,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(5),
            // 传入靠近底边的控件 frame。
            frame,
            // 传入当前逻辑表面。
            surface,
        )
        // 打开状态必须产生浮层登记。
        .expect("打开的选择器应生成浮层登记")
        // 读取登记的绝对弹层矩形。
        .bounds_rect()
        // 选择弹层登记必须声明边界。
        .expect("选择弹层应声明边界");

        // 下方空间不足时弹层应位于控件上方。
        assert!(overlay.y + overlay.h <= frame.y);
        // 最终弹层不得越出当前表面底边。
        assert!(overlay.y + overlay.h <= surface.y + surface.h);
        // 最终弹层不得越出当前表面顶边。
        assert!(overlay.y >= surface.y);
    }

    // 控件靠近窄表面右边缘时，弹层必须横向收敛到表面内。
    #[test]
    // 测试名称说明横向表面约束职责。
    fn overlay_entry_constrains_popup_to_narrow_surface() {
        // 创建一行选项以保持纵向场景简单。
        let mut select = Select::new().options(["一"]);
        // 打开选择弹层参与登记。
        select.open();
        // 构造宽于表面且靠近右边缘的控件。
        let frame = Rect::new(70.0, 20.0, 120.0, 32.0);
        // 使用一百像素宽的窄逻辑表面。
        let surface = Rect::new(0.0, 0.0, 100.0, 120.0);
        // 通过组件树使用的显式表面入口创建登记。
        let overlay = crate::ui::component::traits::WidgetRender::overlay_entry_for_surface(
            // 传入被测选择组件。
            &select,
            // 使用稳定的测试组件标识。
            crate::core::ComponentId::new(6),
            // 传入靠近右边缘的控件 frame。
            frame,
            // 传入当前逻辑表面。
            surface,
        )
        // 打开状态必须产生浮层登记。
        .expect("打开的选择器应生成浮层登记")
        // 读取登记的绝对弹层矩形。
        .bounds_rect()
        // 选择弹层登记必须声明边界。
        .expect("选择弹层应声明边界");

        // 最终弹层不得越出当前表面左边。
        assert!(overlay.x >= surface.x);
        // 最终弹层不得越出当前表面右边。
        assert!(overlay.x + overlay.w <= surface.x + surface.w);
        // 最终弹层宽度不得超过当前表面。
        assert!(overlay.w <= surface.w);
    }
}
