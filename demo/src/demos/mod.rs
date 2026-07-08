//! 组件库分类页面 — 对应 [`component.md`](../../docs/systems/component.md) 目录。

pub mod charts;
pub mod data;
pub mod feedback;
pub mod general;
pub mod input;
pub mod layout;
pub mod nav;
pub mod other;

use uix::prelude::*;

pub use charts::page_charts;
pub use data::page_data;
pub use feedback::page_feedback;
pub use general::page_general;
pub use input::page_input;
pub use layout::page_layout;
pub use nav::page_nav;
pub use other::page_other;

/// 按侧边栏索引构建对应分类页。
pub fn build_page(page_index: usize, tk: &DesignTokens) -> ViewNode {
    match page_index {
        0 => page_general(tk),
        1 => page_layout(tk),
        2 => page_nav(tk),
        3 => page_input(tk),
        4 => page_data(tk),
        5 => page_feedback(tk),
        6 => page_charts(tk),
        7 => page_other(tk),
        _ => page_general(tk),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::page::PAGE_TITLES;

    #[test]
    fn all_pages_build() {
        let tk = DesignTokens::antd_light();
        for i in 0..PAGE_TITLES.len() {
            let _ = build_page(i, &tk);
        }
    }
}
