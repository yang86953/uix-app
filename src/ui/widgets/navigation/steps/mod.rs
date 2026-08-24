//! Steps widget — 步骤条，Ant Design 风格。
//!
//! 支持横向步骤条，步骤状态（wait/process/finish/error），
//! 自定义当前步骤，可点击切换。

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::widget;
// 引入受控 current 的响应式状态句柄。
use crate::ui::State;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, View, ViewNode,
    WidgetId, WidgetTree,
};
use std::cell::Cell;
use std::rc::Rc;

mod presentation;
use presentation::*;

/// 步骤状态。
///
/// 与 ValidateStatus/InputStatus/BadgeStatus/UploadStatus 共享「组件状态」命名模式，
/// 但各自语义与变体独立（本枚举是向导流程阶段态），勿强行合并。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepStatus {
    /// 步骤尚未开始。
    Wait,
    /// 步骤当前正在处理。
    Process,
    /// 步骤已经成功完成。
    Finish,
    /// 步骤以错误状态结束或需要修正。
    Error,
}

/// 步骤条排列步骤与连接线的方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepsDirection {
    /// 从左向右水平排列步骤。
    Horizontal,
    /// 从上向下垂直排列步骤。
    Vertical,
}

/// 单个步骤定义。
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// 步骤的主要标题。
    pub title: String,
    /// 显示在标题旁或下方的补充说明。
    pub description: String,
    /// 步骤当前的流程状态。
    pub status: StepStatus,
    /// 显式状态覆盖；未设置时由 Steps.current 推导流程状态。
    status_override: Option<StepStatus>,
    /// 替代默认状态标记的可选 Lucide 图标名称。
    pub icon: String,
}

