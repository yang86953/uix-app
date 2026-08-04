//! platform 图形系统的薄 RHI 契约。
//!
//! 本模块只描述 UIX 通用 GPU Renderer 需要的底层事实，不携带任何
//! `draw_glyphs`、`draw_rounded_rect` 或其他 UI 高层操作。

#![allow(dead_code)]

// 使用框架统一错误类型，保证 surface、device 和资源失败保持 typed error。
use crate::core::error::{Errc, Error, Result};
// 使用统一的最终呈现 damage 值，RHI 不重新定义呈现损坏语义。
use crate::core::PresentDamage;

// 定义 GPU 资源尺寸，避免把平台 API 的 extent 类型泄漏到通用层。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RhiExtent {
    // 保存资源宽度，零值只允许在构造前的临时描述中出现。
    pub(crate) width: u32,
    // 保存资源高度，零值只允许在构造前的临时描述中出现。
    pub(crate) height: u32,
}

// 为资源尺寸提供一个不携带平台状态的值构造器。
impl RhiExtent {
    // 创建资源尺寸值。
    pub(crate) const fn new(width: u32, height: u32) -> Self {
        // 返回调用方提供的尺寸，不在 RHI 值层偷偷修正无效输入。
        Self { width, height }
    }

    // 判断尺寸是否可以进入资源或 surface 生命周期。
    pub(crate) const fn is_positive(self) -> bool {
        // GPU 资源和呈现 surface 都要求两个轴严格大于零。
        self.width != 0 && self.height != 0
    }
}

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

// 声明动态 buffer 句柄。
opaque_handle!(BufferHandle);
// 声明采样纹理句柄。
opaque_handle!(TextureHandle);
// 声明采样器句柄。
opaque_handle!(SamplerHandle);
// 声明固定 pipeline 句柄。
opaque_handle!(PipelineHandle);
// 声明 render target 句柄。
opaque_handle!(RenderTargetHandle);
// 声明一次 submit 返回的提交序号。
opaque_handle!(SubmissionHandle);

// 固定扇形 uniform 的四个 float4 ABI 总字节数。
pub(crate) const SECTOR_UNIFORM_BYTES: usize = 64;

// 定义 RHI 资源格式，只保留 UIX 当前需要的有限集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TextureFormat {
    // 定义 premultiplied BGRA 八位格式。
    Bgra8Unorm,
    // 定义 premultiplied RGBA 八位格式。
    Rgba8Unorm,
    // 定义单通道覆盖率格式。
    R8Unorm,
}

// 定义通用 renderer 与 native adapter 共享的有限 pipeline key。
pub(crate) mod pipeline_keys {
    // 使用位置 float2 与 MeshConstants uniform 绘制实心三角形。
    pub(crate) const SOLID_MESH: u64 = 1;
    // 使用位置 float2、纹理坐标 float2 与颜色 float4 绘制采样图元。
    pub(crate) const TEXTURED_QUAD: u64 = 2;
    // 使用单位 quad 与 GradientConstants uniform 绘制线性/径向渐变。
    pub(crate) const GRADIENT_RECT: u64 = 3;
    // 使用 R8 coverage 与 glyph shader 绘制覆盖率 quad。
    pub(crate) const GLYPH_COVERAGE_QUAD: u64 = 4;
    // 使用 RectConstants 与 SDF shader 绘制圆角或描边矩形。
    pub(crate) const SHAPE_RECT: u64 = 5;
    // 使用同一 RectConstants 与 SDF shader 执行饱和加法矩形。
    pub(crate) const SHAPE_RECT_ADDITIVE: u64 = 11;
    // 使用 ShadowConstants 与 SDF shader 绘制软阴影。
    pub(crate) const BOX_SHADOW: u64 = 6;
    // Additive sampled quad 复用采样 ABI，但使用独立加法 blend。
    pub(crate) const TEXTURED_QUAD_ADDITIVE: u64 = 7;
    // 使用 BlurConstants 与全屏区域 quad 执行一个方向的高斯 blur pass。
    pub(crate) const BLUR_PASS: u64 = 8;
    // 使用 RGBA8 MSDF 与仿射 quad 绘制可缩放字形。
    pub(crate) const MSDF_GLYPH_QUAD: u64 = 9;
    // 使用单位 quad 与 SectorConstants 绘制分析抗锯齿扇形。
    pub(crate) const SECTOR: u64 = 10;
}

