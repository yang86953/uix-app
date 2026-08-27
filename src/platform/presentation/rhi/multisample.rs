//! 跨图形 API 共用的采样覆盖状态契约。

// 定义全部原生 Adapter 必须穷尽映射的采样覆盖语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineMultisampleState {
    // 使用一个样本、关闭 alpha/sample coverage 并写入全部样本位。
    SingleSample,
}

// 为共享采样覆盖语义提供不暴露原生枚举的事实查询。
impl PipelineMultisampleState {
    // 判断是否允许 alpha-to-coverage 修改片元覆盖率。
    pub(crate) const fn alpha_to_coverage_enabled(self) -> bool {
        // 当前封闭集合只允许关闭该转换。
        match self {
            // 单样本路径不得启用 alpha-to-coverage。
            Self::SingleSample => false,
        }
    }

    // 判断 rasterizer 是否允许多样本光栅化。
    pub(crate) const fn raster_multisample_enabled(self) -> bool {
        // 当前封闭集合只允许单样本光栅化。
        match self {
            // 单样本路径关闭多样本光栅化。
            Self::SingleSample => false,
        }
    }

    // 返回输出合并阶段允许写入的共享样本位。
    pub(crate) const fn sample_mask(self) -> u32 {
        // 当前封闭集合始终允许全部可用样本位。
        match self {
            // 单样本路径仍使用全位掩码，禁止 Adapter 私自裁剪覆盖率。
            Self::SingleSample => u32::MAX,
        }
    }
}
