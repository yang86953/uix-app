//! Splitter 分割面板 — 可拖拽调整子面板大小。
//!
//! 支持水平/垂直方向，任意数量面板，最小尺寸约束。

use std::cell::{Cell, RefCell};

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::children::WidgetChildren;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, LayoutChild, MouseButton, SemanticEvent, SystemEvent, View, ViewNode,
    Widget, WidgetId, WidgetTree,
};
use crate::widget;

// 保存 UIX 声明的默认面板、尺寸、交互步长与手柄绘制几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SplitterVisual {
    default_vertical: bool,
    default_panel_count: usize,
    default_ratio: f32,
    default_min_size: f32,
    handle_size: f32,
    intrinsic_width: f32,
    intrinsic_height: f32,
    flex_grow: f32,
    keyboard_step: f32,
    grip_extent: f32,
    grip_icon_extent: f32,
    grip_icon_scale: f32,
    center_ratio: f32,
    active_stroke_width: f32,
    vertical_grip_icon: &'static str,
    horizontal_grip_icon: &'static str,
    background: ColorValue,
    handle_color: ColorValue,
    grip_color: ColorValue,
    active_color: ColorValue,
}

// 同目录 UIX 生成唯一视觉值及静态借用。
crate::uix_items!("src/ui/widgets/containers/splitter/splitter.uix");

// 保存一次绘制内解析后的主题颜色，避免在手柄循环内重复查令牌。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedSplitterVisual {
    background: Color,
    handle_color: Color,
    grip_color: Color,
    active_color: Color,
}

impl SplitterVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedSplitterVisual {
        ResolvedSplitterVisual {
            background: self.background.resolve(tokens),
            handle_color: self.handle_color.resolve(tokens),
            grip_color: self.grip_color.resolve(tokens),
            active_color: self.active_color.resolve(tokens),
        }
    }
}

// 向 UIX 提供静态、零分配的主题角色。
const fn splitter_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

const fn splitter_handle_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}

const fn splitter_grip_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

const fn splitter_active_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

