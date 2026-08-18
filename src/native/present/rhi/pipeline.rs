//! Drawing System 与原生图形 Adapter 共享的 pipeline ABI 契约。

// 引入同层纹理格式与固定 uniform 字节数。
use super::{
    BLUR_UNIFORM_BYTES, GRADIENT_UNIFORM_BYTES, MESH_UNIFORM_BYTES, MSDF_UNIFORM_BYTES,
    PipelineHandle, SAMPLED_UNIFORM_BYTES, SECTOR_UNIFORM_BYTES, SHADOW_UNIFORM_BYTES,
    SHAPE_UNIFORM_BYTES, SamplerDesc, TextureFormat,
};

// 定义通用 renderer 与 native Adapter 共享的封闭 pipeline 语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineKind {
    // 使用位置 float2 与 MeshConstants uniform 绘制实心三角形。
    SolidMesh,
    // 使用位置 float2、纹理坐标 float2 与颜色 float4 绘制采样图元。
    TexturedQuad,
    // 使用单位 quad 与 GradientConstants uniform 绘制线性或径向渐变。
    GradientRect,
    // 使用 R8 coverage 与 glyph shader 绘制覆盖率 quad。
    GlyphCoverageQuad,
    // 使用 Shape 共享契约与 SDF shader 绘制圆角或描边矩形。
    ShapeRect,
    // 使用同一 Shape 共享契约与 SDF shader 执行饱和加法矩形。
    ShapeRectAdditive,
    // 使用 ShadowConstants 与 SDF shader 绘制软阴影。
    BoxShadow,
    // 复用采样 ABI 并使用独立加法 blend 绘制纹理 quad。
    TexturedQuadAdditive,
    // 使用 BlurConstants 与全屏区域 quad 执行一个方向的高斯 blur pass。
    BlurPass,
    // 使用 RGBA8 MSDF 与仿射 quad 绘制可缩放字形。
    MsdfGlyphQuad,
    // 使用单位 quad 与 SectorConstants 绘制分析抗锯齿扇形。
    Sector,
}

// 把不透明原生句柄与创建时的共享 pipeline 语义绑定为不可拆身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineBinding {
    // 保存只由对应 Adapter 解释的资源句柄。
    handle: PipelineHandle,
    // 保存 Drawing 与 Adapter 共用的 ABI、采样和混合语义。
    kind: PipelineKind,
}

// 为绑定后的 pipeline 身份提供只读访问和共享契约查询。
impl PipelineBinding {
    // 由 GraphicsDevice 创建成功后绑定原生句柄与同一个描述语义。
    pub(crate) const fn new(handle: PipelineHandle, kind: PipelineKind) -> Self {
        // 两个事实必须同时构造，禁止 FramePlan 只持有裸句柄。
        Self { handle, kind }
    }

    // 返回只允许 Adapter 资源表消费的不透明句柄。
    pub(crate) const fn handle(self) -> PipelineHandle {
        // 句柄不携带任何上层绘制语义。
        self.handle
    }

    // 返回创建时已经冻结的共享 pipeline 语义。
    pub(crate) const fn kind(self) -> PipelineKind {
        // FramePlan 与 Adapter 必须读取同一个身份。
        self.kind
    }

    // 返回本绑定唯一允许的跨 Adapter 资源与混合契约。
    pub(crate) const fn contract(self) -> PipelineContract {
        // 委托 PipelineKind 的闭集映射，禁止另存布局副本。
        self.kind.contract()
    }
}

// 定义 Adapter 必须映射的有限顶点布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineVertexLayout {
    // 只包含 position float2。
    PositionF32x2,
    // 包含 position float2、uv float2 与 color float4。
    PositionUvColorF32,
}

// 为顶点布局集中提供字节步长。
impl PipelineVertexLayout {
    // 返回一个顶点的固定字节数。
    pub(crate) const fn stride_bytes(self) -> u32 {
        // 逐个封闭布局返回唯一 ABI。
        match self {
            // 两个 f32 固定占八字节。
            Self::PositionF32x2 => 2 * std::mem::size_of::<f32>() as u32,
            // 八个 f32 固定占三十二字节。
            Self::PositionUvColorF32 => 8 * std::mem::size_of::<f32>() as u32,
        }
    }
}

