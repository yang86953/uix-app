//! 浮层定位几何的单一共享实现。
//!
//! 收敛此前在气泡组件族与输入下拉族各自复制的两套定位机制：
//! - [`placement`]：气泡类浮层相对触发器的方向候选、正交对齐、主轴翻转、
//!   溢出评分、表面钳制与箭头绘制，供 tooltip、popover、popconfirm 共用；
//! - [`dropdown`]：垂直下拉弹层的有限收敛、矩形归一化、相对/绝对互换、
//!   表面合并、上下翻转决策与回退表面，供 select、autocomplete、日期时间
//!   面板等输入弹层共用。
//!
//! 行为契约：共享实现逐行保持被收敛副本的既有语义（优先作者方向、平局不翻转、
//! 两侧不足取更大一侧、钳制到逻辑表面）；组件特有差异（视觉常量来源、动画
//! 偏移、双月布局、命中结构等）仍留在各组件自己的解析入口。

pub(crate) mod dropdown;
pub(crate) mod placement;

pub(crate) use dropdown::{
    absolute_rect, finite_nonnegative, local_rect, normalize_rect, resolve_vertical_dropdown_rect,
    surface_union_rect, vertical_fallback_surface,
};
pub(crate) use placement::{
    OverlayArrowVisual, OverlayBubbleGeometry, OverlayPlacement, draw_overlay_arrow,
    resolve_overlay_bubble,
};
