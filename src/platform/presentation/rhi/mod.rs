//! platform System 的 API 无关薄 RHI 契约与共用机制。
//!
//! 本模块只描述 UIX 通用 GPU Renderer 需要的底层事实，不携带任何
//! `draw_glyphs`、`draw_rounded_rect` 或其他 UI 高层操作。资源表、pipeline ABI、
//! pass/surface 状态机、提交事务、错误与坐标语义只在此处定义一次；Vulkan、
//! D3D11、OpenGL 等 Adapter 只能实现原生映射。

#![allow(dead_code)]

// Drawing 的唯一向下入口同时暴露 recipe 选择值与 opaque owner；具体实现仍由
// native factory 创建，Drawing 不能越过本门面取得任何 Adapter 类型。
pub(crate) use super::{
    GpuRecipeOwner, GraphicsApi, GraphicsRecipe, GraphicsRecipeOwner, GraphicsSelection,
    NativeSurfaceHandle, PixelUploadRecipeOwner, PresentTestResult, describe_backend_availability,
    gpu_recipe_candidates, try_create_gpu_recipe_with_queue,
};
// 使用框架统一错误类型，保证 surface、device 和资源失败保持 typed error。
use crate::core::error::{Errc, Error, Result};
// 将 Shape 像素语义拆到独立共享契约文件，避免主 RHI 文件越过行数边界。
mod shape;
// 将 Gradient 仿射几何、颜色和模式参数收归共享 ABI 值对象。
mod gradient;
// 将 Shadow 仿射几何、颜色和软边参数收归共享 ABI 值对象。
mod shadow;
// 将 Blur 尺寸、区域、方向和高斯权重收归共享 ABI 值对象。
mod blur;
// 将 MSDF viewport、atlas extent 与距离范围收归共享 ABI 值对象。
mod msdf;
// 将 Mesh、sampled/coverage 与 Sector 固定常量收归共享基础图元 ABI。
mod primitive;
// 将 pipeline ABI 拆到独立共享契约文件，禁止 Drawing 与 Adapter 各自维护状态。
mod pipeline;
// 将顶点属性语义、格式、位置和偏移拆到独立共享 ABI 值对象。
mod vertex_layout;
// 将 extent、viewport 与 scissor 的共同原生投影拆到独立共享几何 Component。
mod geometry;
// 将 sample coverage 状态拆到独立共享契约，禁止 Surface 与 Adapter 各自选择。
mod multisample;
// 将颜色编码与混合值域拆到独立契约，禁止 Adapter 启用隐藏颜色转换。
mod color;
// 显式 parity feature 才编译 API 中立测试端口，不进入生产合同表面。
#[cfg(feature = "graphics-parity-test")]
mod parity;
// 将 Device 与 Surface 能力拆到独立契约，禁止两种角色反向读取彼此状态。
mod capabilities;
// 将 draw packet 拆到独立类型化契约文件，保持 pipeline 身份不可拆分。
mod draw_packet;
// 将 Draw 的条件采样资源从 pass 状态中独立为共享值对象。
mod draw_sampling;
// 将每次 Draw 的 viewport 与 scissor 独立为共享动态栅格值对象。
mod draw_raster;
// 将 render-pass 生命周期和资源冲突门禁收归共享状态机，禁止 Adapter 各自解释顺序。
mod pass_state;
// 将纹理复制与移动的格式、范围和同资源规则收归共享契约。
mod transfer;
// Buffer Component 统一描述、上传语义与两个原生 API 的值域投影。
mod buffer;
// Texture Component 统一格式布局、资源描述和共同原生尺寸投影。
mod texture;
// ResourceTable Component 统一不透明资源句柄的分配、查询与检查式销毁语义。
mod resource_table;
// 将 texture 资源描述与 render-target 能力提升收归共享资源表。
mod texture_resource_table;
// 将 pipeline 资源与共享 kind 绑定，禁止 Adapter 私自组合句柄语义。
mod pipeline_resource_table;
// 将 Buffer 资源描述与 Draw 角色验证收归共享资源表。
mod buffer_resource_table;
// PresentTransaction Component 绑定 acquire、submit 与 damage 并统一 Surface 门禁。
mod present_transaction;
// SurfaceResizeTransaction Component 统一 resize 前置值域与成功后的 token 门禁。
mod resize_transaction;
// SurfaceLifecycle Component 统一原生重建、代际推进与当前帧收尾语义。
mod surface_lifecycle;
// RenderTarget Component 封闭 Surface 与 texture 两种互斥目标身份。
mod render_target;
// 向 Drawing System 与各原生 Adapter 暴露同一份类型化 pipeline 契约。
#[allow(unused_imports)]
pub(crate) use pipeline::{
    PIPELINE_DEPTH_STENCIL_DISABLED, PIPELINE_RASTER_2D, PipelineBinding, PipelineBlend,
    PipelineBlendFactor, PipelineBlendOperation, PipelineBlendState, PipelineColorWriteMask,
    PipelineContract, PipelineCullMode, PipelineDepthClip, PipelineDepthState,
    PipelineDepthStencilState, PipelineDesc, PipelineDitherState, PipelineFrontFace, PipelineKind,
    PipelinePrimitiveTopology, PipelineRasterState, PipelineSampling, PipelineStencilState,
    PipelineUniformLayout,
};
// 向 pipeline 与两个 Adapter 暴露唯一类型化顶点输入布局。
pub(crate) use vertex_layout::{
    PIPELINE_VERTEX_ATTRIBUTE_SLOT_COUNT, PipelineVertexFormat, PipelineVertexLayout,
    PipelineVertexSemantic,
};
// 向 Drawing、Surface、pass 与各 Adapter 暴露唯一目标几何值对象。
pub(crate) use geometry::{RhiExtent, RhiScissor, RhiViewport};
// 向 pipeline、Surface 配方和两个 Adapter 暴露同一采样覆盖事实。
pub(crate) use multisample::PipelineMultisampleState;
// 向 Drawing、Surface 与原生 Adapter 暴露唯一颜色与清理输出解释。
pub(crate) use color::{
    RhiColorClearContract, RhiColorContract, UIX_COLOR_CLEAR_CONTRACT, UIX_COLOR_CONTRACT,
};
// 向显式 parity 组合根与原生 Adapter 暴露同一套测试端口。
#[cfg(feature = "graphics-parity-test")]
pub(crate) use parity::{
    HeadlessUiParityAdapter, WsiParityAdapter, WsiParityFramePresenter, WsiParityProfile,
};
// 向组合根和 Adapter 暴露两个正交的事实快照。
pub(crate) use capabilities::{GraphicsDeviceCapabilities, GraphicsSurfaceCapabilities};
// 向 FramePlan 与 Adapter 暴露类型化 pipeline 与索引格式绑定的绘制包。
#[allow(unused_imports)]
pub(crate) use draw_packet::{
    DrawBufferBindings, DrawPacket, DrawRange, IndexBufferBinding, IndexFormat,
};
// 向 Drawing、FramePlan 与 Adapter 暴露唯一条件采样资源契约。
pub(crate) use draw_sampling::{DrawSamplingBinding, SampledTextureBinding};
// 向 Drawing、FramePlan 与 Adapter 暴露唯一 Draw 动态栅格状态。
pub(crate) use draw_raster::DrawRasterState;
// 向各原生 Adapter 暴露唯一的 render-pass 状态事实。
#[allow(unused_imports)]
pub(crate) use pass_state::RhiPassState;
// 向 Drawing、FramePlan 与各 Adapter 暴露唯一类型化纹理区域和传输契约。
#[allow(unused_imports)]
pub(crate) use transfer::{
    RhiTextureOrigin, RhiTextureRegion, RhiTextureRegionBounds, RhiTextureTransfer,
    RhiTextureTransferBounds, RhiTextureUpload, TextureCopy, TextureMove,
    ValidatedRhiTextureUpload,
};
// 重新导出薄 RHI 消费的类型化 Buffer 契约。
#[allow(unused_imports)]
pub(crate) use buffer::{BufferDesc, BufferUsage, RhiBufferUpload, RhiBufferUploadPreflight};
// 重新导出薄 RHI 消费的封闭纹理描述与格式契约。
pub(crate) use texture::{TextureDesc, TextureFormat};
// 向两个 Adapter 暴露唯一类型化资源槽位状态机。
pub(crate) use resource_table::{RhiResourceHandle, RhiResourceTable};
// 向 Adapter 与 FramePlan 暴露唯一 texture 资源表及描述投影。
pub(crate) use texture_resource_table::{RhiTextureResource, RhiTextureResourceTable};
// 向两个 Adapter 暴露唯一的 pipeline 资源表组件。
pub(crate) use pipeline_resource_table::RhiPipelineResourceTable;
// 向两个 Adapter 与 FramePlan 执行器暴露唯一的 Buffer 资源表组件。
pub(crate) use buffer_resource_table::{RhiBufferResource, RhiBufferResourceTable};
// 向 FramePlan 与两个 Adapter 暴露不可拆的 Surface 呈现事务。
pub(crate) use present_transaction::{RhiPresentTransaction, SurfaceFrame, ValidatedRhiPresent};
// 向两个 Surface Adapter 暴露唯一的类型化 resize 事务。
pub(crate) use resize_transaction::RhiSurfaceResizeTransaction;
// 向 Surface Adapter 暴露唯一的 API 无关重建状态机与事务结果。
pub(crate) use surface_lifecycle::{
    RhiSurfaceLifecycle, RhiSurfaceRecreateCommit, RhiSurfaceRecreateReason,
    RhiSurfaceRecreateTransaction,
};
// 显式 Vulkan 验证 feature 复用同一共享状态机，不复制测试实现。
#[cfg(feature = "vulkan-parity-test")]
pub(crate) use surface_lifecycle::run_surface_lifecycle_contract_test;
// 向 FramePlan、pass 状态与 Adapter 暴露不含裸哨兵的目标身份。
pub(crate) use render_target::RenderTargetHandle;
// 向 Drawing System 暴露唯一 Gradient 常量构造器和固定字节数。
pub(crate) use gradient::{GRADIENT_UNIFORM_BYTES, RhiGradientRasterParams};
// 只有 OpenGL Adapter 需要把共享 Gradient 字节 ABI 映射为逐个原生 uniform。
#[cfg(feature = "opengles")]
pub(crate) use gradient::{
    GRADIENT_COLOR_A_FLOAT_OFFSET, GRADIENT_COLOR_B_FLOAT_OFFSET, GRADIENT_EDGE_Y_FLOAT_OFFSET,
    GRADIENT_ORIGIN_EDGE_X_FLOAT_OFFSET, GRADIENT_PARAMS_FLOAT_OFFSET,
    GRADIENT_VIEWPORT_FLOAT_OFFSET,
};
// 向 Drawing System 与 probe 暴露同一 Shape ABI 值对象和总尺寸。
pub(crate) use shape::{RhiShapeRasterParams, SHAPE_UNIFORM_BYTES};
// 向 Drawing System 暴露唯一 Shadow 常量构造器和固定字节数。
pub(crate) use shadow::{RhiShadowRasterParams, SHADOW_UNIFORM_BYTES};
// 只有 OpenGL Adapter 需要把共享 Shadow 字节 ABI 映射为逐个原生 uniform。
#[cfg(feature = "opengles")]
pub(crate) use shadow::{
    SHADOW_BODY_SIZE_AMBIENT_FLOAT_OFFSET, SHADOW_COLOR_FLOAT_OFFSET,
    SHADOW_EDGE_Y_BLUR_FLOAT_OFFSET, SHADOW_ORIGIN_EDGE_X_FLOAT_OFFSET, SHADOW_RADIUS_FLOAT_OFFSET,
    SHADOW_VIEWPORT_FLOAT_OFFSET,
};
// 向 Drawing System 暴露唯一 Blur 几何门禁、方向、常量和固定字节数。
pub(crate) use blur::{
    BLUR_UNIFORM_BYTES, BLUR_WEIGHT_COUNT, RhiBlurDirection, RhiBlurPassGeometry,
    RhiBlurRasterParams,
};
// OpenGL Adapter 与共享契约测试需要按字段核对 Blur 字节 ABI。
#[cfg(any(feature = "opengles", test))]
// 不同构建只消费当前 Adapter 需要的字段，统一重导出仍保留完整共享契约。
#[allow(unused_imports)]
pub(crate) use blur::{
    BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET, BLUR_UV_BOUNDS_FLOAT_OFFSET, BLUR_WEIGHTS_FLOAT_OFFSET,
};
// 向 Drawing System 暴露唯一 MSDF 常量构造器和固定字节数。
pub(crate) use msdf::{MSDF_UNIFORM_BYTES, RhiMsdfRasterParams};
// OpenGL Adapter 与共享契约测试需要按字段核对 MSDF 字节 ABI。
#[cfg(any(feature = "opengles", test))]
// 不同构建只消费当前 Adapter 需要的字段，统一重导出仍保留完整共享契约。
#[allow(unused_imports)]
pub(crate) use msdf::{
    MSDF_RANGE_FLOAT_OFFSET, MSDF_TEXTURE_SIZE_FLOAT_OFFSET, MSDF_VIEWPORT_FLOAT_OFFSET,
};
// 向 Drawing System 暴露基础图元值对象和各自固定字节数。
pub(crate) use primitive::{
    MESH_UNIFORM_BYTES, RhiMeshRasterParams, RhiSampledRasterParams, RhiSectorRasterParams,
    SAMPLED_UNIFORM_BYTES, SECTOR_UNIFORM_BYTES,
};
// OpenGL Adapter 与共享契约测试需要按字段核对基础图元字节 ABI。
#[cfg(any(feature = "opengles", test))]
// 不同构建只消费当前 Adapter 需要的字段，统一重导出仍保留完整共享契约。
#[allow(unused_imports)]
pub(crate) use primitive::{
    MESH_COLOR_FLOAT_OFFSET, MESH_VIEWPORT_FLOAT_OFFSET, SAMPLED_CORNER_RADIUS_FLOAT_OFFSET,
    SAMPLED_SHADOW_ALPHA_BL_FLOAT_OFFSET, SAMPLED_SHADOW_ALPHA_BR_FLOAT_OFFSET,
    SAMPLED_SHADOW_ALPHA_TL_FLOAT_OFFSET, SAMPLED_SHADOW_ALPHA_TR_FLOAT_OFFSET,
    SAMPLED_SHADOW_RANGE_FLOAT_OFFSET, SAMPLED_VIEWPORT_FLOAT_OFFSET, SECTOR_ANGLES_FLOAT_OFFSET,
    SECTOR_COLOR_FLOAT_OFFSET, SECTOR_RECT_FLOAT_OFFSET, SECTOR_VIEWPORT_FLOAT_OFFSET,
};
// 只有 OpenGL Adapter 需要把字节 ABI 解码为逐个原生 uniform 字段。
#[cfg(feature = "opengles")]
pub(crate) use shape::{
    SHAPE_COLOR_FLOAT_OFFSET, SHAPE_DRAW_RECT_FLOAT_OFFSET, SHAPE_RADIUS_FLOAT_OFFSET,
    SHAPE_RECT_FLOAT_OFFSET, SHAPE_STROKE_FLOAT_OFFSET, SHAPE_VIEWPORT_FLOAT_OFFSET,
};

