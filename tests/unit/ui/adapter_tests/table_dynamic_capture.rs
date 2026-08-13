// 向两个表格动态捕获子模块公开父级适配器测试入口。
use super::ViewAdapter;
// 挂载 DataTable 单元格动态私有 State 与生命周期回归。
#[path = "table_cell_dynamic_capture.rs"]
// 编译 DataTable 单元格 renderer 的树级动态捕获行为门禁。
mod table_cell_dynamic_capture;
// 挂载 Table 展开行动态私有 State 与生命周期回归。
#[path = "table_expand_dynamic_capture.rs"]
// 编译 Table expand renderer 的树级动态捕获行为门禁。
mod table_expand_dynamic_capture;
