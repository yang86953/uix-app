//! RHI Picture blur 的双 pass 与资源生命周期测试。

// 引入被测 blur 模块可见的 renderer、目标和 RHI 类型。
use super::*;
// 引入测试 recording context 需要的 device/surface 契约。
use crate::native::present::rhi::{
    // Blur 方向与 tap 字段位置由共享 RHI 契约唯一声明。
    BLUR_DIRECTION_TAPS_FLOAT_OFFSET,
    // Blur ABI 字节数由共享 RHI 契约唯一声明。
    BLUR_UNIFORM_BYTES,
    // Blur 权重区间位置由共享 RHI 契约唯一声明。
    BLUR_WEIGHTS_FLOAT_OFFSET,
    // device 原语由 mock 显式记录。
    GraphicsDevice,
    // 动态能力事实由 mock 显式声明。
    GraphicsDeviceCapabilities,
    // surface 原语用于证明离屏路径不 acquire/present。
    GraphicsSurface,
    // pipeline 绑定用于同时返回缓存资源与共享语义。
    PipelineBinding,
    // 原始句柄只在 mock 组合绑定时创建。
    PipelineHandle,
    // render target 句柄用于记录两个 pass 的写入顺序。
    RenderTargetHandle,
    // Buffer 上传值对象用于记录类型化载荷。
    RhiBufferUpload,
    // Buffer 预检值对象用于放行 FramePlan 资源预检。
    RhiBufferUploadPreflight,
    // 不可拆呈现事务用于补齐 Surface 契约。
    RhiPresentTransaction,
    // submission 句柄用于建立唯一提交边界。
    SubmissionHandle,
    // surface frame 只供意外 acquire 的可观察路径使用。
    SurfaceFrame,
    // surface token 用于提交前后代际校验。
    SurfaceToken,
    // texture copy 原语用于拒绝意外计划变化。
    TextureCopy,
    // texture 句柄用于测试目标能力解析。
    TextureHandle,
};

// 保存 blur 执行期间可审计的底层命令事实。
struct RecordingContext {
    // 分配稳定且非零的测试资源句柄。
    next_resource: u64,
    // 保存当前 surface 代际，供无 present 提交校验。
    token: SurfaceToken,
    // 记录 render pass 目标和是否使用 Clear。
    passes: Vec<(RenderTargetHandle, bool)>,
    // 记录两个 pass 下发的 scissor。
    scissors: Vec<Option<RhiScissor>>,
    // 保存所有 buffer 更新载荷以检查方向和高斯核。
    updates: Vec<Vec<u8>>,
    // 记录按 painter order 绑定的 source texture。
    bound_textures: Vec<u64>,
    // 保存本次创建的临时纹理。
    created_textures: Vec<TextureHandle>,
    // 保存已检查式销毁的临时纹理。
    destroyed_textures: Vec<TextureHandle>,
    // 记录 draw 次数。
    draw_count: usize,
    // 记录 pass 结束次数。
    end_pass_count: usize,
    // 记录 device submit 次数。
    submit_count: usize,
    // 记录意外 surface acquire 次数。
    acquire_count: usize,
    // 记录意外 surface present 次数。
    present_count: usize,
    // 允许失败测试在 submit 边界注入 device lost。
    fail_submit: bool,
}

// 为 recording context 提供确定性初值。
impl RecordingContext {
    // 构造一个满足通用 GPU 基线的内存记录器。
    fn new(fail_submit: bool) -> Self {
        // 返回不触碰真实图形 API 的 owner-thread context。
        Self {
            // 从非零值开始分配资源。
            next_resource: 10,
            // 使用稳定 surface 代际验证提交前后没有漂移。
            token: SurfaceToken::new(1, RhiExtent::new(64, 64)),
            // 初始没有 render pass。
            passes: Vec::new(),
            // 初始没有 scissor 命令。
            scissors: Vec::new(),
            // 初始没有 buffer 上传。
            updates: Vec::new(),
            // 初始没有纹理绑定。
            bound_textures: Vec::new(),
            // 初始没有临时纹理。
            created_textures: Vec::new(),
            // 初始没有销毁记录。
            destroyed_textures: Vec::new(),
            // 初始没有 draw。
            draw_count: 0,
            // 初始没有关闭 pass。
            end_pass_count: 0,
            // 初始没有提交。
            submit_count: 0,
            // 初始没有获取 surface image。
            acquire_count: 0,
            // 初始没有呈现 surface image。
            present_count: 0,
            // 保存调用方选择的失败模式。
            fail_submit,
        }
    }

