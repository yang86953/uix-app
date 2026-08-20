    // 引入当前 Component 的私有契约。
    use super::*;
    // 引入构造测试 pipeline 的共享值。
    use crate::native::present::rhi::{PipelineHandle, PipelineKind};

    // 创建 coverage pipeline 身份。
    const COVERAGE: PipelineBinding = PipelineBinding::for_test(
        // 使用稳定非零句柄。
        PipelineHandle::from_raw(1),
        // 选择 R8 最近点采样语义。
        PipelineKind::GlyphCoverageQuad,
    );
    // 创建普通颜色采样 pipeline 身份。
    const TEXTURED: PipelineBinding = PipelineBinding::for_test(
        // 使用另一稳定非零句柄。
        PipelineHandle::from_raw(2),
        // 选择四通道线性采样语义。
        PipelineKind::TexturedQuad,
    );
    // 创建不读取纹理的 pipeline 身份。
    const SOLID: PipelineBinding = PipelineBinding::for_test(
        // 使用第三个稳定非零句柄。
        PipelineHandle::from_raw(3),
        // 选择无采样的 mesh 语义。
        PipelineKind::SolidMesh,
    );

    // 验证完整采样绑定拥有资源身份、语义与真实描述门禁。
    #[test]
    fn sampled_binding_owns_pipeline_and_resource_contract() {
        // 从 coverage pipeline 构造完整绑定。
        let binding = SampledTextureBinding::for_pipeline(
            // 使用稳定纹理身份。
            TextureHandle::from_raw(4),
            // 使用稳定 sampler 身份。
            SamplerHandle::from_raw(5),
            // 采样语义只从共享 pipeline 派生。
            COVERAGE,
        );
        // 绑定必须保持原始纹理身份。
        assert_eq!(binding.texture().raw(), 4);
        // 绑定必须保持原始 sampler 身份。
        assert_eq!(binding.sampler().raw(), 5);
        // 绑定语义必须等于 coverage 契约。
        assert_eq!(binding.sampling(), PipelineSampling::Coverage);
        // 正确格式与 sampler 必须通过。
        assert!(
            binding
                .validate_resources(TextureFormat::R8Unorm, SamplerDesc::nearest_clamp())
                .is_ok()
        );
        // 错误纹理格式必须失败。
        assert!(
            binding
                .validate_resources(TextureFormat::Rgba8Unorm, SamplerDesc::nearest_clamp())
                .is_err()
        );
        // 错误过滤方式必须失败。
        assert!(
            binding
                .validate_resources(TextureFormat::R8Unorm, SamplerDesc::linear_clamp())
                .is_err()
        );
    }

    // 验证条件角色只接受与 pipeline 完全一致的有无采样组合。
    #[test]
    fn draw_sampling_binding_closes_pipeline_combination() {
        // 无采样角色必须匹配 SolidMesh。
        assert!(DrawSamplingBinding::none().matches_pipeline(SOLID));
        // 采样 pipeline 不得缺失资源。
        assert!(!DrawSamplingBinding::none().matches_pipeline(COVERAGE));
        // 构造 coverage 完整资源绑定。
        let coverage = DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
            // 使用稳定纹理身份。
            TextureHandle::from_raw(4),
            // 使用稳定 sampler 身份。
            SamplerHandle::from_raw(5),
            // 冻结 coverage 采样语义。
            COVERAGE,
        ));
        // 同一 coverage pipeline 必须匹配。
        assert!(coverage.matches_pipeline(COVERAGE));
        // 不同采样语义 pipeline 必须拒绝。
        assert!(!coverage.matches_pipeline(TEXTURED));
        // 无采样 pipeline 不得携带多余资源。
        assert!(!coverage.matches_pipeline(SOLID));
    }
