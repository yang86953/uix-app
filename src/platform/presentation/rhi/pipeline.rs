//! Drawing System 与原生图形 Adapter 共享的 pipeline ABI 契约。

// 引入同层纹理格式与固定 uniform 字节数。
use super::{
    BLUR_UNIFORM_BYTES, GRADIENT_UNIFORM_BYTES, MESH_UNIFORM_BYTES, MSDF_UNIFORM_BYTES,
    PipelineHandle, PipelineMultisampleState, SAMPLED_UNIFORM_BYTES, SECTOR_UNIFORM_BYTES,
    SHADOW_UNIFORM_BYTES, SHAPE_UNIFORM_BYTES, SamplerAddressMode, SamplerDesc, SamplerFilter,
    SamplerMipMode, TextureFormat,
};
// 引入独立共享 Component 拥有的顶点布局值对象。
use super::vertex_layout::PipelineVertexLayout;

// 定义通用 renderer 与 native Adapter 共享的封闭 pipeline 语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineKind {
    // 使用位置 float2、coverage float1 与 MeshConstants 绘制抗锯齿实心三角形。
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
    // 使用绝对 source UV 顶点与 BlurConstants 执行一个方向的高斯 blur pass。
    BlurPass,
    // 使用 RGBA8 MSDF 与仿射 quad 绘制可缩放字形。
    MsdfGlyphQuad,
    // 使用单位 quad 与 SectorConstants 绘制分析抗锯齿扇形。
    Sector,
    // 使用单位 quad 与共享四组常量绘制解析抗锯齿线段。
    LineSegment,
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
    // 仅由共享 pipeline 资源表把新句柄与其真实描述语义绑定。
    pub(super) const fn new(handle: PipelineHandle, kind: PipelineKind) -> Self {
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
        let filter_matches = match self {
            // 无纹理 pipeline 不允许 sampler 绑定成为隐式依赖。
            Self::None => false,
            // 颜色、MSDF 与 Blur 都要求无 mip 的线性 min/mag 过滤。
            Self::PremultipliedColor | Self::Msdf => {
                // 只接受共享描述明确声明的线性过滤。
                matches!(sampler.filter(), SamplerFilter::Linear)
            }
            // Coverage atlas 保持离散像素，只允许最近点过滤。
            Self::Coverage => {
                // 只接受共享描述明确声明的最近点过滤。
                matches!(sampler.filter(), SamplerFilter::Nearest)
            }
        };
        // 所有二维 pipeline 都必须共享唯一的边缘限制语义。
        let address_matches = matches!(sampler.address_mode(), SamplerAddressMode::ClampToEdge);
        // 所有现有纹理都只允许访问创建时唯一存在的第零级。
        let mip_matches = matches!(sampler.mip_mode(), SamplerMipMode::SingleLevel);
        // 任一事实不匹配都必须在进入原生 draw 前失败。
        format_matches && filter_matches && address_matches && mip_matches
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

// 定义两个 Adapter 都必须穷尽映射的原语拓扑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelinePrimitiveTopology {
    // 把每三个顶点或索引解释为一个独立三角形。
    TriangleList,
}

// 定义两个 Adapter 必须共同实现的颜色抖动语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineDitherState {
    // 禁止原生 API 在八位颜色目标上修改最低位。
    Disabled,
}
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
    // 保存固定原语拓扑。
    pub(crate) topology: PipelinePrimitiveTopology,
    // 保存固定颜色抖动语义。
    pub(crate) dither: PipelineDitherState,
    // 保存固定采样覆盖状态。
    pub(crate) multisample: PipelineMultisampleState,
    // 保存固定二维光栅状态。
    pub(crate) raster: PipelineRasterState,
    // 保存固定深度模板状态。
    pub(crate) depth_stencil: PipelineDepthStencilState,
}