// 描述 surface 的代际和 drawable extent，隔离重建后的迟到命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SurfaceToken {
    // 保存 surface 重建序号。
    pub(crate) generation: u64,
    // 保存该代 surface 的物理 extent。
    pub(crate) extent: RhiExtent,
}

// 为 surface token 提供集中构造入口。
impl SurfaceToken {
    // 创建一个带代际的 surface token。
    pub(crate) const fn new(generation: u64, extent: RhiExtent) -> Self {
        // 保留原始代际和尺寸，代际检查由执行边界负责。
        Self { generation, extent }
    }
}

// 定义不透明的资源句柄，具体 API 对象只能存在 native adapter 内部。
macro_rules! opaque_handle {
    ($name:ident) => {
        // 资源句柄只携带 adapter 可解释的整数身份。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) struct $name(u64);

        // 资源句柄只允许 RHI 内部和测试适配器从原始值构造。
        impl $name {
            // 创建一个测试或 adapter 分配的句柄。
            pub(crate) const fn from_raw(raw: u64) -> Self {
                // 不在通用层解释原始资源身份。
                Self(raw)
            }

            // 读取句柄身份供日志和代际审计使用。
            pub(crate) const fn raw(self) -> u64 {
                // 返回不透明身份的数值表示。
                self.0
            }
        }
    };
}

