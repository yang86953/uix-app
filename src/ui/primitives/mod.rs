//! Neutral native view primitives, reusable by independent design systems.
pub(crate) mod binding;
mod container;
mod label;
mod scroll_view;
mod text;
pub mod text_edit;
use crate::ui::{FlexDirection, IntoViewChildren, ViewNode};
pub use container::Container;
pub use label::Label;
pub use scroll_view::ScrollView;
pub use text::{IntoLabelContent, label};
pub use text_edit::{TextEditState, TextEditor};
pub fn column(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::Container::new()
            .dir(FlexDirection::Column)
            .flex_grow(1.0),
        children.into_view_children(),
    )
}

/// 列容器，保持 intrinsic 高度（flex_grow = 0）。
///
/// 用于顶栏、侧栏品牌区、卡片内文案组等不应吞掉父列剩余空间的局部内容。
pub fn column_fit(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::Container::new().dir(FlexDirection::Column),
        children.into_view_children(),
    )
}

/// 行容器（Flex 方向为 Row）。
///
/// 同质数组 / `Vec`，或异质元组 / [`crate::views!`]（公开用法见仓库 `docs/使用/布局.md`）。
pub fn row(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        crate::ui::Container::new().dir(FlexDirection::Row),
        children.into_view_children(),
    )
}

mod snapshot;
pub use snapshot::PrimitiveSnapshot;

/// An unstyled grid; tracks and gaps are provided through the node's Style.
pub fn grid(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(
        Container::new().style(
            crate::ui::Style::default().with_display(crate::ui::theme::style::DisplayMode::Grid),
        ),
        children.into_view_children(),
    )
}
/// An unstyled scroll viewport containing the supplied children.
pub fn scroll(children: impl IntoViewChildren) -> ViewNode {
    ViewNode::new(ScrollView::default(), children.into_view_children())
}
