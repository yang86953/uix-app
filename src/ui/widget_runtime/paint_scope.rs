//! 当前绘制 widget 上下文 — 渲染期间供 widget 查询自身 id（如懒加载图片失效）。

use std::cell::RefCell;

use crate::core::WidgetId;

thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static CURRENT_PAINT_WIDGET: RefCell<Option<WidgetId>> = const { RefCell::new(None) };
}

/// 设置当前正在绘制的 widget id（合成器 paint 入口调用）。
pub(crate) fn set_current_paint_widget(id: Option<WidgetId>) {
    CURRENT_PAINT_WIDGET.with(|slot| {
        *slot.borrow_mut() = id;
    });
}

// 以 RAII 维护当前绘制组件，覆盖嵌套调用与 panic 展开路径。
pub(crate) struct PaintWidgetScope {
    // 保存进入当前绘制前的外层组件身份。
    previous: Option<WidgetId>,
}

// 提供当前绘制组件作用域的唯一进入入口。
impl PaintWidgetScope {
    // 安装新的当前绘制组件并保存外层上下文。
    pub(crate) fn enter(id: WidgetId) -> Self {
        // 原子替换线程私有槽并取回外层身份。
        let previous = CURRENT_PAINT_WIDGET.with(|slot| slot.replace(Some(id)));
        // 返回将在离开作用域时恢复外层身份的守卫。
        Self { previous }
    }
}

// 无论正常返回或 panic 都恢复进入前的绘制组件上下文。
impl Drop for PaintWidgetScope {
    // 在守卫生命周期结束时归还外层身份。
    fn drop(&mut self) {
        // 取出保存值，防止异常重复析构再次写入。
        let previous = self.previous.take();
        // 恢复外层组件或明确清空当前绘制身份。
        set_current_paint_widget(previous);
    }
}

/// 读取当前绘制 widget id。
pub fn current_paint_widget() -> Option<WidgetId> {
    CURRENT_PAINT_WIDGET.with(|slot| *slot.borrow())
}
