//! FramePlan 的离屏执行边界。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入薄 RHI 的 Device、Surface、组合 context、目标句柄与提交句柄。
use crate::platform::presentation::rhi::{
    GraphicsContextRhi, GraphicsDevice, GraphicsSurface, RenderTargetHandle, RhiBufferUpload,
    RhiBufferUploadPreflight, RhiPresentTransaction, SubmissionHandle, SurfaceFrame,
};

// 引入父模块的计划私有结构。
use super::{
    FrameCommit, FramePlan, FramePlanCommand, FramePlanStep, RenderPassPlan, RenderTargetRef,
};

// 描述唯一命令执行器允许解析的 render target 范围。
#[derive(Debug, Clone, Copy)]
enum FramePlanTargetMode {
    // 把逻辑 Surface 解析为当前 acquire 得到的不透明目标。
    AcquiredSurface(RenderTargetHandle),
    // 禁止逻辑 Surface，只允许显式 texture target。
    OffscreenOnly,
}

// 封闭只读资源预检的 FramePlan scope，禁止 Surface 与离屏语义混用。
#[derive(Debug, Clone, Copy)]
enum FramePlanResourceScope {
    // Surface 计划允许逻辑 Surface target，但不获取 acquired image。
    SurfaceAllowed,
    // 离屏计划只允许显式 texture target。
    OffscreenOnly,
}

// 在任何 Surface acquire 或 Device activate 前完成全部只读资源预检。
struct FramePlanResourcePreflight<'device, D>
where
    // 只借用 GraphicsDevice 的只读预检能力，不取得执行所有权。
    D: GraphicsDevice + ?Sized,
{
    // 保存当前 owner 的只读 Device 借用。
    device: &'device D,
    // 保存本次 FramePlan 的封闭执行 scope。
    scope: FramePlanResourceScope,
}

