//! overlay backdrop 薄 RHI 复制与失败清理测试。

// 引入统一错误和结果类型。
use crate::core::{Errc, Error, Rect, Result};
// 引入被测 helper。
use super::render_backend_backdrop::{
    // 快照 helper 负责创建、复制、提交和失败清理。
    create_rhi_overlay_backdrop,
    // 区域 helper 负责逻辑坐标、DPR 与物理 extent 的唯一 lowering。
    lower_overlay_blur_region,
    // 恢复 helper 负责反向复制与唯一提交。
    restore_rhi_overlay_backdrop,
};
// 引入 recording context 所需的薄 RHI 契约和值类型。
use crate::native::present::rhi::{
    // draw packet 只用于拒绝意外绘制调用。
    DrawPacket,
    // device trait 提供被测资源原语。
    GraphicsDevice,
    // 能力 profile 声明 recording device 基线。
    GraphicsDeviceCapabilities,
    // surface trait 提供 acquire/present 负面观测。
    GraphicsSurface,
    // load action 只用于拒绝意外 render pass。
    LoadAction,
    // render target 用于完整 mock surface 契约。
    RenderTargetHandle,
    // extent 保存固定物理尺寸。
    RhiExtent,
    // 不可拆呈现事务用于补齐 Surface 契约。
    RhiPresentTransaction,
    // scissor 与 viewport 只用于拒绝意外 raster 状态。
    RhiScissor,
    // viewport 类型补齐 device trait。
    RhiViewport,
    // submission 建立唯一提交证据。
    SubmissionHandle,
    // surface frame 补齐 acquire/present 契约。
    SurfaceFrame,
    // token 提供稳定 generation 与 extent。
    SurfaceToken,
    // copy 载荷用于审计方向和范围。
    TextureCopy,
    // texture 描述用于校验创建事实。
    TextureDesc,
    // texture 格式用于校验 BGRA 一致性。
    TextureFormat,
    // opaque texture 句柄区分 source/destination。
    TextureHandle,
};

// 记录 backdrop helper 可触碰的所有 device/surface 事实。
struct RecordingBackdropContext {
    // 保存测试 surface 的稳定代际与尺寸。
    token: SurfaceToken,
    // 分配稳定且非零的资源身份。
    next_resource: u64,
    // 记录创建的纹理。
    created: Vec<TextureHandle>,
    // 记录检查式销毁的纹理。
    destroyed: Vec<TextureHandle>,
    // 记录所有有向纹理复制。
    copies: Vec<TextureCopy>,
    // 记录 device submit 次数。
    submits: usize,
    // 记录意外 surface acquire 次数。
    acquires: usize,
    // 记录意外 surface present 次数。
    presents: usize,
    // 允许测试在创建边界注入内存失败。
    fail_create: bool,
    // 允许测试在 submit 边界注入设备丢失。
    fail_submit: bool,
}

// 为 recording context 提供可读构造器。
impl RecordingBackdropContext {
    // 创建指定失败模式的固定 80×50 surface。
    fn new(fail_create: bool, fail_submit: bool) -> Self {
        // 返回尚未发生任何命令的记录器。
        Self {
            // 使用可辨识的第五代 surface。
            token: SurfaceToken::new(5, RhiExtent::new(80, 50)),
            // 从 100 开始分配，避免与手写 source handle 混淆。
            next_resource: 100,
            // 初始没有创建资源。
            created: Vec::new(),
            // 初始没有销毁资源。
            destroyed: Vec::new(),
            // 初始没有复制命令。
            copies: Vec::new(),
            // 初始没有提交。
            submits: 0,
            // 初始没有获取 surface image。
            acquires: 0,
            // 初始没有呈现 surface image。
            presents: 0,
            // 保存创建失败模式。
            fail_create,
            // 保存提交失败模式。
            fail_submit,
        }
    }

    // 分配下一个 opaque 资源身份。
    fn allocate(&mut self) -> u64 {
        // 保存当前身份作为返回值。
        let raw = self.next_resource;
        // 推进后续分配身份。
        self.next_resource += 1;
        // 返回本次身份。
        raw
    }
}

