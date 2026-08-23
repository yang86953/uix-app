    // 引入当前模块全部类型。
    use super::*;

    // 把共享混合因子转换为参考像素公式中的标量。
    fn factor_value(factor: PipelineBlendFactor, source_alpha: f32) -> f32 {
        // 只实现共享层允许的封闭因子集合。
        match factor {
            // Zero 不贡献当前像素。
            PipelineBlendFactor::Zero => 0.0,
            // One 完整贡献当前像素。
            PipelineBlendFactor::One => 1.0,
            // SourceAlpha 使用源透明度。
            PipelineBlendFactor::SourceAlpha => source_alpha,
            // OneMinusSourceAlpha 使用剩余透明度。
            PipelineBlendFactor::OneMinusSourceAlpha => 1.0 - source_alpha,
        }
    }

    // 对一对已经乘过因子的通道值执行共享混合运算。
    fn apply_operation(operation: PipelineBlendOperation, source: f32, destination: f32) -> f32 {
        // 只实现共享层允许的封闭运算集合。
        match operation {
            // Add 把源贡献与目标贡献相加。
            PipelineBlendOperation::Add => source + destination,
        }
    }

    // 用 API 无关公式计算一个 RGBA 参考像素。
    fn blend_reference_pixel(
        // 接收共享混合语义。
        blend: PipelineBlend,
        // 接收 shader 实际输出的源像素。
        source: [f32; 4],
        // 接收 render target 中已有的目标像素。
        destination: [f32; 4],
    ) -> [f32; 4] {
        // 读取两个 Adapter 共用的完整状态。
        let state = blend.state();
        // 关闭混合时硬件直接写入源像素。
        if !state.enabled {
            // 返回未修改的 shader 输出。
            return source;
        }
        // 计算源 RGB 贡献因子。
        let source_color = factor_value(state.source_color, source[3]);
        // 计算目标 RGB 贡献因子。
        let destination_color = factor_value(state.destination_color, source[3]);
        // 计算源 alpha 贡献因子。
        let source_alpha = factor_value(state.source_alpha, source[3]);
        // 计算目标 alpha 贡献因子。
        let destination_alpha = factor_value(state.destination_alpha, source[3]);
        // 对 RGB 和 alpha 分别应用共享运算公式。
        [
            // 按共享 RGB 运算计算红色通道。
            apply_operation(
                // 读取共享 RGB 运算。
                state.color_operation,
                // 传入源红色贡献。
                source[0] * source_color,
                // 传入目标红色贡献。
                destination[0] * destination_color,
            ),
            // 按共享 RGB 运算计算绿色通道。
            apply_operation(
                // 读取共享 RGB 运算。
                state.color_operation,
                // 传入源绿色贡献。
                source[1] * source_color,
                // 传入目标绿色贡献。
                destination[1] * destination_color,
            ),
            // 按共享 RGB 运算计算蓝色通道。
            apply_operation(
                // 读取共享 RGB 运算。
                state.color_operation,
                // 传入源蓝色贡献。
                source[2] * source_color,
                // 传入目标蓝色贡献。
                destination[2] * destination_color,
            ),
            // 按共享 alpha 运算计算透明度通道。
            apply_operation(
                // 读取共享 alpha 运算。
                state.alpha_operation,
                // 传入源 alpha 贡献。
                source[3] * source_alpha,
                // 传入目标 alpha 贡献。
                destination[3] * destination_alpha,
            ),
        ]
    }

    // 普通与 Additive 变体只能改变 blend，不能改变资源 ABI。
    #[test]
    fn additive_variants_only_change_blend() {
        // 读取普通图片与加法图片契约。
        let textured = PipelineKind::TexturedQuad.contract();
        // 读取图片加法契约。
        let textured_additive = PipelineKind::TexturedQuadAdditive.contract();
        // 两种图片必须共享顶点、uniform 与采样事实。
        assert_eq!(textured.vertex, textured_additive.vertex);
        // 两种图片必须共享 uniform 布局。
        assert_eq!(textured.uniform, textured_additive.uniform);
        // 两种图片必须共享纹理格式要求。
        assert_eq!(textured.sampling, textured_additive.sampling);
        // 加法图片必须只切换 blend。
        assert_eq!(textured_additive.blend, PipelineBlend::Additive);
        // 读取普通 Shape 与加法 Shape 契约。
        let shape = PipelineKind::ShapeRect.contract();
        // 读取 Shape 加法契约。
        let shape_additive = PipelineKind::ShapeRectAdditive.contract();
        // 两种 Shape 必须共享顶点、uniform 与采样事实。
        assert_eq!(shape.vertex, shape_additive.vertex);
        // 两种 Shape 必须共享 uniform 布局。
        assert_eq!(shape.uniform, shape_additive.uniform);
        // 两种 Shape 必须共享无纹理事实。
        assert_eq!(shape.sampling, shape_additive.sampling);
        // 加法 Shape 必须只切换 blend。
        assert_eq!(shape_additive.blend, PipelineBlend::Additive);
    }

    // 锁定曾在 D3D11 与 OpenGL 间分叉的颜色混合语义。
    #[test]
    fn straight_and_premultiplied_sources_have_one_blend_contract() {
        // 实心 shader 输出 straight-alpha。
        assert_eq!(
            PipelineKind::SolidMesh.contract().blend,
            PipelineBlend::StraightAlpha
        );
        // 渐变 shader 输出 straight-alpha。
        assert_eq!(
            PipelineKind::GradientRect.contract().blend,
            PipelineBlend::StraightAlpha
        );
        // 图片像素使用 premultiplied-alpha。
        assert_eq!(
            PipelineKind::TexturedQuad.contract().blend,
            PipelineBlend::PremultipliedAlpha
        );
        // 两阶段 blur 必须完整替换目标。
        assert_eq!(
            PipelineKind::BlurPass.contract().blend,
            PipelineBlend::Replace
        );
    }

    // 所有现有混合语义必须显式拥有运算与颜色写掩码。
    #[test]
    fn blend_semantics_own_complete_output_state() {
        // 遍历当前封闭混合集合，避免任何变体继续依赖 Adapter 默认状态。
        for blend in [
            // 验证 straight-alpha 语义。
            PipelineBlend::StraightAlpha,
            // 验证 premultiplied-alpha 语义。
            PipelineBlend::PremultipliedAlpha,
            // 验证 additive 语义。
            PipelineBlend::Additive,
            // 验证 replace 的等价公式。
            PipelineBlend::Replace,
        ] {
            // 读取该语义的完整共享状态。
            let state = blend.state();
            // RGB 运算必须由共享契约明确指定。
            assert_eq!(state.color_operation, PipelineBlendOperation::Add);
            // alpha 运算必须由共享契约明确指定。
            assert_eq!(state.alpha_operation, PipelineBlendOperation::Add);
            // 全部现有 pipeline 必须显式写入完整 RGBA。
            assert_eq!(state.write_mask, PipelineColorWriteMask::All);
        }
    }

    // 所有现有 pipeline 必须显式使用同一二维几何与输出状态。
    #[test]
    fn pipeline_contracts_own_fixed_geometry_and_output_state() {
        // 遍历完整 pipeline 闭集，禁止新增语义遗漏固定状态。
        for kind in [
            // 验证实心网格。
            PipelineKind::SolidMesh,
            // 验证普通采样 quad。
            PipelineKind::TexturedQuad,
            // 验证渐变。
            PipelineKind::GradientRect,
            // 验证 coverage 字形。
            PipelineKind::GlyphCoverageQuad,
            // 验证普通 Shape。
            PipelineKind::ShapeRect,
            // 验证加法 Shape。
            PipelineKind::ShapeRectAdditive,
            // 验证阴影。
            PipelineKind::BoxShadow,
            // 验证加法采样 quad。
            PipelineKind::TexturedQuadAdditive,
            // 验证 Blur。
            PipelineKind::BlurPass,
            // 验证 MSDF 字形。
            PipelineKind::MsdfGlyphQuad,
            // 验证扇形。
            PipelineKind::Sector,
            // 验证解析抗锯齿线段。
            PipelineKind::LineSegment,
        ] {
            // 读取该 pipeline 的唯一共享契约。
            let contract = kind.contract();
            // 全部二维图元必须使用独立三角形列表。
            assert_eq!(contract.topology, PipelinePrimitiveTopology::TriangleList);
            // 全部二维图元必须关闭会改变八位输出最低位的颜色抖动。
            assert_eq!(contract.dither, PipelineDitherState::Disabled);
            // 全部二维图元必须冻结为同一单样本覆盖语义。
            assert_eq!(contract.multisample, PipelineMultisampleState::SingleSample);
            // 全部二维图元必须使用相同光栅状态。
            assert_eq!(contract.raster, PIPELINE_RASTER_2D);
            // 全部二维图元必须显式关闭深度与模板。
            assert_eq!(contract.depth_stencil, PIPELINE_DEPTH_STENCIL_DISABLED);
        }
    }

    // 验证同一视觉颜色的 straight 与 premultiplied 输入产生相同参考像素。
    #[test]
    fn blend_factors_produce_one_cross_adapter_pixel_formula() {
        // 使用能够精确表示的目标像素，避免测试容差掩盖因子错误。
        let destination = [0.25, 0.5, 0.75, 0.25];
        // straight-alpha shader 输出未乘透明度的颜色。
        let straight_source = [1.0, 0.5, 0.25, 0.5];
        // premultiplied shader 输出同一颜色乘以二分之一透明度后的值。
        let premultiplied_source = [0.5, 0.25, 0.125, 0.5];
        // 计算 straight-alpha 的共享参考结果。
        let straight = blend_reference_pixel(
            // 使用 straight SrcOver 语义。
            PipelineBlend::StraightAlpha,
            // 传入 straight shader 输出。
            straight_source,
            // 传入相同目标像素。
            destination,
        );
        // 计算 premultiplied-alpha 的共享参考结果。
        let premultiplied = blend_reference_pixel(
            // 使用 premultiplied SrcOver 语义。
            PipelineBlend::PremultipliedAlpha,
            // 传入已经乘 alpha 的 shader 输出。
            premultiplied_source,
            // 传入相同目标像素。
            destination,
        );
        // 两种表示必须得到完全相同的可见像素。
        assert_eq!(straight, premultiplied);
        // 锁定本例的精确 RGBA 输出，防止两个公式一起漂移。
        assert_eq!(straight, [0.625, 0.5, 0.5, 0.625]);
        // Replace 必须忽略已有目标并直接写入源像素。
        assert_eq!(
            // 用同一个参考入口验证关闭混合语义。
            blend_reference_pixel(PipelineBlend::Replace, straight_source, destination),
            // 预期结果就是原始源像素。
            straight_source
        );
    }

    // 采样格式与过滤方式必须由共享契约共同拒绝错误组合。
    #[test]
    fn sampling_contract_accepts_only_declared_format_and_filter_pairs() {
        // 构造图片、MSDF 与 Blur 共用的线性 clamp sampler。
        let linear = SamplerDesc::linear_clamp();
        // 构造 coverage 专用的最近点 clamp sampler。
        let nearest = SamplerDesc::nearest_clamp();
        // 普通颜色接受 BGRA8 与线性过滤。
        assert!(PipelineSampling::PremultipliedColor.accepts(TextureFormat::Bgra8Unorm, linear));
        // 普通颜色接受 RGBA8 与线性过滤。
        assert!(PipelineSampling::PremultipliedColor.accepts(TextureFormat::Rgba8Unorm, linear));
        // 普通颜色拒绝 R8 coverage。
        assert!(!PipelineSampling::PremultipliedColor.accepts(TextureFormat::R8Unorm, linear));
        // 普通颜色拒绝会产生跨像素差异的最近点过滤。
        assert!(!PipelineSampling::PremultipliedColor.accepts(TextureFormat::Bgra8Unorm, nearest));
        // coverage 只接受 R8 与最近点过滤。
        assert!(PipelineSampling::Coverage.accepts(TextureFormat::R8Unorm, nearest));
        // coverage 拒绝会跨 glyph 像素插值的线性过滤。
        assert!(!PipelineSampling::Coverage.accepts(TextureFormat::R8Unorm, linear));
        // MSDF 接受 RGBA8 与线性过滤。
        assert!(PipelineSampling::Msdf.accepts(TextureFormat::Rgba8Unorm, linear));
        // MSDF 拒绝 BGRA8 atlas。
        assert!(!PipelineSampling::Msdf.accepts(TextureFormat::Bgra8Unorm, linear));
        // MSDF 拒绝破坏距离场连续性的最近点过滤。
        assert!(!PipelineSampling::Msdf.accepts(TextureFormat::Rgba8Unorm, nearest));
    }
