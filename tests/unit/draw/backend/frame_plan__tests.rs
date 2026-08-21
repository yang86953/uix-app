// 引入测试用集合，记录 RHI 的执行顺序。
use std::collections::VecDeque;
// 引入框架错误类型和最终 damage。
use crate::core::error::{Errc, Error, Result};
// 引入测试计划依赖的薄 RHI 类型。
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawBufferBindings, DrawPacket, DrawRange, DrawSamplingBinding,
    GraphicsDevice, GraphicsDeviceCapabilities, GraphicsSurface, IndexBufferBinding, IndexFormat,
    LoadAction, PipelineBinding, PipelineHandle, PipelineKind, RenderTargetHandle, RhiBufferUpload,
    RhiBufferUploadPreflight, RhiColor, RhiExtent, RhiGradientRasterParams, RhiMeshRasterParams,
    RhiPresentTransaction, RhiSampledRasterParams, RhiScissor, RhiTextureTransfer, RhiViewport,
    SampledTextureBinding, SamplerHandle, SubmissionHandle, SurfaceFrame, SurfaceToken,
    TextureCopy, TextureHandle, TextureMove,
};
// 引入当前文件的计划类型。
use super::{
    FrameIndexPayload, FramePlan, FramePlanCommand, FramePlanScope, FramePlanStep,
    FrameUniformPayload, FrameVertexPayload, RenderPassPlan, RenderTargetRef,
};

// 记录一个不会触碰真实图形 API 的 mock device。
struct RecordingDevice {
    // 保存薄 RHI 调用顺序。
    log: VecDeque<&'static str>,
    // 保存测试适配器报告的底层能力。
    capabilities: GraphicsDeviceCapabilities,
    // 保存是否强制 submit 失败。
    fail_submit: bool,
    // 保存是否强制 draw 失败以验证 pass 中途收尾。
    fail_draw: bool,
    // 保存是否强制目标能力解析失败。
    fail_target_resolution: bool,
    // 保存是否强制普通 texture copy 预检失败。
    fail_copy_preflight: bool,
    // 保存是否强制 texture move 预检失败。
    fail_move_preflight: bool,
    // 保存可选真实 Buffer 描述，用于验证类型化上传预检。
    upload_preflight_desc: Option<(BufferHandle, BufferDesc)>,
    // 保存是否强制 Draw 资源预检失败。
    fail_draw_preflight: bool,
    // 保存是否强制 sampled 资源预检失败。
    fail_sampled_preflight: bool,
}

