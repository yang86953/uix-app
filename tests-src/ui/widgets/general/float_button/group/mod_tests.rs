//! `src/ui/widgets/general/float_button/group/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl FloatButtonGroup） ——

impl FloatButtonGroup {
    // 测试入口：把当前过渡推进到完成，避免向父模块暴露状态字段。
    #[cfg(test)]
    pub(super) fn finish_transition_for_test(&mut self) {
        self.transition.update(1.0);
    }

    // 测试入口：返回组内布局记录数量。
    #[cfg(test)]
    pub(super) fn item_layout_count_for_test(&self) -> usize {
        self.item_layouts.len()
    }

    // 测试入口：返回当前公开展开语义。
    #[cfg(test)]
    pub(super) fn is_expanded_for_test(&self) -> bool {
        self.expanded
    }
}