    // 分配下一个非零 opaque handle 身份。
    fn allocate(&mut self) -> u64 {
        // 保存当前身份作为返回值。
        let raw = self.next_resource;
        // 推进到下一个稳定身份。
        self.next_resource += 1;
        // 返回本次分配结果。
        raw
    }
}

// 实现 blur 计划实际使用的薄 RHI device 原语。
impl GraphicsDevice for RecordingContext {
    // 声明完整 GPU 基线，使测试只聚焦 blur lowering。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 返回包含 render-to-texture、sampling 与 scissor 的能力集合。
        GraphicsDeviceCapabilities::full_gpu_baseline()
    }

    // 测试设备把 blur fixture texture 提升为已验证目标。
    fn resolve_render_target(&self, texture: TextureHandle) -> Result<RenderTargetHandle> {
        // 测试 mock 直接返回稳定的可渲染目标身份。
        Ok(RenderTargetHandle::for_test(texture))
    }

    // blur fixture 不重复验证共享契约，只放行 buffer 预检。
    fn preflight_buffer_upload(&self, _upload: RhiBufferUploadPreflight) -> Result<()> {
        // 资源预检事实由共享 FramePlan/RHI 测试覆盖。
        Ok(())
    }

    // blur fixture 不重复验证共享契约，只放行 draw 资源预检。
    fn preflight_draw_resources(&self, _packet: DrawPacket) -> Result<()> {
        // 资源预检事实由共享 FramePlan/RHI 测试覆盖。
        Ok(())
    }

    // blur fixture 不重复验证共享契约，只放行 sampled 资源预检。
    fn preflight_sampled_binding(&self, _binding: SampledTextureBinding) -> Result<()> {
        // 资源预检事实由共享 FramePlan/RHI 测试覆盖。
        Ok(())
    }

    // 创建 renderer 缓存使用的动态 buffer。
    fn create_buffer(&mut self, _desc: BufferDesc) -> Result<BufferHandle> {
        // 返回稳定测试句柄。
        Ok(BufferHandle::from_raw(self.allocate()))
    }

    // 记录顶点和 uniform 更新载荷。
    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 复制载荷，保证计划释放后仍可审计。
        self.updates.push(upload.data().to_vec());
        // 记录成功。
        Ok(())
    }

    // 创建每次 blur 独占的 scratch texture。
    fn create_texture(&mut self, _desc: TextureDesc) -> Result<TextureHandle> {
        // 分配临时纹理身份。
        let texture = TextureHandle::from_raw(self.allocate());
        // 保存创建事实。
        self.created_textures.push(texture);
        // 返回临时纹理。
        Ok(texture)
    }

    // 创建 blur 使用的线性 clamp sampler。
    fn create_sampler(&mut self, _desc: SamplerDesc) -> Result<SamplerHandle> {
        // 返回稳定测试句柄。
        Ok(SamplerHandle::from_raw(self.allocate()))
    }

    // 创建固定 blur pipeline。
    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        // pipeline 语义必须指向唯一 blur ABI。
        assert_eq!(desc.kind, PipelineKind::BlurPass);
        // 把稳定测试句柄与同一 blur 语义绑定后返回。
        Ok(PipelineBinding::for_test(
            // 分配 mock 原生句柄。
            PipelineHandle::from_raw(self.allocate()),
            // 保留调用方已经验证的创建语义。
            desc.kind,
        ))
    }

    // 检查式销毁 scratch texture。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        // 保存销毁身份用于与创建记录配对。
        self.destroyed_textures.push(texture);
        // 记录成功。
        Ok(())
    }

    // 开始一个有序 render pass。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        // 记录目标和 Clear/Load 边界。
        self.passes
            // Clear 为 true，Load 为 false；目标身份保持封闭类型。
            .push((target, matches!(load, LoadAction::Clear(_))));
        // 记录成功。
        Ok(())
    }

    // 接收完整目标 viewport。
    fn set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        // blur viewport 必须始终有效。
        assert!(viewport.is_valid());
        // 记录成功。
        Ok(())
    }

    // 记录已经裁到纹理范围的 scissor。
    fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        // 保存当前 pass 的可见区域。
        self.scissors.push(scissor);
        // 记录成功。
        Ok(())
    }

    // 记录两个 pass 的输入纹理顺序。
    fn bind_sampled_texture(
        // 修改记录器。
        &mut self,
        // 保存不可拆分的采样纹理与 sampler。
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 保存 source→scratch 顺序。
        self.bound_textures.push(binding.texture().raw());
        // 记录成功。
        Ok(())
    }

    // 记录固定六顶点 draw。
    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        // 每个方向都必须绘制同一矩形的两个三角形。
        assert_eq!(packet.range.vertex_count(), 6);
        // 累加 draw 事实。
        self.draw_count += 1;
        // 记录成功。
        Ok(())
    }

    // blur 计划不使用纹理 copy，但 trait 要求显式实现。
    fn copy_texture(&mut self, _copy: TextureCopy) -> Result<()> {
        // 任何 copy 都说明双 pass 计划发生意外变化。
        panic!("blur plan must not issue texture copy")
    }

    // 结束当前 render pass。
    fn end_render_pass(&mut self) -> Result<()> {
        // 累加 pass 结束事实。
        self.end_pass_count += 1;
        // 记录成功。
        Ok(())
    }

    // 提交离屏命令但不呈现。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        // 累加唯一提交边界。
        self.submit_count += 1;
        // 失败模式返回稳定 device-lost 错误。
        if self.fail_submit {
            // 模拟 adapter 在提交阶段丢失。
            return Err(Error::new(
                // 保留 device-lost 分类。
                Errc::GraphicsDeviceLost,
                // 保存稳定诊断文本。
                "recording submit failed",
            ));
        }
        // 返回稳定提交句柄。
        Ok(SubmissionHandle::from_raw(90))
    }
}

