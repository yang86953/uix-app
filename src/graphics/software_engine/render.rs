use super::engine::SoftwareEngine;
use crate::graphics::FontHandle;
use crate::ui::theme::TokenProvider;
use crate::ui::widget::WidgetTree;

// ════════════════════════════════════════════════════════════════════════════
// SoftwareEngine — LayerTree 绘制（固有方法，非 trait）
// ════════════════════════════════════════════════════════════════════════════

impl SoftwareEngine {
    /// 渲染 LayerTree（与 GraphicsEngine 借用分离）。
    ///
    /// 使用 `addr_of_mut!` 做字段级拆分：`layer_tree` 和 GraphicsEngine
    /// 的字段（`rt`、`assets`、`font_service` 等）物理上不重叠，
    /// 分别可变借用在语义上是安全的。
    pub(crate) fn paint_layer_tree(&mut self, tree: &WidgetTree, tokens: &dyn TokenProvider) {
        use core::ptr::addr_of_mut;
        // 原始指针不产生 borrow，所以可以和 `self as &mut dyn GraphicsEngine` 共存
        let lt_ptr = addr_of_mut!(self.layer_tree);
        // SAFETY: font_service 字段与 engine 使用的字段（rt/assets）物理分离
        let fs_ptr = &self.font_service as *const crate::graphics::font_service::FontService;
        let engine: &mut dyn crate::graphics::GraphicsEngine = self;
        let lt = unsafe { &mut *lt_ptr };
        lt.render(engine, tree, tokens, FontHandle::default(), unsafe {
            &*fs_ptr
        });
    }
}
