//! `src/draw/backend/gpu/canvas.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl NativeGpuCanvas2D） ——

impl NativeGpuCanvas2D {
    // hybrid canvas 只作为 soft fallback 参考语义的回归测试 fixture 保留。
    #[cfg(test)]
    // 创建允许测试显式进入 hybrid 分支的 canvas。
    pub(crate) fn new(width: i32, height: i32, native_caps: NativeRasterCaps) -> Self {
        Self::new_with_mode(width, height, native_caps, false)
    }

    // 原生测试目标保留 mesh 数量观测入口，供提交顺序测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn pending_mesh_count(&self) -> usize {
        self.pending_native
            .iter()
            .filter(|op| matches!(op, PendingNativeOp::SolidMesh(_)))
            .count()
    }
}
