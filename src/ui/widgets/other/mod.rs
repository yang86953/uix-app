//! 其他组件：图表、富文本、滚动视图与杂项。

// 图表 capability 关闭时不解析基础与高级图表实现目录。
#[cfg(feature = "charts")]
// 启用后保留既有 chart 子模块路径。
pub mod chart;
pub mod misc;
// 富文本 capability 关闭时不解析对应组件实现目录。
#[cfg(feature = "rich-text")]
pub mod rich_text;
pub mod scroll_view;
pub mod theme_toggle;

// 图表 capability 启用时才汇入其他组件公开面。
#[cfg(feature = "charts")]
// 启用后保持全部图表类型的既有扁平重导出。
pub use chart::*;
pub use misc::*;
// 富文本 capability 启用时才汇入其他组件公开面。
#[cfg(feature = "rich-text")]
pub use rich_text::*;
pub use scroll_view::*;
pub use theme_toggle::*;