// 定义每种 shader 常量的封闭布局身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineUniformLayout {
    // viewport、padding 与颜色组成的 MeshConstants。
    Mesh,
    // 只携带 viewport 与 padding 的 sampled constants。
    Sampled,
    // 仿射 quad、两色和参数组成的 GradientConstants。
    Gradient,
    // Shape 共享值对象编码的 ShapeConstants。
    Shape,
    // 仿射几何、颜色和软边参数组成的 ShadowConstants。
    Shadow,
    // 区域、方向、tap 数和权重组成的 BlurConstants。
    Blur,
    // viewport、atlas 尺寸和 range 组成的 MsdfConstants。
    Msdf,
    // viewport、矩形、颜色和角度组成的 SectorConstants。
    Sector,
}

// 为 uniform 布局集中提供总字节数。
impl PipelineUniformLayout {
    // 返回 Adapter 必须完整接收的 uniform ABI 大小。
    pub(crate) const fn size_bytes(self) -> usize {
        // 逐个封闭布局返回唯一 ABI。
        match self {
            // Mesh 值对象拥有自己的冻结 ABI。
            Self::Mesh => MESH_UNIFORM_BYTES,
            // Sampled 值对象拥有自己的冻结 ABI。
            Self::Sampled => SAMPLED_UNIFORM_BYTES,
            // Gradient 值对象拥有自己的冻结 ABI。
            Self::Gradient => GRADIENT_UNIFORM_BYTES,
            // Shape 值对象拥有自己的冻结 ABI。
            Self::Shape => SHAPE_UNIFORM_BYTES,
            // Shadow 值对象拥有自己的冻结 ABI。
            Self::Shadow => SHADOW_UNIFORM_BYTES,
            // Blur 值对象拥有自己的冻结 ABI。
            Self::Blur => BLUR_UNIFORM_BYTES,
            // MSDF 值对象拥有自己的冻结 ABI。
            Self::Msdf => MSDF_UNIFORM_BYTES,
            // Sector 值对象使用固定的四个 float4。
            Self::Sector => SECTOR_UNIFORM_BYTES,
        }
    }
}

// 定义 pipeline 对采样纹理格式与过滤方式的完整要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineSampling {
    // pipeline 不读取纹理。
    None,
    // pipeline 读取 BGRA8 或 RGBA8 premultiplied 颜色。
    PremultipliedColor,
    // pipeline 只读取单通道 R8 coverage。
    Coverage,
    // pipeline 只读取 RGBA8 MSDF 距离纹理。
    Msdf,
}

// 为采样契约提供统一纹理格式与 sampler 过滤门禁。
impl PipelineSampling {
    // 判断实际纹理格式和 sampler 是否同时满足当前 pipeline。
    pub(crate) const fn accepts(self, format: TextureFormat, sampler: SamplerDesc) -> bool {
        // 先按采样语义而不是原生 API 枚举检查格式。
        let format_matches = match self {
            // 无纹理 pipeline 不接受任何绑定格式。
            Self::None => false,
            // 普通颜色采样接受两种通用四通道排列。
            Self::PremultipliedColor => matches!(
                format,
                TextureFormat::Bgra8Unorm | TextureFormat::Rgba8Unorm
            ),
            // coverage 只接受单通道 Unorm。
            Self::Coverage => matches!(format, TextureFormat::R8Unorm),
            // MSDF 明确只接受 RGBA8，避免误用 BGRA atlas。
            Self::Msdf => matches!(format, TextureFormat::Rgba8Unorm),
        };
        // 再由同一个封闭语义决定唯一过滤方式。
        let sampler_matches = match self {
            // 无纹理 pipeline 不允许 sampler 绑定成为隐式依赖。
            Self::None => false,
            // 颜色、MSDF 与 Blur 都要求无 mip 的线性 min/mag 过滤。
            Self::PremultipliedColor | Self::Msdf => sampler.uses_linear_filter(),
            // Coverage atlas 保持离散像素，只允许最近点过滤。
            Self::Coverage => !sampler.uses_linear_filter(),
        };
        // 任一事实不匹配都必须在进入原生 draw 前失败。
        format_matches && sampler_matches
    }
}

