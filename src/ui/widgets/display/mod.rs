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
/// 用于展示层级数据的树组件。
pub mod tree;
// 终端 capability 关闭时不解析终端实现目录。
#[cfg(feature = "terminal")]
// 启用后保留 display::terminal 子模块路径。
/// 用于命令行交互与滚动输出的终端组件。
pub mod terminal;

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
// 终端 capability 启用时才汇入数据展示公开面。
#[cfg(feature = "terminal")]
// 启用后保持 Terminal 与行模型的扁平重导出。
pub use terminal::*;
