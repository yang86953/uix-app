//! 输入组件：表单控件与数据录入。

pub mod autocomplete;
pub mod cascader;
pub mod checkbox;
pub mod color_picker;
mod date_calendar;
pub mod date_picker;
pub mod date_range_picker;
#[allow(clippy::module_inception)]
pub mod input;
pub mod input_group;
pub mod input_number;
pub mod mentions;
pub mod radio;
pub mod range_slider;
pub mod rate;
pub mod segmented;
/// 单选与多选下拉选择组件。
pub mod select;
pub mod slider;
pub mod switch;
pub mod time_picker;
// 树组件 capability 关闭时不解析树选择器实现。
#[cfg(feature = "tree-widgets")]
// 启用后保留既有 input::tree_select 子模块路径。
/// 支持层级数据的树形选择组件。
pub mod tree_select;

pub use autocomplete::*;
pub use cascader::*;
pub use checkbox::*;
pub use color_picker::*;
pub use date_picker::*;
pub use date_range_picker::*;
pub use input::*;
pub use input_group::*;
pub use input_number::*;
pub use mentions::*;
pub use radio::*;
pub use range_slider::*;
pub use rate::*;
pub use segmented::*;
pub use select::*;
pub use slider::*;
pub use switch::*;
pub use time_picker::*;
// 树组件 capability 启用时才汇入 TreeSelect 公开面。
#[cfg(feature = "tree-widgets")]
// 启用后保持 TreeSelect 的既有扁平重导出。
pub use tree_select::*;

// 把字符索引转换为字节偏移；索引越界时收敛到文本末尾。
// 输入族组件共享的光标/选区换算辅助。
pub(crate) fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}
