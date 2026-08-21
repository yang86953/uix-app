//! Drawing GPU 启动探针的动态资源、FramePlan 命令和失败回滚契约测试。

// 引入测试 fixture 所需的错误类型与结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入探针直接依赖的 RHI 契约。
use super::device_probe::probe_device;
// 引入启动探针测试所需的共享薄 RHI 契约。
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities, LoadAction,
    PipelineBinding, PipelineDesc, PipelineHandle, PipelineKind, RenderTargetHandle,
    RhiBufferUpload, RhiBufferUploadPreflight, RhiTextureUpload, SamplerDesc, SamplerHandle,
    SubmissionHandle, TextureCopy, TextureDesc, TextureHandle, TextureMove,
};
// 描述 fixture 需要注入的单点故障。
#[derive(Clone, Copy)]
enum Failure {
    // 在任何资源创建前的 Device 健康检查处失败。
    Maintain,
    // 在指定序号的 buffer 创建处失败。
    CreateBuffer(usize),
    // 在指定序号的 buffer 上传处失败。
    UpdateBuffer(usize),
    // 在 FramePlan Draw 资源预检处失败。
    DrawPreflight,
    // 在首次 draw 处失败。
    Draw,
    // 在指定原始 texture 句柄销毁处失败。
    DestroyTexture(u64),
}

// 记录资源和命令的可观察身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resource {
    // 记录 buffer 资源。
    Buffer(BufferHandle),
    // 记录 texture 资源。
    Texture(TextureHandle),
    // 记录 sampler 资源。
    Sampler(SamplerHandle),
    // 记录 pipeline 资源。
    Pipeline(PipelineBinding),
}

// 记录探针与 fixture 交互的最小动态事实。
#[derive(Default)]
struct RecordingDevice {
    // 保存按创建顺序产生的资源。
    created: Vec<Resource>,
    // 保存按调用顺序销毁的资源。
    destroyed: Vec<Resource>,
    // 保存关键命令名称。
    commands: Vec<&'static str>,
    // 保存收到的 Solid uniform 上传载荷。
    solid_uniform_uploads: Vec<Vec<u8>>,
    // 保存待注入故障。
    failure: Option<Failure>,
    // 保存 buffer 创建调用次数。
    buffer_creates: usize,
    // 保存 buffer 上传调用次数。
    buffer_updates: usize,
    // 保存下一个稳定的句柄值。
    next_handle: u64,
}

// 为动态探针 fixture 提供稳定的故障和资源查询入口。
impl RecordingDevice {
    // 创建无故障的记录设备。
    fn healthy() -> Self {
        // 为测试句柄保留非零起始身份。
        Self {
            // 让资源创建顺序在断言中稳定可观察。
            next_handle: 1,
            // 复用所有记录字段的默认空状态。
            ..Self::default()
        }
    }

    // 创建带单点故障的记录设备。
    fn failing(failure: Failure) -> Self {
        // 先建立正常的记录设备。
        let mut device = Self::healthy();
        // 安排本次 fixture 的单点故障。
        device.failure = Some(failure);
        // 返回已经配置故障的设备。
        device
    }

    // 分配一个稳定且单调递增的资源身份。
    fn allocate(&mut self) -> u64 {
        // 读取当前尚未使用的句柄值。
        let handle = self.next_handle;
        // 推进句柄分配游标以保证身份唯一。
        self.next_handle += 1;
        // 返回稳定的资源身份。
        handle
    }

    // 构造携带指定错误码和文本的故障结果。
    fn injected_error(operation: &'static str) -> Error {
        // 返回带有固定错误码和操作文本的注入错误。
        Error::new(
            Errc::PlatformError,
            format!("probe injected {operation} failure"),
        )
    }

    // 判断指定资源是否要求注入销毁故障。
    fn destroy_texture_error(&self, texture: TextureHandle) -> Option<Error> {
        // 根据当前故障策略匹配待销毁纹理。
        match self.failure {
            // 对目标纹理返回 cleanup 错误。
            Some(Failure::DestroyTexture(raw)) if raw == texture.raw() => {
                // 保留统一的 cleanup 错误文本。
                Some(Self::injected_error("destroy_texture"))
            }
            // 其它资源销毁继续成功。
            _ => None,
        }
    }
}

