// 向既有 Select 子模块保留原测试父级提供的适配器入口。
use super::ViewAdapter;

// 复用既有 Select 动态选项的完整行为门禁。
#[path = "select_dynamic_capture.rs"]
// 编译 Select option renderer 的树级动态捕获回归。
mod select_dynamic_capture;
// 挂载 Transfer 动态条目的所有权与生命周期门禁。
#[path = "transfer_dynamic_capture.rs"]
// 编译 Transfer item renderer 的树级动态捕获回归。
mod transfer_dynamic_capture;