// 实现 backdrop helper 实际使用的 device 原语。
impl GraphicsDevice for RecordingBackdropContext {
    // 声明完整 GPU 基线，聚焦资源与命令顺序。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // 返回带纹理复制能力的稳定 profile。
        GraphicsDeviceCapabilities::full_gpu_baseline()
    }

    // 创建独立 backdrop texture。
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        // helper 必须保持 surface token 的完整物理尺寸。
        assert_eq!(desc.extent(), self.token.extent);
        // backdrop 必须与 retained 主颜色目标保持 BGRA 格式一致。
        assert_eq!(desc.format(), TextureFormat::Bgra8Unorm);
        // 按测试配置注入创建失败。
        if self.fail_create {
            // 返回可恢复的 GPU 内存错误。
            return Err(Error::new(
                Errc::GraphicsOutOfMemory,
                "recorded backdrop texture allocation failed",
            ));
        }
        // 分配新纹理身份。
        let texture = TextureHandle::from_raw(self.allocate());
        // 保存创建事实。
        self.created.push(texture);
        // 返回新纹理。
        Ok(texture)
    }

    // 检查式销毁失败事务创建的纹理。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        // 保存销毁事实用于和创建记录配对。
        self.destroyed.push(texture);
        // 记录成功。
        Ok(())
    }

    // 未使用的 pass 创建入口显式拒绝意外调用。
    fn begin_render_pass(
        // 借用记录器。
        &mut self,
        // 忽略不应出现的目标。
        _target: RenderTargetHandle,
        // 忽略不应出现的 load action。
        _load: LoadAction,
    ) -> Result<()> {
        // backdrop 纹理复制不允许开启 render pass。
        panic!("backdrop copy must not begin a render pass")
    }

    // 未使用的 viewport 入口显式拒绝意外调用。
    fn set_viewport(&mut self, _viewport: RhiViewport) -> Result<()> {
        // 纯复制事务不设置 viewport。
        panic!("backdrop copy must not set a viewport")
    }

    // 未使用的 scissor 入口显式拒绝意外调用。
    fn set_scissor(&mut self, _scissor: Option<RhiScissor>) -> Result<()> {
        // 全幅 texture copy 不通过 raster scissor 裁剪。
        panic!("backdrop copy must not set a scissor")
    }

    // 未使用的 draw 入口显式拒绝意外调用。
    fn draw(&mut self, _packet: DrawPacket) -> Result<()> {
        // 快照和恢复只允许复制，不能用采样绘制冒充。
        panic!("backdrop copy must not draw")
    }

    // 记录有向全幅纹理复制。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        // 保存完整复制载荷。
        self.copies.push(copy);
        // 记录成功。
        Ok(())
    }

    // 未使用的 pass 结束入口显式拒绝意外调用。
    fn end_render_pass(&mut self) -> Result<()> {
        // 没有 begin 就不允许出现 end。
        panic!("backdrop copy must not end a render pass")
    }

    // 提交一次纯 device 复制事务。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        // 记录每次提交事实。
        self.submits += 1;
        // 按测试配置注入设备丢失。
        if self.fail_submit {
            // 返回 typed device-lost 失败。
            return Err(Error::new(
                Errc::GraphicsDeviceLost,
                "recorded backdrop submit failed",
            ));
        }
        // 返回与提交序号对应的稳定句柄。
        Ok(SubmissionHandle::from_raw(self.submits as u64))
    }
}

// 实现 surface 契约以证明 helper 不 acquire/present。
impl GraphicsSurface for RecordingBackdropContext {
    // 返回固定 surface token。
    fn token(&self) -> SurfaceToken {
        // 保持第五代 80×50 surface。
        self.token
    }

