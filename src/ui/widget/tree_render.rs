//! 此文件已弃用 —— 渲染路径已统一到 LayerTree。
//!
//! - render_geometry / render_pass → LayerTree::render
//! - render_overlays / post_render_pass → LayerTree::render_overlays
//!
//! WidgetTree 不再提供直接的渲染方法，所有渲染通过 LayerTree 编排。

use super::tree_core::WidgetTree;
use super::*;

impl WidgetTree {
    /// 保留兼容性方法——委托给 LayerTree 的 render 流程。
    /// 当前无调用方，保留签名为将来可能的统一入口。
    #[allow(dead_code)]
    pub fn render_tree(&self, _ctx: &mut crate::ui::render_context::RenderContext) {
        // 已弃用
    }
}