// 定义 buffer 的底层用途，adapter 只需据此选择绑定旗标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BufferUsage {
    // 声明顶点 buffer。
    Vertex,
    // 声明索引 buffer。
    Index,
    // 声明常量或动态 uniform buffer。
    Uniform,
}

// 定义 buffer 创建描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BufferDesc {
    // 保存 buffer 的字节容量。
    pub(crate) size_bytes: usize,
    // 保存 buffer 的单元素步长，供通用 draw packet 绑定。
    pub(crate) stride_bytes: u32,
    // 保存 buffer 的底层用途。
    pub(crate) usage: BufferUsage,
}

// 定义 texture 创建描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextureDesc {
    // 保存 texture 的二维 extent。
    pub(crate) extent: RhiExtent,
    // 保存 texture 的通用格式。
    pub(crate) format: TextureFormat,
}

// 定义 sampler 创建描述。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SamplerDesc {
    // 记录是否使用线性过滤。
    pub(crate) linear: bool,
}

// 定义 pipeline 创建描述，具体 shader 和 layout 由 adapter 解释。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineDesc {
    // 使用稳定 key 标识通用 renderer 选定的 pipeline 语义。
    pub(crate) key: u64,
}

// 定义事实型 GPU 能力，不列举任何 UI 操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphicsCapabilities {
    // 记录是否支持动态 buffer 创建和更新。
    pub(crate) dynamic_buffers: bool,
    // 记录是否支持纹理上传。
    pub(crate) texture_upload: bool,
    // 记录是否支持纹理复制。
    pub(crate) texture_copy: bool,
    // 记录是否支持带重叠安全语义的纹理区域移动。
    pub(crate) texture_region_move: bool,
    // 记录是否支持 render pass 内的局部颜色清理。
    pub(crate) clear_rect: bool,
    // 记录是否支持采样纹理绑定。
    pub(crate) sampled_textures: bool,
    // 记录是否支持 render-to-texture。
    pub(crate) render_to_texture: bool,
    // 记录是否支持 scissor。
    pub(crate) scissor: bool,
    // 记录是否支持 premultiplied-alpha blend。
    pub(crate) premultiplied_alpha_blend: bool,
    // 记录是否支持 sampled Additive blend；这是可选绘制能力而非 GPU 基线。
    pub(crate) additive_blend: bool,
    // 记录主 framebuffer 是否在提交间保留。
    pub(crate) retained_framebuffer: bool,
    // 记录 surface 是否支持窄 partial present。
    pub(crate) partial_present: bool,
    // 记录 surface 是否能提供可靠的遮挡状态。
    pub(crate) occlusion: bool,
}

// 为能力快照提供 GPU 基线和缺口检查。
impl GraphicsCapabilities {
    // 创建文档要求的完整 GPU 基线能力。
    pub(crate) const fn full_gpu_baseline() -> Self {
        // 返回不依赖具体 API 的底层能力集合。
        Self {
            dynamic_buffers: true,
            texture_upload: true,
            texture_copy: true,
            texture_region_move: false,
            clear_rect: false,
            sampled_textures: true,
            render_to_texture: true,
            scissor: true,
            premultiplied_alpha_blend: true,
            additive_blend: true,
            retained_framebuffer: false,
            partial_present: false,
            occlusion: false,
        }
    }

