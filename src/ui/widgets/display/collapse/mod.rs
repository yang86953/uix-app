//! Collapse widget — 折叠面板。

mod methods;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::animation::{TransitionPlayer, presets};
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入展开面板稳定 key 集合的受控状态句柄。
use crate::ui::reactive::state::State;
use crate::ui::{
    EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SnapshotCollapsePanel,
    SnapshotFields, SystemEvent, WidgetId, WidgetTree,
};
use std::cell::{Cell, Ref, RefCell};
use std::rc::Rc;

// 保存由 UIX 声明的 Collapse 默认宽度、行高、图标槽与内容留白。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CollapseGeometryVisual {
    default_width: f32,
    max_intrinsic_width: f32,
    header_height: f32,
    header_icon_slot: f32,
    header_right_padding: f32,
    content_horizontal_padding: f32,
    content_vertical_padding: f32,
}

// 保存由 UIX 声明的标题与内容字号、行高比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CollapseTypographyVisual {
    header_font_size: f32,
    content_font_size: f32,
    content_line_height_ratio: f32,
}

// 保存由 UIX 声明的标题图标、焦点圈、边框与分隔线几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CollapseHeaderVisual {
    icon_x: f32,
    icon_width: f32,
    icon_size: f32,
    icon_height_ratio: f32,
    expanded_icon: &'static str,
    collapsed_icon: &'static str,
    focus_inset: f32,
    focus_stroke_width: f32,
    border_width: f32,
    separator_width: f32,
    center_ratio: f32,
}

// Collapse 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollapseRadiusRole {
    Small,
}

impl CollapseRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存由 UIX 声明的 Collapse 主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CollapsePaletteVisual {
    header_background: ColorValue,
    body_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    hover_background: ColorValue,
    pressed_background: ColorValue,
    radius: CollapseRadiusRole,
}

// 完整视觉配置由全部 Collapse 实例共享，实例只保存一个静态引用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CollapseVisual {
    geometry: CollapseGeometryVisual,
    typography: CollapseTypographyVisual,
    header: CollapseHeaderVisual,
    palette: CollapsePaletteVisual,
    borderless_default: bool,
}

// 同目录 UIX 生成全部分组视觉、根视觉记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/collapse/collapse.uix");

// 保存 Collapse 每帧只解析一次的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedCollapseVisual {
    header_background: Color,
    body_background: Color,
    border: Color,
    text: Color,
    text_secondary: Color,
    primary: Color,
    hover_background: Color,
    pressed_background: Color,
    radius: f32,
}

impl CollapseVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedCollapseVisual {
        ResolvedCollapseVisual {
            header_background: self.palette.header_background.resolve(tokens),
            body_background: self.palette.body_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            hover_background: self.palette.hover_background.resolve(tokens),
            pressed_background: self.palette.pressed_background.resolve(tokens),
            radius: self.palette.radius.resolve(tokens),
        }
    }
}

// 向 UIX 提供受限表达式不能直接写入的主题角色。
const fn collapse_small_radius() -> CollapseRadiusRole {
    CollapseRadiusRole::Small
}
const fn collapse_header_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
const fn collapse_body_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn collapse_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
const fn collapse_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn collapse_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn collapse_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn collapse_hover_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillQuaternary)
}
const fn collapse_pressed_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

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

// 保存一个内容宽度下全部面板的测量高度，并跨布局帧复用数组容量。
#[derive(Default)]
struct CollapseContentHeightCacheEntry {
    text_width_bits: Option<u32>,
    heights: Vec<f32>,
}

// 固有宽度与最近两个布局宽度共同保留，不让稳定尺寸切换重新测量全部文本。
#[derive(Default)]
struct CollapseContentHeightCache {
    recent: CollapseContentHeightCacheEntry,
    previous: CollapseContentHeightCacheEntry,
    older: CollapseContentHeightCacheEntry,
}

