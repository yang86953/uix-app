//! 当前绘制 component 上下文 — 渲染期间供 widget 查询自身 id（如懒加载图片失效）。

use std::cell::RefCell;

use crate::core::ComponentId;

thread_local! {
    static CURRENT_PAINT_WIDGET: RefCell<Option<ComponentId>> = const { RefCell::new(None) };
}

/// 设置当前正在绘制的 component id（合成器 paint 入口调用）。
pub(crate) fn set_current_paint_widget(id: Option<ComponentId>) {
    CURRENT_PAINT_WIDGET.with(|slot| {
        *slot.borrow_mut() = id;
    });
}

/// 读取当前绘制 component id。
pub(crate) fn current_paint_widget() -> Option<ComponentId> {
    CURRENT_PAINT_WIDGET.with(|slot| *slot.borrow())
}