// 为唯一资源预检 Component 提供 Surface 与离屏两种构造和运行入口。
impl<'device, D> FramePlanResourcePreflight<'device, D>
where
    // Component 只依赖共享 GraphicsDevice 查询契约。
    D: GraphicsDevice + ?Sized,
{
    // 创建允许逻辑 Surface target 的只读预检器。
    fn surface(device: &'device D) -> Self {
        // 保存 SurfaceAllowed scope，不触碰 acquired image。
        Self {
            // 保存只读 Device 借用。
            device,
            // 固定 Surface 目标语义。
            scope: FramePlanResourceScope::SurfaceAllowed,
        }
    }

    // 创建只允许显式 texture target 的只读预检器。
    fn offscreen(device: &'device D) -> Self {
        // 保存 OffscreenOnly scope，不取得 Surface 能力。
        Self {
            // 保存只读 Device 借用。
            device,
            // 固定离屏目标语义。
            scope: FramePlanResourceScope::OffscreenOnly,
        }
    }

    // 按统一顺序完成 target、transfer、upload 和完整 Draw 资源预检。
    fn run(&self, steps: &[FramePlanStep]) -> Result<()> {
        // 先完成 scope 专属 target 校验。
        self.validate_targets(steps)?;
        // 再预检所有顶层 texture copy/move。
        self.validate_transfers(steps)?;
        // 再预检所有 pass 内类型化 Buffer 上传。
        self.validate_buffer_uploads(steps)?;
        // 最后预检所有 pass 内 Draw 的全部真实资源。
        self.validate_draw_resources(steps)
    }

    // 校验 scope 与所有 render pass target 的组合关系。
    fn validate_targets(&self, steps: &[FramePlanStep]) -> Result<()> {
        // 检查任一 pass 是否请求逻辑 Surface target。
        let contains_surface = steps.iter().any(|step| {
            // 只匹配 render pass 中的逻辑 Surface target。
            matches!(
                step,
                FramePlanStep::Pass(pass) if matches!(pass.target, RenderTargetRef::Surface)
            )
        });
        // 离屏执行不得隐含 acquire 或主 surface 写入。
        if matches!(self.scope, FramePlanResourceScope::OffscreenOnly) && contains_surface {
            // 在任何 Device 或 Surface 副作用前返回稳定参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "offscreen FramePlan cannot target the presentation surface",
            ));
        }
        // Surface scope 中逻辑 Surface 由 acquire 后解析，texture target 仍需预检。
        for step in steps {
            // 只读取 render pass 的逻辑目标。
            if let FramePlanStep::Pass(pass) = step {
                // 只有 texture target 需要向资源 owner 查询真实描述。
                if let RenderTargetRef::Texture(texture) = pass.target {
                    // 共享资源表必须在任何 native side effect 前证明目标能力。
                    self.device.resolve_render_target(texture)?;
                }
            }
        }
        // 所有 target 都已满足当前 FramePlan scope。
        Ok(())
    }

    // 预检所有 pass 内类型化 Buffer 上传的真实资源与精确范围。
    fn validate_buffer_uploads(&self, steps: &[FramePlanStep]) -> Result<()> {
        // 按 FramePlan 顶层顺序逐个观察 render pass。
        for step in steps {
            // 只有 render pass 包含 Buffer 上传命令。
            if let FramePlanStep::Pass(pass) = step {
                // 保持 pass 内上传命令的 painter order。
                for command in &pass.commands {
                    // 顶点上传必须用类型化载荷的精确编码长度预检真实 Buffer。
                    if let FramePlanCommand::UploadVertex { buffer, data } = command {
                        // 预检值不编码或借用裸字节，因此不会产生 Device 副作用。
                        self.device.preflight_buffer_upload(
                            // 将句柄、字节数与类型化布局的唯一步长绑定为共享查询值。
                            RhiBufferUploadPreflight::vertex(
                                // 直接读取载荷布局拥有的共享元素步长。
                                *buffer,
                                // 读取类型化顶点载荷的精确编码长度。
                                data.size_bytes(),
                                // 读取布局值对象的唯一元素步长。
                                data.layout().stride_bytes(),
                            ),
                        )?;
                    }
                    // 索引上传必须用类型化载荷的精确编码长度预检真实 Buffer。
                    if let FramePlanCommand::UploadIndex { buffer, data } = command {
                        // 复用共享索引用途预检，不在 FramePlan 复制格式或容量规则。
                        self.device.preflight_buffer_upload(
                            // 将句柄、字节数与类型化格式的唯一步长绑定为共享查询值。
                            RhiBufferUploadPreflight::index(
                                // 直接读取目标索引 Buffer 身份。
                                *buffer,
                                // 读取类型化索引载荷的精确编码长度。
                                data.size_bytes(),
                                // 读取索引格式值对象的唯一元素步长。
                                data.format().stride_bytes(),
                            ),
                        )?;
                    }
                    // Uniform 上传必须在编码前证明完整替换真实 Buffer。
                    if let FramePlanCommand::UploadUniform { buffer, data } = command {
                        // 复用同一共享预检值，不在 FramePlan 复制用途或容量规则。
                        self.device.preflight_buffer_upload(
                            // 固定 Uniform ABI 已经能无分配地给出精确编码长度。
                            RhiBufferUploadPreflight::uniform(*buffer, data.size_bytes()),
                        )?;
                    }
                }
            }
        }
        // 每一条类型化 Buffer 上传都已由真实资源描述证明。
        Ok(())
    }

    // 预检所有 pass 外纹理传输。
    fn validate_transfers(&self, steps: &[FramePlanStep]) -> Result<()> {
        // 按 FramePlan 顶层顺序逐项观察纹理传输命令。
        for step in steps {
            // 复制命令必须由 Device 的共享资源表预检。
            if let FramePlanStep::Copy(copy) = step {
                // 只调用只读 preflight，不激活或写入原生状态。
                self.device.preflight_texture_copy(*copy)?;
            }
            // 移动命令必须由 Device 的共享资源表预检。
            if let FramePlanStep::Move(movement) = step {
                // 只调用只读 preflight，不激活或写入原生状态。
                self.device.preflight_texture_move(*movement)?;
            }
        }
        // 所有顶层传输都已通过共享资源与几何门禁。
        Ok(())
    }

    // 预检所有 pass 内 Draw 的真实 Buffer 与条件采样资源。
    fn validate_draw_resources(&self, steps: &[FramePlanStep]) -> Result<()> {
        // 按 FramePlan 顶层顺序逐个观察 render pass。
        for step in steps {
            // 只有 render pass 包含 Draw 资源引用。
            if let FramePlanStep::Pass(pass) = step {
                // 保持 pass 内命令顺序扫描。
                for command in &pass.commands {
                    // 只预检实际 Draw packet。
                    if let FramePlanCommand::Draw(packet) = command {
                        // 共享资源表必须在任何 native side effect 前证明全部资源角色。
                        self.device.preflight_draw_resources(*packet)?;
                    }
                }
            }
        }
        // 所有 Draw 的真实资源描述都已通过共享预检。
        Ok(())
    }
}

