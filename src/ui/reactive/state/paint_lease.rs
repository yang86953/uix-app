// 将一份绘制订阅的解绑责任绑定到实际组件节点生命周期。
pub(crate) struct PaintBindLease {
    // 保存仍可在析构时执行解绑的响应式源。
    source: std::sync::Arc<dyn super::StatePaintBind>,
    // 保存所属实际组件的代际身份。
    widget_id: crate::core::WidgetId,
    // 保存用于精确区分窗口端点的失效队列。
    queue: crate::draw::renderer::InvalidationQueueHandle,
}

// 为绘制租约提供创建与自动解绑契约。
impl PaintBindLease {
    // 为指定响应式源和实际组件建立一份绘制订阅。
    pub(crate) fn bind(
        // 接收 State 或 Computed 的共享绑定接口。
        source: std::sync::Arc<dyn super::StatePaintBind>,
        // 接收当前实际组件的代际身份。
        widget_id: crate::core::WidgetId,
        // 接收所属窗口的失效队列。
        queue: crate::draw::renderer::InvalidationQueueHandle,
        // 接收当前布局解析出的绘制范围。
        rect: Option<crate::core::Rect>,
    ) -> Self {
        // 在返回所有者租约前增加源端站点计数。
        source.bind_paint_site(widget_id, queue.clone(), rect);
        // 保存完成精确解绑所需的全部端点身份。
        Self {
            // 持有源直到租约完成解绑。
            source,
            // 保存组件代际身份。
            widget_id,
            // 保存同一窗口队列身份。
            queue,
        }
    }
}

// 真实移除、换根或窗口关闭时释放对应绘制订阅。
impl Drop for PaintBindLease {
    // 在生命周期所有者丢弃时撤销一份端点持有计数。
    fn drop(&mut self) {
        // 最后一份租约离开时由源端移除站点和队列强引用。
        self.source.unbind_paint_site(self.widget_id, &self.queue);
    }
}

// 以作用域守卫确保嵌套绘制和 panic 都能恢复捕获栈。
pub(crate) struct StateBindCaptureGuard {
    // 保存本捕获作用域对应的实际组件身份。
    widget_id: crate::core::WidgetId,
    // 标记正常交接是否已经弹出捕获上下文。
    active: bool,
}

// 管理一次组件绘制依赖捕获的开始与正常交接。
impl StateBindCaptureGuard {
    // 开始只属于当前组件的最内层依赖捕获。
    pub(crate) fn begin(
        // 接收当前实际组件的代际身份。
        widget_id: crate::core::WidgetId,
        // 接收所属窗口失效队列。
        queue: crate::draw::renderer::InvalidationQueueHandle,
        // 接收当前组件可精确失效的绘制范围。
        rect: Option<crate::core::Rect>,
    ) -> Self {
        // 把独立捕获帧压入线程私有栈。
        super::begin_state_bind_capture(widget_id, queue, rect);
        // 返回负责正常交接或异常清理的作用域守卫。
        Self {
            // 保存弹栈时需要校验的组件身份。
            widget_id,
            // 新捕获尚未完成租约交接。
            active: true,
        }
    }

    // 正常结束捕获并把完整绘制租约集合交给实际节点。
    pub(crate) fn finish(mut self) -> Vec<PaintBindLease> {
        // 先标记已接管，避免本方法完成后的 Drop 重复弹栈。
        self.active = false;
        // 弹出本层捕获并建立由节点拥有的租约。
        super::end_state_bind_capture(self.widget_id)
    }
}

// 覆盖组件绘制 panic 路径，禁止捕获上下文泄漏到后续节点。
impl Drop for StateBindCaptureGuard {
    // 仅为尚未正常完成的作用域执行无绑定清理。
    fn drop(&mut self) {
        // 正常 finish 已经弹栈且交接租约。
        if !self.active {
            // 不应重复操作外层捕获帧。
            return;
        }
        // 异常路径只丢弃本层读取，不建立无所有者订阅。
        super::discard_state_bind_capture(self.widget_id);
        // 防御性记录本守卫已经完成清理。
        self.active = false;
    }
}