// 定义所有 Adapter 必须一致实现的颜色混合语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineBlend {
    // shader 输出直通 RGB 与独立 alpha，使用 SrcAlpha SrcOver。
    StraightAlpha,
    // shader 输出已经乘过 alpha 的颜色，使用 One SrcOver。
    PremultipliedAlpha,
    // 源与目标都使用 One，实现饱和加法。
    Additive,
    // 关闭混合并完整替换目标像素。
    Replace,
}

// 定义图形 API 无关的固定混合因子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineBlendFactor {
    // 贡献固定为零。
    Zero,
    // 贡献固定为一。
    One,
    // 贡献使用源像素 alpha。
    SourceAlpha,
    // 贡献使用一减源像素 alpha。
    OneMinusSourceAlpha,
}

// 定义两个 Adapter 都必须显式编码的混合运算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineBlendOperation {
    // 将源贡献与目标贡献相加。
    Add,
}

// 定义两个 Adapter 都必须显式编码的颜色通道写掩码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineColorWriteMask {
    // 写入红、绿、蓝与 alpha 全部通道。
    All,
}

// 保存两个 Adapter 必须逐项映射的完整混合状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineBlendState {
    // 记录硬件颜色混合是否开启。
    pub(crate) enabled: bool,
    // 保存源 RGB 因子。
    pub(crate) source_color: PipelineBlendFactor,
    // 保存目标 RGB 因子。
    pub(crate) destination_color: PipelineBlendFactor,
    // 保存 RGB 通道的混合运算。
    pub(crate) color_operation: PipelineBlendOperation,
    // 保存源 alpha 因子。
    pub(crate) source_alpha: PipelineBlendFactor,
    // 保存目标 alpha 因子。
    pub(crate) destination_alpha: PipelineBlendFactor,
    // 保存 alpha 通道的混合运算。
    pub(crate) alpha_operation: PipelineBlendOperation,
    // 保存 render target 的颜色通道写掩码。
    pub(crate) write_mask: PipelineColorWriteMask,
}

