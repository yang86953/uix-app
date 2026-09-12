//! `src/ui/layout/flex/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use super::super::FlexOutput;

// —— 自源文件移入的自由 cfg(test) 项 ——

/// Compute flex layout from input constraints.
///
/// Pure function with no side effects. Handles all justify-content modes,
/// cross-axis alignment via AlignItems and per-child align_self,
/// flex-grow/shrink distribution, flex-basis, min_size/max_size constraints,
/// and multi-line wrapping.
#[cfg(test)]
pub(crate) fn compute_flex_layout(input: &FlexInput<'_>) -> FlexOutput {
    // 兼容独立求解调用方：临时工作区的所有权随返回结果移交。
    let mut scratch = FlexComputeScratch::default();
    let _ = compute_flex_layout_into(input, &mut scratch);
    FlexOutput {
        child_rects: std::mem::take(&mut scratch.child_rects),
    }
}