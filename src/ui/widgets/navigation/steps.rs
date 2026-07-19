//! Steps widget — 步骤条，Ant Design 风格。
//!
//! 支持横向步骤条，步骤状态（wait/process/finish/error），
//! 自定义当前步骤，可点击切换。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;

const STEP_EXTENT: f32 = 80.0;

/// 步骤状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepStatus {
    Wait,
    Process,
    Finish,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepsDirection {
    Horizontal,
    Vertical,
}

/// 单个步骤定义。
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub title: String,
    pub description: String,
    pub status: StepStatus,
}

// Steps — 步骤条组件。
component! {
    pub struct Steps {
        steps: Vec<Step>,
        current: Cell<usize>,
        direction: StepsDirection,
        focused: bool,
        pending_change: Cell<Option<usize>>,
        /// 缓存 render 时的 frame 和 step_w，供 on_event 定位点击区域
        last_frame_and_step_w: Cell<Option<(Rect, f32)>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.steps.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(index) = self.step_at(pos.x, pos.y) {
                    self.select(index);
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let _success = ctx.tokens().color_success();
        let error = ctx.tokens().color_error();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill();
        let white = Color::white();
        let count = self.steps.len();
        if count == 0 { return; }

        if self.direction == StepsDirection::Horizontal {
            // 横向
            let step_w = (frame.w / count as f32).min(200.0);
            let total_w = step_w * count as f32;
            let start_x = frame.x + (frame.w - total_w) * 0.5;
            let circle_r = 14.0;
            let circle_y = frame.y + 28.0;

            // 缓存 frame 和 step_w 供 on_event 点击定位使用
            self.last_frame_and_step_w.set(Some((frame, step_w)));

            for (i, step) in self.steps.iter().enumerate() {
                let cx = start_x + i as f32 * step_w + step_w * 0.5;
                // 连接线（前）
                if i > 0 {
                    let line_x1 = start_x + (i - 1) as f32 * step_w + step_w * 0.5 + circle_r;
                    let line_x2 = cx - circle_r;
                    let line_y = circle_y;
                    let line_c = if i <= self.current.get() { primary } else { fill };
                    ctx.fill_rect(Rect::new(line_x1, line_y - 1.0, line_x2 - line_x1, 2.0), line_c, None);
                }
                // 圆圈
                let (bg_c, text_c, border_c) = match step.status {
                    StepStatus::Finish => (primary, white, primary),
                    StepStatus::Process => (primary, white, primary),
                    StepStatus::Error => (error, white, error),
                    StepStatus::Wait => (Color::transparent(), text_sec, fill),
                };
                if bg_c.a > 0 {
                    ctx.fill_circle(cx, circle_y, circle_r, bg_c);
                }
                ctx.canvas_2d().stroke_circle(cx, circle_y, circle_r, border_c, 2.0);
                // 步骤编号/图标（在圆圈内居中）
                let circle_rect = Rect::new(cx - circle_r, circle_y - circle_r, circle_r * 2.0, circle_r * 2.0);
                if step.status == StepStatus::Finish {
                    crate::ui::widgets::icon::paint_icon_in_frame(
                        ctx,
                        "check",
                        circle_rect,
                        text_c,
                        14.0,
                    );
                } else {
                    ctx.text_center(&(i + 1).to_string(), circle_rect, text_c, 14.0);
                }
                // 标题（在圆圈下方居中）
                let title_c = if i <= self.current.get() { text } else { text_sec };
                let title_rect = Rect::new(cx - step_w * 0.5, circle_y + circle_r + 4.0, step_w, 20.0);
                ctx.text_center(&step.title, title_rect, title_c, 13.0);
                if !step.description.is_empty() {
                    let description_rect = Rect::new(
                        cx - step_w * 0.5,
                        circle_y + circle_r + 24.0,
                        step_w,
                        18.0,
                    );
                    ctx.text_center(&step.description, description_rect, text_sec, 11.0);
                }
            }
        } else {
            self.last_frame_and_step_w.set(None);
            let circle_r = 14.0;
            let circle_x = frame.x + 28.0;
            for (i, step) in self.steps.iter().enumerate() {
                let cy = frame.y + i as f32 * STEP_EXTENT + 28.0;
                if i > 0 {
                    let line_y1 = frame.y + (i - 1) as f32 * STEP_EXTENT + 28.0 + circle_r;
                    let line_y2 = cy - circle_r;
                    let line_c = if i <= self.current.get() { primary } else { fill };
                    ctx.fill_rect(
                        Rect::new(circle_x - 1.0, line_y1, 2.0, line_y2 - line_y1),
                        line_c,
                        None,
                    );
                }
                let (bg_c, text_c, border_c) = match step.status {
                    StepStatus::Finish | StepStatus::Process => (primary, white, primary),
                    StepStatus::Error => (error, white, error),
                    StepStatus::Wait => (Color::transparent(), text_sec, fill),
                };
                if bg_c.a > 0 {
                    ctx.fill_circle(circle_x, cy, circle_r, bg_c);
                }
                ctx.canvas_2d()
                    .stroke_circle(circle_x, cy, circle_r, border_c, 2.0);
                let circle_rect = Rect::new(
                    circle_x - circle_r,
                    cy - circle_r,
                    circle_r * 2.0,
                    circle_r * 2.0,
                );
                if step.status == StepStatus::Finish {
                    crate::ui::widgets::icon::paint_icon_in_frame(
                        ctx,
                        "check",
                        circle_rect,
                        text_c,
                        14.0,
                    );
                } else {
                    ctx.text_center(&(i + 1).to_string(), circle_rect, text_c, 14.0);
                }
                let title_color = if i <= self.current.get() { text } else { text_sec };
                ctx.draw_text_in_frame(
                    &step.title,
                    Rect::new(circle_x + 24.0, cy - 20.0, frame.w - 60.0, 24.0),
                    title_color,
                    13.0,
                );
                if !step.description.is_empty() {
                    ctx.draw_text_in_frame(
                        &step.description,
                        Rect::new(circle_x + 24.0, cy + 4.0, frame.w - 60.0, 22.0),
                        text_sec,
                        11.0,
                    );
                }
            }
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                primary,
                1.5,
                Some(crate::draw::Radius::uniform(
                    ctx.tokens().border_radius_sm(),
                )),
            );
        }
    }
}