    // 判断通用 GPU Renderer 的最小原语是否全部存在。
    pub(crate) const fn has_gpu_baseline(self) -> bool {
        // 基线缺一项就不能进入正常 GPU 渲染流程。
        self.dynamic_buffers
            && self.texture_upload
            && self.texture_copy
            && self.sampled_textures
            && self.render_to_texture
            && self.scissor
            && self.premultiplied_alpha_blend
    }

    // 返回第一个缺失的基线能力，便于 bootstrap 产生稳定诊断。
    pub(crate) const fn first_missing_gpu_baseline(self) -> Option<&'static str> {
        // 检查动态 buffer 能力。
        if !self.dynamic_buffers {
            // 返回稳定的 capability 名称。
            return Some("dynamic_buffers");
        }
        // 检查纹理上传能力。
        if !self.texture_upload {
            // 返回稳定的 capability 名称。
            return Some("texture_upload");
        }
        // 检查纹理复制能力。
        if !self.texture_copy {
            // 返回稳定的 capability 名称。
            return Some("texture_copy");
        }
        // 检查采样纹理能力。
        if !self.sampled_textures {
            // 返回稳定的 capability 名称。
            return Some("sampled_textures");
        }
        // 检查 render-to-texture 能力。
        if !self.render_to_texture {
            // 返回稳定的 capability 名称。
            return Some("render_to_texture");
        }
        // 检查 scissor 能力。
        if !self.scissor {
            // 返回稳定的 capability 名称。
            return Some("scissor");
        }
        // 检查 premultiplied-alpha blend 能力。
        if !self.premultiplied_alpha_blend {
            // 返回稳定的 capability 名称。
            return Some("premultiplied_alpha_blend");
        }
        // 所有 GPU 基线能力都存在。
        None
    }
}

// 定义 premultiplied-alpha 颜色值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiColor(pub(crate) [f32; 4]);

// 为颜色值提供有限性校验。
impl RhiColor {
    // 判断四个通道是否都是有限数。
    pub(crate) fn is_finite(self) -> bool {
        // 任何 NaN 或无穷值都会破坏跨 API 的像素语义。
        self.0.iter().all(|channel| channel.is_finite())
    }
}

// 定义 viewport 的物理尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RhiViewport {
    // 保存 viewport 宽度。
    pub(crate) width: f32,
    // 保存 viewport 高度。
    pub(crate) height: f32,
}

// 为 viewport 提供跨 adapter 一致的输入检查。
impl RhiViewport {
    // 判断 viewport 尺寸是否为有限正数。
    pub(crate) fn is_valid(self) -> bool {
        // 零、负数、NaN 和无穷值都不能进入原生 viewport。
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

// 定义整数 scissor，坐标仍保持左上角原点约定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RhiScissor {
    // 保存裁剪矩形左侧。
    pub(crate) x: i32,
    // 保存裁剪矩形顶部。
    pub(crate) y: i32,
    // 保存裁剪矩形宽度。
    pub(crate) width: i32,
    // 保存裁剪矩形高度。
    pub(crate) height: i32,
}

// 为 scissor 提供跨 adapter 一致的输入检查。
impl RhiScissor {
    // 判断 scissor 的 extent 是否为正数且坐标没有负值。
    pub(crate) const fn is_valid(self) -> bool {
        // RHI 使用物理 surface 坐标，禁止负尺寸和负起点。
        self.x >= 0 && self.y >= 0 && self.width > 0 && self.height > 0
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

// 描述通用 renderer 已经选定的 draw packet。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawPacket {
    // 保存通用 pipeline 句柄。
    pub(crate) pipeline: PipelineHandle,
    // 保存顶点 buffer 句柄。
    pub(crate) vertex_buffer: BufferHandle,
    // 保存可选索引 buffer 句柄。
    pub(crate) index_buffer: Option<BufferHandle>,
    // 保存可选的 pipeline uniform buffer 句柄。
    pub(crate) uniform_buffer: Option<BufferHandle>,
    // 保存顶点数量。
    pub(crate) vertex_count: u32,
    // 保存索引数量；零表示非索引绘制。
    pub(crate) index_count: u32,
    // 保存首个顶点位置。
    pub(crate) first_vertex: u32,
    // 保存首个索引位置。
    pub(crate) first_index: u32,
    // 保存索引绘制的基顶点。
    pub(crate) base_vertex: i32,
}

// 为 draw packet 提供常用的非索引三角形构造器。
impl DrawPacket {
    // 创建一个使用指定 pipeline 的非索引 draw packet。
    pub(crate) const fn triangles(pipeline: PipelineHandle, vertex_count: u32) -> Self {
        // 返回从零开始的非索引绘制范围。
        Self {
            pipeline,
            vertex_buffer: BufferHandle::from_raw(0),
            index_buffer: None,
            uniform_buffer: None,
            vertex_count,
            index_count: 0,
            first_vertex: 0,
            first_index: 0,
            base_vertex: 0,
        }
    }