    // 记录任何意外 acquire。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // 增加可观察计数。
        self.acquires += 1;
        // 返回有效 frame 以免错误路径因 mock 本身中断。
        Ok(SurfaceFrame::new(
            self.token,
            RenderTargetHandle::from_raw(900),
        ))
    }

    // 本组测试不允许 resize。
    fn resize(&mut self, _extent: RhiExtent) -> Result<SurfaceToken> {
        // resize 不是快照/恢复事务的一部分。
        panic!("backdrop copy must not resize the surface")
    }

    // 记录任何意外 present。
    fn present(
        // 借用记录器。
        &mut self,
        // 忽略不应出现的完整呈现事务。
        _transaction: RhiPresentTransaction,
    ) -> Result<()> {
        // 增加可观察计数。
        self.presents += 1;
        // 返回成功，使断言负责揭示意外调用。
        Ok(())
    }
}

// 验证快照按 retained→backdrop 全幅复制且只提交 device。
#[test]
fn snapshot_creates_and_submits_full_retained_copy_without_present() {
    // 创建正常 recording context。
    let mut context = RecordingBackdropContext::new(false, false);
    // 使用可辨识的 retained source。
    let retained = TextureHandle::from_raw(7);
    // 在可变借用 context 前复制 surface extent。
    let extent = context.token.extent;
    // 执行快照事务。
    let backdrop = match create_rhi_overlay_backdrop(&mut context, retained, extent) {
        // 成功时取得新纹理身份。
        Ok(texture) => texture,
        // 合法记录器不应失败。
        Err(error) => panic!("backdrop snapshot should succeed: {error:?}"),
    };
    // 新 backdrop 必须是唯一创建资源。
    assert_eq!(context.created, vec![backdrop]);
    // 成功事务不得销毁可复用快照。
    assert!(context.destroyed.is_empty());
    // 只允许一次纹理复制。
    let [copy] = context.copies.as_slice() else {
        // 额外或缺失复制都破坏原子边界。
        panic!("expected one snapshot copy");
    };
    // 复制方向必须从 retained 指向新 backdrop。
    assert_eq!((copy.source(), copy.destination()), (retained, backdrop));
    // 偏移和尺寸必须覆盖完整 80×50 surface。
    let source_region = copy.transfer().source();
    // 目标区域必须由同一传输尺寸派生。
    let destination_region = copy.transfer().destination();
    assert_eq!(
        (
            source_region.origin().x(),
            source_region.origin().y(),
            destination_region.origin().x(),
            destination_region.origin().y(),
            copy.transfer().extent().width,
            copy.transfer().extent().height,
        ),
        (0, 0, 0, 0, 80, 50)
    );
    // 快照只产生一次 device submit。
    assert_eq!(context.submits, 1);
    // 快照不得获取或呈现 swapchain image。
    assert_eq!((context.acquires, context.presents), (0, 0));
}

// 验证恢复按 backdrop→retained 反向复制并保持唯一提交。
#[test]
fn restore_submits_full_backdrop_copy_without_present() {
    // 创建正常 recording context。
    let mut context = RecordingBackdropContext::new(false, false);
    // 使用可区分的源和目标身份。
    let backdrop = TextureHandle::from_raw(11);
    // 指定当前 retained surface。
    let retained = TextureHandle::from_raw(12);
    // 在可变借用 context 前复制 surface extent。
    let extent = context.token.extent;
    // 执行恢复事务。
    let result = restore_rhi_overlay_backdrop(&mut context, backdrop, retained, extent);
    // 正常记录器必须提交成功。
    assert!(result.is_ok());
    // 恢复不创建或销毁任何资源。
    assert!(context.created.is_empty() && context.destroyed.is_empty());
    // 只允许一个反向复制。
    let [copy] = context.copies.as_slice() else {
        // 额外命令说明恢复边界不再原子。
        panic!("expected one restore copy");
    };
    // 复制方向必须从 backdrop 指向 retained。
    assert_eq!((copy.source(), copy.destination()), (backdrop, retained));
    // 恢复同样覆盖完整物理尺寸。
    assert_eq!(
        (
            copy.transfer().extent().width,
            copy.transfer().extent().height
        ),
        (80, 50)
    );
    // 恢复只提交一次且不触碰 surface present。
    assert_eq!(
        (context.submits, context.acquires, context.presents),
        (1, 0, 0)
    );
}