// Steps — 步骤条组件。
widget! {
    /// 按声明顺序展示步骤并通过可选受控索引提交进度变化的组件。
    pub struct Steps {
        steps: Vec<Step>,
        current: Cell<usize>,
        // 保存声明端 current 的唯一受控状态来源。
        current_binding: Option<State<usize>>,
        direction: StepsDirection,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        clickable: bool,
        dot: bool,
        step_callback: Option<Rc<dyn Fn(usize, &Step)>>,
        /// 缓存 render 时的 frame 和 step_w，供 on_event 定位点击区域
        last_frame_and_step_w: Cell<Option<(Rect, f32)>>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static StepsVisual,
    }

    tab_index => (&self) -> i32 { i32::from(!self.steps.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 每次处理输入前吸收声明端可能发生的 current 更新。
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(index) = self.step_at(pos.x, pos.y) {
                    if self.clickable {
                        self.select(index);
                    }
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
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match (self.direction, key) {
                (StepsDirection::Horizontal, KeyCode::Right)
                | (StepsDirection::Vertical, KeyCode::Down) => {
                    self.move_current(true);
                    EventResult::Handled
                }
                (StepsDirection::Horizontal, KeyCode::Left)
                | (StepsDirection::Vertical, KeyCode::Up) => {
                    self.move_current(false);
                    EventResult::Handled
                }
                (_, KeyCode::Home) => {
                    self.select(0);
                    EventResult::Handled
                }
                (_, KeyCode::End) if !self.steps.is_empty() => {
                    self.select(self.steps.len() - 1);
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
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        // 捕获受控 State 依赖，使外部更新触发声明树重建。
        self.capture_bound_value_dependency();
        // 横向与纵向步骤同帧共享一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let layout = &self.visual.layout;
        let typography = &self.visual.typography;
        let primary = visual.primary;
        let error = visual.error;
        let text = visual.text;
        let text_sec = visual.text_secondary;
        let fill = visual.fill;
        // 步骤圆点反白色：白色 token。
        let white = visual.white;
        let count = self.steps.len();
        if count == 0 { return; }

        if self.direction == StepsDirection::Horizontal {
            // 横向
            let step_w = (frame.w / count as f32).min(layout.horizontal_step_max_width);
            let total_w = step_w * count as f32;
            let start_x = frame.x + (frame.w - total_w) * 0.5;
            let circle_r = layout.circle_radius;
            let circle_y = frame.y + layout.center_offset;

            // 缓存 frame 和 step_w 供 on_event 点击定位使用
            self.last_frame_and_step_w.set(Some((frame, step_w)));

            let current = self.current.get();
            for (i, step) in self.steps.iter().enumerate() {
                // 未显式指定的步骤状态随 current 自动推进。
                let status = step.resolved_status(i, current);
                let cx = start_x + i as f32 * step_w + step_w * 0.5;
                // 连接线（前）
                if i > 0 {
                    let line_x1 = start_x + (i - 1) as f32 * step_w + step_w * 0.5 + circle_r;
                    let line_x2 = cx - circle_r;
                    let line_y = circle_y;
                    let line_c = if i <= current { primary } else { fill };
                    ctx.fill_rect(
                        Rect::new(
                            line_x1,
                            line_y - layout.line_half_width,
                            line_x2 - line_x1,
                            layout.line_thickness,
                        ),
                        line_c,
                        None,
                    );
                }
                // 圆圈
                let (bg_c, text_c, border_c) = match status {
                    StepStatus::Finish => (primary, white, primary),
                    StepStatus::Process => (primary, white, primary),
                    StepStatus::Error => (error, white, error),
                    StepStatus::Wait => (Color::transparent(), text_sec, fill),
                };
                if bg_c.a > 0 {
                    ctx.fill_circle(cx, circle_y, circle_r, bg_c);
                }
                ctx.stroke_circle(
                    cx,
                    circle_y,
                    circle_r,
                    border_c,
                    self.visual.chrome.marker_border_width,
                );
                // 步骤编号/图标（在圆圈内居中）
                let circle_rect = Rect::new(cx - circle_r, circle_y - circle_r, circle_r * 2.0, circle_r * 2.0);
                // 步骤图标/编号字号：统一使用主题 font_size token。
                if self.dot {
                    ctx.fill_circle(cx, circle_y, circle_r * layout.dot_radius_factor, text_c);
                } else if !step.icon.is_empty() {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        &step.icon,
                        circle_rect,
                        text_c,
                        visual.marker_font_size,
                    );
                } else if status == StepStatus::Finish {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        self.visual.icons.finish,
                        circle_rect,
                        text_c,
                        visual.marker_font_size,
                    );
                } else {
                    ctx.text_center(
                        &(i + 1).to_string(),
                        circle_rect,
                        text_c,
                        visual.marker_font_size,
                    );
                }
                // 标题（在圆圈下方居中）
                let title_c = if i <= current { text } else { text_sec };
                let title_rect = Rect::new(
                    cx - step_w * 0.5,
                    circle_y + circle_r + layout.title_gap,
                    step_w,
                    layout.title_height,
                );
                ctx.text_center(&step.title, title_rect, title_c, typography.title);
                if !step.description.is_empty() {
                    let description_rect = Rect::new(
                        cx - step_w * 0.5,
                        circle_y + circle_r + layout.description_offset,
                        step_w,
                        layout.description_height,
                    );
                    ctx.text_center(
                        &step.description,
                        description_rect,
                        text_sec,
                        typography.description,
                    );
                }
            }
        } else {
            self.last_frame_and_step_w.set(None);
            let circle_r = layout.circle_radius;
            let circle_x = frame.x + layout.center_offset;
            let current = self.current.get();
            for (i, step) in self.steps.iter().enumerate() {
                // 纵向与横向共享同一状态推导规则。
                let status = step.resolved_status(i, current);
                let cy = frame.y + i as f32 * layout.step_extent + layout.center_offset;
                if i > 0 {
                    let line_y1 = frame.y
                        + (i - 1) as f32 * layout.step_extent
                        + layout.center_offset
                        + circle_r;
                    let line_y2 = cy - circle_r;
                    let line_c = if i <= current { primary } else { fill };
                    ctx.fill_rect(
                        Rect::new(
                            circle_x - layout.line_half_width,
                            line_y1,
                            layout.line_thickness,
                            line_y2 - line_y1,
                        ),
                        line_c,
                        None,
                    );
                }
                let (bg_c, text_c, border_c) = match status {
                    StepStatus::Finish | StepStatus::Process => (primary, white, primary),
                    StepStatus::Error => (error, white, error),
                    StepStatus::Wait => (Color::transparent(), text_sec, fill),
                };
                if bg_c.a > 0 {
                    ctx.fill_circle(circle_x, cy, circle_r, bg_c);
                }
                ctx.stroke_circle(
                    circle_x,
                    cy,
                    circle_r,
                    border_c,
                    self.visual.chrome.marker_border_width,
                );
                let circle_rect = Rect::new(
                    circle_x - circle_r,
                    cy - circle_r,
                    circle_r * 2.0,
                    circle_r * 2.0,
                );
                if self.dot {
                    ctx.fill_circle(
                        circle_x,
                        cy,
                        circle_r * layout.dot_radius_factor,
                        text_c,
                    );
                } else if !step.icon.is_empty() {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        &step.icon,
                        circle_rect,
                        text_c,
                        visual.marker_font_size,
                    );
                } else if status == StepStatus::Finish {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        self.visual.icons.finish,
                        circle_rect,
                        text_c,
                        visual.marker_font_size,
                    );
                } else {
                    ctx.text_center(
                        &(i + 1).to_string(),
                        circle_rect,
                        text_c,
                        visual.marker_font_size,
                    );
                }
                let title_color = if i <= current { text } else { text_sec };
                ctx.draw_text_in_frame(
                    &step.title,
                    Rect::new(
                        circle_x + layout.vertical_text_start,
                        cy + layout.vertical_title_offset,
                        frame.w - layout.vertical_text_end_reserve,
                        layout.vertical_title_height,
                    ),
                    title_color,
                    typography.title,
                );
                if !step.description.is_empty() {
                    ctx.draw_text_in_frame(
                        &step.description,
                        Rect::new(
                            circle_x + layout.vertical_text_start,
                            cy + layout.vertical_description_offset,
                            frame.w - layout.vertical_text_end_reserve,
                            layout.vertical_description_height,
                        ),
                        text_sec,
                        typography.description,
                    );
                }
            }
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                primary,
                self.visual.chrome.focus_width,
                Some(crate::draw::Radius::uniform(visual.radius)),
            );
        }
    }
}