// 为真正拥有 Adapter 资源槽位的句柄补充共享表身份。
macro_rules! opaque_resource_handle {
    ($name:ident, $kind:literal) => {
        // 先生成与其它不透明身份一致的公开形状。
        opaque_handle!($name);

        // 只允许共享资源表解释该句柄的槽位数值。
        impl RhiResourceHandle for $name {
            // 保存稳定的资源种类诊断名称。
            const KIND: &'static str = $kind;

            // 由共享资源表签发从一开始递增的身份。
            fn from_resource_raw(raw: u64) -> Self {
                // 复用同一不透明句柄构造入口。
                Self::from_raw(raw)
            }

            // 只向共享资源表暴露槽位身份。
            fn resource_raw(self) -> u64 {
                // 复用同一只读原始值投影。
                self.raw()
            }
        }
    };
}

// 声明动态 buffer 句柄。
opaque_resource_handle!(BufferHandle, "buffer");
// 声明采样纹理句柄。
opaque_resource_handle!(TextureHandle, "texture");
// 声明采样器句柄。
opaque_resource_handle!(SamplerHandle, "sampler");
// 声明固定 pipeline 句柄。
opaque_resource_handle!(PipelineHandle, "pipeline");
// 声明一次 submit 返回的提交序号。
opaque_handle!(SubmissionHandle);
// 将提交身份签发和最新值校验收归 API 无关的共享状态机。
mod submission;
// 向各原生 Adapter 暴露同一份 Device submit 与 Surface present 关联契约。
#[allow(unused_imports)]
pub(crate) use submission::RhiSubmissionSequence;

