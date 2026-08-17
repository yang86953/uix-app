use super::tree_core::WidgetTree;
use super::*;
use crate::ui::event::{ClickEvent, SemanticEvent, WindowAction};

mod events;
// 查询当前指针路由目标的可继承平台光标。
mod cursor;
mod keyboard_routing;
mod pointer;
mod pointer_routing;
mod window_lifecycle;

fn reveal_axis_delta(target_start: f32, target_end: f32, view_start: f32, view_end: f32) -> f32 {
    if target_start < view_start {
        target_start - view_start
    } else if target_end > view_end {
        target_end - view_end
    } else {
        0.0
    }
}
