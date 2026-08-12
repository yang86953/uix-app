// 将一个结构性 State 订阅的解绑责任绑定到树或节点生命周期。
pub(crate) struct ReconcileBindLease {
    // 保存仍可在析构时执行解绑的状态绑定源。
    source: std::sync::Arc<dyn super::StatePaintBind>,
    // 保存所属 WidgetTree 的稳定请求端口键。
    key: usize,
}

// 为租约提供创建和析构时精确解绑的生命周期契约。
impl ReconcileBindLease {
    // 为指定源和树请求端口建立一份持有者租约。
    pub(crate) fn bind(
        // 接收结构性 State 绑定源。
        source: std::sync::Arc<dyn super::StatePaintBind>,
        // 接收 WidgetTree 请求端口去重键。
        key: usize,
        // 接收 WidgetTree 请求端口回调。
        callback: super::ReconcileCallback,
    ) -> Self {
        // 增加源在该树请求端口上的持有计数。
        source.bind_reconcile_site(key, callback);
        // 返回将在 Drop 时撤销本次计数的租约。
        Self { source, key }
    }
}

// 真实移除、替换或关闭树时释放对应的结构性 State 订阅。
impl Drop for ReconcileBindLease {
    // 在生命周期所有者丢弃时撤销一次同源同树持有计数。
    fn drop(&mut self) {
        // 让绑定源在最后一个租约离开时移除回调站点。
        self.source.unbind_reconcile_site(self.key);
    }
}