// 定义采样器的 min/mag 过滤语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SamplerFilter {
    // 保持离散 texel，不跨相邻像素插值。
    Nearest,
    // 在相邻 texel 之间执行线性插值。
    Linear,
}

// 定义二维纹理坐标越界时的地址语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SamplerAddressMode {
    // 把 U/V 坐标限制到纹理边缘 texel。
    ClampToEdge,
}

// 定义当前单级纹理资源允许的 mip 采样语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SamplerMipMode {
    // 只允许采样创建时唯一存在的第零级纹理。
    SingleLevel,
}

// 定义不能被调用方拆散修改的完整 sampler 创建描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SamplerDesc {
    // 保存 min/mag 共用的过滤语义。
    filter: SamplerFilter,
    // 保存二维纹理两个轴共用的地址语义。
    address_mode: SamplerAddressMode,
    // 保存单级纹理的 mip 选择语义。
    mip_mode: SamplerMipMode,
}

// 为有限 sampler 契约提供不含裸布尔值的命名构造器。
impl SamplerDesc {
    // 创建不使用 mip 过滤的线性 clamp sampler。
    pub(crate) const fn linear_clamp() -> Self {
        // 三项事实必须作为一个不可拆描述进入资源生命周期。
        Self {
            // 图片、MSDF 与 Blur 使用线性 min/mag 过滤。
            filter: SamplerFilter::Linear,
            // 所有二维采样都固定限制到边缘 texel。
            address_mode: SamplerAddressMode::ClampToEdge,
            // 当前纹理资源只创建并采样第零级。
            mip_mode: SamplerMipMode::SingleLevel,
        }
    }

