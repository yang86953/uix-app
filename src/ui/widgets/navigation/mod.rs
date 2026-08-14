//! 导航组件：菜单、标签页、面包屑与分页。

pub mod anchor;
pub mod breadcrumb;
pub mod dropdown;
pub mod menu;
pub mod nav;
pub mod pagination;
pub mod steps;
pub mod tabs;

// 集中验证 Breadcrumb 稳定 link、reconcile 与语义事件契约。
#[cfg(test)]
mod breadcrumb_tests;
// 集中验证 Menu typed key、受控状态与重复身份门禁。
#[cfg(test)]
mod menu_tests;
// 集中验证 Tabs 面板门控、状态保留与焦点迁移契约。
#[cfg(test)]
mod tabs_tests;

pub use anchor::*;
pub use breadcrumb::*;
pub use dropdown::*;
pub use menu::*;
pub use nav::*;
pub use pagination::*;
pub use steps::*;
pub use tabs::*;
