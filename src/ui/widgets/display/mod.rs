//! 数据展示：表格、列表、卡片与信息呈现。

pub mod avatar;
pub mod badge;
pub mod calendar;
pub mod card;
pub mod carousel;
pub mod collapse;
pub mod descriptions;
pub mod empty;
pub mod image;
pub mod image_group;
// 图片组件共享的主题绘制值保持在 display Module 私有边界。
mod image_presentation;
// 单 Image 的加载生命周期状态保持在私有模块。
mod image_state;
pub mod list;
pub mod result;
pub mod selectable_list;
pub mod skeleton;
// 表格 capability 关闭时不解析基础表格、泛型数据表与动态渲染实现。
#[cfg(feature = "table")]
// 启用后保留既有 display::table 子模块路径。
pub mod table;
pub mod tag;
pub mod timeline;
// 树组件 capability 关闭时不解析展示树实现目录。
#[cfg(feature = "tree-widgets")]
// 启用后保留既有 display::tree 子模块路径。
pub mod tree;

pub use avatar::*;
pub use badge::*;
pub use calendar::*;
pub use card::*;
pub use carousel::*;
pub use collapse::*;
pub use descriptions::*;
pub use empty::*;
pub use image::*;
pub use image_group::*;
pub use list::*;
pub use result::*;
pub use selectable_list::*;
pub use skeleton::*;
// 表格 capability 启用时才汇入数据展示公开面。
#[cfg(feature = "table")]
// 启用后保持全部表格类型的既有扁平重导出。
pub use table::*;
pub use tag::*;
pub use timeline::*;
// 树组件 capability 启用时才汇入数据展示公开面。
#[cfg(feature = "tree-widgets")]
// 启用后保持 Tree 与 TreeNode 的既有扁平重导出。
pub use tree::*;