// 用唯一公共边界构造现有 UI 二维 pipeline 契约。
const fn ui_2d_pipeline_contract(
    // 接收 pipeline 特有的顶点布局。
    vertex: PipelineVertexLayout,
    // 接收 pipeline 特有的 uniform 布局。
    uniform: PipelineUniformLayout,
    // 接收 pipeline 特有的采样语义。
    sampling: PipelineSampling,
    // 接收 pipeline 特有的混合语义。
    blend: PipelineBlend,
) -> PipelineContract {
    // 返回同时冻结公共几何和输出状态的完整契约。
    PipelineContract {
        // 保留调用分支选择的顶点布局。
        vertex,
        // 保留调用分支选择的 uniform 布局。
        uniform,
        // 保留调用分支选择的采样语义。
        sampling,
        // 保留调用分支选择的混合语义。
        blend,
        // 全部现有 UI draw 使用独立三角形列表。
        topology: PipelinePrimitiveTopology::TriangleList,
        // 全部现有 UI draw 禁止原生颜色抖动。
        dither: PipelineDitherState::Disabled,
        // 全部现有 UI draw 使用单样本且关闭 coverage 转换。
        multisample: PipelineMultisampleState::SingleSample,
        // 全部现有 UI draw 使用同一二维光栅状态。
        raster: PIPELINE_RASTER_2D,
        // 全部现有 UI draw 显式关闭深度与模板。
        depth_stencil: PIPELINE_DEPTH_STENCIL_DISABLED,
    }
}

