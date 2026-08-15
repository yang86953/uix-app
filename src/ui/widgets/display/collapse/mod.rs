//! Collapse widget — 折叠面板。

mod free;
mod methods;

use self::free::*;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::component::paint_context::PaintContext;
// 引入展开面板稳定 key 集合的受控状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotCollapsePanel,
    SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// 折叠头部高度（36.0）；同名常量在 selectable_list/calendar/date_calendar 各为 48/40/32。
const HEADER_HEIGHT: f32 = 36.0;
const HEADER_FONT_SIZE: f32 = 14.0;
const CONTENT_FONT_SIZE: f32 = 12.0;
const TEXT_LINE_HEIGHT: f32 = 1.5;
const HEADER_ICON_SLOT: f32 = 28.0;
const HEADER_RIGHT_PADDING: f32 = 12.0;
const CONTENT_HORIZONTAL_PADDING: f32 = 16.0;
const CONTENT_VERTICAL_PADDING: f32 = 8.0;
// 组件默认尺寸（本组件设计值）；其他组件同名常量值不同，属各自设计。
const DEFAULT_WIDTH: f32 = 240.0;
const MAX_INTRINSIC_WIDTH: f32 = 320.0;

/// 单个折叠面板。
#[derive(Debug, Clone)]
pub struct CollapsePanel {
    // 保存可选显式稳定 key，缺省时兼容使用 header。
    key: Option<String>,
    /// 面板标题与未显式设置键时使用的兼容身份。
    pub header: String,
    /// 面板展开后显示的文字内容。
    pub content: String,
    /// 面板当前是否展开。
    pub expanded: bool,
}

