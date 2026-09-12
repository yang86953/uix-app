//! `src/ui/layout/grid/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use super::super::GridOutput;

// —— 自源文件移入的自由 cfg(test) 项 ——

/// 测试兼容入口把临时工作区的结果所有权移交给调用方。
#[cfg(test)]
pub(crate) fn compute_grid_layout(input: &GridInput<'_>) -> GridOutput {
    let mut scratch = GridComputeScratch::default();
    let total_size = compute_grid_layout_into(input, &mut scratch);
    GridOutput {
        child_rects: std::mem::take(&mut scratch.child_rects),
        total_size,
    }
}