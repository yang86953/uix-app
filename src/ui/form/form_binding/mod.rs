//! `FormModel` 与声明式输入 View 的绑定。

mod choice;
mod input;
mod picker;
mod select;

pub use choice::*;
pub use input::*;
pub use picker::*;
pub use select::*;

/// 断言视图绑定契约成立：绑定缺失属 API 误用，给出带明确提示的
/// 可诊断 panic（开发者契约错误，非运行时失败；生产路径不传播）。
fn bound_required<T>(value: Option<T>, contract: &str) -> T {
    match value {
        Some(v) => v,
        None => panic!("{contract}"),
    }
}
