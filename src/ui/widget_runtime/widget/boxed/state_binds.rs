// 节点结构性 State 租约与实际节点生命周期保持同一所有权边界。
impl super::BoxedWidget {
    // 整体替换节点本轮绘制读取产生的租约集合。
    pub(crate) fn replace_paint_state_binds(
        // 绘制路径只持有节点共享借用，租约集合使用内部可变性。
        &self,
        // 接收本轮由布局探测或实际绘制捕获的全部租约。
        leases: Vec<crate::ui::reactive::state::PaintBindLease>,
    ) {
        // 先安装新租约再释放旧集合，避免同一端点出现短暂空窗。
        *self.paint_state_binds.borrow_mut() = leases;
    }

    // 在窗口关闭前主动释放节点绘制 State 租约。
    pub(crate) fn clear_paint_state_binds(&self) {
        // 清空集合触发每份租约精确解绑并释放窗口队列强引用。
        self.paint_state_binds.borrow_mut().clear();
    }

    // 整体替换节点本轮固有尺寸读取产生的布局租约集合。
    pub(crate) fn replace_layout_state_binds(
        &self,
        leases: Vec<crate::ui::reactive::state::PaintBindLease>,
    ) {
        // 与绘制租约分开持有，避免首帧 render 覆盖布局依赖。
        *self.layout_state_binds.borrow_mut() = leases;
    }

    // 在窗口关闭前主动释放节点布局 State 租约。
    pub(crate) fn clear_layout_state_binds(&self) {
        self.layout_state_binds.borrow_mut().clear();
    }

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
