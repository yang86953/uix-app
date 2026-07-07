//! Steps widget — 步骤条，Ant Design 风格。
//!
//! 支持横向步骤条，步骤状态（wait/process/finish/error），
//! 自定义当前步骤，可点击切换。

use crate::core::{Constraints, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SemanticEvent, SystemEvent, WidgetId, WidgetTree};
use std::cell::Cell;

/// 步骤状态。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepStatus {
    Wait,
    Process,
    Finish,
    Error,
}

/// 单个步骤定义。
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub title: String,
    pub description: String,
    pub status: StepStatus,
}

// Steps — 步骤条组件。
define_widget! {
    pub struct Steps {
        steps: Vec<Step>,
        current: Cell<usize>,
        direction: bool, // true=horizontal, false=vertical
        pending_change: Cell<Option<usize>>,
        /// 缓存 render 时的 frame 和 step_w，供 on_event 定位点击区域
        last_frame_and_step_w: Cell<Option<(Rect, f32)>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            if self.direction && !self.steps.is_empty() {
                // 使用与 render 相同的 step_w 计算，缓存由 render 设置
                let count = self.steps.len();
                if let Some((frame, step_w)) = self.last_frame_and_step_w.get() {
                    let total_w = step_w * count as f32;
                    let start_x = (frame.w - total_w) * 0.5;
                    let rel_x = pos.x - start_x;
                    if rel_x >= 0.0 {
                        let idx = (rel_x / step_w) as usize;
                        if idx < count {
                            if self.current.get() != idx {
                                self.current.set(idx);
                                self.pending_change.set(Some(idx));
                            }
                            return EventResult::Handled;
                        }
                    }
                }
            }
        }
        EventResult::NotHandled
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|idx| SemanticEvent::change(id, idx.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let _success = ctx.tokens().color_success();
        let error = ctx.tokens().color_error();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill();
        let white = Color::white();
        let count = self.steps.len();
        if count == 0 { return; }

        if self.direction {
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
                let num = if step.status == StepStatus::Finish { "✓" } else { &(i + 1).to_string() };
                let circle_rect = Rect::new(cx - circle_r, circle_y - circle_r, circle_r * 2.0, circle_r * 2.0);
                ctx.text_center(num, circle_rect, text_c, 14.0);
                // 标题（在圆圈下方居中）
                let title_c = if i <= self.current.get() { text } else { text_sec };
                let title_rect = Rect::new(cx - step_w * 0.5, circle_y + circle_r + 4.0, step_w, 20.0);
                ctx.text_center(&step.title, title_rect, title_c, 13.0);
            }
        }
    }
}

impl Steps {
    fn intrinsic_size(&self) -> Size {
        if self.direction {
            Size::new(600.0, 80.0)
        } else {
            Size::new(200.0, self.steps.len() as f32 * 80.0)
        }
    }

    pub fn new(steps: Vec<Step>) -> Self {
        let current = Cell::new(0);
        Self {
            steps,
            current,
            direction: true,
            pending_change: Cell::new(None),
            last_frame_and_step_w: Cell::new(None),
        }
    }
    pub fn current(self, v: usize) -> Self {
        self.current.set(v);
        self
    }
    pub fn get_current(&self) -> usize {
        self.current.get()
    }
    pub fn set_current(&self, v: usize) {
        self.current.set(v);
    }
    pub fn horizontal(mut self) -> Self {
        self.direction = true;
        self
    }
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Steps {
            steps: self.steps.clone(),
            direction: self.direction,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_steps_size() {
        let measured = Steps::new(vec![Step::new("One"), Step::new("Two")])
            .measure(Constraints::loose(Size::new(120.0, 40.0)));

        assert_eq!(measured, Size::new(120.0, 40.0));
    }
}