impl Steps {
    fn intrinsic_size(&self) -> Size {
        if self.direction == StepsDirection::Horizontal {
            Size::new(600.0, 96.0)
        } else {
            Size::new(200.0, self.steps.len() as f32 * STEP_EXTENT)
        }
    }

    pub fn new(steps: Vec<Step>) -> Self {
        let current = Cell::new(0);
        Self {
            steps,
            current,
            direction: StepsDirection::Horizontal,
            focused: false,
            pending_change: Cell::new(None),
            last_frame_and_step_w: Cell::new(None),
        }
    }
    pub fn current(self, v: usize) -> Self {
        self.current.set(self.clamp_index(v));
        self
    }
    pub fn get_current(&self) -> usize {
        self.current.get()
    }
    pub fn set_current(&self, v: usize) {
        self.current.set(self.clamp_index(v));
    }
    pub fn horizontal(mut self) -> Self {
        self.direction = StepsDirection::Horizontal;
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = StepsDirection::Vertical;
        self
    }
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.steps = next.steps;
        self.direction = next.direction;
        self.current.set(self.clamp_index(self.current.get()));
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
            self.current.set(index);
            self.pending_change.set(Some(index));
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
                let index = (y / STEP_EXTENT) as usize;
                (index < self.steps.len()).then_some(index)
            }
        }
    }
}

impl Step {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            description: String::new(),
            status: StepStatus::Wait,
        }
    }
    pub fn description(mut self, d: &str) -> Self {
        self.description = d.to_string();
        self
    }
    pub fn status(mut self, s: StepStatus) -> Self {
        self.status = s;
        self
    }
}