// 为封闭混合语义提供唯一的 API 无关状态公式。
impl PipelineBlend {
    // 返回 OpenGL 与 D3D11 都必须直接翻译的因子集合。
    pub(crate) const fn state(self) -> PipelineBlendState {
        // 每种语义只在共享层声明一次颜色与 alpha 公式。
        match self {
            // straight-alpha 的 RGB 由源 alpha 缩放，alpha 保持 SrcOver。
            Self::StraightAlpha => PipelineBlendState {
                // 打开硬件混合。
                enabled: true,
                // RGB 使用源 alpha。
                source_color: PipelineBlendFactor::SourceAlpha,
                // 目标 RGB 使用剩余透明度。
                destination_color: PipelineBlendFactor::OneMinusSourceAlpha,
                // RGB 使用共享加法运算。
                color_operation: PipelineBlendOperation::Add,
                // 源 alpha 直接参与输出。
                source_alpha: PipelineBlendFactor::One,
                // 目标 alpha 使用剩余透明度。
                destination_alpha: PipelineBlendFactor::OneMinusSourceAlpha,
                // alpha 使用共享加法运算。
                alpha_operation: PipelineBlendOperation::Add,
                // 所有现有颜色 pipeline 都写入完整 RGBA。
                write_mask: PipelineColorWriteMask::All,
            },
            // premultiplied-alpha 的 RGB 已被源 alpha 缩放。
            Self::PremultipliedAlpha => PipelineBlendState {
                // 打开硬件混合。
                enabled: true,
                // 源 RGB 不得再次乘 alpha。
                source_color: PipelineBlendFactor::One,
                // 目标 RGB 使用剩余透明度。
                destination_color: PipelineBlendFactor::OneMinusSourceAlpha,
                // RGB 使用共享加法运算。
                color_operation: PipelineBlendOperation::Add,
                // 源 alpha 直接参与输出。
                source_alpha: PipelineBlendFactor::One,
                // 目标 alpha 使用剩余透明度。
                destination_alpha: PipelineBlendFactor::OneMinusSourceAlpha,
                // alpha 使用共享加法运算。
                alpha_operation: PipelineBlendOperation::Add,
                // 所有现有颜色 pipeline 都写入完整 RGBA。
                write_mask: PipelineColorWriteMask::All,
            },
            // Additive 对颜色和 alpha 都执行 source + destination。
            Self::Additive => PipelineBlendState {
                // 打开硬件混合。
                enabled: true,
                // 完整保留源 RGB。
                source_color: PipelineBlendFactor::One,
                // 完整保留目标 RGB。
                destination_color: PipelineBlendFactor::One,
                // RGB 使用共享加法运算。
                color_operation: PipelineBlendOperation::Add,
                // 完整保留源 alpha。
                source_alpha: PipelineBlendFactor::One,
                // 完整保留目标 alpha。
                destination_alpha: PipelineBlendFactor::One,
                // alpha 使用共享加法运算。
                alpha_operation: PipelineBlendOperation::Add,
                // 所有现有颜色 pipeline 都写入完整 RGBA。
                write_mask: PipelineColorWriteMask::All,
            },
            // Replace 关闭混合；因子仍描述等价的 source 覆盖公式。
            Self::Replace => PipelineBlendState {
                // 关闭硬件混合。
                enabled: false,
                // 等价覆盖公式完整保留源 RGB。
                source_color: PipelineBlendFactor::One,
                // 等价覆盖公式丢弃目标 RGB。
                destination_color: PipelineBlendFactor::Zero,
                // 禁用混合时仍保留等价的共享 RGB 运算。
                color_operation: PipelineBlendOperation::Add,
                // 等价覆盖公式完整保留源 alpha。
                source_alpha: PipelineBlendFactor::One,
                // 等价覆盖公式丢弃目标 alpha。
                destination_alpha: PipelineBlendFactor::Zero,
                // 禁用混合时仍保留等价的共享 alpha 运算。
                alpha_operation: PipelineBlendOperation::Add,
                // Replace 同样必须完整覆盖 RGBA。
                write_mask: PipelineColorWriteMask::All,
            },
        }
    }
}

// 定义两个 Adapter 都必须显式编码的面剔除语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineCullMode {
    // UIX 二维三角形不剔除任何绕序。
    None,
}

// 定义两个 Adapter 共享的正面绕序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineFrontFace {
    // 使用逆时针作为统一正面绕序。
    CounterClockwise,
}

// 定义两个 Adapter 共享的深度裁剪语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineDepthClip {
    // 按 OpenGL ES 固定裁剪体启用深度裁剪。
    Enabled,
}

// 保存两个 Adapter 必须逐项映射的二维光栅状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineRasterState {
    // 保存面剔除模式。
    pub(crate) cull_mode: PipelineCullMode,
    // 保存统一正面绕序。
    pub(crate) front_face: PipelineFrontFace,
    // 保存深度裁剪开关。
    pub(crate) depth_clip: PipelineDepthClip,
}

// 定义两个 Adapter 共享的深度测试与写入语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineDepthState {
    // 同时关闭深度测试与深度写入。
    Disabled,
}

// 定义两个 Adapter 共享的模板测试语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineStencilState {
    // 关闭模板测试与模板写入。
    Disabled,
}

// 保存两个 Adapter 必须逐项映射的深度模板状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineDepthStencilState {
    // 保存深度测试与写入语义。
    pub(crate) depth: PipelineDepthState,
    // 保存模板测试语义。
    pub(crate) stencil: PipelineStencilState,
}

// 定义全部现有 UI pipeline 共用的二维光栅状态。
pub(crate) const PIPELINE_RASTER_2D: PipelineRasterState = PipelineRasterState {
    // 二维 UI 图元不做面剔除。
    cull_mode: PipelineCullMode::None,
    // 两个 Adapter 都使用逆时针正面绕序。
    front_face: PipelineFrontFace::CounterClockwise,
    // D3D11 必须与 OpenGL ES 的固定裁剪体保持一致。
    depth_clip: PipelineDepthClip::Enabled,
};