widget! {
    /// Splitter — 可拖拽分割面板容器。
    ///
    /// 子面板之间显示拖拽手柄，支持水平（左右排列）和垂直（上下排列）方向。
    pub struct Splitter {
        children: WidgetChildren,
        /// 水平（false=Row）或垂直（true=Column）
        vertical: bool,
        /// 面板比例（0.0~1.0 之间，各面板占比）
        ratios: Vec<f32>,
        /// 拖拽中的手柄索引
        dragging: Option<usize>,
        focused: bool,
        active_handle: usize,
        /// 各面板最小尺寸（像素）
        min_sizes: Vec<f32>,
        /// 手柄宽度
        handle_size: f32,
        #[snapshot(skip)]
        /// UIX 声明的默认尺寸、手柄几何与主题角色。
        pub(crate) visual: &'static SplitterVisual,
        /// 当前 frame（用于 hit-test）
        last_frame: Cell<Option<Rect>>,
        layout_requested: Cell<bool>,
        pending_change: RefCell<Option<String>>,
    }

    tab_index => (&self) -> i32 { i32::from(self.ratios.len() > 1) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.visual.flex_grow }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(frame) = self.last_frame.get() {
                    if let Some(idx) = self.hit_test_handle(frame, *pos) {
                        self.dragging = Some(idx);
                        self.active_handle = idx;
                        self.focused = true;
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                if self.dragging.is_none() {
                    return EventResult::NotHandled;
                }
                self.dragging = None;
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if let Some(idx) = self.dragging {
                    if let Some(frame) = self.last_frame.get() {
                        self.update_ratios(frame, idx, *pos);
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.dragging = None;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.ratios.len() > 1 => {
                match key {
                    KeyCode::PageUp => {
                        self.active_handle = self.active_handle.saturating_sub(1);
                        return EventResult::Handled;
                    }
                    KeyCode::PageDown => {
                        self.active_handle =
                            (self.active_handle + 1).min(self.ratios.len().saturating_sub(2));
                        return EventResult::Handled;
                    }
                    _ => {}
                }
                let Some(frame) = self.last_frame.get() else {
                    return EventResult::NotHandled;
                };
                match (self.vertical, key) {
                    (false, KeyCode::Left) | (true, KeyCode::Up) => {
                        self.move_active_handle(frame, -self.visual.keyboard_step);
                    }
                    (false, KeyCode::Right) | (true, KeyCode::Down) => {
                        self.move_active_handle(frame, self.visual.keyboard_step);
                    }
                    (_, KeyCode::Home) => {
                        self.move_active_handle_to_limit(frame, false);
                    }
                    (_, KeyCode::End) => {
                        self.move_active_handle_to_limit(frame, true);
                    }
                    _ => return EventResult::NotHandled,
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|ratios| SemanticEvent::change(id, ratios))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    wants_continuous_pointer_move => (&self) -> bool { self.dragging.is_some() }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let visual = self.visual.resolve(ctx.tokens());
        ctx.fill_rect(frame, visual.background, None);

        let n = self.ratios.len();
        if n <= 1 { return; }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = (total - handle_total).max(0.0);
        let mut pos = 0.0;
        for i in 0..n - 1 {
            pos += self.ratios[i] * content_total;
            let handle_rect = if self.vertical {
                Rect::new(frame.x, frame.y + pos, frame.w, self.handle_size)
            } else {
                Rect::new(frame.x + pos, frame.y, self.handle_size, frame.h)
            };
            let active = self.focused && i == self.active_handle || self.dragging == Some(i);
            ctx.fill_rect(
                handle_rect,
                if active {
                    visual.active_color
                } else {
                    visual.handle_color
                },
                None,
            );
            let grip_size = self.visual.grip_extent.min(frame.w).min(frame.h);
            let grip_frame = Rect::new(
                handle_rect.x + (handle_rect.w - grip_size) * self.visual.center_ratio,
                handle_rect.y + (handle_rect.h - grip_size) * self.visual.center_ratio,
                grip_size,
                grip_size,
            );
            crate::ui::widgets::Icon::paint_in_frame(
                ctx,
                if self.vertical {
                    self.visual.vertical_grip_icon
                } else {
                    self.visual.horizontal_grip_icon
                },
                grip_frame,
                visual.grip_color,
                self.visual
                    .grip_icon_extent
                    .min(grip_size * self.visual.grip_icon_scale),
            );
            if active {
                ctx.stroke_rect(
                    handle_rect,
                    visual.active_color,
                    self.visual.active_stroke_width,
                    None,
                );
            }
            pos += self.handle_size;
        }
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
        // Splitter 精确分配面板尺寸，树级测量只需保存身份与零尺寸占位。
        Self::measure_children_reusing(children, output);
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len().min(self.ratios.len()));
        self.layout_children_reusing(frame, children, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        // 比例属于 Splitter，最终位置数组由布局树跨帧持有。
        self.layout_children_reusing(frame, children, output);
    }
}

impl Default for Splitter {
    fn default() -> Self {
        Self::new()
    }
}

impl Splitter {
    // 把精确填充面板的测量身份写入调用方缓冲。
    fn measure_children_reusing(children: &[WidgetId], output: &mut Vec<LayoutChild>) {
        output.clear();
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| LayoutChild::new(id, Size::zero())),
        );
    }

    // 复用调用方位置数组并保持比例、手柄和坐标语义不变。
    fn layout_children_reusing(
        &self,
        frame: Rect,
        children: &[LayoutChild],
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        output.clear();
        let n = children.len().min(self.ratios.len());
        if n == 0 {
            return;
        }

        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = (total - handle_total).max(0.0);
        let mut pos = if self.vertical { frame.y } else { frame.x };
        output.reserve(n);

        for (child, ratio) in children.iter().zip(&self.ratios).take(n) {
            let size = ratio * content_total;
            let child_frame = if self.vertical {
                Rect::new(frame.x, pos, frame.w, size)
            } else {
                Rect::new(pos, frame.y, size, frame.h)
            };
            output.push((child.id, child_frame));
            pos += size + self.handle_size;
        }
    }

    /// 创建两个等分、水平排列且最小尺寸均为 50 像素的分割面板。
    pub fn new() -> Self {
        let visual = SPLITTER_VISUAL_REF;
        Self {
            children: WidgetChildren::new(),
            vertical: visual.default_vertical,
            ratios: vec![visual.default_ratio; visual.default_panel_count],
            dragging: None,
            focused: false,
            active_handle: 0,
            min_sizes: vec![visual.default_min_size; visual.default_panel_count],
            handle_size: visual.handle_size,
            visual,
            last_frame: Cell::new(None),
            layout_requested: Cell::new(false),
            pending_change: RefCell::new(None),
        }
    }

    /// 设置运行时面板数量并重置为等分比例；零会归一化为一个面板。
    pub fn panels(mut self, count: usize) -> Self {
        let count = count.max(1);
        let ratio = 1.0 / count as f32;
        self.ratios = vec![ratio; count];
        self.min_sizes = vec![self.visual.default_min_size; count];
        self
    }

    /// 设置文档化双面板的初始左侧或上侧比例。
    pub fn default_ratio(mut self, ratio: f32) -> Self {
        // 把动态输入收敛到运行时可维护的比例范围。
        let ratio = Self::normalize_ratio(ratio);
        // Splitter 文档契约固定拥有两个互补面板。
        self.ratios = vec![ratio, 1.0 - ratio];
        // 返回已经归一化的构建器。
        self
    }

    /// 设置面板是否沿垂直方向上下排列。
    pub fn vertical(mut self, v: bool) -> Self {
        self.vertical = v;
        self
    }
    /// 设置指定面板的非负最小尺寸；无效索引不产生变化。
    pub fn min_size(mut self, index: usize, size: f32) -> Self {
        if index < self.min_sizes.len() {
            self.min_sizes[index] = Self::normalize_size(size);
        }
        self
    }

    /// 返回各面板当前所占内容空间的比例。
    pub fn ratios(&self) -> &[f32] {
        &self.ratios
    }

    /// 返回当前键盘操作或拖拽选中的分隔手柄索引。
    pub fn active_handle(&self) -> usize {
        self.active_handle
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.visual.intrinsic_width, self.visual.intrinsic_height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.vertical = next.vertical;
        self.min_sizes = next.min_sizes;
        self.handle_size = next.handle_size;
        self.visual = next.visual;
        if self.ratios.len() != next.ratios.len() {
            self.ratios = next.ratios;
        }
        if self
            .dragging
            .is_some_and(|idx| idx + 1 >= self.ratios.len())
        {
            self.dragging = None;
        }
        self.active_handle = self.active_handle.min(self.ratios.len().saturating_sub(2));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Splitter {
            vertical: self.vertical,
            panel_count: self.ratios.len(),
            min_sizes: self.min_sizes.clone(),
            handle_size: self.handle_size,
            ratios: self.ratios.clone(),
            active_handle: self.active_handle,
        }
    }

    fn hit_test_handle(&self, frame: Rect, pos: Point) -> Option<usize> {
        if !frame.contains(pos) {
            return None;
        }
        let n = self.ratios.len();
        if n <= 1 {
            return None;
        }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        if content_total <= 0.0 {
            return None;
        }

        let mut cursor = 0.0;
        for i in 0..n - 1 {
            cursor += self.ratios[i] * content_total;
            let hit = if self.vertical {
                pos.y >= frame.y + cursor && pos.y <= frame.y + cursor + self.handle_size
            } else {
                pos.x >= frame.x + cursor && pos.x <= frame.x + cursor + self.handle_size
            };
            if hit {
                return Some(i);
            }
            cursor += self.handle_size;
        }
        None
    }

    fn update_ratios(&mut self, frame: Rect, idx: usize, pos: Point) -> bool {
        let n = self.ratios.len();
        if idx >= n - 1 {
            return false;
        }
        let total = if self.vertical { frame.h } else { frame.w };
        let handle_total = self.handle_size * (n - 1) as f32;
        let content_total = total - handle_total;
        if content_total <= 0.0 {
            return false;
        }

        let raw_pos = if self.vertical {
            pos.y - frame.y
        } else {
            pos.x - frame.x
        };
        let adjusted = (raw_pos - idx as f32 * self.handle_size)
            .max(0.0)
            .min(content_total);
        let prefix = self.ratios[..idx].iter().sum::<f32>() * content_total;
        self.set_handle_left_size(frame, idx, adjusted - prefix)
    }

    fn move_active_handle(&mut self, frame: Rect, delta: f32) -> bool {
        let Some((_, left, _)) = self.handle_pair_sizes(frame, self.active_handle) else {
            return false;
        };
        self.set_handle_left_size(frame, self.active_handle, left + delta)
    }

    fn move_active_handle_to_limit(&mut self, frame: Rect, towards_end: bool) -> bool {
        let idx = self.active_handle;
        let Some((_, left, right)) = self.handle_pair_sizes(frame, idx) else {
            return false;
        };
        let desired = if towards_end {
            left + right - self.min_sizes[idx + 1]
        } else {
            self.min_sizes[idx]
        };
        self.set_handle_left_size(frame, idx, desired)
    }

    fn set_handle_left_size(&mut self, frame: Rect, idx: usize, desired: f32) -> bool {
        let Some((content_total, left, right)) = self.handle_pair_sizes(frame, idx) else {
            return false;
        };
        let pair_total = left + right;
        let min_left = self.min_sizes[idx];
        let max_left = pair_total - self.min_sizes[idx + 1];
        if min_left > max_left {
            return false;
        }
        let next_left = desired.clamp(min_left, max_left);
        if (next_left - left).abs() <= f32::EPSILON {
            return false;
        }
        self.ratios[idx] = next_left / content_total;
        self.ratios[idx + 1] = (pair_total - next_left) / content_total;
        self.layout_requested.set(true);
        self.pending_change.replace(Some(self.ratios_payload()));
        true
    }

    fn handle_pair_sizes(&self, frame: Rect, idx: usize) -> Option<(f32, f32, f32)> {
        if idx + 1 >= self.ratios.len() {
            return None;
        }
        let total = if self.vertical { frame.h } else { frame.w };
        let handles = self.handle_size * self.ratios.len().saturating_sub(1) as f32;
        let content_total = total - handles;
        (content_total > 0.0).then(|| {
            (
                content_total,
                self.ratios[idx] * content_total,
                self.ratios[idx + 1] * content_total,
            )
        })
    }

    fn ratios_payload(&self) -> String {
        self.ratios
            .iter()
            .map(|ratio| format!("{ratio:.6}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn normalize_size(size: f32) -> f32 {
        if size.is_finite() { size.max(0.0) } else { 0.0 }
    }

    // 把声明式初始比例收敛到稳定的双面板区间。
    fn normalize_ratio(ratio: f32) -> f32 {
        // 有限值按文档规定限制在闭区间内。
        if ratio.is_finite() {
            // 防止动态表达式破坏布局不变量。
            ratio.clamp(0.0, 1.0)
        } else {
            // 非有限输入回退到文档默认等分比例。
            SPLITTER_VISUAL_REF.default_ratio
        }
    }
}

// 把 Splitter Rust 交互内核与 UIX 静态视觉组合为单一组件节点。
fn build_splitter_view(mut kernel: Splitter, visual: &'static SplitterVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Splitter {
    fn build(self) -> ViewNode {
        build_splitter_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_splitter_uix_root(kernel: Splitter) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/splitter/splitter.uix")
}