    // 创建不使用 mip 过滤的最近点 clamp sampler。
    pub(crate) const fn nearest_clamp() -> Self {
        // 仅过滤语义不同，地址与 mip 事实保持统一。
        Self {
            // Coverage atlas 必须保持离散 texel。
            filter: SamplerFilter::Nearest,
            // 所有二维采样都固定限制到边缘 texel。
            address_mode: SamplerAddressMode::ClampToEdge,
            // 当前纹理资源只创建并采样第零级。
            mip_mode: SamplerMipMode::SingleLevel,
        }
    }

    // 返回原生 Adapter 必须穷尽映射的过滤语义。
    pub(crate) const fn filter(self) -> SamplerFilter {
        // 复制构造时冻结的封闭枚举值。
        self.filter
    }

    // 返回原生 Adapter 必须穷尽映射的地址语义。
    pub(crate) const fn address_mode(self) -> SamplerAddressMode {
        // 复制构造时冻结的封闭枚举值。
        self.address_mode
    }

    // 返回原生 Adapter 必须穷尽映射的 mip 语义。
    pub(crate) const fn mip_mode(self) -> SamplerMipMode {
        // 复制构造时冻结的封闭枚举值。
        self.mip_mode
    }
}

// 定义 premultiplied-alpha 颜色值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiColor([f32; 4]);

// 为颜色值提供唯一构造、读取与不变量校验。
impl RhiColor {
    // 构造不依赖任何颜色输入的透明预乘黑色。
    pub(crate) const fn transparent() -> Self {
        // alpha 为零时 RGB 也必须为零。
        Self([0.0, 0.0, 0.0, 0.0])
    }

    // 把 straight-alpha RGBA 转换成 render target 使用的预乘值。
    pub(crate) fn from_straight_rgba(rgba: [f32; 4]) -> Self {
        // alpha 保持直通，三个颜色通道只在共享契约层乘一次。
        let alpha = rgba[3];
        // 不裁剪非法输入，后续 FramePlan 校验必须显式拒绝。
        Self([
            // 红色进入目标前乘源透明度。
            rgba[0] * alpha,
            // 绿色进入目标前乘源透明度。
            rgba[1] * alpha,
            // 蓝色进入目标前乘源透明度。
            rgba[2] * alpha,
            // alpha 自身保持不变。
            alpha,
        ])
    }

    // 接收已经由上层证明为预乘值的内部颜色。
    pub(crate) const fn from_premultiplied_rgba(rgba: [f32; 4]) -> Self {
        // 值仍必须在 FramePlan 或 Adapter 边界通过 is_valid。
        Self(rgba)
    }

    // 返回供原生 Adapter 机械编码的预乘通道副本。
    pub(crate) const fn components(self) -> [f32; 4] {
        // 数组只有四个浮点，复制不会建立跨边界借用。
        self.0
    }

    // 判断四通道是否满足有限、规范范围和预乘 alpha 不变量。
    pub(crate) fn is_valid(self) -> bool {
        // 解构稳定通道顺序供范围与 alpha 关系检查。
        let [red, green, blue, alpha] = self.0;
        // 所有通道必须落在规范 Unorm 范围。
        [red, green, blue, alpha]
            // 逐通道检查有限性与闭区间。
            .iter()
            // NaN、无穷、负值和大于一都必须拒绝。
            .all(|channel| channel.is_finite() && (0.0..=1.0).contains(channel))
            // 预乘颜色的任一 RGB 都不得大于 alpha。
            && red <= alpha
            // 绿色遵循同一预乘约束。
            && green <= alpha
            // 蓝色遵循同一预乘约束。
            && blue <= alpha
    }
}

// 保存已经规范化为左上原点与 0xAARRGGBB 的 surface 回读结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RhiSurfaceReadback {
    // 保存回读区域在当前 surface 中的物理坐标。
    pub(crate) region: RhiScissor,
    // 保存按顶部到底部、每行从左到右紧密排列的规范像素。
    pixels: Vec<u32>,
}