// 为记录型 device 实现薄 RHI 的执行原语。
impl GraphicsDevice for RecordingDevice {
    // 返回完整 GPU 基线，测试重点放在顺序而不是能力拒绝。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 返回测试实例配置的能力快照。
        self.capabilities
    }

    // 测试设备把 fixture texture 提升为已验证的目标身份。
    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        // 在失败模式下模拟共享资源表拒绝不可渲染目标。
        if self.fail_target_resolution {
            // 返回稳定参数错误，证明失败发生在激活前。
            return Err(Error::new(
                // 目标格式或身份不满足共享契约。
                Errc::InvalidArgument,
                // 测试诊断不泄漏具体图形 API。
                "recording texture is not renderable",
            ));
        }
        // 测试入口不模拟真实资源表，只提供稳定的可渲染 mock 事实。
        Ok(RenderTargetHandle::for_test(texture))
    }

    // 在任何 Device 原语前预检普通 texture copy。
    fn preflight_texture_copy(&self, _copy: TextureCopy) -> Result<()> {
        // 注入失败时保持 Device 日志为空。
        if self.fail_copy_preflight {
            // 返回共享参数错误，模拟资源表或传输契约拒绝。
            return Err(Error::new(
                Errc::InvalidArgument,
                "recording copy preflight failed",
            ));
        }
        // 关闭注入后允许合法 copy 继续执行。
        Ok(())
    }

    // 在任何 Device 原语前预检 texture move。
    fn preflight_texture_move(&self, _movement: TextureMove) -> Result<()> {
        // 注入失败时保持 Device 日志为空。
        if self.fail_move_preflight {
            // 返回共享参数错误，模拟资源表或传输契约拒绝。
            return Err(Error::new(
                Errc::InvalidArgument,
                "recording move preflight failed",
            ));
        }
        // 关闭注入后允许合法 move 继续执行。
        Ok(())
    }

    // 在任何 Device 原语前预检类型化 Buffer 上传。
    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        // 只有指定身份的上传才消费 fixture 提供的真实描述。
        if let Some((buffer, desc)) = self.upload_preflight_desc {
            // 同一资源身份必须进入共享用途、元素 ABI 与范围门禁。
            if buffer == upload.buffer() {
                // 直接返回共享预检结果，不在 mock Device 复制判断。
                return upload.validate(desc);
            }
        }
        // 未配置描述或身份不同时允许其它合法 fixture 继续。
        Ok(())
    }

    // 在任何 Device 原语前预检 Draw 的全部真实资源。
    fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        // 注入失败时保持 Device 日志为空。
        if self.fail_draw_preflight {
            // 返回共享参数错误，模拟资源表角色或容量拒绝。
            return Err(Error::new(
                Errc::InvalidArgument,
                "recording draw resource preflight failed",
            ));
        }
        // 只有携带条件采样资源的 Draw 才消费 sampled 失败注入。
        if packet.sampling().sampled_texture().is_some() && self.fail_sampled_preflight {
            // 返回共享参数错误，模拟纹理格式或 sampler 过滤拒绝。
            return Err(Error::new(
                Errc::InvalidArgument,
                "recording sampled resource preflight failed",
            ));
        }
        // 关闭注入后允许合法完整 Draw 继续执行。
        Ok(())
    }

    // 记录任何原生命令前必须发生的 owner-context 激活。
    fn activate(&mut self) -> Result<()> {
        // 记录统一计划执行器触发的激活边界。
        self.log.push_back("activate");
        // 返回成功，让测试继续观察健康检查与后续命令顺序。
        Ok(())
    }

    // 记录激活后必须发生的设备健康检查。
    fn maintain(&mut self) -> Result<()> {
        // 记录统一计划执行器触发的 health preflight。
        self.log.push_back("maintain");
        // 返回成功，让测试继续观察后续命令顺序。
        Ok(())
    }

    // 记录 render pass 开始。
    fn begin_render_pass(&mut self, _target: RenderTargetHandle, _load: LoadAction) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("begin_pass");
        // 返回成功。
        Ok(())
    }

    // 记录局部清理，验证 retained damage 不会被 adapter 静默丢弃。
    fn clear_rect(&mut self, _color: RhiColor, _scissor: RhiScissor) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("clear_rect");
        // 返回成功。
        Ok(())
    }

    // 记录 buffer 上传，验证它位于 draw 前且不会被 adapter 忽略。
    fn update_buffer(&mut self, _upload: RhiBufferUpload<'_>) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("update_buffer");
        // 返回成功。
        Ok(())
    }

    // 记录 draw packet。
    fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("draw");
        // 在失败模式下模拟 Adapter 于 pass 内拒绝 draw。
        if self.fail_draw {
            // 返回稳定平台错误，供执行器证明主错误不会被 cleanup 覆盖。
            return Err(Error::new(Errc::PlatformError, "recording draw failed"));
        }
        // 返回成功。
        Ok(())
    }

    // 记录纹理复制。
    fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("copy");
        // 返回成功。
        Ok(())
    }

    // 记录重叠安全的纹理区域移动。
    fn move_texture_region(&mut self, _movement: TextureMove) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("move");
        // 返回成功。
        Ok(())
    }

    // 记录 render pass 结束。
    fn end_render_pass(&mut self) -> Result<()> {
        // 记录调用事件。
        self.log.push_back("end_pass");
        // 返回成功。
        Ok(())
    }

    // 记录唯一 submit，并允许测试提交失败。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        // 记录调用事件。
        self.log.push_back("submit");
        // 在失败模式下返回 typed device error。
        if self.fail_submit {
            // 返回设备丢失，禁止进入 present。
            return Err(Error::new(
                Errc::GraphicsDeviceLost,
                "recording submit failed",
            ));
        }
        // 返回稳定的测试提交句柄。
        Ok(SubmissionHandle::from_raw(7))
    }
}

// 将记录型 Surface fixture 拆出，保持 FramePlan 核心文件低于上限。
include!("frame_plan_recording_surface.rs");

// 组合记录型 device 与 surface，覆盖迁移期 context 执行入口。
struct RecordingContext {
    // 保存所有 device 命令及提交结果。
    device: RecordingDevice,
    // 保存 acquire、generation 与最终 present 状态。
    surface: RecordingSurface,
}