widget! {
    /// Collapse — 可折叠面板组。
    pub struct Collapse {
        pub(crate) panels: Vec<CollapsePanel>,
        accordion: bool,
        // 外部状态只拥有稳定展开 key 集合。
        #[snapshot(skip)]
        active_keys_binding: Option<State<Vec<String>>>,
        borderless: bool,
        #[snapshot(skip)]
        borderless_authored: bool,
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
        // 面板文本未变化时复用与宽度无关的固有宽度。
        #[snapshot(skip)]
        preferred_width_cache: Cell<Option<f32>>,
        // 内容高度按三个常用文本宽度缓存，并保留已申请数组。
        #[snapshot(skip)]
        content_height_cache: RefCell<CollapseContentHeightCache>,
        #[snapshot(skip)]
        visual: &'static CollapseVisual,
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

    measure_children => (&self, _frame: Rect, children: &[WidgetId], _tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut output = Vec::with_capacity(children.len());
        Self::measure_children_reusing(children, &mut output);
        output
    }

    measure_children_into => (
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        // 动态内容 frame 完全由面板状态计算，测量阶段只保留子节点身份。
        Self::measure_children_reusing(children, output);
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len());
        self.layout_children_reusing(frame, children, tree, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        // 动态内容位置直接写入布局树跨帧复用的结果数组。
        self.layout_children_reusing(frame, children, tree, output);
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
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
        let resolved = self.visual.resolve(ctx.tokens());
        let r = (!self.borderless).then(|| Radius::uniform(resolved.radius));
        let mut y = frame.y;
        let frame_bottom = frame.y + frame.h;
        let content_heights = self.content_heights(frame.w);
        ctx.push_clip(frame);

        for (idx, p) in self.panels.iter().enumerate() {
            if y >= frame_bottom {
                break;
            }
            let header_rect = Rect::new(
                frame.x,
                y,
                frame.w,
                self.visual.geometry.header_height.min(frame_bottom - y),
            );
            let header_bg = if self.pressed_header.get() == Some(idx) {
                resolved.pressed_background
            } else if self.hovered_header.get() == Some(idx) {
                resolved.hover_background
            } else {
                resolved.header_background
            };
            ctx.fill_rect(header_rect, header_bg, r);
            if !self.borderless {
                ctx.stroke_rect(
                    header_rect,
                    resolved.border,
                    self.visual.header.border_width,
                    r,
                );
            }
            if self.focused && tree.keyboard_focus_visible() && idx == self.focused_header {
                let inset = self
                    .visual
                    .header
                    .focus_inset
                    .min(header_rect.w * self.visual.header.center_ratio)
                    .min(header_rect.h * self.visual.header.center_ratio);
                let focus_rect = Rect::new(
                    header_rect.x + inset,
                    header_rect.y + inset,
                    (header_rect.w - inset * 2.0).max(0.0),
                    (header_rect.h - inset * 2.0).max(0.0),
                );
                if focus_rect.w > 0.0 && focus_rect.h > 0.0 {
                    ctx.stroke_rect(
                        focus_rect,
                        resolved.primary,
                        self.visual.header.focus_stroke_width,
                        r,
                    );
                }
            }
            let icon_rect = Rect::new(
                header_rect.x + self.visual.header.icon_x,
                header_rect.y,
                self.visual.header.icon_width,
                header_rect.h,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if p.expanded {
                    self.visual.header.expanded_icon
                } else {
                    self.visual.header.collapsed_icon
                },
                icon_rect,
                resolved.text_secondary,
                self.visual
                    .header
                    .icon_size
                    .min(header_rect.h * self.visual.header.icon_height_ratio),
            );
            let text_width = (header_rect.w
                - self.visual.geometry.header_icon_slot
                - self.visual.geometry.header_right_padding)
                .max(0.0);
            // 复用 UI 绘制上下文拥有的保守单行省略算法。
            if let Some(visible_header) = ctx.elide_single_line_cow(
                &p.header,
                self.visual.typography.header_font_size,
                text_width,
            ) {
                let header_y =
                    ctx.visual_center_y(header_rect, self.visual.typography.header_font_size);
                let text_rect = Rect::new(
                    header_rect.x + self.visual.geometry.header_icon_slot,
                    header_rect.y,
                    text_width,
                    header_rect.h,
                );
                ctx.push_clip(text_rect);
                ctx.draw_text(
                    &visible_header,
                    Point::new(text_rect.x, header_y),
                    resolved.text,
                    self.visual.typography.header_font_size,
                );
                ctx.pop_clip();
            }
            y += self.visual.geometry.header_height;

            if self.panel_present(idx, p) {
                let content_height = content_heights.get(idx).copied().unwrap_or(0.0);
                let body_height = content_height.min((frame_bottom - y).max(0.0));
                let body_rect = Rect::new(frame.x, y, frame.w, body_height);
                if body_rect.w > 0.0 && body_rect.h > 0.0 {
                    ctx.fill_rect(body_rect, resolved.body_background, r);
                    if !self.borderless {
                        ctx.stroke_rect(
                            body_rect,
                            resolved.border,
                            self.visual.header.border_width,
                            r,
                        );
                    }
                }
                y += content_height;
            }
            if self.borderless && idx + 1 < self.panels.len() && y < frame_bottom {
                ctx.draw_line(
                    frame.x,
                    y,
                    frame.x + frame.w,
                    y,
                    resolved.border,
                    self.visual.header.separator_width,
                );
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

// 把折叠状态、动态内容子树与 UIX 视觉表融合为单一根节点。
fn build_collapse_view(mut kernel: Collapse, visual: &'static CollapseVisual) -> ViewNode {
    if !kernel.borderless_authored {
        kernel.borderless = visual.borderless_default;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Collapse {
    fn build(self) -> ViewNode {
        // UIX 拥有公开根与静态视觉；Rust 内核继续拥有稳定 key、状态与动态内容子树。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/collapse/collapse.uix")
    }
}

impl Default for Collapse {
    fn default() -> Self {
        Self::new()
    }
}

impl Collapse {
    // 把动态内容子节点身份写入调用方测量缓冲。
    fn measure_children_reusing(children: &[WidgetId], output: &mut Vec<LayoutChild>) {
        output.clear();
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| LayoutChild::new(id, Size::zero())),
        );
    }

    // 按稳定 key 恢复原面板身份，并复用调用方位置数组。
    fn layout_children_reusing(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        tree: &WidgetTree,
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        // 借用全部已物化内容条目，稳定 key 保留原面板身份。
        let entries = self.materialized_content.borrow();
        output.clear();
        output.reserve(children.len());
        // 通用布局入口只传入当前有效可见子集，不能再使用压缩后的索引。
        output.extend(
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
                }),
        );
    }
}

// 集中验证稳定 key、受控展开集合与手风琴写回边界。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/display/collapse/tests.rs"]
mod tests;
