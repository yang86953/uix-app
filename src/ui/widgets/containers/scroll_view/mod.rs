//! ScrollView widget: a scrollable viewport that clips and scrolls children.

use crate::ui::widget_runtime::widget::WidgetCore;
// 将声明式 View 样式适配与核心滚动状态拆分，保持组件文件规模边界。
mod view_style;

use std::cell::Cell;

use self::scrollbar::{ScrollBar, ScrollbarOrientation};
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::ui::children::WidgetChildren;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::ColorValue;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_natural_constraints;
use crate::widget;
// 复用共享布局边界的有限化与 margin 归一规则。
use crate::ui::layout::LayoutChild;
use crate::ui::layout::engine::{finite_non_negative, finite_or_zero, normalize_margin};
use crate::ui::reactive::state::State;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SnapshotFields, SystemEvent, View, ViewNode, Widget,
    WidgetId, WidgetTree,
};

pub use crate::platform::windowing::ScrollDirection;

// 滚轮和键盘步长属于 Rust 输入机制，不进入 UIX 视觉事实。
const WHEEL_VIEWPORT_FACTOR: f32 = 0.25;
const KEYBOARD_LINE_FACTOR: f32 = 0.1;
const KEYBOARD_MIN_LINE: f32 = 16.0;
const KEYBOARD_PAGE_FACTOR: f32 = 0.9;
// 合成差量与滚动条收敛阈值属于 Rust 数值稳定机制。
const SCROLL_DELTA_EPSILON: f32 = 0.01;
const SCROLLBAR_OVERFLOW_EPSILON: f32 = 0.5;

// 保存 UIX 声明的滚动条厚度、轨道几何与主题角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScrollbarVisual {
    thickness: f32,
    edge_padding: f32,
    thumb_min_extent: f32,
    corner_radius: f32,
    track_color: ColorValue,
    thumb_color: ColorValue,
    active_thumb_color: ColorValue,
}

// 保存 UIX 声明的视口默认尺寸、弹性策略、方向与滚动条视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScrollViewVisual {
    default_width: f32,
    default_height: f32,
    default_flex_grow: f32,
    default_flex_shrink: f32,
    default_scrollbar_visible: bool,
    default_direction: ScrollDirection,
    background: ColorValue,
    scrollbar: ScrollbarVisual,
}

// 同目录 UIX 生成唯一视口及滚动条视觉值与静态借用。
crate::uix_items!("src/ui/widgets/containers/scroll_view/scroll_view.uix");

// 向 UIX 提供零分配的默认方向和主题语义角色。
const fn scroll_view_default_direction() -> ScrollDirection {
    ScrollDirection::Vertical
}

const fn scroll_view_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

const fn scrollbar_track_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}

const fn scrollbar_thumb_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}

const fn scrollbar_active_thumb_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Fill)
}

pub mod scrollbar;
#[allow(unused_imports)]
pub(crate) use scrollbar::*;

