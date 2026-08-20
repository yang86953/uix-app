//! 导航组件：菜单、标签页、面包屑与分页。

pub mod anchor;
pub mod breadcrumb;
pub mod dropdown;
pub mod menu;
pub mod nav;
// Navigation 组合组件的侧栏外壳实现。
pub mod navigation_shell;
pub mod pagination;
pub mod steps;
pub mod tabs;

// 集中验证 Breadcrumb 稳定 link、reconcile 与语义事件契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/navigation/breadcrumb_tests.rs"]
mod breadcrumb_tests;
// 集中验证 Menu typed key、受控状态与重复身份门禁。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/navigation/menu_tests.rs"]
mod menu_tests;
// 集中验证 Tabs 面板门控、状态保留与焦点迁移契约。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/navigation/tabs_tests.rs"]
mod tabs_tests;
// 集中验证 Navigation 外壳与受控 Menu 的唯一所有权边界。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/navigation/navigation_shell_tests.rs"]
mod navigation_shell_tests;

pub use anchor::*;
pub use breadcrumb::*;
pub use dropdown::*;
pub use menu::*;
pub use nav::*;
pub use navigation_shell::*;
pub use pagination::*;
pub use steps::*;
pub use tabs::*;
