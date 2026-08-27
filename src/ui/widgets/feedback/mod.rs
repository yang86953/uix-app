//! 反馈组件：弹层、提示与加载状态。

// 引入反馈组件共享的已解析绘制颜色值。
use crate::draw::Color;

// 按动画不透明度衰减主题 token 已有的 alpha。
fn fade_token_color(color: Color, opacity: f32) -> Color {
    // 将动画范围限制到有效不透明度区间。
    let opacity = opacity.clamp(0.0, 1.0);
    // 保留 token 自身基础 alpha，只缩放当前动画进度。
    let alpha = (f32::from(color.a) * opacity).round().clamp(0.0, 255.0) as u8;
    // 返回保持原 RGB 的动画颜色。
    color.with_alpha(alpha)
}

pub mod alert;
// 声明租约组件与关闭事实共享反馈 capability。
pub mod declaration;
/// 抽屉式窗口内浮层组件。
pub mod drawer;
pub mod message;
/// 模态对话框与焦点陷阱组件。
pub mod modal;
pub mod notification;
/// 由触发器拥有的气泡确认组件。
pub mod popconfirm;
/// 由触发器拥有的通用气泡内容组件。
pub mod popover;
pub mod progress;
pub mod spin;
pub(crate) mod toast_motion;
/// 提示文字浮层组件。
pub mod tooltip;

pub use alert::*;
pub use declaration::*;
pub use drawer::*;
pub use message::*;
pub use modal::*;
pub use notification::*;
pub use popconfirm::*;
pub use popover::*;
pub use progress::*;
pub use spin::*;
pub use tooltip::*;