// 为跨 Adapter 回读结果集中验证范围、长度与像素布局边界。
impl RhiSurfaceReadback {
    // 在进入原生 API 前验证区域完整落在当前 surface 内。
    pub(crate) fn validate_region(region: RhiScissor, surface: RhiExtent) -> Result<()> {
        // 负坐标、空区域和负尺寸不得交给各 Adapter 自行裁切。
        if !region.is_valid() {
            // 使用统一参数错误避免 OpenGL 与 D3D11 产生不同结果。
            return Err(Error::new(
                // 回读请求由调用方构造，因此使用无效参数分类。
                Errc::InvalidArgument,
                // 保留稳定的 RHI 层诊断文本。
                "RHI surface readback region must be positive",
            ));
        }
        // 已验证非负后把横坐标转换为无符号物理坐标。
        let x = region.x as u32;
        // 已验证非负后把纵坐标转换为无符号物理坐标。
        let y = region.y as u32;
        // 已验证为正后把宽度转换为无符号物理尺寸。
        let width = region.width as u32;
        // 已验证为正后把高度转换为无符号物理尺寸。
        let height = region.height as u32;
        // 使用检查式加法拒绝横向坐标溢出。
        let right = x.checked_add(width).ok_or_else(|| {
            // 溢出仍属于无效回读区域。
            Error::new(Errc::InvalidArgument, "RHI surface readback x overflows")
        })?;
        // 使用检查式加法拒绝纵向坐标溢出。
        let bottom = y.checked_add(height).ok_or_else(|| {
            // 溢出仍属于无效回读区域。
            Error::new(Errc::InvalidArgument, "RHI surface readback y overflows")
        })?;
        // 两个轴都必须完整落在当前 surface extent 内。
        if right > surface.width || bottom > surface.height {
            // 禁止 Adapter 静默裁切或依赖驱动未定义行为。
            return Err(Error::new(
                // 越界请求由调用方修正。
                Errc::InvalidArgument,
                // 使用 API 无关的稳定诊断。
                "RHI surface readback region is out of range",
            ));
        }
        // 区域满足跨 Adapter 统一前置条件。
        Ok(())
    }

    // 从 Adapter 已规范化的像素构造完整回读结果。
    pub(crate) fn try_new(
        // 接收已经请求的左上原点区域。
        region: RhiScissor,
        // 接收读取时观察到的 surface extent。
        surface: RhiExtent,
        // 接收按 0xAARRGGBB 编码的紧密像素。
        pixels: Vec<u32>,
    ) -> Result<Self> {
        // 先复用唯一范围验证，禁止结果描述越过 surface。
        Self::validate_region(region, surface)?;
        // 使用检查式乘法计算契约要求的像素总数。
        let expected = (region.width as usize)
            // 乘以正数高度得到紧密载荷长度。
            .checked_mul(region.height as usize)
            // 极端尺寸溢出必须保持 typed failure。
            .ok_or_else(|| {
                // 载荷尺寸由请求决定，归类为无效参数。
                Error::new(
                    Errc::InvalidArgument,
                    "RHI surface readback payload length overflows",
                )
            })?;
        // Adapter 不得返回空载荷、裁切载荷或带行距载荷。
        if pixels.len() != expected {
            // 长度不匹配属于 Adapter 契约破坏而非成功的部分结果。
            return Err(Error::new(
                // 使用平台错误标记下层实现违约。
                Errc::PlatformError,
                // 保留不携带具体 API 名称的稳定诊断。
                "RHI surface readback payload length does not match its region",
            ));
        }
        // 返回已经满足统一行序、通道和长度约束的结果。
        Ok(Self { region, pixels })
    }

    // 消耗回读结果并交付规范像素所有权。
    pub(crate) fn into_pixels(self) -> Vec<u32> {
        // 不复制全帧载荷地返回像素。
        self.pixels
    }
}

// 描述一次 render pass 的加载动作。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LoadAction {
    // 用统一 premultiplied-alpha 颜色清理目标。
    Clear(RhiColor),
    // 保留目标已有内容。
    Load,
}

// 定义 GPU device 的薄原语接口。
pub(crate) trait GraphicsDevice {
    // 返回本 device 的事实型能力快照。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities;

    // 创建动态 buffer，具体内存类型由 adapter 选择。
    fn create_buffer(&mut self, _desc: BufferDesc) -> Result<BufferHandle> {
        // 默认实现显式拒绝，防止 adapter 漏实现时静默成功。
        Err(rhi_not_implemented("create_buffer"))
    }

    // 上传 buffer 内容，不暴露映射指针或 API 状态。
    fn update_buffer(&mut self, _upload: RhiBufferUpload<'_>) -> Result<()> {
        // 默认实现显式拒绝，要求真实 adapter 提供上传能力。
        Err(rhi_not_implemented("update_buffer"))
    }

    // 在激活 Device 前预检 Buffer 上传的真实句柄、用途、元素 ABI 与范围。
    fn preflight_buffer_upload(&self, _upload: RhiBufferUploadPreflight) -> Result<()> {
        // 默认实现显式拒绝未声明共享 Buffer 资源表能力的 Adapter。
        Err(rhi_not_implemented("preflight_buffer_upload"))
    }

    // 创建 sampled texture 或 render target texture。
    fn create_texture(&mut self, _desc: TextureDesc) -> Result<TextureHandle> {
        // 默认实现显式拒绝，防止能力声明和实现脱节。
        Err(rhi_not_implemented("create_texture"))
    }

    // 在共享资源描述证明后提升 texture 为 render-target 身份。
    fn resolve_render_target(&self, _texture: TextureHandle) -> Result<RenderTargetHandle> {
        // 默认实现拒绝未声明资源表能力的 Adapter。
        Err(rhi_not_implemented("resolve_render_target"))
    }