impl CollapsePanel {
    /// 创建以标题作为兼容稳定身份、初始折叠的面板。
    pub fn new(header: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            // 缺省稳定身份继续兼容现有 header。
            key: None,
            header: header.into(),
            content: content.into(),
            expanded: false,
        }
    }
    /// 将面板初始状态设置为展开。
    pub fn expanded(mut self) -> Self {
        self.expanded = true;
        self
    }
    /// 设置非空稳定面板键；空白值保持标题兼容身份。
    pub fn key(mut self, key: impl Into<String>) -> Self {
        // 空 key 不覆盖兼容 header 身份。
        let key = key.into();
        // 只接受至少包含一个非空白字符的显式身份。
        if !key.trim().is_empty() {
            // 保留调用方提供的精确稳定 key。
            self.key = Some(key);
        }
        self
    }
    /// 返回显式稳定键，未设置时返回标题兼容身份。
    pub fn stable_key(&self) -> &str {
        // 旧调用方无需修改即可取得确定身份。
        self.key.as_deref().unwrap_or(&self.header)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollapseContentEntry {
    panel_index: usize,
    key: String,
    content: String,
}

component! {
    /// Collapse — 可折叠面板组。
    pub struct Collapse {
        pub(crate) panels: Vec<CollapsePanel>,
        accordion: bool,
        // 外部状态只拥有稳定展开 key 集合。
        #[snapshot(skip)]
        active_keys_binding: Option<State<Vec<String>>>,
        borderless: bool,
        destroy_on_hide: bool,
        focused: bool,
        focused_header: usize,
        hovered_header: Cell<Option<usize>>,
        pressed_header: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<usize>>,
        pub(crate) transitions: Vec<TransitionPlayer>,
        transition_dirty: bool,
        layout_requested: Cell<bool>,
        #[snapshot(skip)]
        content_opacities: Vec<Rc<Cell<f32>>>,
        #[snapshot(skip)]
        materialized_content: RefCell<Vec<CollapseContentEntry>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.panels.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        let preferred_width = self.preferred_width();
        let width = constraints.clamp(Size::new(preferred_width, 0.0)).w;
        constraints.clamp(Size::new(width, self.intrinsic_height(width)))
    }

    build_view_children => (&self) -> Vec<crate::ui::view::ViewNode> {
        let entries = self.desired_content_entries();
        let views = self.content_views(&entries);
        self.materialized_content.replace(entries);
        views
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        // 借用全部已物化内容条目，稳定 key 保留原面板身份。
        let entries = self.materialized_content.borrow();
        // 通用布局入口只传入当前有效可见子集，不能再使用压缩后的索引。
        children
            .iter()
            // 按动态子节点自身稳定 key 回查原始面板条目。
            .filter_map(|child| {
                // 读取当前树中仍存活的真实子节点。
                let child_node = tree.get(child.id)?;
                // 动态 Collapse 内容必须保留协调时登记的稳定 key。
                let child_key = child_node.key()?;
                // 在完整条目表中恢复未压缩的原始 panel_index。
                let entry = entries.iter().find(|entry| entry.key == child_key)?;
                // 使用真实面板索引计算标题之后的内容 frame。
                self.content_frame(frame, entry.panel_index)
                    .map(|content_frame| (child.id, content_frame))
            })
            // 返回当前可见子集的确定放置结果。
            .collect()
    }

    child_visible => (&self, index: usize) -> bool {
        let entries = self.materialized_content.borrow();
        let Some(entry) = entries.get(index) else {
            return false;
        };
        self.panels
            .get(entry.panel_index)
            .is_some_and(|panel| self.panel_present(entry.panel_index, panel))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 每次交互前重新采用外部唯一展开事实。
        self.sync_bound_active_keys();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(index) = self.header_at_point(*pos) {
                    self.focused_header = index;
                    self.pressed_header.set(Some(index));
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let pressed = self.pressed_header.replace(None);
                if let Some(index) = pressed {
                    if self.header_at_point(*pos) == Some(index) {
                        self.focused_header = index;
                        self.toggle_panel(index);
                    }
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerMove { pos, .. } => {
                let next = self.header_at_point(*pos);
                if self.hovered_header.replace(next) != next {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered_header.replace(None).is_some()
                    | self.pressed_header.replace(None).is_some();
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.pressed_header.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    self.move_focus(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_focus(false);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.focused_header = 0;
                    EventResult::Handled
                }
                KeyCode::End if !self.panels.is_empty() => {
                    self.focused_header = self.panels.len() - 1;
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.panels.is_empty() => {
                    self.toggle_panel(self.focused_header);
                    EventResult::Handled
                }
                KeyCode::Right if !self.panels.is_empty() => {
                    self.set_panel_expanded(self.focused_header, true);
                    EventResult::Handled
                }
                KeyCode::Left if !self.panels.is_empty() => {
                    self.set_panel_expanded(self.focused_header, false);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            // 只为仍存在的面板发布稳定 key。
            .and_then(|idx| self.panels.get(idx))
            // Change 载荷不再泄漏易变索引。
            .map(|panel| SemanticEvent::change(id, panel.stable_key().to_owned()))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 登记受控 key 集合依赖，外部更新会触发声明视图重建。
        self.capture_bound_active_keys_dependency();
        let frame = Self::normalized_frame(frame);
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = ctx.tokens().color_bg_elevated();
        let body_bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let hover_bg = ctx.tokens().color_fill_quaternary();
        let pressed_bg = ctx.tokens().color_fill_tertiary();
        let r = (!self.borderless).then(|| Radius::uniform(ctx.tokens().border_radius_sm()));
        let mut y = frame.y;
        let frame_bottom = frame.y + frame.h;
        ctx.push_clip(frame);

        for (idx, p) in self.panels.iter().enumerate() {
            if y >= frame_bottom {
                break;
            }
            let header_rect = Rect::new(frame.x, y, frame.w, HEADER_HEIGHT.min(frame_bottom - y));
            let header_bg = if self.pressed_header.get() == Some(idx) {
                pressed_bg
            } else if self.hovered_header.get() == Some(idx) {
                hover_bg
            } else {
                bg
            };
            ctx.fill_rect(header_rect, header_bg, r);
            if !self.borderless {
                ctx.stroke_rect(header_rect, border, 1.0, r);
            }
            if self.focused && tree.keyboard_focus_visible() && idx == self.focused_header {
                let inset = 0.75_f32.min(header_rect.w * 0.5).min(header_rect.h * 0.5);
                let focus_rect = Rect::new(
                    header_rect.x + inset,
                    header_rect.y + inset,
                    (header_rect.w - inset * 2.0).max(0.0),
                    (header_rect.h - inset * 2.0).max(0.0),
                );
                if focus_rect.w > 0.0 && focus_rect.h > 0.0 {
                    ctx.stroke_rect(focus_rect, primary, 1.5, r);
                }
            }
            let icon_rect = Rect::new(header_rect.x + 4.0, header_rect.y, 24.0, header_rect.h);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if p.expanded { "chevron-down" } else { "chevron-right" },
                icon_rect,
                text_secondary,
                12.0_f32.min(header_rect.h * 0.6),
            );
            let text_width = (header_rect.w - HEADER_ICON_SLOT - HEADER_RIGHT_PADDING).max(0.0);
            if let Some(visible_header) = elide_single_line(
                ctx,
                &p.header,
                HEADER_FONT_SIZE,
                text_width,
            ) {
                let header_y = ctx.visual_center_y(header_rect, HEADER_FONT_SIZE);
                let text_rect = Rect::new(
                    header_rect.x + HEADER_ICON_SLOT,
                    header_rect.y,
                    text_width,
                    header_rect.h,
                );
                ctx.push_clip(text_rect);
                ctx.draw_text(
                    &visible_header,
                    Point::new(text_rect.x, header_y),
                    text_color,
                    HEADER_FONT_SIZE,
                );
                ctx.pop_clip();
            }
            y += HEADER_HEIGHT;

            if self.panel_present(idx, p) {
                let content_height = Self::content_height(&p.content, frame.w);
                let body_height = content_height.min((frame_bottom - y).max(0.0));
                let body_rect = Rect::new(frame.x, y, frame.w, body_height);
                if body_rect.w > 0.0 && body_rect.h > 0.0 {
                    ctx.fill_rect(body_rect, body_bg, r);
                    if !self.borderless {
                        ctx.stroke_rect(body_rect, border, 1.0, r);
                    }
                }
                y += content_height;
            }
            if self.borderless && idx + 1 < self.panels.len() && y < frame_bottom {
                ctx.draw_line(frame.x, y, frame.x + frame.w, y, border, 1.0);
            }
        }
        ctx.pop_clip();
    }

    // NOTE(布局): dirty_rect 目前返回所有面板最大展开时的全量区域（frame），
    // 而不是仅返回变化区域（delta）。因为 collapse 无法可靠追踪哪个面板的
    // expanded 状态在上帧到本帧之间发生了变化（on_event 中修改 expanded 时
    // 未保存旧状态），返回全量可确保展开/折叠时残留像素被清除。
    // 优化方向：在 on_event 中记录 changed_panel index，dirty_rect 仅返回
    // 该 header + 内容区域的变化部分。
    // 始终包含最大展开高度，确保 expanded 切换时残留像素被清除
    dirty_rect => (&self, frame: Rect) -> Rect {
        self.full_dirty_rect(frame)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        self.ensure_transition_count();
        let mut had_active = false;
        let mut still_active = false;
        for (index, transition) in self.transitions.iter_mut().enumerate() {
            if !transition.finished {
                had_active = true;
                transition.update(dt);
                if let Some(opacity) = self.content_opacities.get(index) {
                    opacity.set(transition.opacity_progress.clamp(0.0, 1.0));
                }
                still_active |= !transition.finished;
                if transition.finished
                    && self.panels.get(index).is_some_and(|panel| !panel.expanded)
                {
                    self.layout_requested.set(true);
                }
            }
        }
        self.transition_dirty = had_active;
        still_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            self.full_dirty_rect(frame)
        } else {
            Rect::zero()
        }
    }
}

impl Default for Collapse {
    fn default() -> Self {
        Self::new()
    }
}

// 集中验证稳定 key、受控展开集合与手风琴写回边界。
#[cfg(test)]
mod tests;