// 把已经验证的 FramePlan 步骤唯一映射到一个薄 RHI device。
pub(super) struct FramePlanExecutor<'device, D>
where
    // 支持独立 device 和同时实现 device/surface 的组合 context。
    D: GraphicsDevice + ?Sized,
{
    // 借用当前 owner-thread device，但不取得其资源生命周期。
    device: &'device mut D,
    // 冻结本次执行允许使用的目标解析方式。
    target_mode: FramePlanTargetMode,
}

// 为唯一 FramePlan 命令执行组件提供原生准备与有序分派。
impl<'device, D> FramePlanExecutor<'device, D>
where
    // 只依赖 GraphicsDevice 窄契约，不依赖 surface 或原生 Adapter。
    D: GraphicsDevice + ?Sized,
{
    // 创建允许写入当前 acquired surface 的执行器。
    pub(super) fn for_surface(device: &'device mut D, target: RenderTargetHandle) -> Self {
        // 绑定当前帧唯一的 surface target。
        Self {
            // 保存执行期间的独占 device 借用。
            device,
            // 记录 acquire 已经解析出的不透明目标。
            target_mode: FramePlanTargetMode::AcquiredSurface(target),
        }
    }

    // 创建只允许写入显式离屏纹理的执行器。
    pub(super) fn offscreen(device: &'device mut D) -> Self {
        // 不取得主 surface image 或 present 能力。
        Self {
            // 保存执行期间的独占 device 借用。
            device,
            // 固定拒绝所有逻辑 Surface target。
            target_mode: FramePlanTargetMode::OffscreenOnly,
        }
    }

    // 在资源预检完成后激活 Device，再按计划顺序执行全部步骤。
    pub(super) fn execute(mut self, steps: &[FramePlanStep]) -> Result<()> {
        // 在第一条原生命令前激活当前 owner；需要线程 current 状态的 adapter 在此恢复。
        self.device.activate()?;
        // 激活成功后再执行设备健康 preflight，失败计划不得进入任何命令。
        self.device.maintain()?;
        // 严格保持 FramePlan 已经确定的步骤顺序。
        for step in steps {
            // 把每类顶层步骤映射到唯一薄 RHI 调用路径。
            match step {
                // render pass 由同一个 pass 执行函数处理。
                FramePlanStep::Pass(pass) => self.execute_pass(pass)?,
                // pass 外纹理复制直接交给 device。
                FramePlanStep::Copy(copy) => self.device.copy_texture(*copy)?,
                // retained 区域移动保持重叠安全语义。
                FramePlanStep::Move(movement) => {
                    // 只调用 device 声明的统一移动原语。
                    self.device.move_texture_region(*movement)?;
                }
            }
        }
        // 全部步骤均已按顺序交付 device。
        Ok(())
    }

    // 执行一个完整 render pass，并保证命令不会跨过 pass 边界。
    fn execute_pass(&mut self, pass: &RenderPassPlan) -> Result<()> {
        // 按执行模式解析通用 target 身份。
        let target = self.resolve_target(pass.target)?;
        // 使用计划冻结的 load action 开始 pass。
        self.device.begin_render_pass(target, pass.load)?;
        // 严格保持 pass 内命令的 painter order。
        for command in &pass.commands {
            // 所有提交模式复用同一个命令分派函数，并在失败时进入共同收尾。
            if let Err(command_error) = self.execute_command(command) {
                // pass 已成功开始，任何中途错误都必须尝试清除共享和原生状态。
                if let Err(cleanup_error) = self.device.end_render_pass() {
                    // cleanup 失败不能覆盖最初的命令根因，但必须留下明确诊断。
                    tracing::error!(
                        // 同时记录主错误和收尾错误，便于判断 Device 是否需要重建。
                        "FramePlan render pass cleanup failed after command error: primary={}; cleanup={}",
                        // 保留最初失败命令的稳定诊断。
                        command_error.what(),
                        // 保留 Adapter 收尾失败诊断。
                        cleanup_error.what(),
                    );
                }
                // 返回最初的命令错误，禁止 cleanup 结果改变恢复分类。
                return Err(command_error);
            }
        }
        // 显式结束 pass，禁止后续 copy 或 pass 嵌套。
        self.device.end_render_pass()?;
        // 当前 pass 已完整交付 device。
        Ok(())
    }

    // 把一条已验证命令映射为唯一的薄 RHI device 调用。
    fn execute_command(&mut self, command: &FramePlanCommand) -> Result<()> {
        // 分派当前命令而不引入任何 API 或平台判断。
        match command {
            // 执行不改变其它 pass 状态的局部清理。
            FramePlanCommand::ClearRect { color, scissor } => {
                // 将规范颜色和物理矩形一起交给 Adapter。
                self.device.clear_rect(*color, *scissor)
            }
            // 在唯一执行边界把类型化顶点编码为 Device 原语所需字节。
            FramePlanCommand::UploadVertex { buffer, data } => {
                // 字节视图只借用 FramePlan 已验证的不可变顶点载荷。
                let bytes = data.as_ne_bytes();
                // 将资源身份与从零开始的类型化载荷绑定成唯一上传命令。
                self.device
                    .update_buffer(RhiBufferUpload::new(*buffer, bytes))
            }
            // 在唯一执行边界把类型化索引编码为 Device 原语所需字节。
            FramePlanCommand::UploadIndex { buffer, data } => {
                // 索引载荷只在执行边界借为 host-order u32 字节。
                let bytes = data.as_ne_bytes();
                // 索引更新通过同一上传值对象进入完整替换门禁。
                self.device
                    .update_buffer(RhiBufferUpload::new(*buffer, bytes))
            }
            // 在唯一执行边界把类型化 Uniform 编码为固定 ABI 字节。
            FramePlanCommand::UploadUniform { buffer, data } => {
                // 每个 Uniform 变体只借用共享值对象拥有的固定 ABI 字段。
                let bytes = data.as_ne_bytes();
                // Uniform 更新通过同一上传值对象进入完整替换门禁。
                self.device
                    .update_buffer(RhiBufferUpload::new(*buffer, bytes))
            }
            // 执行已经完成高层语义 lowering 的 draw packet。
            FramePlanCommand::Draw(packet) => self.device.draw(*packet),
        }
    }

    // 把通用 target 引用解析为本次 device 调用使用的不透明句柄。
    fn resolve_target(&self, target: RenderTargetRef) -> Result<RenderTargetHandle> {
        // 只允许 acquired surface 或显式 texture 两种确定映射。
        match (self.target_mode, target) {
            // 当前 acquired image 是逻辑 Surface 的唯一物理目标。
            (FramePlanTargetMode::AcquiredSurface(surface), RenderTargetRef::Surface) => {
                // 返回 acquire 交付的不透明 target。
                Ok(surface)
            }
            // 显式 texture target 在唯一 Device 边界提升为 render target。
            (_, RenderTargetRef::Texture(texture)) => self.device.resolve_render_target(texture),
            // offscreen 模式的 Surface 已由前置校验拒绝，此分支保留防御。
            (FramePlanTargetMode::OffscreenOnly, RenderTargetRef::Surface) => Err(Error::new(
                Errc::InvalidArgument,
                "offscreen FramePlan cannot target the presentation surface",
            )),
        }
    }
}

