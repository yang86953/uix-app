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

pub use anchor::*;
pub use breadcrumb::*;
pub use dropdown::*;
pub use menu::*;
pub use nav::*;
pub use pagination::*;
pub use steps::*;
pub use tabs::*;