impl Steps {
    fn intrinsic_size(&self) -> Size {
        if self.direction == StepsDirection::Horizontal {
            Size::new(
                self.visual.layout.horizontal_width,
                self.visual.layout.horizontal_height,
            )
        } else {
            Size::new(
                self.visual.layout.vertical_width,
                self.steps.len() as f32 * self.visual.layout.step_extent,
            )
        }
    }

    /// 使用步骤列表创建默认水平、可点击的步骤条。
    pub fn new(steps: Vec<Step>) -> Self {
        let current = Cell::new(0);
        let visual = STEPS_VISUAL_REF;
        Self {
            steps,
            current,
            // 默认构造保持非受控模式。
            current_binding: None,
            direction: visual.defaults.direction,
            focused: false,
            pending_change: Cell::new(None),
            clickable: visual.defaults.clickable,
            dot: visual.defaults.dot,
            step_callback: None,
            last_frame_and_step_w: Cell::new(None),
            visual,
        }
    }
    /// 设置非受控模式的初始步骤索引，并夹取到有效范围。
    pub fn current(self, v: usize) -> Self {
        // 显式数值构造切回非受控初始值模式。
        let mut this = self;
        // 清除可能存在的声明端状态句柄。
        this.current_binding = None;
        // 按当前步骤集合归一化初始索引。
        this.current.set(this.clamp_index(v));
        // 返回完成配置的组件。
        this
    }
    /// 将当前步骤双向绑定到声明端 `State<usize>`。
    pub fn current_state(mut self, state: &State<usize>) -> Self {
        // 克隆轻量状态句柄，保持声明端为唯一事实源。
        self.current_binding = Some(state.clone());
        // 立即吸收并归一化当前外部索引。
        self.sync_bound_value();
        // 返回受控组件。
        self
    }
    /// 返回组件当前采用的已归一化步骤索引。
    pub fn get_current(&self) -> usize {
        // 读取组件本地镜像；外部 State 更新会通过 reconcile 或事件入口同步。
        self.current.get()
    }
    /// 命令式更新当前步骤并写回绑定状态，但不伪造用户变更事件。
    pub fn set_current(&self, v: usize) {
        // 把请求索引限制在当前步骤集合中。
        let current = self.clamp_index(v);
        // 更新组件本地镜像。
        self.current.set(current);
        // 受控模式同时写回声明端状态。
        self.write_bound_value(current);
    }
    /// 将步骤与连接线切换为水平排列。
    pub fn horizontal(mut self) -> Self {
        self.direction = StepsDirection::Horizontal;
        self
    }
    /// 将步骤与连接线切换为垂直排列。
    pub fn vertical(mut self) -> Self {
        self.direction = StepsDirection::Vertical;
        self
    }
    /// 返回步骤条中声明的步骤数量。
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        // 受控声明携带已经从 State 归一化出的 current 镜像。
        let controlled_current = next.current_binding.as_ref().map(|_| next.current.get());
        // 替换步骤数据后再计算有效索引范围。
        self.steps = next.steps;
        // 采用新声明持有的 State 句柄。
        self.current_binding = next.current_binding;
        self.direction = next.direction;
        self.clickable = next.clickable;
        self.dot = next.dot;
        self.step_callback = next.step_callback;
        // 同步 UIX 生成的视觉表引用，不保留 Rust 视觉副本。
        self.visual = next.visual;
        // 受控模式服从外部 State；非受控模式保留原交互状态。
        let current = controlled_current.unwrap_or_else(|| self.current.get());
        // 在新步骤集合下归一化 current 镜像。
        self.current.set(self.clamp_index(current));
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Steps {
            steps: self.steps.clone(),
            current: self.current.get(),
            direction: self.direction,
        }
    }

    fn clamp_index(&self, index: usize) -> usize {
        index.min(self.steps.len().saturating_sub(1))
    }

    fn select(&self, index: usize) {
        if self.steps.is_empty() {
            return;
        }
        let index = self.clamp_index(index);
        if index != self.current.get() {
            // 先更新组件镜像，供同一输入周期的绘制和快照消费。
            self.current.set(index);
            // 再写回唯一的受控状态所有者。
            self.write_bound_value(index);
            self.pending_change.set(Some(index));
            if let Some(callback) = self.step_callback.as_ref() {
                if let Some(step) = self.steps.get(index) {
                    callback(index, step);
                }
            }
        }
    }

    // 从声明端 State 吸收 current，并把越界值归一化回同一状态源。
    fn sync_bound_value(&mut self) {
        // 非受控模式不执行状态同步。
        let Some(state) = self.current_binding.as_ref() else {
            // 直接返回，保留组件自身交互状态。
            return;
        };
        // 读取声明端最新索引。
        let requested = state.get();
        // 按当前步骤数量得到有效索引。
        let current = self.clamp_index(requested);
        // 更新运行时镜像。
        self.current.set(current);
        // 越界输入统一回写，避免状态源与组件长期分叉。
        if requested != current {
            // 写回归一化值。
            state.set(current);
        }
    }

    // 捕获声明端 current 的响应式依赖。
    fn capture_bound_value_dependency(&self) {
        // 仅受控组件需要登记 State 依赖。
        if let Some(state) = self.current_binding.as_ref() {
            // 读取值以接入当前追踪上下文。
            let _ = state.get();
        }
    }

    // 把组件 current 写回受控 State。
    fn write_bound_value(&self, current: usize) {
        // 非受控模式没有外部写回目标。
        let Some(state) = self.current_binding.as_ref() else {
            // 直接返回，保持原有非受控语义。
            return;
        };
        // 避免相同值产生多余 generation 与 reconcile。
        if state.get() != current {
            // 提交新的受控 current。
            state.set(current);
        }
    }

    fn move_current(&self, forward: bool) {
        if self.steps.is_empty() {
            return;
        }
        let current = self.current.get();
        let next = if forward {
            (current + 1).min(self.steps.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.select(next);
    }

    fn step_at(&self, x: f32, y: f32) -> Option<usize> {
        if self.steps.is_empty() || x < 0.0 || y < 0.0 {
            return None;
        }
        match self.direction {
            StepsDirection::Horizontal => {
                let (frame, step_width) = self.last_frame_and_step_w.get()?;
                if y > frame.h {
                    return None;
                }
                let total_width = step_width * self.steps.len() as f32;
                let start_x = (frame.w - total_width) * 0.5;
                let relative_x = x - start_x;
                if relative_x < 0.0 || relative_x >= total_width {
                    return None;
                }
                Some((relative_x / step_width) as usize)
            }
            StepsDirection::Vertical => {
                let index = (y / self.visual.layout.step_extent) as usize;
                (index < self.steps.len()).then_some(index)
            }
        }
    }
}

// 受控 current 的运行时所有权与 reconcile 回归。
#[cfg(test)]
// 声明 Steps 私有测试模块。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/navigation/steps__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;

impl Step {
    /// 使用标题创建由 Steps.current 自动决定状态且无描述和图标的步骤。
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            description: String::new(),
            status: StepStatus::Wait,
            status_override: None,
            icon: String::new(),
        }
    }
    /// 设置步骤的补充说明。
    pub fn description(mut self, d: &str) -> Self {
        self.description = d.to_string();
        self
    }
    /// 设置步骤当前的流程状态。
    pub fn status(mut self, s: StepStatus) -> Self {
        self.status = s;
        self.status_override = Some(s);
        self
    }

    /// 解析当前索引下的最终流程状态，显式配置优先于自动进度。
    fn resolved_status(&self, index: usize, current: usize) -> StepStatus {
        // 兼容通过公开字段直接设置的非 Wait 状态。
        if let Some(status) = self
            .status_override
            .or_else(|| (self.status != StepStatus::Wait).then_some(self.status))
        {
            return status;
        }
        if index < current {
            StepStatus::Finish
        } else if index == current {
            StepStatus::Process
        } else {
            StepStatus::Wait
        }
    }

    /// 设置替代默认状态标记的 Lucide 图标名称。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

impl Steps {
    /// 设置用户是否可以点击或按键切换步骤。
    pub fn clickable(mut self, clickable: bool) -> Self {
        self.clickable = clickable;
        self
    }

    /// 设置是否用圆点替代编号或状态图标。
    pub fn dot(mut self, dot: bool) -> Self {
        self.dot = dot;
        self
    }

    /// 注册用户切换步骤时接收目标索引与步骤定义的回调。
    pub fn on_step<F>(mut self, callback: F) -> Self
    where
        F: Fn(usize, &Step) + 'static,
    {
        self.step_callback = Some(Rc::new(callback));
        self
    }
}

// UIX 只注入静态视觉表，Rust 内核继续拥有 current 状态、状态推导和输入。
fn build_steps_view(mut kernel: Steps, visual: &'static StepsVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Steps {
    fn build(self) -> ViewNode {
        build_steps_view(self, STEPS_VISUAL_REF)
    }
}