// 让组合记录 context 通过同一个 GraphicsDevice 契约接收命令。
impl GraphicsDevice for RecordingContext {
    // 返回内嵌 device 的事实能力。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 不建立第二份测试能力来源。
        self.device.device_capabilities()
    }

    // 把 texture target 能力解析委托给内嵌记录 Device。
    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        // Surface 事务的只读预检必须观察同一 Device 资源事实。
        self.device.resolve_render_target(texture)
    }

    // 把普通 texture copy 预检委托给内嵌记录 device。
    fn preflight_texture_copy(&self, copy: TextureCopy) -> Result<()> {
        // 复用唯一测试资源预检路径。
        self.device.preflight_texture_copy(copy)
    }

    // 把 texture move 预检委托给内嵌记录 device。
    fn preflight_texture_move(&self, movement: TextureMove) -> Result<()> {
        // 复用唯一测试资源预检路径。
        self.device.preflight_texture_move(movement)
    }

    // 把 Buffer 上传预检委托给内嵌记录 device。
    fn preflight_buffer_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        // 复用唯一测试上传资源预检路径。
        self.device.preflight_buffer_upload(upload)
    }

    // 把 Draw 资源预检委托给内嵌记录 device。
    fn preflight_draw_resources(&self, packet: DrawPacket) -> Result<()> {
        // 复用唯一测试资源预检路径。
        self.device.preflight_draw_resources(packet)
    }

    // 把 owner-context 激活委托给内嵌记录 device。
    fn activate(&mut self) -> Result<()> {
        // surface 与 offscreen 模式必须观察同一激活边界。
        self.device.activate()
    }

    // 把设备健康检查委托给内嵌记录 device。
    fn maintain(&mut self) -> Result<()> {
        // surface 与 offscreen 模式必须观察同一 health preflight。
        self.device.maintain()
    }

    // 把 pass 开始委托给内嵌记录 device。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        // 保持独立 device 与组合 context 的记录行为相同。
        self.device.begin_render_pass(target, load)
    }

    // 把局部清理委托给内嵌记录 device。
    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        // 复用唯一测试记录路径。
        self.device.clear_rect(color, scissor)
    }

    // 把不可变上传载荷委托给内嵌记录 device。
    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 复用唯一测试记录路径。
        self.device.update_buffer(upload)
    }

    // 把 draw packet 委托给内嵌记录 device。
    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        // 复用唯一测试记录路径。
        self.device.draw(packet)
    }

    // 把纹理复制委托给内嵌记录 device。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        // 复用唯一测试记录路径。
        self.device.copy_texture(copy)
    }

    // 把重叠安全移动委托给内嵌记录 device。
    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // 复用唯一测试记录路径。
        self.device.move_texture_region(movement)
    }

    // 把 pass 结束委托给内嵌记录 device。
    fn end_render_pass(&mut self) -> Result<()> {
        // 复用唯一测试记录路径。
        self.device.end_render_pass()
    }

    // 把唯一提交委托给内嵌记录 device。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        // 复用唯一测试记录路径。
        self.device.submit()
    }
}

// 让组合记录 context 通过同一个 GraphicsSurface 契约管理呈现状态。
impl GraphicsSurface for RecordingContext {
    // 返回内嵌 surface 的当前代际。
    fn token(&self) -> SurfaceToken {
        // 不缓存或复制动态 surface 事实。
        self.surface.token()
    }

    // 从内嵌 surface 获取当前 image。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // 复用独立 surface 的 acquire 语义。
        self.surface.acquire()
    }

    // 通过内嵌 surface 原子推进代际。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // 复用独立 surface 的 resize 语义。
        self.surface.resize(extent)
    }

    // 把最终 present 委托给内嵌 surface。
    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        // 复用独立 surface 的成功与失败边界。
        self.surface.present(transaction)
    }
}

// 创建具备完整 GPU 基线的组合记录 context。
fn recording_context(token: SurfaceToken) -> RecordingContext {
    // 组合唯一 device 与唯一 surface owner。
    RecordingContext {
        // 创建可记录全部命令的 device。
        device: RecordingDevice {
            // 初始尚未执行任何 native 原语。
            log: VecDeque::new(),
            // 允许计划进入正常 GPU 执行路径。
            capabilities: GraphicsDeviceCapabilities::full_gpu_baseline(),
            // 默认允许 submit 成功。
            fail_submit: false,
            // 默认允许 pass 内 draw 成功。
            fail_draw: false,
            // 默认允许目标能力解析成功。
            fail_target_resolution: false,
            // 默认允许普通 copy 预检成功。
            fail_copy_preflight: false,
            // 默认允许 texture move 预检成功。
            fail_move_preflight: false,
            // 默认不为任何 Buffer 注入额外真实描述。
            upload_preflight_desc: None,
            // 默认允许 Draw 资源预检成功。
            fail_draw_preflight: false,
            // 默认允许 sampled 资源预检成功。
            fail_sampled_preflight: false,
        },
        // 创建与计划代际一致的 surface。
        surface: RecordingSurface {
            // 保存调用方提供的 surface token。
            token,
            // 初始尚未发生最终 present。
            present_count: 0,
            // 初始尚未取得任何 Surface image。
            acquire_count: 0,
            // 默认允许 present 成功。
            fail_present: false,
            // 默认允许 acquire 成功。
            fail_acquire: false,
        },
    }
}

// 将 Surface 与 Offscreen 共用的最小计划构造器拆到独立测试支持文件。
include!("frame_plan_test_support.rs");
// 将组合 context 的多执行模式回归测试拆到独立载荷，保持核心文件小于上限。
include!("frame_plan_context_tests.rs");
// 将资源移动与 submit 失败边界拆到独立测试载荷，保持计划核心文件短小。
include!("frame_plan_test_tail.rs");
// 将 DrawPacket 动态栅格拒绝边界拆到独立测试载荷，保持各文件低于上限。
include!("frame_plan_raster_tests.rs");