// 为记录 fixture 实现 probe 所需的全部 GraphicsDevice 原语。
impl GraphicsDevice for RecordingDevice {
    // 返回完整基础能力，同时关闭可选清理和区域移动分支。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 复用共享基线，确保测试只观察探针生命周期。
        GraphicsDeviceCapabilities::full_gpu_baseline()
    }

    // 在任何资源创建前执行可注入失败的 Device 健康检查。
    fn maintain(&mut self) -> Result<()> {
        // 按故障策略拒绝尚未开始资源事务的探针。
        if matches!(self.failure, Some(Failure::Maintain)) {
            // 返回稳定平台错误供早期失败测试断言。
            return Err(Self::injected_error("maintain"));
        }
        // 健康 Device 允许 probe 继续建立资源。
        Ok(())
    }

    // 记录 buffer 创建并按故障策略拒绝指定序号。
    fn create_buffer(&mut self, _desc: BufferDesc) -> Result<BufferHandle> {
        // 统计本次 buffer 创建序号。
        self.buffer_creates += 1;
        // 在指定创建点返回注入错误。
        if matches!(self.failure, Some(Failure::CreateBuffer(index)) if index == self.buffer_creates)
        {
            // 让探针进入中途资源回滚路径。
            return Err(Self::injected_error("create_buffer"));
        }
        // 分配当前 buffer 的稳定句柄。
        let handle = BufferHandle::from_raw(self.allocate());
        // 记录成功创建的 buffer 所有权。
        self.created.push(Resource::Buffer(handle));
        // 返回新建的 buffer。
        Ok(handle)
    }

    // 记录 buffer 上传并保留 Solid uniform 的原始 ABI 字节。
    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 统计本次 buffer 上传序号。
        self.buffer_updates += 1;
        // 在指定上传点返回注入错误。
        if matches!(self.failure, Some(Failure::UpdateBuffer(index)) if index == self.buffer_updates)
        {
            // 让探针回滚已经创建的前置资源。
            return Err(Self::injected_error("update_buffer"));
        }
        // 识别 Mesh ABI 大小的 Solid uniform 上传。
        if upload.data().len() == PipelineKind::SolidMesh.contract().uniform.size_bytes() {
            // 复制载荷以便测试解码其前两个浮点数。
            self.solid_uniform_uploads.push(upload.data().to_vec());
        }
        // 报告上传成功。
        Ok(())
    }

    // 在任何 FramePlan 原生命令前预检类型化 Buffer 上传。
    fn preflight_buffer_upload(&self, _upload: RhiBufferUploadPreflight) -> Result<()> {
        // 预检阶段不改变 fixture 的命令与资源记录。
        Ok(())
    }

    // 在任何 FramePlan 原生命令前预检 Draw 的真实 Buffer 资源。
    fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
        // 按故障策略在 begin、draw、submit 前拒绝计划。
        if matches!(self.failure, Some(Failure::DrawPreflight)) {
            // 返回稳定的预检错误，证明资源仍由 probe scope 清理。
            return Err(Self::injected_error("draw preflight"));
        }
        // 健康路径允许 FramePlan 继续执行。
        Ok(())
    }

    // 在任何 FramePlan 原生移动命令前预检纹理移动。
    fn preflight_texture_move(&self, _movement: TextureMove) -> Result<()> {
        // 记录型 fixture 接受能力开启时的合法移动计划。
        Ok(())
    }

    // 记录 texture 创建并返回稳定句柄。
    fn create_texture(&mut self, _desc: TextureDesc) -> Result<TextureHandle> {
        // 分配当前 texture 的稳定句柄。
        let handle = TextureHandle::from_raw(self.allocate());
        // 记录成功创建的 texture 所有权。
        self.created.push(Resource::Texture(handle));
        // 返回新建的 texture。
        Ok(handle)
    }

    // 将 probe texture 提升为可渲染目标。
    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        // 将 fixture texture 直接提升为测试 render target。
        Ok(RenderTargetHandle::for_test(texture))
    }

    // 记录纹理上传而不改变资源状态。
    fn update_texture(&mut self, _upload: RhiTextureUpload<'_>) -> Result<()> {
        // fixture 接受探针的有效紧密纹理载荷。
        Ok(())
    }

    // 记录 sampler 创建。
    fn create_sampler(&mut self, _desc: SamplerDesc) -> Result<SamplerHandle> {
        // 分配当前 sampler 的稳定句柄。
        let handle = SamplerHandle::from_raw(self.allocate());
        // 记录成功创建的 sampler 所有权。
        self.created.push(Resource::Sampler(handle));
        // 返回新建的 sampler。
        Ok(handle)
    }

    // 记录 pipeline 创建并绑定对应的固定语义。
    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        // 将新句柄与调用方指定的 pipeline 语义绑定。
        let binding = PipelineBinding::for_test(
            // 分配当前 pipeline 的稳定句柄。
            PipelineHandle::from_raw(self.allocate()),
            // 保留探针请求的固定 pipeline 类型。
            desc.kind,
        );
        // 记录成功创建的 pipeline 所有权。
        self.created.push(Resource::Pipeline(binding));
        // 返回新建的 pipeline 绑定。
        Ok(binding)
    }

    // 记录 buffer 销毁。
    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        // 记录 buffer 的销毁顺序。
        self.destroyed.push(Resource::Buffer(buffer));
        // 报告销毁成功。
        Ok(())
    }

    // 记录 texture 销毁并可注入单点 cleanup 失败。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        // 记录 texture 的销毁顺序。
        self.destroyed.push(Resource::Texture(texture));
        // 查询该 texture 是否命中 cleanup 故障点。
        if let Some(error) = self.destroy_texture_error(texture) {
            // 返回 cleanup 错误但允许探针继续调用后续销毁。
            return Err(error);
        }
        // 报告当前 texture 销毁成功。
        Ok(())
    }

    // 记录 sampler 销毁。
    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        // 记录 sampler 的销毁顺序。
        self.destroyed.push(Resource::Sampler(sampler));
        // 报告销毁成功。
        Ok(())
    }

    // 记录 pipeline 销毁。
    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        // 记录 pipeline 的销毁顺序。
        self.destroyed.push(Resource::Pipeline(pipeline));
        // 报告销毁成功。
        Ok(())
    }

    // 记录 render pass 开始。
    fn begin_render_pass(&mut self, _target: RenderTargetHandle, _load: LoadAction) -> Result<()> {
        // 记录 pass 开始边界。
        self.commands.push("begin");
        // 报告命令成功。
        Ok(())
    }

    // 记录 draw，并在首次 draw 前注入主错误。
    fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
        // 记录 draw 命令到达 pass 内部。
        self.commands.push("draw");
        // 在首次 draw 处注入主错误。
        if matches!(self.failure, Some(Failure::Draw)) {
            // 保留可断言的主错误码和文本。
            return Err(Error::new(
                Errc::InvalidState,
                "probe injected draw failure",
            ));
        }
        // 报告 draw 成功。
        Ok(())
    }

    // 记录不应在 probe 中发生的 texture copy。
    fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
        // 记录不应出现的 copy 命令。
        self.commands.push("copy");
        // 报告命令成功。
        Ok(())
    }

    // 记录同纹理区域移动。
    fn move_texture_region(&mut self, _movement: TextureMove) -> Result<()> {
        // 记录可选区域移动命令。
        self.commands.push("move");
        // 报告命令成功。
        Ok(())
    }

    // 记录 render pass 结束。
    fn end_render_pass(&mut self) -> Result<()> {
        // 记录 pass 结束边界。
        self.commands.push("end");
        // 报告命令成功。
        Ok(())
    }

    // 记录提交并返回提交身份。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        // 记录唯一提交边界。
        self.commands.push("submit");
        // 返回稳定的提交身份。
        Ok(SubmissionHandle::from_raw(1))
    }
}

