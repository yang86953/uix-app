//! PipelineBinding 测试构造辅助（自 src/platform/presentation/rhi/pipeline.rs 移入，零调用探针原样保留待接线）。
//! 经 #[path] 引用，不进发布包。
#![allow(dead_code)]

use super::*;

impl PipelineBinding {
// 仅为共享 RHI 单元测试构造可控的句柄与语义组合。
    #[cfg(test)]
    pub(crate) const fn for_test(handle: PipelineHandle, kind: PipelineKind) -> Self {
        // 测试伪造入口不向生产 Module 暴露资源创建能力。
        Self { handle, kind }
    }
}
