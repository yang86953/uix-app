//! DrawPacket 条件采样资源与共享 shader 语义契约。

// 引入统一错误分类和结果类型。
use crate::core::error::{Errc, Error, Result};

// 引入 pipeline 采样语义、资源描述和不透明身份。
use super::{
    PipelineBinding, PipelineSampling, SamplerDesc, SamplerHandle, TextureFormat, TextureHandle,
};

// 原子保存共享 shader ABI 唯一开放的采样纹理、sampler 与 pipeline 语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SampledTextureBinding {
    // 保存被采样纹理的不透明身份。
    texture: TextureHandle,
    // 保存与纹理同时生效的 sampler 身份。
    sampler: SamplerHandle,
    // 保存只由绑定时 pipeline 契约派生的采样语义。
    sampling: PipelineSampling,
}

// 为 DrawPacket 与 Adapter 提供不暴露槽位的采样资源构造与投影。
impl SampledTextureBinding {
    // 从完整 pipeline 身份构造采样绑定，禁止调用方另造采样语义。
    pub(crate) const fn for_pipeline(
        // 保存不透明纹理身份。
        texture: TextureHandle,
        // 保存不透明 sampler 身份。
        sampler: SamplerHandle,
        // 从已绑定的 pipeline 派生唯一采样语义。
        pipeline: PipelineBinding,
    ) -> Self {
        // 两个资源身份与 pipeline 语义必须作为一个事实共同传递。
        Self {
            // 保存纹理资源身份。
            texture,
            // 保存 sampler 资源身份。
            sampler,
            // 采样语义只能从 pipeline 的共享 contract 派生。
            sampling: pipeline.contract().sampling,
        }
    }

    // 返回供 Adapter 资源表解析的纹理身份。
    pub(crate) const fn texture(self) -> TextureHandle {
        // 复制轻量句柄，不暴露成员改写能力。
        self.texture
    }

    // 返回供 Adapter 资源表解析的 sampler 身份。
    pub(crate) const fn sampler(self) -> SamplerHandle {
        // 复制轻量句柄，不暴露成员改写能力。
        self.sampler
    }

    // 返回由 pipeline contract 冻结的采样语义。
    pub(crate) const fn sampling(self) -> PipelineSampling {
        // 复制轻量共享枚举值。
        self.sampling
    }

    // 验证真实资源描述是否满足绑定时冻结的采样契约。
    pub(crate) fn validate_resources(
        // 接收资源表提供的实际纹理格式。
        self,
        format: TextureFormat,
        // 接收资源表提供的实际 sampler 描述。
        sampler: SamplerDesc,
    ) -> Result<()> {
        // 统一调用保存的 PipelineSampling 门禁。
        if !self.sampling.accepts(format, sampler) {
            // 返回不携带 Adapter 名称的共享参数错误。
            return Err(invalid_sampling(
                "sampled binding resources do not match pipeline sampling contract",
            ));
        }
        // 资源描述与绑定语义一致。
        Ok(())
    }

    // 判断绑定语义是否与当前 Draw pipeline 完全一致。
    pub(crate) fn matches_pipeline(self, pipeline: PipelineBinding) -> bool {
        // 只比较共享采样语义，不重新解释原生句柄。
        self.sampling == pipeline.contract().sampling
    }
}

// 封闭一次 Draw 是否拥有完整采样资源的条件角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DrawSamplingBinding {
    // None 只允许用于 PipelineSampling::None，其它 pipeline 必须携带完整绑定。
    sampled: Option<SampledTextureBinding>,
}

// 为条件采样角色提供命名构造、只读投影和 pipeline 匹配门禁。
impl DrawSamplingBinding {
    // 创建明确不读取纹理的 Draw 采样事实。
    pub(crate) const fn none() -> Self {
        // 用私有 Option 表达条件角色，不暴露 packet 字段回填能力。
        Self { sampled: None }
    }

    // 创建拥有完整纹理与 sampler 身份的 Draw 采样事实。
    pub(crate) const fn sampled(binding: SampledTextureBinding) -> Self {
        // 原子保存已经冻结采样语义的资源绑定。
        Self {
            // Some 只能携带完整 SampledTextureBinding。
            sampled: Some(binding),
        }
    }

    // 返回当前 Draw 明确携带的完整采样资源。
    pub(crate) const fn sampled_texture(self) -> Option<SampledTextureBinding> {
        // 复制封闭的条件绑定，不暴露内部字段。
        self.sampled
    }

    // 判断有无采样资源是否与 pipeline 的唯一共享契约一致。
    pub(crate) fn matches_pipeline(self, pipeline: PipelineBinding) -> bool {
        // 无采样 pipeline 只能与明确 None 组合。
        if pipeline.contract().sampling == PipelineSampling::None {
            // 任一多余采样绑定都属于矛盾 packet。
            return self.sampled.is_none();
        }
        // 采样 pipeline 必须携带语义匹配的完整资源绑定。
        self.sampled
            // 只接受绑定创建时冻结的同一采样语义。
            .is_some_and(|binding| binding.matches_pipeline(pipeline))
    }
}

// 构造不包含任一原生 API 名称的共享采样参数错误。
fn invalid_sampling(message: &'static str) -> Error {
    // 资源与采样语义错配统一属于调用参数错误。
    Error::new(Errc::InvalidArgument, message)
}
