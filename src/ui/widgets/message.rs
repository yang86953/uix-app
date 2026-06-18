//! Message widget — 轻量级全局消息提示（顶部飘入，自动消失）。
//!
//! 与 Alert 不同，Message 浮动在所有内容之上，支持 success/info/warning/error
//! 四类状态，可配置持续时长，支持手动关闭。通过静态队列管理多消息叠加。

use crate::base::{Rect, Size};
use crate::define_widget;
use crate::graphics::Color;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// 消息类型。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    Success,
    Info,
    Warning,
    Error,
}

/// 单条消息数据。
#[derive(Debug, Clone)]
pub struct MessageItem {
    pub type_: MessageType,
    pub content: String,
    pub duration_ms: u64,  // 0 = 不自动消失
    pub closable: bool,
}

/// Message — 全局消息提示容器。
///
/// 作为 WidgetTree 根级别的浮动层注册，通过静态队列接收消息。
/// 每条消息显示为顶部居中的横幅，自动排列避免重叠。
define_widget! {
    /// Message 容器 — 持有消息队列，在 post_render 中绘制浮层。
    pub struct Message {
        queue: Rc<RefCell<Vec<MessageItem>>>,
        /// 每条消息的剩余毫秒数（用于自动消失）
        remaining: Rc<RefCell<Vec<u64>>>,
        msg_height: Cell<f32>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        // Message 不占用布局空间，全在 post_render 中绘制
        Size::zero()
    }

    on_update => (&mut self, dt: f32) {
        let dt_ms = (dt * 1000.0) as u64;
        let mut changed = false;
        let mut i = 0;
        let len = self.queue.borrow().len();
        while i < len {
            let should_remove = {
                let mut remain = self.remaining.borrow_mut();
                let mut queue = self.queue.borrow_mut();
                if i >= queue.len() || i >= remain.len() { break; }
                if queue[i].duration_ms > 0 {
                    remain[i] = remain[i].saturating_sub(dt_ms);
                    if remain[i] == 0 {
                        queue.remove(i);
                        remain.remove(i);
                        changed = true;
                        continue;
                    }
                }
                false
            };
            if should_remove { continue; }
            i += 1;
        }
        if changed { self.msg_height.set(self.queue.borrow().len() as f32 * 48.0); }
    }

    needs_continuous_update => (&self) -> bool {
        !self.queue.borrow().is_empty()
    }

    // 消息在 post_render 中作为浮层绘制，不影响布局
    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {}

    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let queue = self.queue.borrow();
        if queue.is_empty() { return; }
        let cw = frame.w;
        let msg_w = (380.0f32).min(cw - 40.0);
        let start_x = (cw - msg_w) * 0.5;
        let mut y = 12.0;
        let radius = Some(crate::graphics::Radius::uniform(ctx.tokens().border_radius_lg()));
        let shadow = ctx.tokens().box_shadow();
        let bg = ctx.tokens().color_bg_elevated();
        let text_c = ctx.tokens().color_text();

        for item in queue.iter() {
            let (icon, accent) = match item.type_ {
                MessageType::Success => ("✓", ctx.tokens().color_success()),
                MessageType::Info    => ("ℹ", ctx.tokens().color_info()),
                MessageType::Warning => ("⚠", ctx.tokens().color_warning()),
                MessageType::Error   => ("✗", ctx.tokens().color_error()),
            };
            let msg_rect = Rect::new(start_x, y, msg_w, 40.0);
            // 阴影（简化：纯色半透明底边）
            if shadow.layer_1.2 > 0.0 {
                ctx.draw_box_shadow(msg_rect, shadow.layer_1.2, shadow.layer_1.0, shadow.layer_1.1, shadow.layer_1.3, radius);
            }
            ctx.fill_rect(msg_rect, bg, radius);
            // 左侧强调色条
            ctx.fill_rect(Rect::new(start_x, y + 4.0, 3.0, 32.0), accent, Some(crate::graphics::Radius::uniform(1.5)));
            ctx.draw_text(icon, crate::base::Point::new(start_x + 14.0, y + 11.0), accent, 14.0);
            // 修复：存储 text_x 避免冲突
            let text_x = start_x + 36.0;
            ctx.draw_text(&item.content, crate::base::Point::new(text_x, y + 12.0), text_c, 13.0);
            if item.closable {
                ctx.draw_text("✕", crate::base::Point::new(start_x + msg_w - 22.0, y + 12.0), ctx.tokens().color_text_quaternary(), 12.0);
            }
            y += 48.0;
        }
    }
}

impl Message {
    /// 创建新的 Message 容器（通常在根 WidgetTree 中注册一个）。
    pub fn new() -> Self {
        Self {
            queue: Rc::new(RefCell::new(Vec::new())),
            remaining: Rc::new(RefCell::new(Vec::new())),
            msg_height: Cell::new(0.0),
        }
    }

    /// 添加一条消息到队列。
    pub fn add(&self, item: MessageItem) {
        let mut q = self.queue.borrow_mut();
        q.push(item);
        self.remaining.borrow_mut().push(q.last().map(|i| i.duration_ms).unwrap_or(3000));
        self.msg_height.set(q.len() as f32 * 48.0);
    }

    /// 便捷方法：成功消息。
    pub fn success(&self, content: &str) { self.add(MessageItem { type_: MessageType::Success, content: content.into(), duration_ms: 3000, closable: true }); }
    /// 便捷方法：信息消息。
    pub fn info(&self, content: &str) { self.add(MessageItem { type_: MessageType::Info, content: content.into(), duration_ms: 3000, closable: true }); }
    /// 便捷方法：警告消息。
    pub fn warning(&self, content: &str) { self.add(MessageItem { type_: MessageType::Warning, content: content.into(), duration_ms: 4000, closable: true }); }
    /// 便捷方法：错误消息。
    pub fn error(&self, content: &str) { self.add(MessageItem { type_: MessageType::Error, content: content.into(), duration_ms: 5000, closable: true }); }
    /// 获取队列引用（供外部管理）。
    pub fn queue(&self) -> Rc<RefCell<Vec<MessageItem>>> { self.queue.clone() }
}