    // 判断 packet 是否包含可执行的顶点或索引范围。
    pub(crate) const fn is_non_empty(self) -> bool {
        // 索引绘制优先检查 index_count，否则检查 vertex_count。
        self.vertex_count != 0 || self.index_count != 0
    }
}

// 描述纹理之间的一次有限复制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextureCopy {
    // 保存源纹理句柄。
    pub(crate) source: TextureHandle,
    // 保存目标纹理句柄。
    pub(crate) destination: TextureHandle,
    // 保存源区域左侧。
    pub(crate) source_x: u32,
    // 保存源区域顶部。
    pub(crate) source_y: u32,
    // 保存目标区域左侧。
    pub(crate) destination_x: u32,
    // 保存目标区域顶部。
    pub(crate) destination_y: u32,
    // 保存复制区域宽度。
    pub(crate) width: u32,
    // 保存复制区域高度。
    pub(crate) height: u32,
}

// 描述一次具有重叠安全语义的纹理区域移动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextureMove {
    // 保存源纹理句柄。
    pub(crate) source: TextureHandle,
    // 保存目标纹理句柄；与 source 相同时仍必须支持重叠移动。
    pub(crate) destination: TextureHandle,
    // 保存源区域左侧。
    pub(crate) source_x: u32,
    // 保存源区域顶部。
    pub(crate) source_y: u32,
    // 保存目标区域左侧。
    pub(crate) destination_x: u32,
    // 保存目标区域顶部。
    pub(crate) destination_y: u32,
    // 保存移动区域宽度。
    pub(crate) width: u32,
    // 保存移动区域高度。
    pub(crate) height: u32,
}

// 描述一次 acquire 得到的 surface image。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SurfaceFrame {
    // 保存 acquire 时的 surface token。
    pub(crate) token: SurfaceToken,
    // 保存 adapter 绑定的 surface render target。
    pub(crate) target: RenderTargetHandle,
}

// 为 surface frame 提供构造入口。
impl SurfaceFrame {
    // 创建一个带代际和目标句柄的 acquired frame。
    pub(crate) const fn new(token: SurfaceToken, target: RenderTargetHandle) -> Self {
        // 返回不可变的 acquired image 身份。
        Self { token, target }
    }
}

// 定义 GPU device 的薄原语接口。
pub(crate) trait GraphicsDevice {
    // 返回本 device 的事实型能力快照。
    fn capabilities(&self) -> GraphicsCapabilities;