// 验证成功探针关闭 pass、提交一次并严格逆序销毁所有资源。
#[test]
fn successful_probe_has_reverse_resource_lifecycle() {
    // 创建无故障记录设备。
    let mut device = RecordingDevice::healthy();
    // 执行共享探针流程。
    let result = probe_device(&mut device);
    // 成功路径必须返回无错误结果。
    assert!(result.is_ok());
    // pass 必须只开始一次。
    assert_eq!(
        device
            .commands
            .iter()
            .filter(|command| **command == "begin")
            .count(),
        1
    );
    // pass 必须只结束一次。
    assert_eq!(
        device
            .commands
            .iter()
            .filter(|command| **command == "end")
            .count(),
        1
    );
    // 成功路径必须只提交一次。
    assert_eq!(
        device
            .commands
            .iter()
            .filter(|command| **command == "submit")
            .count(),
        1
    );
    // 销毁序列必须严格等于创建序列的逆序。
    assert_eq!(
        device.destroyed,
        device.created.iter().rev().copied().collect::<Vec<_>>()
    );
    // 读取探针记录的 Solid uniform 上传。
    let solid = device
        .solid_uniform_uploads
        .first()
        .expect("Solid uniform upload");
    // 解码 Solid uniform 的第一个浮点数。
    let first = f32::from_ne_bytes(solid[0..4].try_into().unwrap());
    // 解码 Solid uniform 的第二个浮点数。
    let second = f32::from_ne_bytes(solid[4..8].try_into().unwrap());
    // 两个 viewport 轴必须是非零的一像素尺寸。
    assert_eq!((first, second), (1.0, 1.0));
}

