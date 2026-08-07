//! 虚拟化 — VirtualScroll / VirtualScrollBuilder。
//!
//! # SMC 边界（SMC-04）
//!
//! virtualization Module 消费 view（View / ViewNode）与 component
//! （PaintContext / WidgetNode），是 ui System 的叶子 Module。

pub mod virtual_scroll;
// 导出可变行高测量缓存及其范围计算协议。
pub mod measurement_cache;

// 保留既有虚拟滚动入口。
pub use virtual_scroll::*;
// 让调用方可以直接构造独立测量缓存。