// 实现 surface 生命周期以证明 texture 目标不会触碰它。
impl GraphicsSurface for RecordingContext {
    // 返回当前代际。
    fn token(&self) -> SurfaceToken {
        // 复制稳定 token。
        self.token
    }

    // 记录任何意外 acquire。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // 累加可见副作用。
        self.acquire_count += 1;
        // 返回合法 frame，让断言而非伪错误识别误调用。
        Ok(SurfaceFrame::new(
            // 使用当前 token，目标种类由类型固定为 Surface。
            self.token,
        ))
    }

    // 更新测试 surface 代际。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // 推进代际并保存新尺寸。
        self.token = SurfaceToken::new(self.token.generation + 1, extent);
        // 返回新 token。
        Ok(self.token)
    }

    // 记录任何意外 present。
    fn present(
        // 修改记录器。
        &mut self,
        // 完整事务内容不影响负面计数。
        _transaction: RhiPresentTransaction,
    ) -> Result<()> {
        // 累加可见副作用。
        self.present_count += 1;
        // 记录成功，让测试检查精确次数。
        Ok(())
    }
}

// 把 host-endian uniform 字节还原为 f32 数组。
fn decode_f32s(bytes: &[u8]) -> Vec<f32> {
    // 按四字节边界遍历载荷。
    bytes
        // 每个 chunk 对应一个 f32。
        .chunks_exact(std::mem::size_of::<f32>())
        // 还原与生产 encode_f32s 相同的 host-endian 值。
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        // 收集为可索引数组。
        .collect()
}