// 验证 Device 健康检查失败发生在任何资源、FramePlan 命令与提交之前。
#[test]
fn maintain_failure_precedes_probe_resource_transaction() {
    // 在 probe 的首个健康门禁注入失败。
    let mut device = RecordingDevice::failing(Failure::Maintain);
    // 执行不得进入资源事务的 probe。
    let error = probe_device(&mut device).expect_err("maintain must fail before resources");
    // 健康检查失败必须保留稳定平台错误。
    assert_eq!(error.code(), Errc::PlatformError);
    // 失败文本必须保留真实生命周期阶段。
    assert_eq!(error.message(), "probe injected maintain failure");
    // 早期失败不得创建或销毁任何探针资源。
    assert!(device.created.is_empty());
    // 没有资源时销毁记录也必须为空。
    assert!(device.destroyed.is_empty());
    // FramePlan 尚未构造执行，不能出现任何原生命令。
    assert!(device.commands.is_empty());
}

// 验证资源创建中途失败只逆序清理此前创建的第一个 Buffer。
#[test]
fn create_failure_rolls_back_prior_resources_without_submit() {
    // 在第二个 buffer 创建点注入故障。
    let mut device = RecordingDevice::failing(Failure::CreateBuffer(2));
    // 执行应当失败并回滚的探针流程。
    let error = probe_device(&mut device).expect_err("buffer creation must fail");
    // 资源创建失败必须保留平台错误码。
    assert_eq!(error.code(), Errc::PlatformError);
    // 创建失败发生在 pass 前，不得开始任何命令。
    assert!(device.commands.is_empty());
    // 失败创建不得出现在资源所有权记录中。
    assert_eq!(device.created.len(), 1);
    // 已创建资源必须只被销毁一次。
    assert_eq!(device.destroyed.len(), 1);
    // 唯一销毁资源必须是第一个 Buffer。
    assert!(matches!(device.destroyed[0], Resource::Buffer(_)));
    // 回滚顺序必须是此前创建资源的逆序。
    assert_eq!(
        device.destroyed,
        device.created.iter().rev().copied().collect::<Vec<_>>()
    );
    // 创建失败不得提交命令。
    assert!(!device.commands.contains(&"submit"));
}

