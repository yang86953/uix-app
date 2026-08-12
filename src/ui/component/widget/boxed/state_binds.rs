// 节点结构性 State 租约与实际节点生命周期保持同一所有权边界。
impl super::BoxedWidget {
    // 整体替换节点本轮声明结构依赖的租约集合。
    pub(crate) fn replace_reconcile_state_binds(
        // 接收当前已挂载节点。
        &mut self,
        // 接收本轮为该节点建立的全部租约。
        leases: Vec<crate::ui::reactive::state::ReconcileBindLease>,
    ) {
        // 替换时释放旧租约，仅在最后持有者离开时解绑源。
        self.reconcile_state_binds = leases;
    }

    // 在窗口关闭前主动释放节点结构性 State 租约。
    pub(crate) fn clear_reconcile_state_binds(&mut self) {
        // 清空集合触发每份租约的精确解绑。
        self.reconcile_state_binds.clear();
    }
}