// 统一保证 acquire 后任一失败都会通知 Surface 释放原生帧所有权。
fn with_acquired_surface_frame<T>(
    context: &mut dyn GraphicsContextRhi,
    frame: SurfaceFrame,
    operation: impl FnOnce(&mut dyn GraphicsContextRhi) -> Result<T>,
) -> Result<T> {
    let result = operation(context);
    if let Err(primary) = &result
        && let Err(cleanup) = context.surface().discard_acquired_frame(frame)
    {
        tracing::error!(
            "Surface frame rollback failed after frame error: primary={}; cleanup={}",
            primary.what(),
            cleanup.what(),
        );
    }
    result
}

// 为 FramePlan 提供不触发 surface present 的离屏执行入口。
impl FramePlan {
    // 在唯一 submit 成功后、最终 present 之前执行一个 owner-thread 观察钩子。
    pub(crate) fn execute_on_context_with_before_present(
        // 借用当前不可变帧计划。
        &self,
        // 借用同时拥有 device 与 surface 的原生 context。
        context: &mut dyn GraphicsContextRhi,
        // 钩子只能观察或回读 Surface，不能重新取得 Device 命令能力。
        before_present: &mut dyn FnMut(&mut dyn GraphicsSurface),
        // 返回仍以最终 present 成功为提交点的 FrameCommit。
    ) -> Result<FrameCommit> {
        // 统一验证计划与组合 context 暴露的窄 device 能力。
        self.validate_for_device(context.device_ref())?;
        // 原子取得计划生成时冻结的 Surface 代际与最终 present damage。
        let (plan_surface, present_damage) = self.required_surface()?;
        // 检查计划生成时观察到的 surface 代际。
        if context.surface_ref().token() != plan_surface {
            // 旧计划不能写入新一代 surface。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "FramePlan surface generation is stale before acquire",
            ));
        }
        // 在 acquire 前完成全部只读资源预检，避免 Surface 副作用后才失败。
        FramePlanResourcePreflight::surface(context.device_ref()).run(&self.steps)?;
        // 获取唯一的本帧 surface image。
        let frame = context.surface().acquire()?;
        with_acquired_surface_frame(context, frame, |context| {
            // 检查 acquire 返回的 image 代际。
            if frame.token() != plan_surface {
                // 迟到或错误代际的 image 不得接收当前计划。
                return Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "acquired surface frame does not match FramePlan generation",
                ));
            }
            // 组合 context 只把窄 device 角色交给唯一命令执行组件。
            FramePlanExecutor::for_surface(context.device(), frame.target())
                .execute(&self.steps)?;
            // 所有内部工作完成后只允许一次 device submit。
            let submission = context.device().submit()?;
            // 检查执行期间 surface 是否被重建。
            if context.surface_ref().token() != frame.token() {
                // 不把旧 image 的 submit 当作当前 surface 的成功 present。
                return Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "surface generation changed before final present",
                ));
            }
            // 此时全部绘制命令已经提交，而 Surface 尚未进入 swap/present。
            before_present(
                // 观察器只取得窄 Surface 角色，无法追加 Device 命令或二次 submit。
                context.surface(),
            );
            // 观察钩子不得使当前 surface 代际失效。
            if context.surface_ref().token() != frame.token() {
                // 代际变化仍按 surface lost 进入统一恢复层。
                return Err(Error::new(
                    // 使用稳定的 surface 生命周期分类。
                    Errc::GraphicsSurfaceLost,
                    // 明确失败发生在提交与最终 present 之间。
                    "surface generation changed at the before-present boundary",
                ));
            }
            // 最终 present 是唯一提交成功边界。
            context
                // 最终阶段只借用 Surface 角色消费 frame 与 submission。
                .surface()
                // Surface Adapter 只接收绑定 frame、submit 与 damage 的完整事务。
                .present(RhiPresentTransaction::new(
                    // 绑定 acquire 返回的同代际 image。
                    frame,
                    // 绑定 Device 刚签发的提交身份。
                    submission,
                    // 绑定本帧唯一最终 damage。
                    present_damage,
                ))?;
            // 只有 present 成功后才返回可消费 damage 的 commit。
            Ok(FrameCommit {
                // 保存已经成功呈现的 surface 代际。
                surface: plan_surface,
                // 保存 device 返回的提交身份。
                submission,
            })
        })
    }

    // 在独立 device 上执行只包含离屏 target 的计划。
    pub(crate) fn execute_offscreen_on_device<D>(&self, device: &mut D) -> Result<SubmissionHandle>
    where
        // 离屏执行只依赖资源、命令和 submit 原语。
        D: GraphicsDevice + ?Sized,
    {
        // 统一验证计划与 device 基线。
        self.validate_for_device(device)?;
        // Surface 计划不能通过 device-only 入口绕过 acquire/present。
        if self.targets_surface() {
            // 在任何 device 副作用前返回稳定作用域错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "surface FramePlan cannot enter the offscreen device boundary",
            ));
        }
        // 离屏入口复用同一只读 Component 且不取得 Surface 能力。
        FramePlanResourcePreflight::offscreen(device).run(&self.steps)?;
        // 资源预检已经拒绝逻辑 Surface target，执行器只消费离屏计划。
        FramePlanExecutor::offscreen(device).execute(&self.steps)?;
        // 离屏计划只提交命令，不获取或呈现主 surface。
        let submission = device.submit()?;
        // 返回离屏工作的提交身份。
        Ok(submission)
    }
}