// 验证 source→scratch→source 的严格双 pass 与同核正交方向。
#[test]
fn blur_executes_two_clipped_passes_without_surface_present() {
    // 创建默认 renderer 缓存。
    let mut renderer = RhiRenderer::default();
    // 创建成功型 recording context。
    let mut context = RecordingContext::new(false);
    // 使用外部 Picture texture 身份。
    let source = TextureHandle::from_raw(70);
    // 执行部分越界的离屏 blur。
    renderer
        // 调用生产双 pass 入口。
        .execute_blur_without_present(
            // 记录全部 RHI 事实。
            &mut context,
            // 水平 pass 从原 Picture 采样。
            source,
            // Picture texture 的实际 extent。
            RhiExtent::new(10, 6),
            // 请求区域左侧和底部越界。
            RhiScissor {
                // 左侧越界应裁到零。
                x: -2,
                // 顶部从第二行开始。
                y: 1,
                // 宽度超过 texture。
                width: 15,
                // 高度超过 texture。
                height: 10,
            },
            // 使用三 tap 半径可清楚检查 uniform。
            2.0,
            // Picture texture 使用预乘 BGRA。
            TextureFormat::Bgra8Unorm,
            // 垂直 pass 写回同一 Picture texture。
            source,
        )
        // 合法计划必须完整执行。
        .expect("blur plan should execute");

    // 每次调用只创建一个 scratch texture。
    assert_eq!(context.created_textures.len(), 1);
    // 读取 scratch 身份供顺序断言复用。
    let scratch = context.created_textures[0];
    // 第一 pass 清空 scratch，第二 pass Load 原 Picture。
    assert_eq!(
        // 比较完整 pass 目标顺序。
        context.passes,
        // 保留 source→scratch→source 的写入事实。
        vec![
            // 水平 pass 写入 scratch texture。
            (RenderTargetHandle::for_test(scratch), true),
            // 垂直 pass 写回 source texture。
            (RenderTargetHandle::for_test(source), false),
        ]
    );
    // 两个 pass 都必须使用裁到 texture 的同一区域。
    assert_eq!(
        // 比较实际 scissor。
        context.scissors,
        // 左侧、底部越界分别裁成 x=0、height=5。
        vec![
            // 水平 pass scissor。
            Some(RhiScissor {
                // 裁到左边界。
                x: 0,
                // 保留原 y。
                y: 1,
                // 裁到完整纹理宽度。
                width: 10,
                // 裁到剩余五行。
                height: 5,
            }),
            // 垂直 pass 必须复用相同区域。
            Some(RhiScissor {
                // 裁到左边界。
                x: 0,
                // 保留原 y。
                y: 1,
                // 裁到完整纹理宽度。
                width: 10,
                // 裁到剩余五行。
                height: 5,
            }),
        ]
    );
    // 水平 pass 绑定 source，垂直 pass 绑定 scratch。
    assert_eq!(context.bound_textures, vec![source.raw(), scratch.raw()]);
    // 每个方向恰好执行一次 draw。
    assert_eq!(context.draw_count, 2);
    // 两个 pass 都必须完整结束。
    assert_eq!(context.end_pass_count, 2);
    // 两个 pass 共享一个 device submit。
    assert_eq!(context.submit_count, 1);
    // texture 目标不得 acquire surface image。
    assert_eq!(context.acquire_count, 0);
    // blur 绝不能提前 present。
    assert_eq!(context.present_count, 0);
    // scratch 必须在提交后检查式销毁。
    assert_eq!(context.destroyed_textures, vec![scratch]);

    // 筛出两个固定 304 字节 BlurConstants 更新。
    let uniforms: Vec<Vec<f32>> = context
        // 遍历全部 vertex/uniform 上传。
        .updates
        // 只借用载荷。
        .iter()
        // BlurConstants 必须匹配共享值对象的固定总字节数。
        .filter(|bytes| bytes.len() == BLUR_UNIFORM_BYTES)
        // 还原浮点常量。
        .map(|bytes| decode_f32s(bytes))
        // 收集两个方向。
        .collect();
    // 必须恰好存在水平和垂直两个 uniform。
    assert_eq!(uniforms.len(), 2);
    // 第一组方向为水平且 tap 半径等于 ceil(radius)。
    assert_eq!(
        // 读取共享方向与 tap 半径三项。
        &uniforms[0][BLUR_DIRECTION_TAPS_FLOAT_OFFSET..BLUR_DIRECTION_TAPS_FLOAT_OFFSET + 3],
        // 比较水平像素方向与半径。
        &[1.0, 0.0, 2.0]
    );
    // 第二组方向为垂直且复用同一 tap 半径。
    assert_eq!(
        // 读取共享方向与 tap 半径三项。
        &uniforms[1][BLUR_DIRECTION_TAPS_FLOAT_OFFSET..BLUR_DIRECTION_TAPS_FLOAT_OFFSET + 3],
        // 比较垂直像素方向与半径。
        &[0.0, 1.0, 2.0]
    );
    // 两个方向必须复用完全相同的归一化高斯核。
    assert_eq!(
        // 读取水平 pass 的完整共享权重区间。
        &uniforms[0][BLUR_WEIGHTS_FLOAT_OFFSET..],
        // 比较垂直 pass 的完整共享权重区间。
        &uniforms[1][BLUR_WEIGHTS_FLOAT_OFFSET..]
    );
    // 计算使用槽位的权重总和。
    let weight_sum: f32 = uniforms[0]
        // 只截取五个有效 tap。
        [BLUR_WEIGHTS_FLOAT_OFFSET..BLUR_WEIGHTS_FLOAT_OFFSET + 5]
        // 遍历共享权重值。
        .iter()
        // 累加当前高斯核能量。
        .sum();
    // 五个有效 tap 的能量必须归一化为一。
    assert!((weight_sum - 1.0).abs() < 1.0e-6);
}