widget! {
    /// A scrollable viewport that clips its children.
    pub struct ScrollView {
        pub(crate) children: WidgetChildren,
        /// 当前水平滚动偏移。
        pub scroll_x: f32,
        /// 当前垂直滚动偏移。
        pub scroll_y: f32,
        scroll_binding: Option<State<Point>>,
        direction: ScrollDirection,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow_val: f32,
        flex_shrink_val: f32,
        pub(crate) content_bounds: Cell<Option<Size>>,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
        scrollbar_v: ScrollBar,
        scrollbar_h: ScrollBar,
        #[snapshot(skip)]
        /// UIX 声明的视口、滚动条几何及主题角色。
        pub(crate) visual: &'static ScrollViewVisual,
        pub(crate) last_frame: Cell<Option<Rect>>,
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { self.flex_shrink_val }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    on_children_changed => (&mut self, child_count: usize) {
        // 只有空集合需要丢弃由旧滚动内容派生的运行态。
        if child_count == 0 {
            // 统一清除内容范围、偏移和交互残留。
            self.reset_empty_content_state();
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { delta, .. } => {
                let view = self.last_frame.get();
                let mut dx = 0.0;
                let mut dy = 0.0;

                if self.direction.can_scroll_y() && delta.y != 0.0 {
                    let view_h = view
                        .map(|f| f.h)
                        .unwrap_or(self.fixed_height.unwrap_or(self.visual.default_height));
                    dy = delta.y * view_h * WHEEL_VIEWPORT_FACTOR;
                }
                if self.direction.can_scroll_x() && delta.x != 0.0 {
                    let view_w = view
                        .map(|f| f.w)
                        .unwrap_or(self.fixed_width.unwrap_or(self.visual.default_width));
                    dx = delta.x * view_w * WHEEL_VIEWPORT_FACTOR;
                }

                if self.scroll_by(dx, dy) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let frame = match self.last_frame.get() {
                    Some(f) => f,
                    None => return EventResult::NotHandled,
                };
                if !self.scrollbar_v.show && !self.scrollbar_h.show {
                    return EventResult::NotHandled;
                }
                if self.direction.can_scroll_y()
                    && self.max_scroll_y() > 0.0
                    && self
                        .scrollbar_v
                        .hit_test_thumb(
                            frame,
                            *pos,
                            self.effective_scroll_y(),
                            self.max_scroll_y(),
                        )
                {
                    self.scrollbar_v
                        .begin_drag(frame, *pos, self.effective_scroll_y(), self.max_scroll_y());
                    return EventResult::Handled;
                }
                if self.direction.can_scroll_x()
                    && self.max_scroll_x() > 0.0
                    && self
                        .scrollbar_h
                        .hit_test_thumb(
                            frame,
                            *pos,
                            self.effective_scroll_x(),
                            self.max_scroll_x(),
                        )
                {
                    self.scrollbar_h
                        .begin_drag(frame, *pos, self.effective_scroll_x(), self.max_scroll_x());
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.scrollbar_v.dragging {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let max_y = self.max_scroll_y();
                    if max_y > 0.0 {
                        let old_y = self.effective_scroll_y();
                        self.scroll_y = self
                            .scrollbar_v
                            .scroll_from_drag(frame, pos.y, old_y, max_y);
                        self.scroll_delta_strip.set((0.0, self.scroll_y - old_y));
                        self.write_bound_offset();
                    }
                    return EventResult::Handled;
                }

                if self.scrollbar_h.dragging {
                    let frame = match self.last_frame.get() {
                        Some(f) => f,
                        None => return EventResult::Handled,
                    };
                    let max_x = self.max_scroll_x();
                    if max_x > 0.0 {
                        let old_x = self.effective_scroll_x();
                        self.scroll_x = self
                            .scrollbar_h
                            .scroll_from_drag(frame, pos.x, old_x, max_x);
                        self.scroll_delta_strip.set((self.scroll_x - old_x, 0.0));
                        self.write_bound_offset();
                    }
                    return EventResult::Handled;
                }

                if self.scrollbar_v.show || self.scrollbar_h.show {
                    if let Some(frame) = self.last_frame.get() {
                        let old_hover_v = self.scrollbar_v.hover;
                        let old_hover_h = self.scrollbar_h.hover;
                        self.scrollbar_v.hover = false;
                        self.scrollbar_h.hover = false;

                        if self.direction.can_scroll_y() && self.max_scroll_y() > 0.0 {
                            self.scrollbar_v.hover = self
                                .scrollbar_v
                                .hit_test_thumb(
                                    frame,
                                    *pos,
                                    self.effective_scroll_y(),
                                    self.max_scroll_y(),
                                );
                        }
                        if self.direction.can_scroll_x() && self.max_scroll_x() > 0.0 {
                            self.scrollbar_h.hover = self
                                .scrollbar_h
                                .hit_test_thumb(
                                    frame,
                                    *pos,
                                    self.effective_scroll_x(),
                                    self.max_scroll_x(),
                                );
                        }
                        if old_hover_v != self.scrollbar_v.hover
                            || old_hover_h != self.scrollbar_h.hover
                        {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                let was_dragging = self.scrollbar_v.dragging || self.scrollbar_h.dragging;
                self.scrollbar_v.end_drag();
                self.scrollbar_h.end_drag();
                if was_dragging {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                self.scrollbar_v.hover = false;
                self.scrollbar_h.hover = false;
                EventResult::NotHandled
            }
            SystemEvent::KeyDown { key, .. } => {
                let view = self.last_frame.get();
                let view_w = view
                    .map(|f| f.w)
                    .unwrap_or(self.fixed_width.unwrap_or(self.visual.default_width));
                let view_h = view
                    .map(|f| f.h)
                    .unwrap_or(self.fixed_height.unwrap_or(self.visual.default_height));
                let line_x = (view_w * KEYBOARD_LINE_FACTOR).max(KEYBOARD_MIN_LINE);
                let line_y = (view_h * KEYBOARD_LINE_FACTOR).max(KEYBOARD_MIN_LINE);
                let page_y = (view_h * KEYBOARD_PAGE_FACTOR).max(line_y);

                let (dx, dy) = match key {
                    KeyCode::Down if self.direction.can_scroll_y() => (0.0, line_y),
                    KeyCode::Up if self.direction.can_scroll_y() => (0.0, -line_y),
                    KeyCode::PageDown if self.direction.can_scroll_y() => (0.0, page_y),
                    KeyCode::PageUp if self.direction.can_scroll_y() => (0.0, -page_y),
                    KeyCode::End if self.direction.can_scroll_y() => {
                        (0.0, self.max_scroll_y() - self.effective_scroll_y())
                    }
                    KeyCode::Home if self.direction.can_scroll_y() => {
                        (0.0, -self.effective_scroll_y())
                    }
                    KeyCode::Right if self.direction.can_scroll_x() => (line_x, 0.0),
                    KeyCode::Left if self.direction.can_scroll_x() => (-line_x, 0.0),
                    _ => return EventResult::NotHandled,
                };

                if self.scroll_by(dx, dy) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > SCROLL_DELTA_EPSILON || delta.1.abs() > SCROLL_DELTA_EPSILON {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    // 像素搬移只覆盖内容视口；滚动条沟槽由树级失效单独重绘。
    scroll_composite_viewport => (&self, frame: Rect) -> Option<Rect> {
        // 使用与布局、裁剪完全相同的滚动条判定，禁止把动态滑块像素一起搬移。
        let need_v = self.needs_v_scrollbar(frame, &[]);
        // 双向视口还需排除底部横向滚动条沟槽。
        let need_h = self.needs_h_scrollbar(frame, &[]);
        // 空内容视口会由树级合成门禁回退为普通重绘。
        Some(self.content_frame(frame, need_v, need_h))
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((self.effective_scroll_x(), self.effective_scroll_y()))
    }

    scroll_descendant_by => (&mut self, dx: f32, dy: f32) -> bool {
        self.scroll_by(dx, dy)
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        // 裁剪/命中子项时扣除滚动条 gutter，避免点滑块落到内容子树上。
        let need_v = self.needs_v_scrollbar(frame, &[]);
        let need_h = self.needs_h_scrollbar(frame, &[]);
        Some(self.content_frame(frame, need_v, need_h))
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        frame
    }

    // 滚动条属于覆盖层，必须在内容子树及裁剪恢复后绘制。
    paint_after_children => (&self) -> bool {
        true
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_offset_dependency();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));

        match ctx.paint_pass() {
            PaintPass::Content => {
                let bg = self.visual.background.resolve(ctx.tokens());
                ctx.fill_rect(frame, bg, None);
            }
            PaintPass::AfterChildren => {
                // 仅在需要滚动时绘制 gutter 内轨道/滑块（与 layout 预留一致）。
                if self.needs_v_scrollbar(frame, &[]) {
                    self.scrollbar_v
                        .render(
                            frame,
                            ctx,
                            self.effective_scroll_y(),
                            self.max_scroll_y(),
                        );
                }
                if self.needs_h_scrollbar(frame, &[]) {
                    self.scrollbar_h
                        .render(
                            frame,
                            ctx,
                            self.effective_scroll_x(),
                            self.max_scroll_x(),
                        );
                }
            }
        }
    }

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let mut output = Vec::with_capacity(children.len());
        self.measure_children_into(frame, children, tree, &mut output);
        output
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        // gutter 依据上一轮 content_bounds / max_scroll；首帧无溢出信息时先满宽，
        // layout_children 仍会按子项高度决定是否缩进，收敛循环下一轮即可对齐 measure。
        let need_v = self.needs_v_scrollbar(frame, &[]);
        let need_h = self.needs_h_scrollbar(frame, &[]);
        let constraints = self.child_constraints(frame, need_v, need_h);
        output.clear();
        output.extend(
            children
                .iter()
                .copied()
                // 滚动轴必须读取子树自然内容尺寸；普通 Flex basis 会把 flex-grow
                // 子项折成视口尺寸，使真实溢出无法进入 content_bounds。
                .map(|id| child_from_tree_with_natural_constraints(id, tree, constraints)),
        );
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut scratch = crate::ui::LayoutEngineScratch::default();
        let mut output = Vec::with_capacity(children.len());
        self.layout_children_into(frame, children, tree, &mut scratch, &mut output);
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
        // 在组件布局边界清除无界哨兵与非有限 frame 分量。
        let frame = Rect::new(
            finite_or_zero(frame.x),
            finite_or_zero(frame.y),
            finite_non_negative(frame.w),
            finite_non_negative(frame.h),
        );
        // 布局尺寸是当前视口真相；最大偏移不能继续使用上一帧窗口尺寸。
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        // 复用布局树拥有的最终子项位置列表。
        output.clear();
        // 空视口直接记录有限的外框尺寸。
        if children.is_empty() {
            self.content_bounds.set(Some(Size::new(frame.w, frame.h)));
            self.write_bound_offset();
            return;
        }

        // 先根据自然外尺寸与上一轮内容范围判断滚动条。
        let need_v = self.needs_v_scrollbar(frame, children);
        // 横向判断与纵向判断共同决定两个方向的沟槽。
        let need_h = self.needs_h_scrollbar(frame, children);
        // 得到扣除当前滚动条沟槽后的真实内容视口。
        let content = self.content_frame(frame, need_v, need_h);

        // 从内容原点开始追踪最右侧可见占用。
        let mut max_right = content.x;
        // 从内容原点开始追踪最下侧可见占用。
        let mut max_bottom = content.y;
        // 横向单行流从零推进。
        let mut cursor_x = 0.0f32;
        // 纵向列流从零推进。
        let mut cursor_y = 0.0f32;
        // 缓存横向滚动能力，避免循环内重复分支查询。
        let can_scroll_x = self.direction.can_scroll_x();
        // 缓存纵向滚动能力，避免循环内重复分支查询。
        let can_scroll_y = self.direction.can_scroll_y();
        // 仅横向模式采用单行推进，双向模式继续保持既有纵列语义。
        let horizontal_flow = can_scroll_x && !can_scroll_y;
        // 按声明顺序放置全部直接子项。
        for child in children {
            // 保留子项标识供最终布局树回写。
            let cid = child.id;
            // 清除非有限 margin，同时保留负 margin 的重叠语义。
            let margin = normalize_margin(child.margin);
            // 自然宽度必须是可写入实际 frame 的有限非负值。
            let natural_w = finite_non_negative(child.measured_size.w);
            // 自然高度必须是可写入实际 frame 的有限非负值。
            let natural_h = finite_non_negative(child.measured_size.h);
            // 填充宽度由真实内容宽减去左右 margin 得到。
            let available_w =
                finite_non_negative(content.w - margin.left - margin.right);
            // 填充高度由真实内容高减去上下 margin 得到。
            let available_h =
                finite_non_negative(content.h - margin.top - margin.bottom);
            // 非滚动轴填满 content（已扣除 gutter）；滚动轴保留自然尺寸。
            // Both：自然宽与视口取 max —— 窄于视口时拉满，宽于视口时允许横向滚动。
            let w = if can_scroll_x {
                if natural_w <= 0.0 {
                    available_w
                } else if can_scroll_y && need_h {
                    natural_w.max(available_w)
                } else if can_scroll_y {
                    // Both + only vertical overflow: the vertical gutter reduces
                    // the usable cross axis. Keeping the pre-gutter measured width
                    // here would place content underneath the scrollbar.
                    available_w
                } else {
                    natural_w
                }
            } else {
                available_w
            };
            // Collapse 等动态后代可能在收敛传播展开内容前暂时测得零高度。
            // 此处保留上一轮已排布高度作为视口状态，但不覆盖本轮精确测量值。
            let current_h = finite_non_negative(
                tree.get(cid).map(|c| c.frame().h).unwrap_or(0.0),
            );
            let h = if can_scroll_y {
                if natural_h > 0.0 {
                    natural_h
                } else if current_h > 0.0 {
                    current_h
                } else {
                    available_h
                }
            } else {
                available_h
            };
            // 主轴起点先推进前侧 margin，交叉轴同样从 margin 后开始。
            let (x, y) = if horizontal_flow {
                (
                    finite_or_zero(content.x + cursor_x + margin.left),
                    finite_or_zero(content.y + margin.top),
                )
            } else {
                (
                    finite_or_zero(content.x + margin.left),
                    finite_or_zero(content.y + cursor_y + margin.top),
                )
            };
            // 构造已有限化的子项内容 frame。
            let r = Rect::new(x, y, finite_non_negative(w), finite_non_negative(h));
            // 保持输入顺序写入布局结果。
            output.push((cid, r));
            // 正右 margin 属于物理内容范围，负 margin 仅改变推进距离。
            let occupied_right = finite_or_zero(r.x + r.w + margin.right.max(0.0));
            // 正下 margin 属于物理内容范围，负 margin 仅改变推进距离。
            let occupied_bottom = finite_or_zero(r.y + r.h + margin.bottom.max(0.0));
            // 合并当前子项的横向可见占用。
            max_right = max_right.max(occupied_right);
            // 合并当前子项的纵向可见占用。
            max_bottom = max_bottom.max(occupied_bottom);
            // 主轴游标消费前 margin、内容尺寸和后 margin。
            if horizontal_flow {
                cursor_x = finite_or_zero(cursor_x + margin.left + w + margin.right);
            } else {
                cursor_y = finite_or_zero(cursor_y + margin.top + h + margin.bottom);
            }
        }

        // 横向内容至少覆盖扣除沟槽后的真实视口。
        let raw_content_w = finite_non_negative(max_right - content.x).max(content.w);
        // 纵向内容至少覆盖扣除沟槽后的真实视口。
        let raw_content_h = finite_non_negative(max_bottom - content.y).max(content.h);
        // 可横向滚动时，对侧纵向沟槽也要进入滚动坐标总范围。
        let content_w = if can_scroll_x {
            finite_non_negative(
                raw_content_w
                    + if need_v {
                        ScrollBar::gutter()
                    } else {
                        0.0
                    },
            )
        } else {
            // 非横向滚动：content_bounds 宽记视口宽（含 gutter），max_scroll_x 仍为 0。
            frame.w
        };
        // 可纵向滚动时，对侧横向沟槽也要进入滚动坐标总范围。
        let content_h = if can_scroll_y {
            finite_non_negative(
                raw_content_h
                    + if need_h {
                        ScrollBar::gutter()
                    } else {
                        0.0
                    },
            )
        } else {
            frame.h
        };
        // 缓存完整滚动坐标范围，供事件、滑块和下一轮布局共同使用。
        self.content_bounds.set(Some(Size::new(content_w, content_h)));
        // 内容缩短或视口变大时，受控状态必须同步到新的合法末端。
        self.write_bound_offset();
    }
}

impl ScrollView {
    /// 清除空视口不再有效的内容派生状态，同时保留声明配置与视口 frame。
    fn reset_empty_content_state(&mut self) {
        // 空视口没有已布局的内容范围。
        self.content_bounds.set(None);
        // 横向偏移必须回到当前空范围内。
        self.scroll_x = 0.0;
        // 纵向偏移必须回到当前空范围内。
        self.scroll_y = 0.0;
        // 旧内容产生的待消费滚动差量不应跨越结构变更。
        self.scroll_delta_strip.set((0.0, 0.0));
        // 子内容消失时终止纵向滑块拖动。
        self.scrollbar_v.dragging = false;
        // 子内容消失时清除纵向滑块悬停。
        self.scrollbar_v.hover = false;
        // 子内容消失时终止横向滑块拖动。
        self.scrollbar_h.dragging = false;
        // 子内容消失时清除横向滑块悬停。
        self.scrollbar_h.hover = false;
        // 若偏移受控，把归一后的零值同步回声明状态。
        self.write_bound_offset();
    }

    pub(crate) fn scroll_direction(&self) -> ScrollDirection {
        self.direction
    }

    pub(crate) fn explicit_size_locks(&self) -> (bool, bool) {
        (self.fixed_width.is_some(), self.fixed_height.is_some())
    }

    fn intrinsic_size(&self) -> Size {
        // flex_grow 视口：未固定边以 0 为 basis，由父级分得剩余客户区；
        // 否则默认 300×200 会阻止窗口缩小时收缩，内容被窗口裁切且 max_scroll=0。
        let grow = self.flex_grow_val > 0.0;
        Size::new(
            self.fixed_width
                .unwrap_or(if grow { 0.0 } else { self.visual.default_width }),
            self.fixed_height.unwrap_or(if grow {
                0.0
            } else {
                self.visual.default_height
            }),
        )
    }

    /// 内容排布区域：需要滚动条时从视口扣除 gutter，避免卡片与滑块重叠。
    fn content_frame(&self, frame: Rect, need_v: bool, need_h: bool) -> Rect {
        // 内容宽度从有限非负的外框宽度开始。
        let mut w = finite_non_negative(frame.w);
        // 内容高度从有限非负的外框高度开始。
        let mut h = finite_non_negative(frame.h);
        // 纵向滚动条占用右侧固定沟槽。
        if need_v {
            w = (w - ScrollBar::gutter()).max(0.0);
        }
        // 横向滚动条占用底部固定沟槽。
        if need_h {
            h = (h - ScrollBar::gutter()).max(0.0);
        }
        // 返回不会携带测量哨兵或非有限坐标的内容视口。
        Rect::new(
            finite_or_zero(frame.x),
            finite_or_zero(frame.y),
            finite_non_negative(w),
            finite_non_negative(h),
        )
    }

    // 计算父滚动流实际消费的子项外尺寸。
    fn child_outer_size(child: &LayoutChild) -> Size {
        // 共享规则只清除非法 margin，继续允许负 margin 形成重叠。
        let margin = normalize_margin(child.margin);
        // 自然宽度先收敛为可写入实际布局树的尺寸。
        let width = finite_non_negative(child.measured_size.w);
        // 自然高度先收敛为可写入实际布局树的尺寸。
        let height = finite_non_negative(child.measured_size.h);
        // 左右 margin 共同构成横向外尺寸。
        let outer_width = finite_non_negative(width + margin.left + margin.right);
        // 上下 margin 共同构成纵向外尺寸。
        let outer_height = finite_non_negative(height + margin.top + margin.bottom);
        // 返回供首轮滚动条判断使用的有限外尺寸。
        Size::new(outer_width, outer_height)
    }

    fn needs_v_scrollbar(&self, frame: Rect, children: &[LayoutChild]) -> bool {
        // 未启用纵向滚动条或方向不支持纵向滚动时不占沟槽。
        if !(self.scrollbar_v.show && self.direction.can_scroll_y()) {
            return false;
        }
        // 上一轮仍有有效纵向范围时先保持沟槽，随后布局可继续收敛。
        if self.max_scroll_y() > 0.0 {
            return true;
        }
        // 读取有限的当前视口高度作为比较基线。
        let frame_h = finite_non_negative(frame.h);
        // 上一轮内容范围已经溢出时继续显示纵向滚动条。
        if self
            .content_bounds
            .get()
            .is_some_and(|b| finite_non_negative(b.h) > frame_h + SCROLLBAR_OVERFLOW_EPSILON)
        {
            return true;
        }
        // 纵向与双向模式都按纵列累加完整子项外高度。
        let content_h = children.iter().fold(0.0f32, |height, child| {
            // 每一步有限化，避免极值子项累加为无穷大。
            finite_non_negative(height + Self::child_outer_size(child).h)
        });
        // 留出半像素容差，避免浮点抖动反复切换沟槽。
        content_h > frame_h + SCROLLBAR_OVERFLOW_EPSILON
    }

    pub(crate) fn needs_h_scrollbar(&self, frame: Rect, children: &[LayoutChild]) -> bool {
        // 未启用横向滚动条或方向不支持横向滚动时不占沟槽。
        if !(self.scrollbar_h.show && self.direction.can_scroll_x()) {
            return false;
        }
        // 上一轮仍有有效横向范围时先保持沟槽，随后布局可继续收敛。
        if self.max_scroll_x() > 0.0 {
            return true;
        }
        // 读取有限的当前视口宽度作为比较基线。
        let frame_w = finite_non_negative(frame.w);
        // 上一轮内容范围已经溢出时继续显示横向滚动条。
        if self
            .content_bounds
            .get()
            .is_some_and(|b| finite_non_negative(b.w) > frame_w + SCROLLBAR_OVERFLOW_EPSILON)
        {
            return true;
        }
        // 仅横向流按单行累加完整子项外宽。
        let row_width = children.iter().fold(0.0f32, |width, child| {
            // 每一步有限化，避免极值子项累加为无穷大。
            finite_non_negative(width + Self::child_outer_size(child).w)
        });
        // 双向模式保持纵列语义，因此横向范围取各子项最大外宽。
        let content_w = if self.direction.can_scroll_y() {
            children
                .iter()
                .map(|child| Self::child_outer_size(child).w)
                .fold(0.0f32, f32::max)
        } else {
            row_width
        };
        // 留出半像素容差，避免浮点抖动反复切换沟槽。
        content_w > frame_w + SCROLLBAR_OVERFLOW_EPSILON
    }

    fn child_constraints(&self, frame: Rect, need_v: bool, need_h: bool) -> Constraints {
        let content = self.content_frame(frame, need_v, need_h);
        let max_w = if self.direction.can_scroll_x() {
            f32::MAX
        } else {
            content.w
        };
        let max_h = if self.direction.can_scroll_y() {
            f32::MAX
        } else {
            content.h
        };
        Constraints::loose(Size::new(max_w, max_h))
    }

    /// 创建指定滚动方向、没有子组件和固定尺寸的滚动视图。
    pub fn new(direction: ScrollDirection) -> Self {
        let visual = SCROLL_VIEW_VISUAL_REF;
        Self {
            children: WidgetChildren::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            scroll_binding: None,
            direction,
            fixed_width: None,
            fixed_height: None,
            flex_grow_val: visual.default_flex_grow,
            flex_shrink_val: visual.default_flex_shrink,
            content_bounds: Cell::new(None),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            scrollbar_v: ScrollBar::new(ScrollbarOrientation::Vertical),
            scrollbar_h: ScrollBar::new(ScrollbarOrientation::Horizontal),
            visual,
            last_frame: Cell::new(None),
        }
    }

    /// 追加一个由此滚动视图拥有的子组件。
    pub fn child(self, w: impl Widget + 'static) -> Self {
        self.children.add(w);
        self
    }

    /// 替换此滚动视图拥有的全部子组件。
    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    /// 设置滚动视口的首选宽度和高度。
    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }

    /// 设置此视图作为 Flex 子项时的扩张系数。
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow_val = v;
        self
    }

    /// 设置此视图作为 Flex 子项时的收缩系数。
    pub fn flex_shrink(mut self, v: f32) -> Self {
        self.flex_shrink_val = v;
        self
    }

    /// 同时设置水平和垂直滚动条是否可见。
    pub fn show_scrollbar(mut self, v: bool) -> Self {
        self.scrollbar_v.show = v;
        self.scrollbar_h.show = v;
        self
    }

    /// 设置非受控初始滚动位置，并移除外部偏移绑定。
    pub fn scroll_to(mut self, x: f32, y: f32) -> Self {
        self.scroll_binding = None;
        self.scroll_x = Self::normalize_axis(x);
        self.scroll_y = Self::normalize_axis(y);
        self
    }

    /// 将运行态滚动位置双向绑定到应用 State。
    pub fn scroll_offset(mut self, state: &State<Point>) -> Self {
        let offset = state.get();
        self.scroll_x = Self::normalize_axis(offset.x);
        self.scroll_y = Self::normalize_axis(offset.y);
        self.scroll_binding = Some(state.clone());
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ScrollView {
            direction: self.direction,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            flex_grow: self.flex_grow_val,
            flex_shrink: self.flex_shrink_val,
            show_scrollbar: self.scrollbar_v.show || self.scrollbar_h.show,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
        }
    }

    /// 返回当前非负水平滚动位置。
    pub fn scroll_x(&self) -> f32 {
        self.effective_scroll_x()
    }

    /// 返回当前非负垂直滚动位置。
    pub fn scroll_y(&self) -> f32 {
        self.effective_scroll_y()
    }

    /// 设置非负水平滚动位置并写回已绑定的外部状态。
    pub fn set_scroll_x(&mut self, x: f32) {
        let old_x = self.effective_scroll_x();
        self.scroll_x = Self::normalize_axis(x);
        self.push_scroll_delta(self.effective_scroll_x() - old_x, 0.0);
        self.write_bound_offset();
    }

    /// 设置非负垂直滚动位置并写回已绑定的外部状态。
    pub fn set_scroll_y(&mut self, y: f32) {
        let old_y = self.effective_scroll_y();
        self.scroll_y = Self::normalize_axis(y);
        self.push_scroll_delta(0.0, self.effective_scroll_y() - old_y);
        self.write_bound_offset();
    }

    /// 同时设置滚动位置，并在已有布局范围内夹到内容末端。
    pub fn scroll_to_xy(&mut self, x: f32, y: f32) {
        let old_x = self.effective_scroll_x();
        let old_y = self.effective_scroll_y();
        self.scroll_x = Self::normalize_axis(x).min(self.max_scroll_x());
        self.scroll_y = Self::normalize_axis(y).min(self.max_scroll_y());
        self.push_scroll_delta(self.scroll_x - old_x, self.scroll_y - old_y);
        self.write_bound_offset();
    }

    /// 返回最近一次内容布局计算出的最大水平滚动位置。
    pub fn max_scroll_x(&self) -> f32 {
        // 只有完成至少一轮内容布局后才存在横向滚动范围。
        match self.content_bounds.get() {
            // 用完整滚动坐标范围减去外视口宽度。
            Some(cs) => {
                // 优先采用渲染记录的真实视口，否则回退到声明宽或默认宽。
                let view_w = self
                    .last_frame
                    .get()
                    .map(|f| f.w)
                    .unwrap_or(self.fixed_width.unwrap_or(self.visual.default_width));
                // 清除非有限值、无界哨兵与负范围。
                finite_non_negative(finite_non_negative(cs.w) - finite_non_negative(view_w))
            }
            // 尚无内容布局时保持零范围。
            None => 0.0,
        }
    }

    /// 返回最近一次内容布局计算出的最大垂直滚动位置。
    pub fn max_scroll_y(&self) -> f32 {
        // 只有完成至少一轮内容布局后才存在纵向滚动范围。
        match self.content_bounds.get() {
            // 用完整滚动坐标范围减去外视口高度。
            Some(cs) => {
                // 优先采用渲染记录的真实视口，否则回退到声明高或默认高。
                let view_h = self
                    .last_frame
                    .get()
                    .map(|f| f.h)
                    .unwrap_or(self.fixed_height.unwrap_or(self.visual.default_height));
                // 清除非有限值、无界哨兵与负范围。
                finite_non_negative(finite_non_negative(cs.h) - finite_non_negative(view_h))
            }
            // 尚无内容布局时保持零范围。
            None => 0.0,
        }
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= SCROLL_DELTA_EPSILON && dy.abs() <= SCROLL_DELTA_EPSILON {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn scroll_by(&mut self, dx: f32, dy: f32) -> bool {
        // 内容收缩后先从当前有效位置继续，不能让旧偏移保留空白尾部。
        let old_x = self.effective_scroll_x();
        let old_y = self.effective_scroll_y();
        self.scroll_x = (old_x + dx).clamp(0.0, self.max_scroll_x());
        self.scroll_y = (old_y + dy).clamp(0.0, self.max_scroll_y());
        let actual_dx = self.scroll_x - old_x;
        let actual_dy = self.scroll_y - old_y;
        self.push_scroll_delta(actual_dx, actual_dy);
        if actual_dx.abs() > SCROLL_DELTA_EPSILON || actual_dy.abs() > SCROLL_DELTA_EPSILON {
            self.write_bound_offset();
        }
        actual_dx.abs() > SCROLL_DELTA_EPSILON || actual_dy.abs() > SCROLL_DELTA_EPSILON
    }

    fn write_bound_offset(&self) {
        let Some(state) = self.scroll_binding.as_ref() else {
            return;
        };
        let offset = Point::new(self.effective_scroll_x(), self.effective_scroll_y());
        if state.get() != offset {
            state.set(offset);
        }
    }

    fn capture_bound_offset_dependency(&self) {
        if let Some(state) = self.scroll_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn clamp_bound_axis(&self, value: f32, horizontal: bool) -> f32 {
        let value = Self::normalize_axis(value);
        if self.content_bounds.get().is_some() {
            value.min(if horizontal {
                self.max_scroll_x()
            } else {
                self.max_scroll_y()
            })
        } else {
            value
        }
    }

    fn normalize_axis(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    // 读取已经按当前内容范围夹取的水平位置，避免内容缩短后暴露空白尾部。
    fn effective_scroll_x(&self) -> f32 {
        Self::normalize_axis(self.scroll_x).min(self.max_scroll_x())
    }

    // 读取已经按当前内容范围夹取的垂直位置，供绘制、命中与状态同步共用。
    fn effective_scroll_y(&self) -> f32 {
        Self::normalize_axis(self.scroll_y).min(self.max_scroll_y())
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new(SCROLL_VIEW_VISUAL_REF.default_direction)
    }
}

// 把 ScrollView Rust 滚动内核与 UIX 静态视口视觉组合为单一组件节点。
fn build_scroll_view_view(mut kernel: ScrollView, visual: &'static ScrollViewVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for ScrollView {
    fn build(self) -> ViewNode {
        build_scroll_view_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_scroll_view_uix_root(kernel: ScrollView) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/scroll_view/scroll_view.uix")
}

// 仅在测试构建中加载 ScrollView 的内部布局契约。
#[cfg(test)]
// 将测试放在独立文件中，避免主实现文件接近九百行上限。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/other/scroll_view/tests.rs"]
mod tests;