    // 在 bootstrap 阶段执行最小资源与固定 pipeline 探针。
    fn probe(&mut self) -> Result<()> {
        // 使用一个真实顶点 buffer 验证动态 buffer 创建和上传。
        let vertex_buffer = self.create_buffer(BufferDesc {
            size_bytes: 24,
            stride_bytes: 8,
            usage: BufferUsage::Vertex,
        })?;
        // 上传三个 float2 顶点，形成最小可执行三角形 draw packet。
        self.update_buffer(vertex_buffer, 0, &[0; 24])?;
        // 创建 Solid mesh 所需的 32 字节 uniform ABI。
        let uniform_buffer = self.create_buffer(BufferDesc {
            size_bytes: 32,
            stride_bytes: 0,
            usage: BufferUsage::Uniform,
        })?;
        // 上传有限 viewport、padding 和透明颜色常量。
        self.update_buffer(uniform_buffer, 0, &[0; 32])?;
        // 使用最小的 RGBA texture 验证 render target 颜色格式。
        let rgba_texture = self.create_texture(TextureDesc {
            extent: RhiExtent::new(1, 1),
            format: TextureFormat::Rgba8Unorm,
        })?;
        // 使用最小的 BGRA texture 验证主 surface/Picture 采样格式。
        let bgra_texture = self.create_texture(TextureDesc {
            extent: RhiExtent::new(1, 1),
            format: TextureFormat::Bgra8Unorm,
        })?;
        // 使用最小的 R8 texture 验证 coverage 采样格式。
        let coverage_texture = self.create_texture(TextureDesc {
            extent: RhiExtent::new(1, 1),
            format: TextureFormat::R8Unorm,
        })?;
        // 上传 RGBA 纹理的一个 premultiplied 像素。
        self.update_texture(rgba_texture, RhiExtent::new(1, 1), &[0, 0, 0, 0])?;
        // 上传 BGRA 纹理的一个 premultiplied 像素。
        self.update_texture(bgra_texture, RhiExtent::new(1, 1), &[0, 0, 0, 0])?;
        // 上传 coverage 纹理的一个覆盖率像素。
        self.update_texture(coverage_texture, RhiExtent::new(1, 1), &[0])?;
        // 创建 2x2 RGBA texture，验证 atlas 所需的带偏移子区域上传。
        let region_texture = self.create_texture(TextureDesc {
            extent: RhiExtent::new(2, 2),
            format: TextureFormat::Rgba8Unorm,
        })?;
        // 把一个像素写入右下角，避免零偏移整块上传冒充 region 能力。
        self.update_texture_region(region_texture, 1, 1, RhiExtent::new(1, 1), &[0, 0, 0, 0])?;
        // 只有声明区域移动能力的 adapter 才进入 retained framebuffer 探针。
        if self.capabilities().texture_region_move {
            // 以同一纹理的重叠区域移动验证 memmove 语义，而不是普通 copy。
            self.move_texture_region(TextureMove {
                source: region_texture,
                destination: region_texture,
                source_x: 1,
                source_y: 1,
                destination_x: 0,
                destination_y: 0,
                width: 1,
                height: 1,
            })?;
        }
        // 创建真实 sampler，验证纹理绑定所需的过滤状态。
        let sampler = self.create_sampler(SamplerDesc { linear: true })?;
        // 编译通用 renderer 当前使用的全部固定 pipeline。
        let pipeline_keys = [
            pipeline_keys::SOLID_MESH,
            pipeline_keys::TEXTURED_QUAD,
            pipeline_keys::GRADIENT_RECT,
            pipeline_keys::GLYPH_COVERAGE_QUAD,
            pipeline_keys::SHAPE_RECT,
            pipeline_keys::SHAPE_RECT_ADDITIVE,
            pipeline_keys::BOX_SHADOW,
            pipeline_keys::TEXTURED_QUAD_ADDITIVE,
            pipeline_keys::BLUR_PASS,
            pipeline_keys::MSDF_GLYPH_QUAD,
            pipeline_keys::SECTOR,
        ];
        // 逐个创建 pipeline，让 adapter 在首帧前暴露 shader/layout 失败。
        let mut pipelines = Vec::with_capacity(pipeline_keys.len());
        for key in pipeline_keys {
            pipelines.push(self.create_pipeline(PipelineDesc { key })?);
        }
        // 在 1x1 离屏颜色 target 上执行真实的 pass、draw 和 submit。
        self.begin_render_pass(
            RenderTargetHandle::from_raw(rgba_texture.raw()),
            LoadAction::Clear(RhiColor([0.0, 0.0, 0.0, 0.0])),
        )?;
        // 绑定最小正 viewport，验证 adapter 的 viewport 状态编码。
        self.set_viewport(RhiViewport {
            width: 1.0,
            height: 1.0,
        })?;
        // 清除显式 scissor，验证 pass 初始 raster 状态可用。
        self.set_scissor(None)?;
        // 只有声明局部清理能力的 adapter 才进入 ClearRect 探针。
        if self.capabilities().clear_rect {
            // 以完整 1x1 区域验证清理颜色和 scissor 代际。
            self.clear_rect(
                RhiColor([0.0, 0.0, 0.0, 0.0]),
                RhiScissor {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            )?;
        }
        // 使用第一个固定 pipeline 执行最小 solid draw。
        self.draw(DrawPacket {
            pipeline: pipelines[0],
            vertex_buffer,
            index_buffer: None,
            uniform_buffer: Some(uniform_buffer),
            vertex_count: 3,
            index_count: 0,
            first_vertex: 0,
            first_index: 0,
            base_vertex: 0,
        })?;
        // 关闭 probe pass，防止资源销毁跨过打开的 render pass。
        self.end_render_pass()?;
        // 提交 probe 命令，验证 adapter 的提交边界和状态清理。
        self.submit()?;
        // 释放 probe 创建的固定 pipeline，资源生命周期仍由 adapter 检查。
        for pipeline in pipelines {
            self.destroy_pipeline(pipeline)?;
        }
        // 释放 probe 创建的 sampler。
        self.destroy_sampler(sampler)?;
        // 释放 probe 创建的 coverage texture。
        self.destroy_texture(coverage_texture)?;
        // 释放 probe 创建的 region texture。
        self.destroy_texture(region_texture)?;
        // 释放 probe 创建的 BGRA texture。
        self.destroy_texture(bgra_texture)?;
        // 释放 probe 创建的 RGBA texture。
        self.destroy_texture(rgba_texture)?;
        // 释放 probe 创建的 vertex buffer。
        self.destroy_buffer(vertex_buffer)?;
        // 释放 probe 创建的 uniform buffer。
        self.destroy_buffer(uniform_buffer)?;
        // 所有固定资源和 shader ABI 均通过首帧前探针。
        Ok(())
    }

    // 创建动态 buffer，具体内存类型由 adapter 选择。
    fn create_buffer(&mut self, _desc: BufferDesc) -> Result<BufferHandle> {
        // 默认实现显式拒绝，防止 adapter 漏实现时静默成功。
        Err(rhi_not_implemented("create_buffer"))
    }

    // 上传 buffer 内容，不暴露映射指针或 API 状态。
    fn update_buffer(&mut self, _buffer: BufferHandle, _offset: usize, _data: &[u8]) -> Result<()> {
        // 默认实现显式拒绝，要求真实 adapter 提供上传能力。
        Err(rhi_not_implemented("update_buffer"))
    }

    // 创建 sampled texture 或 render target texture。
    fn create_texture(&mut self, _desc: TextureDesc) -> Result<TextureHandle> {
        // 默认实现显式拒绝，防止能力声明和实现脱节。
        Err(rhi_not_implemented("create_texture"))
    }

    // 上传纹理的紧密排列像素。
    fn update_texture(
        &mut self,
        _texture: TextureHandle,
        _extent: RhiExtent,
        _data: &[u8],
    ) -> Result<()> {
        // 默认实现显式拒绝，避免把空上传当作成功。
        Err(rhi_not_implemented("update_texture"))
    }

    // 上传纹理中一个带目标偏移的紧密排列子区域。
    fn update_texture_region(
        &mut self,
        _texture: TextureHandle,
        _destination_x: u32,
        _destination_y: u32,
        _extent: RhiExtent,
        _data: &[u8],
    ) -> Result<()> {
        // 默认实现只允许 adapter 明确声明 atlas 子区域上传能力。
        Err(rhi_not_implemented("update_texture_region"))
    }

    // 创建采样器。
    fn create_sampler(&mut self, _desc: SamplerDesc) -> Result<SamplerHandle> {
        // 默认实现显式拒绝，要求真实 adapter 绑定采样语义。
        Err(rhi_not_implemented("create_sampler"))
    }

    // 创建通用 pipeline。
    fn create_pipeline(&mut self, _desc: PipelineDesc) -> Result<PipelineHandle> {
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
    fn destroy_pipeline(&mut self, _pipeline: PipelineHandle) -> Result<()> {
        // 默认实现显式拒绝，保证 adapter 明确声明生命周期。
        Err(rhi_not_implemented("destroy_pipeline"))
    }

    // 开始一个有序 render pass。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()>;

    // 设置当前 pass 的 viewport。
    fn set_viewport(&mut self, viewport: RhiViewport) -> Result<()>;

    // 设置或清除当前 pass 的 scissor。
    fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()>;

    // 在当前 pass 的指定整数区域内清理颜色，并保持其它 pass 状态不变。
    fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
        // 默认 adapter 必须显式声明局部清理原语，不能静默忽略 damage。
        Err(rhi_not_implemented("clear_rect"))
    }

    // 绑定通用采样资源。
    fn bind_texture(
        &mut self,
        _slot: u32,
        _texture: TextureHandle,
        _sampler: SamplerHandle,
    ) -> Result<()> {
        // 默认实现显式拒绝，避免纹理命令被忽略。
        Err(rhi_not_implemented("bind_texture"))
    }

    // 执行一个已经完成高层降级的 draw packet。
    fn draw(&mut self, packet: DrawPacket) -> Result<()>;

    // 在 pass 外执行一次纹理复制。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()>;

    // 在 pass 外移动纹理区域，并保证同一纹理的重叠区域按 memmove 语义执行。
    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // 不同纹理可以复用普通 copy；同一纹理必须由 adapter 提供重叠安全实现。
        if movement.source != movement.destination {
            // 将不同纹理的区域移动降级为已有的无重叠 copy 原语。
            return self.copy_texture(TextureCopy {
                source: movement.source,
                destination: movement.destination,
                source_x: movement.source_x,
                source_y: movement.source_y,
                destination_x: movement.destination_x,
                destination_y: movement.destination_y,
                width: movement.width,
                height: movement.height,
            });
        }
        // 同一纹理的重叠移动不能交给普通 copy 猜测覆盖顺序。
        Err(rhi_not_implemented("move_texture_region"))
    }