// 验证 overlay wrapper 复用已捕获 texture 并保持最终 present 所有权。
#[test]
fn overlay_backdrop_blur_writes_back_to_captured_texture_without_present() {
    // 创建独立 renderer 缓存。
    let mut renderer = RhiRenderer::default();
    // 创建成功型 recording context。
    let mut context = RecordingContext::new(false);
    // 使用已捕获 overlay backdrop 的 opaque 纹理身份。
    let backdrop = TextureHandle::from_raw(72);
    // 执行完整区域的 overlay backdrop blur。
    renderer
        // 调用语义型 wrapper，不让 backend 重复组装目标身份。
        .execute_overlay_backdrop_blur(
            // 记录全部 RHI 命令事实。
            &mut context,
            // source 与最终 target 必须是同一快照纹理。
            backdrop,
            // 使用快照登记的物理 extent。
            RhiExtent::new(6, 4),
            // 模糊完整快照。
            RhiScissor {
                // 从左边界开始。
                x: 0,
                // 从顶边开始。
                y: 0,
                // 覆盖完整宽度。
                width: 6,
                // 覆盖完整高度。
                height: 4,
            },
            // 使用有效半径生成两个方向的高斯核。
            2.0,
        )
        // 合法 backdrop 事务必须成功。
        .expect("overlay backdrop blur should execute");
    // 每次事务只创建一个 scratch texture。
    assert_eq!(context.created_textures.len(), 1);
    // 保存 scratch 身份供顺序断言。
    let scratch = context.created_textures[0];
    // 水平 pass 写 scratch，垂直 pass 原位写回 backdrop。
    assert_eq!(
        // 比较两个 pass 的 target 和 load 动作。
        context.passes,
        // 保持 backdrop→scratch→backdrop 的严格顺序。
        vec![
            // 水平 pass 写入 scratch texture。
            (RenderTargetHandle::for_test(scratch), true),
            // 垂直 pass 写回 backdrop texture。
            (RenderTargetHandle::for_test(backdrop), false),
        ],
    );
    // 两个 pass 共享唯一 device submit。
    assert_eq!(context.submit_count, 1);
    // texture 事务不得 acquire 原生 surface。
    assert_eq!(context.acquire_count, 0);
    // texture 事务不得提前 present。
    assert_eq!(context.present_count, 0);
    // scratch 必须在提交后检查式销毁。
    assert_eq!(context.destroyed_textures, vec![scratch]);
}