    // 在激活 Device 前预检普通 texture copy 的资源与传输契约。
    fn preflight_texture_copy(&self, _copy: TextureCopy) -> Result<()> {
        // 默认实现拒绝未声明共享 texture 资源表能力的 Adapter。
        Err(rhi_not_implemented("preflight_texture_copy"))
    }

    // 在激活 Device 前预检 texture move 的资源与传输契约。
    fn preflight_texture_move(&self, _movement: TextureMove) -> Result<()> {
        // 默认实现拒绝未声明共享 texture 资源表能力的 Adapter。
        Err(rhi_not_implemented("preflight_texture_move"))
    }

    // 在激活 Device 前预检 DrawPacket 所有真实资源角色与容量。
    fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
        // 默认实现显式拒绝未声明完整 Draw 资源表能力的 Adapter。
        Err(rhi_not_implemented("preflight_draw_resources"))
    }

    // 上传一个已经绑定纹理身份、区域与紧密像素载荷的命令。
    fn update_texture(&mut self, _upload: RhiTextureUpload<'_>) -> Result<()> {
        // 默认实现显式拒绝，避免把空上传当作成功。
        Err(rhi_not_implemented("update_texture"))
    }

    // 创建采样器。
    fn create_sampler(&mut self, _desc: SamplerDesc) -> Result<SamplerHandle> {
        // 默认实现显式拒绝，要求真实 adapter 绑定采样语义。
        Err(rhi_not_implemented("create_sampler"))
    }

    // 创建通用 pipeline。
    fn create_pipeline(&mut self, _desc: PipelineDesc) -> Result<PipelineBinding> {
        // 默认实现显式拒绝，具体 shader 二进制仍留在 adapter。
        Err(rhi_not_implemented("create_pipeline"))
    }

    // 销毁 buffer，并把失败保留为 typed error。
    fn destroy_buffer(&mut self, _buffer: BufferHandle) -> Result<()> {
        // 默认实现显式拒绝，资源所有者必须实现检查式销毁。
        Err(rhi_not_implemented("destroy_buffer"))
    }

    // 销毁 texture，并把失败保留为 typed error。
    fn destroy_texture(&mut self, _texture: TextureHandle) -> Result<()> {
        // 默认实现显式拒绝，不能用丢弃句柄冒充资源释放。
        Err(rhi_not_implemented("destroy_texture"))
    }

    // 销毁 sampler，并把失败保留为 typed error。
    fn destroy_sampler(&mut self, _sampler: SamplerHandle) -> Result<()> {
        // 默认实现显式拒绝，避免泄漏被隐藏。
        Err(rhi_not_implemented("destroy_sampler"))
    }

    // 销毁 pipeline，并把失败保留为 typed error。
    fn destroy_pipeline(&mut self, _pipeline: PipelineBinding) -> Result<()> {
        // 默认实现显式拒绝，保证 adapter 明确声明生命周期。
        Err(rhi_not_implemented("destroy_pipeline"))
    }

    // 开始一个有序 render pass。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()>;

    // 在当前 pass 的指定整数区域内清理颜色，不建立后续 Draw 状态。
    fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
        // 默认 adapter 必须显式声明局部清理原语，不能静默忽略 damage。
        Err(rhi_not_implemented("clear_rect"))
    }

    // 执行一个已经完成高层降级的 draw packet。
    fn draw(&mut self, packet: DrawPacket) -> Result<()>;

    // 在 pass 外执行一次纹理复制。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()>;

    // 在 pass 外移动纹理区域，并保证同一纹理的重叠区域按 memmove 语义执行。
    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // 不同纹理可以复用普通 copy；同一纹理必须由 adapter 提供重叠安全实现。
        if movement.source() != movement.destination() {
            // 将不同纹理的完整类型化传输降级为已有 copy 原语。
            return self.copy_texture(movement.into_copy());
        }
        // 同一纹理的重叠移动不能交给普通 copy 猜测覆盖顺序。
        Err(rhi_not_implemented("move_texture_region"))
    }

    // 结束当前 render pass。
    fn end_render_pass(&mut self) -> Result<()>;

    // 提交当前 device 命令并返回提交身份。
    fn submit(&mut self) -> Result<SubmissionHandle>;

    // 激活当前 owner-thread 的原生 device context，但不检查设备健康或提交命令。
    fn activate(&mut self) -> Result<()> {
        // 不依赖隐式线程 current 状态的 adapter 可以安全保持无操作。
        Ok(())
    }

    // 在已经激活的 device context 上进行非破坏性健康维护。
    fn maintain(&mut self) -> Result<()> {
        // 没有维护工作的 adapter 可以安全返回成功。
        Ok(())
    }

    // test-harness 只允许 adapter 在 owner thread 安排一次可控设备丢失。
    #[cfg(feature = "test-harness")]
    fn inject_device_lost_for_test(&mut self) -> Result<()> {
        // 非参考 adapter 默认保持旧的恢复包装器回退语义。
        Err(rhi_not_implemented("inject_device_lost_for_test"))
    }
}

