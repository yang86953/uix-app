// 引入 View 动画捕获的成对入口。
use crate::ui::animation::{begin_animated_capture, end_animated_capture, AnimatedSource};
// 引入 View 响应式 State 捕获的成对入口。
use crate::ui::reactive::state::{begin_state_capture, end_state_capture, StateCaptureOutput};
// 保存捕获到的动画源动态句柄。
use std::sync::Arc;

// 把 View 捕获入口放在独立模块，避免适配器主体超过规模上限。
impl crate::ui::adapter::ViewAdapter {
    /// Builds a ViewNode while capturing State bindings.
    pub fn capture_root<F>(build_root: F) -> crate::ui::view::ViewNode
    where
        // 接收任意一次性根工厂闭包。
        F: FnOnce() -> crate::ui::view::ViewNode,
    {
        // 为直接捕获创建独立状态所有者，随后由根节点转交给 WidgetTree。
        let store = crate::ui::component_state::ComponentStateStore::new();
        // 安装临时捕获上下文以隔离每个根的私有状态。
        let mut node = Self::capture_root_with_store(store.clone(), build_root);
        // 把首次捕获使用的存储随根节点传递给建树入口。
        node.set_component_state_store(store);
        // 返回已捕获并标注状态所有权的根。
        node
    }

    // 使用既有窗口树存储捕获根，供每次 reconcile 复用状态身份。
    pub(crate) fn capture_root_with_store<F>(
        // 接收所属窗口树的状态存储。
        store: crate::ui::component_state::ComponentStateStore,
        // 接收根 View 构建闭包。
        build_root: F,
    ) -> crate::ui::view::ViewNode
    where
        // 接收任意一次性根工厂闭包。
        F: FnOnce() -> crate::ui::view::ViewNode,
    {
        // 使用作用域守卫保持正常与异常路径的捕获生命周期成对。
        let capture_guard = ViewCaptureGuard::begin();
        // 仅在构建期间安装窗口私有组件状态上下文。
        let (mut node, receipt) = crate::ui::component_state::with_component_state_capture(
            store,
            build_root,
        );
        // 把未提交 journal 附着到声明根，等待 WidgetTree 事务成功后接纳。
        node.push_component_state_receipt(receipt);
        // 正常结束并取得当前根专属的 State、Effect 与动画输出。
        let output = capture_guard.finish();
        // 追加当前帧的 State 绑定，保留 build_root 已携带的内层捕获输出。
        node.captured_state_binds.extend(output.state.state_binds);
        // 追加当前帧的 Effect，保留 build_root 已携带的内层捕获输出。
        node.captured_effects.extend(output.state.effects);
        // 追加当前帧动画源，保留 build_root 已携带的内层捕获输出。
        node.animated_sources.extend(output.animated_sources);
        // 返回完成所有捕获的声明根。
        node
    }
}

// 迭代取走一个声明根及全部后代尚未提交的组件状态回执。
pub(super) fn take_component_state_receipts(
    // 接收将要被展开或协调的声明根。
    root: &mut crate::ui::view::ViewNode,
    // 返回转移到适配器事务边界的全部回执。
) -> Vec<crate::ui::component_state::ComponentStateCaptureReceipt> {
    // 建立不会消耗线程调用栈的待访问节点栈。
    let mut nodes = vec![root];
    // 汇总整棵声明树的未提交 journal。
    let mut receipts = Vec::new();
    // 深度优先访问每个声明节点。
    while let Some(node) = nodes.pop() {
        // 先转移当前节点的回执，避免展开时过早触发 Drop 回滚。
        receipts.append(&mut node.component_state_receipts);
        // 将全部直接子节点压入显式栈，避免深树递归。
        nodes.extend(node.children.iter_mut());
    }
    // 交还完整的成功提交候选集。
    receipts
}

// 逐个取走一组动态子树尚未提交的组件状态回执。
pub(super) fn take_component_state_receipts_from_children(
    // 接收动态协调即将消费的所有声明子节点。
    children: &mut [crate::ui::view::ViewNode],
    // 返回属于同一动态协调事务的全部回执。
) -> Vec<crate::ui::component_state::ComponentStateCaptureReceipt> {
    // 建立动态协调的回执汇总。
    let mut receipts = Vec::new();
    // 收集每个动态子树的回执。
    for child in children {
        // 将完整子树的回执交给当前事务。
        receipts.append(&mut take_component_state_receipts(child));
    }
    // 交还完整的成功提交候选集。
    receipts
}

// 在 View 构建异常时恢复既有 State 与动画捕获上下文。
pub(super) struct ViewCaptureGuard {
    // 标记成功结束路径已经显式取出动画源。
    finished: bool,
}

// 聚合一次 View 构建需要转交给声明根的所有运行时捕获输出。
pub(super) struct ViewCaptureOutput {
    // 保存当前根专属的响应式 State 与 Effect 输出。
    pub(super) state: StateCaptureOutput,
    // 保存当前根专属的动画源输出。
    pub(super) animated_sources: Vec<Arc<dyn AnimatedSource>>,
}

// 为一次 View 捕获安装所有临时运行时上下文。
impl ViewCaptureGuard {
    // 同时开启 State 依赖与动画源捕获。
    pub(super) fn begin() -> Self {
        // 开始登记 View 构建读取的响应式状态。
        begin_state_capture();
        // 开始收集本次根构建创建的动画源。
        begin_animated_capture();
        // 返回负责异常恢复的作用域守卫。
        Self { finished: false }
    }

    // 正常结束捕获并返回本轮所有显式输出。
    pub(super) fn finish(mut self) -> ViewCaptureOutput {
        // 先取出当前动画捕获栈顶。
        let animated_sources = end_animated_capture();
        // 再关闭 State 依赖捕获。
        let state = end_state_capture();
        // 标记 Drop 无需重复清理。
        self.finished = true;
        // 返回交给声明根保存的全部捕获输出。
        ViewCaptureOutput {
            // 转交当前根的 State 与 Effect 输出。
            state,
            // 转交当前根的动画源输出。
            animated_sources,
        }
    }
}

// 确保 panic 展开不会污染同线程后续窗口构建。
impl Drop for ViewCaptureGuard {
    // 在未走正常结束路径时回滚所有临时捕获。
    fn drop(&mut self) {
        // 正常结束后不执行重复弹栈。
        if self.finished {
            // 提前返回保持配对调用唯一。
            return;
        }
        // 丢弃异常构建期间登记的动画源并恢复栈深度。
        let _ = end_animated_capture();
        // 关闭异常构建遗留的 State 捕获标志。
        // 丢弃仅属于异常构建帧的 State 与 Effect 输出。
        let _ = end_state_capture();
    }
}
