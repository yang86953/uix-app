//! 输入与导航组件共享的双向绑定机械骨架。
//!
//! 各组件的越界 clamp / 取整 / 归一化等属于刻意保留的组件语义，留在各自实现内；
//! 这里只收敛两件完全相同的机械事：读取绑定 [`State`] 登记响应式依赖，以及
//! 「相等则不写」的受控写回，避免相同值产生多余 generation 与 reconcile。

use crate::ui::reactive::state::State;

/// 受控绑定时读取一次外部状态，把该 [`State`] 登记到当前响应式追踪上下文。
///
/// 非受控（`None`）不产生任何读取；与各组件手写的
/// `if let Some(state) = binding { let _ = state.get(); }` 逐字等价。
pub fn capture_dependency<T>(binding: Option<&State<T>>)
where
    T: Clone + Send + Sync + 'static,
{
    if let Some(state) = binding {
        let _ = state.get();
    }
}

/// 受控写回原语：仅在外部状态与目标值不同时提交。
///
/// 非受控（`None`）不产生外部写入；相等时不调用 [`State::set`]，
/// 保持与各组件手写守卫完全一致的 generation / reconcile 语义。
pub fn write_if_changed<T>(binding: Option<&State<T>>, value: T)
where
    T: PartialEq + Clone + Send + Sync + 'static,
{
    let Some(state) = binding else {
        return;
    };
    if state.get() != value {
        state.set(value);
    }
}
