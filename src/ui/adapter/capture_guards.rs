// 引入 View 动画捕获的成对入口。
use crate::ui::animation::{begin_animated_capture, end_animated_capture, AnimatedSource};
// 引入 View 响应式 State 捕获的成对入口。
use crate::ui::reactive::state::{begin_state_capture, end_state_capture};
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
        let mut node = crate::ui::component_state::with_component_state_capture(
            store,
            build_root,
        );
        // 正常结束并把捕获到的动画源交给声明根。
        node.animated_sources = capture_guard.finish();
        // 返回完成所有捕获的声明根。
        node
    }
}

// 在 View 构建异常时恢复既有 State 与动画捕获上下文。
pub(super) struct ViewCaptureGuard {
    // 标记成功结束路径已经显式取出动画源。
    finished: bool,
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

    // 正常结束捕获并返回本轮动画源。
    pub(super) fn finish(mut self) -> Vec<Arc<dyn AnimatedSource>> {
        // 先取出当前动画捕获栈顶。
        let animated_sources = end_animated_capture();
        // 再关闭 State 依赖捕获。
        end_state_capture();
        // 标记 Drop 无需重复清理。
        self.finished = true;
        // 返回交给声明根保存的动画源。
        animated_sources
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
        end_state_capture();
    }
}
