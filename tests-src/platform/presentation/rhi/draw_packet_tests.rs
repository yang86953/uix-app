//! DrawPacket 测试构造辅助（自 src/platform/presentation/rhi/draw_packet.rs 移入，零调用探针原样保留待接线）。
//! 经 #[path] 引用，不进发布包。
#![allow(dead_code)]

use super::*;

impl DrawPacket {
// 仅为共享契约测试保留完整 Buffer 角色并替换 pipeline 事实。
    #[cfg(test)]
    pub(crate) const fn with_pipeline(self, pipeline: PipelineBinding) -> Self {
        // 测试变体仍保留全部资源与范围事实。
        Self { pipeline, ..self }
    }

    // 仅为共享契约测试保留其它事实并替换条件采样角色。
    #[cfg(test)]
    pub(crate) const fn with_sampling(self, sampling: DrawSamplingBinding) -> Self {
        // 测试变体仍只能产生字段完整、但可由门禁判定组合关系的 packet。
        Self { sampling, ..self }
    }

    // 仅为共享契约测试保留 pipeline 与 Buffer 角色并替换范围事实。
    #[cfg(test)]
    pub(crate) const fn with_range(self, range: DrawRange) -> Self {
        // 测试变体仍保留 pipeline 与全部资源事实。
        Self { range, ..self }
    }
}