// 验证执行失败仍会释放已经创建的 scratch texture。
#[test]
fn blur_submit_failure_still_destroys_scratch() {
    // 创建独立 renderer。
    let mut renderer = RhiRenderer::default();
    // 在 submit 边界注入失败。
    let mut context = RecordingContext::new(true);
    // 使用外部 Picture texture。
    let source = TextureHandle::from_raw(71);
    // 执行合法 blur 并取得 typed failure。
    let error = renderer
        // 调用生产入口。
        .execute_blur_without_present(
            // 记录失败路径事实。
            &mut context,
            // 指定源 texture。
            source,
            // 使用小型合法 extent。
            RhiExtent::new(4, 4),
            // 模糊完整区域。
            RhiScissor {
                // 从左边界开始。
                x: 0,
                // 从上边界开始。
                y: 0,
                // 覆盖完整宽度。
                width: 4,
                // 覆盖完整高度。
                height: 4,
            },
            // 使用有效半径。
            1.0,
            // 使用 Picture BGRA 格式。
            TextureFormat::Bgra8Unorm,
            // 写回源 texture。
            source,
        )
        // submit 失败不得伪装成功。
        .expect_err("submit failure should surface");
    // 保留底层 device-lost 类型。
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    // scratch 在失败前已经创建一次。
    assert_eq!(context.created_textures.len(), 1);
    // 失败路径也必须销毁同一个 scratch。
    assert_eq!(context.destroyed_textures, context.created_textures);
    // 失败路径同样不得 acquire surface。
    assert_eq!(context.acquire_count, 0);
    // 失败路径同样不得 present surface。
    assert_eq!(context.present_count, 0);
}

// 验证裁剪后为空的 region 是无资源、无提交的有序 no-op。
#[test]
fn empty_blur_region_is_a_resource_free_noop() {
    // 创建独立 renderer。
    let mut renderer = RhiRenderer::default();
    // 创建成功型 recording context。
    let mut context = RecordingContext::new(false);
    // 使用外部 Picture texture。
    let source = TextureHandle::from_raw(72);
    // 请求完全位于 texture 外的 blur。
    renderer
        // 调用生产入口。
        .execute_blur_without_present(
            // 记录所有潜在副作用。
            &mut context,
            // 指定源 texture。
            source,
            // 使用四像素 extent。
            RhiExtent::new(4, 4),
            // 把区域放到右下边界之外。
            RhiScissor {
                // 起点超过宽度。
                x: 8,
                // 起点超过高度。
                y: 8,
                // 提供正宽度。
                width: 2,
                // 提供正高度。
                height: 2,
            },
            // 使用有效半径。
            1.0,
            // 使用 Picture BGRA 格式。
            TextureFormat::Bgra8Unorm,
            // 指定原 Picture 目标。
            source,
        )
        // 空区域必须安全成功。
        .expect("empty region should be a no-op");
    // no-op 不应创建 scratch。
    assert!(context.created_textures.is_empty());
    // no-op 不应开始 pass。
    assert!(context.passes.is_empty());
    // no-op 不应提交 device 命令。
    assert_eq!(context.submit_count, 0);
    // no-op 不应 acquire surface。
    assert_eq!(context.acquire_count, 0);
    // no-op 不应 present surface。
    assert_eq!(context.present_count, 0);
}
