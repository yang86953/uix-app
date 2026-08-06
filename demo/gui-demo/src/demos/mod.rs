//! 组件库分类页面 — 对照 `src/ui/widgets/` 与 Gallery 覆盖矩阵。

pub mod charts;
pub mod component_qa;
pub mod context;
pub mod data;
pub mod feedback;
pub mod framework;
pub mod gallery;
pub mod general;
pub mod home;
pub mod input;
pub mod layout;
pub mod nav;
pub mod other;
pub mod runtime;

use uix::prelude::*;

pub use charts::page_charts;
pub use component_qa::page_component_qa;
pub use context::DemoCtx;
pub use data::page_data;
pub use feedback::page_feedback;
pub use framework::page_framework;
pub use gallery::page_gallery;
pub use general::page_general;
pub use home::page_home;
pub use input::page_input;
pub use layout::page_layout;
pub use nav::page_nav;
pub use other::page_other;
pub use runtime::page_runtime;

use crate::common::page::{
    PAGE_APP, PAGE_CHARTS, PAGE_COMPONENT_QA, PAGE_DATA, PAGE_FEEDBACK, PAGE_FRAMEWORK,
    PAGE_GALLERY, PAGE_GENERAL, PAGE_HOME, PAGE_INPUT, PAGE_LAYOUT, PAGE_NAV, PAGE_OTHER,
};

/// 按侧边栏索引构建对应分类页。
pub fn build_page(page_index: usize, ctx: &DemoCtx<'_>) -> ViewNode {
    match page_index {
        PAGE_HOME => page_home(ctx),
        PAGE_APP => page_runtime(ctx),
        PAGE_GENERAL => page_general(ctx),
        PAGE_LAYOUT => page_layout(ctx),
        PAGE_NAV => page_nav(ctx),
        PAGE_INPUT => page_input(ctx),
        PAGE_DATA => page_data(ctx),
        PAGE_FEEDBACK => page_feedback(ctx),
        PAGE_CHARTS => page_charts(ctx),
        PAGE_OTHER => page_other(ctx),
        PAGE_FRAMEWORK => page_framework(ctx),
        PAGE_GALLERY => page_gallery(ctx),
        PAGE_COMPONENT_QA => page_component_qa(ctx),
        _ => page_home(ctx),
    }
}