// 验证创建失败不会产生半成品命令或伪造清理。
#[test]
fn snapshot_create_failure_leaves_no_resource_or_commands() {
    // 在 texture 创建边界注入内存失败。
    let mut context = RecordingBackdropContext::new(true, false);
    // 在可变借用 context 前复制 surface extent。
    let extent = context.token.extent;
    // 尝试从稳定 retained source 捕获背景。
    let result = create_rhi_overlay_backdrop(&mut context, TextureHandle::from_raw(21), extent);
    // 错误分类必须保持 GPU 内存失败。
    assert!(matches!(result, Err(error) if error.code() == Errc::GraphicsOutOfMemory));
    // 创建失败前后都没有归属不明的资源。
    assert!(context.created.is_empty() && context.destroyed.is_empty());
    // 没有资源时不能继续复制或提交。
    assert!(context.copies.is_empty() && context.submits == 0);
    // surface 生命周期同样保持未触碰。
    assert_eq!((context.acquires, context.presents), (0, 0));
}

// 验证 submit 失败会检查式销毁刚创建的 backdrop。
#[test]
fn snapshot_submit_failure_destroys_new_texture_without_present() {
    // 在纯 device submit 边界注入设备丢失。
    let mut context = RecordingBackdropContext::new(false, true);
    // 在可变借用 context 前复制 surface extent。
    let extent = context.token.extent;
    // 执行必然失败的快照事务。
    let result = create_rhi_overlay_backdrop(&mut context, TextureHandle::from_raw(31), extent);
    // 错误分类必须保持设备丢失。
    assert!(matches!(result, Err(error) if error.code() == Errc::GraphicsDeviceLost));
    // 新纹理必须创建一次并被同身份销毁一次。
    assert_eq!(context.destroyed, context.created);
    // 复制已编码且 submit 只尝试一次。
    assert_eq!((context.copies.len(), context.submits), (1, 1));
    // 失败清理仍不得 acquire 或 present。
    assert_eq!((context.acquires, context.presents), (0, 0));
}

// 验证 overlay blur 区域只应用一次主 surface DPR 并裁到快照 extent。
#[test]
fn overlay_blur_region_uses_surface_dpr_and_snapshot_extent() {
    // 使用会产生分数物理边界的逻辑区域。
    let region = Rect::new(1.25, 2.5, 4.0, 3.0);
    // 以 1.5 DPR lower 到 10x8 的 backdrop texture。
    let physical = lower_overlay_blur_region(
        // 传入逻辑区域。
        region,
        // 传入原子 surface 快照中的 DPR。
        1.5,
        // 底边会越过物理高度并被裁剪。
        RhiExtent::new(10, 8),
    );
    // 左上向外取整，右下按物理 extent 裁剪。
    assert_eq!(
        // 比较完整物理 scissor。
        physical,
        // 逻辑边界映射为 [1,3] 到 [8,8]。
        RhiScissor {
            // floor(1.25 * 1.5)。
            x: 1,
            // floor(2.5 * 1.5)。
            y: 3,
            // ceil(5.25 * 1.5) - 1。
            width: 7,
            // 物理底边裁到 8。
            height: 5,
        },
    );
}

// 验证非法逻辑区域不会创建可执行的物理 scissor。
#[test]
fn overlay_blur_region_rejects_invalid_geometry() {
    // 使用非有限横坐标触发稳定空区域。
    let physical = lower_overlay_blur_region(
        // 直接构造异常值，避免 Rect::new 的 NaN 安全归一化改变测试输入。
        Rect {
            // 保留非有限横坐标。
            x: f32::NAN,
            // 其余几何保持合法。
            y: 0.0,
            // 使用正宽度。
            w: 4.0,
            // 使用正高度。
            h: 4.0,
        },
        // 使用合法 DPR。
        2.0,
        // 使用合法快照 extent。
        RhiExtent::new(8, 8),
    );
    // 空区域由通用 renderer 作为无资源 no-op。
    assert_eq!(
        // 比较完整空 scissor。
        physical,
        // 零尺寸不会进入 render pass。
        RhiScissor {
            // 保持零起点。
            x: 0,
            // 保持零起点。
            y: 0,
            // 空宽度。
            width: 0,
            // 空高度。
            height: 0,
        },
    );
}