// 定义 surface 的薄 acquire/resize/present 接口。
pub(crate) trait GraphicsSurface {
    // 返回只属于 acquire、readback 与 present 的 Surface 能力快照。
    fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities {
        // 未声明可选操作的 Surface 默认不伪造任何能力。
        GraphicsSurfaceCapabilities::default()
    }

    // 返回当前 surface 的代际和 extent。
    fn token(&self) -> SurfaceToken;

    // 返回平台窗口外观已归一化后的物理圆角半径。
    fn surface_corner_radius(&self) -> f32 {
        // 系统装饰或不支持透明窗口的 surface 默认无需额外遮罩。
        0.0
    }

    // 返回客户端阴影环外观事实 (逐角补画峰值 [左上,右上,左下,右下], 物理衰减距离)。
    fn surface_shadow_fill(&self) -> ([f32; 4], f32) {
        // 只有平台窗口层确实绘制阴影环时才需要合成端补画圆角缺口。
        ([0.0; 4], 0.0)
    }

    // 获取当前可呈现 image，不提交任何命令。
    fn acquire(&mut self) -> Result<SurfaceFrame>;

    // 帧在最终 present 前失败时，释放 acquire 产生的原生所有权。
    fn discard_acquired_frame(&mut self, _frame: SurfaceFrame) -> Result<()> {
        // 不显式持有 image 的 adapter 无需执行原生回滚。
        Ok(())
    }

    // 重建 surface 并推进代际。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken>;

    // 可选地同步读取当前 surface 的像素，默认返回 typed 未实现错误。
    fn read_surface_pixels(
        // 借用 owner-thread surface。
        &mut self,
        // 接收已经使用左上原点表达的物理区域。
        _region: RhiScissor,
    ) -> Result<RhiSurfaceReadback> {
        // 未声明能力的 adapter 不得伪造空像素结果。
        Err(rhi_not_implemented("read_surface_pixels"))
    }

    // 将一次不可拆的 Device submit 事务最终呈现到原生窗口。
    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()>;

    // 探测已经遮挡的 surface 是否可恢复呈现，不提交任何帧数据。
    fn test_present(&mut self) -> Result<PresentTestResult> {
        // 未声明该低层能力的 surface 必须返回 typed 未实现错误。
        Err(rhi_not_implemented("test_present"))
    }

    // 进行 surface 级维护，不产生新的 frame。
    fn maintain(&mut self) -> Result<()> {
        // 没有维护工作的 adapter 可以安全返回成功。
        Ok(())
    }

    // test-harness 只允许 adapter 在 owner thread 安排一次可控 surface lost。
    #[cfg(feature = "test-harness")]
    fn inject_surface_lost_for_test(&mut self) -> Result<()> {
        // 非参考 adapter 默认保持恢复包装器兼容回退语义。
        Err(rhi_not_implemented("inject_surface_lost_for_test"))
    }
}

// 组合 owner-thread device 与 surface，只供完整帧事务在 composition root 借用。
pub(crate) trait GraphicsContextRhi {
    // 返回只允许读取资源、命令与提交能力的 device 视图。
    fn device_ref(&self) -> &dyn GraphicsDevice;

    // 返回只允许资源、命令和 submit 操作的 device 视图。
    fn device(&mut self) -> &mut dyn GraphicsDevice;

    // 返回只允许读取 acquire、resize 与 present 状态的 surface 视图。
    fn surface_ref(&self) -> &dyn GraphicsSurface;

    // 返回只允许 acquire、resize 和 present 操作的 surface 视图。
    fn surface(&mut self) -> &mut dyn GraphicsSurface;
}

// 任何同时实现 device 和 surface 原语的原生 context 都自动获得组合视图。
impl<T> GraphicsContextRhi for T
where
    // 真实 Adapter context 在 owner thread 同时实现两个正交角色。
    T: GraphicsDevice + GraphicsSurface,
{
    // 把组合 owner 收窄为只读 device 角色。
    fn device_ref(&self) -> &dyn GraphicsDevice {
        // 不复制资源表或原生 context，只限制只读调用能力。
        self
    }

    // 把组合 owner 收窄为 device 角色。
    fn device(&mut self) -> &mut dyn GraphicsDevice {
        // 不复制资源表或原生 context，只限制调用能力。
        self
    }

    // 把组合 owner 收窄为只读 surface 角色。
    fn surface_ref(&self) -> &dyn GraphicsSurface {
        // 不复制 swapchain 状态，只限制只读调用能力。
        self
    }

    // 把组合 owner 收窄为 surface 角色。
    fn surface(&mut self) -> &mut dyn GraphicsSurface {
        // 不复制 swapchain 状态，只限制调用能力。
        self
    }
}

// 统一生成 RHI 尚未实现的 typed error。
fn rhi_not_implemented(operation: &'static str) -> Error {
    // 用稳定前缀区分底层契约缺口和上层 UI 操作缺口。
    Error::new(
        Errc::NotImplemented,
        format!("thin RHI operation is not implemented: {operation}"),
    )
}