    // 结束当前 render pass。
    fn end_render_pass(&mut self) -> Result<()>;

    // 提交当前 device 命令并返回提交身份。
    fn submit(&mut self) -> Result<SubmissionHandle>;

    // 进行非破坏性的设备维护。
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
    // 返回当前 surface 的代际和 extent。
    fn token(&self) -> SurfaceToken;

    // 获取当前可呈现 image，不提交任何命令。
    fn acquire(&mut self) -> Result<SurfaceFrame>;

    // 重建 surface 并推进代际。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken>;

    // 将一次 device submit 最终呈现到原生窗口。
    fn present(
        &mut self,
        frame: SurfaceFrame,
        submission: SubmissionHandle,
        damage: PresentDamage,
    ) -> Result<()>;

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

// 组合 owner-thread device 与 surface，供迁移期 renderer 以单个借用执行 FramePlan。
pub(crate) trait GraphicsContextRhi: GraphicsDevice + GraphicsSurface {}

// 任何同时实现 device 和 surface 原语的原生 context 都自动获得组合视图。
impl<T> GraphicsContextRhi for T where T: GraphicsDevice + GraphicsSurface {}

// 统一生成 RHI 尚未实现的 typed error。
fn rhi_not_implemented(operation: &'static str) -> Error {
    // 用稳定前缀区分底层契约缺口和上层 UI 操作缺口。
    Error::new(
        Errc::NotImplemented,
        format!("thin RHI operation is not implemented: {operation}"),
    )
}
