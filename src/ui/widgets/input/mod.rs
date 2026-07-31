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
pub mod select;
pub mod slider;
pub mod switch;
pub mod time_picker;
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
pub use tree_select::*;