// 为 pipeline 身份提供唯一共享契约。
impl PipelineKind {
    // 返回 Drawing 创建资源与 Adapter 执行 draw 共同读取的 ABI。
    pub(crate) const fn contract(self) -> PipelineContract {
        // 每个 pipeline 必须显式完成闭集映射，新增变体时由编译器要求补齐。
        match self {
            // 实心颜色由 shader 以 straight-alpha 输出。
            Self::SolidMesh => ui_2d_pipeline_contract(
                // solid 使用 position float2 与逐顶点 coverage。
                PipelineVertexLayout::PositionCoverageF32,
                // solid 使用 MeshConstants。
                PipelineUniformLayout::Mesh,
                // solid 不读取纹理。
                PipelineSampling::None,
                // solid 颜色保持 straight-alpha SrcOver。
                PipelineBlend::StraightAlpha,
            ),
            // 普通采样 quad 使用 premultiplied SrcOver。
            Self::TexturedQuad => ui_2d_pipeline_contract(
                // sampled quad 使用 float8 顶点。
                PipelineVertexLayout::PositionUvColorF32,
                // sampled quad 只需要 viewport。
                PipelineUniformLayout::Sampled,
                // sampled quad 读取四通道颜色。
                PipelineSampling::PremultipliedColor,
                // 图片像素在进入 RHI 前已经 premultiply。
                PipelineBlend::PremultipliedAlpha,
            ),
            // 渐变颜色由 shader 以 straight-alpha 输出。
            Self::GradientRect => ui_2d_pipeline_contract(
                // 渐变使用单位 position float2 quad。
                PipelineVertexLayout::PositionF32x2,
                // 渐变使用完整仿射常量。
                PipelineUniformLayout::Gradient,
                // 渐变不读取纹理。
                PipelineSampling::None,
                // 插值颜色保持 straight-alpha SrcOver。
                PipelineBlend::StraightAlpha,
            ),
            // coverage quad 把覆盖率乘入输出颜色。
            Self::GlyphCoverageQuad => ui_2d_pipeline_contract(
                // coverage 复用 sampled float8 顶点。
                PipelineVertexLayout::PositionUvColorF32,
                // coverage 复用 viewport 常量。
                PipelineUniformLayout::Sampled,
                // coverage 只接受 R8。
                PipelineSampling::Coverage,
                // shader 已把 coverage 乘入 rgba。
                PipelineBlend::PremultipliedAlpha,
            ),
            // 普通 Shape 使用 premultiplied coverage 输出。
            Self::ShapeRect => ui_2d_pipeline_contract(
                // Shape 使用单位 position float2 quad。
                PipelineVertexLayout::PositionF32x2,
                // Shape 使用共享值对象 ABI。
                PipelineUniformLayout::Shape,
                // Shape 不读取纹理。
                PipelineSampling::None,
                // shader 已把分析 coverage 乘入 rgba。
                PipelineBlend::PremultipliedAlpha,
            ),
            // Additive Shape 只改变混合语义。
            Self::ShapeRectAdditive => ui_2d_pipeline_contract(
                // Additive Shape 保持相同顶点布局。
                PipelineVertexLayout::PositionF32x2,
                // Additive Shape 保持相同 uniform。
                PipelineUniformLayout::Shape,
                // Additive Shape 不读取纹理。
                PipelineSampling::None,
                // 仅目标混合切换为加法。
                PipelineBlend::Additive,
            ),
            // Shadow shader 输出 straight-alpha 颜色与覆盖率。
            Self::BoxShadow => ui_2d_pipeline_contract(
                // Shadow 复用单位 position float2 quad。
                PipelineVertexLayout::PositionF32x2,
                // Shadow 使用独立仿射常量语义。
                PipelineUniformLayout::Shadow,
                // Shadow 不读取纹理。
                PipelineSampling::None,
                // Shadow 与既有 D3D11 像素契约保持 straight-alpha。
                PipelineBlend::StraightAlpha,
            ),
            // Additive sampled quad 只改变混合语义。
            Self::TexturedQuadAdditive => ui_2d_pipeline_contract(
                // Additive 图片保持 float8 顶点。
                PipelineVertexLayout::PositionUvColorF32,
                // Additive 图片保持 viewport 常量。
                PipelineUniformLayout::Sampled,
                // Additive 图片仍读取四通道颜色。
                PipelineSampling::PremultipliedColor,
                // 目标混合切换为加法。
                PipelineBlend::Additive,
            ),
            // Blur pass 直接替换目标区域。
            Self::BlurPass => ui_2d_pipeline_contract(
                // Blur 使用共享几何生成的 NDC position 与绝对 source UV。
                PipelineVertexLayout::PositionUvF32,
                // Blur 使用固定高斯核常量。
                PipelineUniformLayout::Blur,
                // Blur 读取四通道颜色。
                PipelineSampling::PremultipliedColor,
                // 两阶段 blur 都完整替换目标区域。
                PipelineBlend::Replace,
            ),
            // MSDF shader 把解析 coverage 乘入输出颜色。
            Self::MsdfGlyphQuad => ui_2d_pipeline_contract(
                // MSDF 复用 sampled float8 顶点。
                PipelineVertexLayout::PositionUvColorF32,
                // MSDF 使用 atlas 尺寸与 range 常量。
                PipelineUniformLayout::Msdf,
                // MSDF 只接受 RGBA8 atlas。
                PipelineSampling::Msdf,
                // shader 已输出 premultiplied coverage。
                PipelineBlend::PremultipliedAlpha,
            ),
            // Sector shader 把分析 coverage 乘入颜色。
            Self::Sector => ui_2d_pipeline_contract(
                // Sector 使用单位 position float2 quad。
                PipelineVertexLayout::PositionF32x2,
                // Sector 使用四个 float4 常量。
                PipelineUniformLayout::Sector,
                // Sector 不读取纹理。
                PipelineSampling::None,
                // shader 已输出 premultiplied coverage。
                PipelineBlend::PremultipliedAlpha,
            ),
            // Line shader 把解析覆盖率乘入颜色。
            Self::LineSegment => ui_2d_pipeline_contract(
                // Line 使用单位 position float2 quad。
                PipelineVertexLayout::PositionF32x2,
                // 两端点、颜色与线宽复用四个 float4 的稳定 ABI。
                PipelineUniformLayout::Sector,
                // Line 不读取纹理。
                PipelineSampling::None,
                // shader 已输出 premultiplied coverage。
                PipelineBlend::PremultipliedAlpha,
            ),
        }
    }
}

// 定义 pipeline 创建描述，具体 shader 与原生对象只由 Adapter 映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineDesc {
    // 使用封闭类型标识通用 renderer 选定的 pipeline 语义。
    pub(crate) kind: PipelineKind,
}

#[cfg(test)]
#[path = "../../../../tests-src/platform/presentation/rhi/pipeline_tests.rs"]
mod pipeline_tests;