// 验证 FramePlan pass 内资源上传失败仍结束 pass 且不提交。
#[test]
fn upload_failure_ends_pass_without_submit_and_rolls_back_resources() {
    // 在第一次 buffer 上传点注入故障。
    let mut device = RecordingDevice::failing(Failure::UpdateBuffer(1));
    // 执行应当失败并回滚的探针流程。
    let error = probe_device(&mut device).expect_err("upload must fail");
    // 上传失败必须保留平台错误码。
    assert_eq!(error.code(), Errc::PlatformError);
    // 上传失败发生在 pass 内，必须已经开始 pass。
    assert!(device.commands.contains(&"begin"));
    // FramePlan 必须在上传失败后结束已经打开的 pass。
    assert!(device.commands.contains(&"end"));
    // pass 内主错误不得继续提交。
    assert!(!device.commands.contains(&"submit"));
    // 所有销毁资源都必须属于此前创建资源。
    assert!(
        device
            .destroyed
            .iter()
            .all(|resource| device.created.contains(resource))
    );
    // 回滚顺序必须是此前创建资源的逆序。
    assert_eq!(
        device.destroyed,
        device.created.iter().rev().copied().collect::<Vec<_>>()
    );
}

// 验证 Draw 资源预检失败发生在 begin、draw、submit 前并完整回滚资源。
#[test]
fn draw_preflight_failure_happens_before_pass_and_submit() {
    // 在 FramePlan 的 Draw 资源预检处注入故障。
    let mut device = RecordingDevice::failing(Failure::DrawPreflight);
    // 执行应当在任何原生命令前失败的探针流程。
    let error = probe_device(&mut device).expect_err("draw preflight must fail");
    // 预检失败必须保留平台错误码。
    assert_eq!(error.code(), Errc::PlatformError);
    // 预检失败不得开始任何 pass。
    assert!(!device.commands.contains(&"begin"));
    // 预检失败不得执行 draw。
    assert!(!device.commands.contains(&"draw"));
    // 预检失败不得提交。
    assert!(!device.commands.contains(&"submit"));
    // 已创建资源必须仍按严格逆序完整销毁。
    assert_eq!(
        device.destroyed,
        device.created.iter().rev().copied().collect::<Vec<_>>()
    );
}

// 验证 pass 内 draw 失败仍关闭 pass、保留主错误并释放所有资源。
#[test]
fn draw_failure_ends_pass_and_preserves_primary_error() {
    // 创建在 draw 点注入故障的记录设备。
    let mut device = RecordingDevice::failing(Failure::Draw);
    // 执行应当失败的探针流程。
    let error = probe_device(&mut device).expect_err("draw must fail");
    // 主错误码必须保持 draw 的 InvalidState。
    assert_eq!(error.code(), Errc::InvalidState);
    // 主错误文本不得被 cleanup 过程覆盖。
    assert_eq!(error.message(), "probe injected draw failure");
    // draw 前必须已经开始 pass。
    assert!(device.commands.contains(&"begin"));
    // draw 失败后仍必须结束 pass。
    assert!(device.commands.contains(&"end"));
    // 主命令失败后不得提交。
    assert!(!device.commands.contains(&"submit"));
    // 资源仍必须完整逆序销毁。
    assert_eq!(
        device.destroyed,
        device.created.iter().rev().copied().collect::<Vec<_>>()
    );
}

// 验证 cleanup 某项失败后仍继续销毁后续资源并返回首个 cleanup 错误。
#[test]
fn cleanup_failure_continues_and_returns_first_cleanup_error() {
    // 选择第二个纹理作为 cleanup 故障点。
    let failed_texture = 5;
    // 创建带纹理销毁故障的记录设备。
    let mut device = RecordingDevice::failing(Failure::DestroyTexture(failed_texture));
    // 执行成功命令但 cleanup 失败的探针流程。
    let error = probe_device(&mut device).expect_err("cleanup must fail");
    // 返回值必须保留首个 cleanup 的平台错误码。
    assert_eq!(error.code(), Errc::PlatformError);
    // 返回值必须保留首个 cleanup 的文本。
    assert_eq!(error.message(), "probe injected destroy_texture failure");
    // cleanup 失败发生在提交之后，提交仍应发生一次。
    assert_eq!(
        device
            .commands
            .iter()
            .filter(|command| **command == "submit")
            .count(),
        1
    );
    // cleanup 失败不得阻断后续资源销毁。
    assert_eq!(device.destroyed.len(), device.created.len());
    // 所有资源仍必须按创建逆序销毁。
    assert_eq!(
        device.destroyed,
        device.created.iter().rev().copied().collect::<Vec<_>>()
    );
}