// 定义全部现有 UI pipeline 共用的关闭深度模板状态。
pub(crate) const PIPELINE_DEPTH_STENCIL_DISABLED: PipelineDepthStencilState =
    PipelineDepthStencilState {
        // UIX 二维 draw 不读取或写入深度附件。
        depth: PipelineDepthState::Disabled,
        // UIX 二维 draw 不读取或写入模板附件。
        stencil: PipelineStencilState::Disabled,
    };

// 汇总一个 pipeline 在所有图形 API 上必须相同的 ABI 事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineContract {
    // 保存固定顶点布局。
    pub(crate) vertex: PipelineVertexLayout,
    // 保存固定 uniform 布局。
    pub(crate) uniform: PipelineUniformLayout,
    // 保存固定采样格式语义。
    pub(crate) sampling: PipelineSampling,
    // 保存固定颜色混合语义。
    pub(crate) blend: PipelineBlend,
    // 保存固定二维光栅状态。
    pub(crate) raster: PipelineRasterState,
    // 保存固定深度模板状态。
    pub(crate) depth_stencil: PipelineDepthStencilState,
}

// 为 pipeline 身份提供唯一共享契约。
impl PipelineKind {
    // 返回 Drawing 创建资源与 Adapter 执行 draw 共同读取的 ABI。
    pub(crate) const fn contract(self) -> PipelineContract {
        // 每个 pipeline 必须显式完成闭集映射，新增变体时由编译器要求补齐。
        match self {
            // 实心颜色由 shader 以 straight-alpha 输出。
            Self::SolidMesh => PipelineContract {
                // solid 使用 position float2。
                vertex: PipelineVertexLayout::PositionF32x2,
                // solid 使用 MeshConstants。
                uniform: PipelineUniformLayout::Mesh,
                // solid 不读取纹理。
                sampling: PipelineSampling::None,
                // solid 颜色保持 straight-alpha SrcOver。
                blend: PipelineBlend::StraightAlpha,
                // solid 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // solid 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // 普通采样 quad 使用 premultiplied SrcOver。
            Self::TexturedQuad => PipelineContract {
                // sampled quad 使用 float8 顶点。
                vertex: PipelineVertexLayout::PositionUvColorF32,
                // sampled quad 只需要 viewport。
                uniform: PipelineUniformLayout::Sampled,
                // sampled quad 读取四通道颜色。
                sampling: PipelineSampling::PremultipliedColor,
                // 图片像素在进入 RHI 前已经 premultiply。
                blend: PipelineBlend::PremultipliedAlpha,
                // sampled quad 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // sampled quad 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // 渐变颜色由 shader 以 straight-alpha 输出。
            Self::GradientRect => PipelineContract {
                // 渐变使用单位 position float2 quad。
                vertex: PipelineVertexLayout::PositionF32x2,
                // 渐变使用完整仿射常量。
                uniform: PipelineUniformLayout::Gradient,
                // 渐变不读取纹理。
                sampling: PipelineSampling::None,
                // 插值颜色保持 straight-alpha SrcOver。
                blend: PipelineBlend::StraightAlpha,
                // gradient 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // gradient 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // coverage quad 把覆盖率乘入输出颜色。
            Self::GlyphCoverageQuad => PipelineContract {
                // coverage 复用 sampled float8 顶点。
                vertex: PipelineVertexLayout::PositionUvColorF32,
                // coverage 复用 viewport 常量。
                uniform: PipelineUniformLayout::Sampled,
                // coverage 只接受 R8。
                sampling: PipelineSampling::Coverage,
                // shader 已把 coverage 乘入 rgba。
                blend: PipelineBlend::PremultipliedAlpha,
                // coverage 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // coverage 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // 普通 Shape 使用 premultiplied coverage 输出。
            Self::ShapeRect => PipelineContract {
                // Shape 使用单位 position float2 quad。
                vertex: PipelineVertexLayout::PositionF32x2,
                // Shape 使用共享值对象 ABI。
                uniform: PipelineUniformLayout::Shape,
                // Shape 不读取纹理。
                sampling: PipelineSampling::None,
                // shader 已把分析 coverage 乘入 rgba。
                blend: PipelineBlend::PremultipliedAlpha,
                // Shape 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // Shape 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // Additive Shape 只改变混合语义。
            Self::ShapeRectAdditive => PipelineContract {
                // Additive Shape 保持相同顶点布局。
                vertex: PipelineVertexLayout::PositionF32x2,
                // Additive Shape 保持相同 uniform。
                uniform: PipelineUniformLayout::Shape,
                // Additive Shape 不读取纹理。
                sampling: PipelineSampling::None,
                // 仅目标混合切换为加法。
                blend: PipelineBlend::Additive,
                // Additive Shape 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // Additive Shape 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // Shadow shader 输出 straight-alpha 颜色与覆盖率。
            Self::BoxShadow => PipelineContract {
                // Shadow 复用单位 position float2 quad。
                vertex: PipelineVertexLayout::PositionF32x2,
                // Shadow 使用独立仿射常量语义。
                uniform: PipelineUniformLayout::Shadow,
                // Shadow 不读取纹理。
                sampling: PipelineSampling::None,
                // Shadow 与既有 D3D11 像素契约保持 straight-alpha。
                blend: PipelineBlend::StraightAlpha,
                // Shadow 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // Shadow 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // Additive sampled quad 只改变混合语义。
            Self::TexturedQuadAdditive => PipelineContract {
                // Additive 图片保持 float8 顶点。
                vertex: PipelineVertexLayout::PositionUvColorF32,
                // Additive 图片保持 viewport 常量。
                uniform: PipelineUniformLayout::Sampled,
                // Additive 图片仍读取四通道颜色。
                sampling: PipelineSampling::PremultipliedColor,
                // 目标混合切换为加法。
                blend: PipelineBlend::Additive,
                // Additive sampled quad 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // Additive sampled quad 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // Blur pass 直接替换目标区域。
            Self::BlurPass => PipelineContract {
                // Blur 使用 NDC position float2 区域 quad。
                vertex: PipelineVertexLayout::PositionF32x2,
                // Blur 使用固定高斯核常量。
                uniform: PipelineUniformLayout::Blur,
                // Blur 读取四通道颜色。
                sampling: PipelineSampling::PremultipliedColor,
                // 两阶段 blur 都完整替换目标区域。
                blend: PipelineBlend::Replace,
                // Blur 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // Blur 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // MSDF shader 把解析 coverage 乘入输出颜色。
            Self::MsdfGlyphQuad => PipelineContract {
                // MSDF 复用 sampled float8 顶点。
                vertex: PipelineVertexLayout::PositionUvColorF32,
                // MSDF 使用 atlas 尺寸与 range 常量。
                uniform: PipelineUniformLayout::Msdf,
                // MSDF 只接受 RGBA8 atlas。
                sampling: PipelineSampling::Msdf,
                // shader 已输出 premultiplied coverage。
                blend: PipelineBlend::PremultipliedAlpha,
                // MSDF 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // MSDF 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
            // Sector shader 把分析 coverage 乘入颜色。
            Self::Sector => PipelineContract {
                // Sector 使用单位 position float2 quad。
                vertex: PipelineVertexLayout::PositionF32x2,
                // Sector 使用四个 float4 常量。
                uniform: PipelineUniformLayout::Sector,
                // Sector 不读取纹理。
                sampling: PipelineSampling::None,
                // shader 已输出 premultiplied coverage。
                blend: PipelineBlend::PremultipliedAlpha,
                // Sector 使用共享二维光栅状态。
                raster: PIPELINE_RASTER_2D,
                // Sector 禁用深度与模板。
                depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
            },
        }
    }
}

// 定义 pipeline 创建描述，具体 shader 与原生对象只由 Adapter 映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineDesc {
    // 使用封闭类型标识通用 renderer 选定的 pipeline 语义。
    pub(crate) kind: PipelineKind,
}

// 锁定跨 Adapter 最容易漂移的 ABI 语义。
#[cfg(test)]
mod tests {
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

    // 所有现有 pipeline 必须显式使用同一二维固定状态。
    #[test]
    fn pipeline_contracts_own_fixed_raster_and_depth_stencil_state() {
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
        ] {
            // 读取该 pipeline 的唯一共享契约。
            let contract = kind.contract();
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
}
