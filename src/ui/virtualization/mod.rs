//! 虚拟化 — VirtualScroll / VirtualScrollBuilder。
//!
//! # SMC 边界（SMC-04）
//!
//! virtualization Module 消费 view（View / ViewNode）与 component
//! （PaintContext / WidgetNode），是 ui System 的叶子 Module。

pub mod virtual_scroll;

pub use virtual_scroll::*;
