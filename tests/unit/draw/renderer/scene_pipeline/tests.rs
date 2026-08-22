// 引入被测场景管线与帧输入输出类型。
use super::*;
// 引入稳定节点身份、图形错误码与测试几何。
use crate::core::{Errc, WidgetId};
// 引入无副作用画布，避免测试创建真实图形设备。
use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
// 引入场景绘制上下文和节点身份。
use crate::draw::painting::PaintContext;
// 引入失败分类，验证 typed error 没有被布尔 fallback 吞掉。
use crate::draw::renderer::GraphicsFailure;
// 引入场景节点别名。
use crate::draw::scene::NodeId;
// 引入画布 trait，供测试 target 返回对象安全引用。
use crate::draw::Canvas2D;

// 固定唯一测试节点，避免每个用例重复构造场景树。
const ROOT_NODE: NodeId = WidgetId::new(1);
// 固定 overlay 子节点，供 clean refresh 两阶段事务测试。
const OVERLAY_NODE: NodeId = WidgetId::new(2);

// 描述本次调用应注入失败的 backdrop 生命周期边界。
#[derive(Clone, Copy, PartialEq, Eq)]
enum BackdropFailurePoint {
    // 不注入失败，用于成功事务测试。
    None,
    // 快照资源事务返回设备丢失。
    Snapshot,
    // 模糊派生事务返回设备丢失。
    Blur,
    // 恢复复制事务返回表面丢失。
    Restore,
    // 显式释放事务返回资源不足。
    Release,
}

// 记录场景管线在 typed failure 前后调用了哪些帧边界。
struct FailingBackdropTarget {
    // 选择当前用例的唯一故障点。
    failure_point: BackdropFailurePoint,
    // 记录是否错误地开始了新帧。
    begin_calls: usize,
    // 记录是否错误地结束并提交了失败帧。
    end_calls: usize,
    // 提供完全无副作用的 Canvas2D 实现。
    canvas: NoopCanvas2D,
    // 记录成功 snapshot 后存在的测试 backdrop owner。
    has_backdrop: bool,
    // 记录 blur 调用的逻辑区域与半径。
    blur_calls: Vec<(Rect, f32)>,
    // 记录正常树中间 FrameEncoder 提交次数。
    encoded_frame_calls: usize,
    // 记录 clean snapshot 次数。
    snapshot_calls: usize,
    // 记录 effect/clean restore 次数。
    restore_calls: usize,
}

// 提供紧凑、确定的测试 target 构造入口。
impl FailingBackdropTarget {
    // 为指定生命周期边界创建故障 target。
    fn new(failure_point: BackdropFailurePoint) -> Self {
        // 初始化所有调用计数为零。
        Self {
            // 保存待注入的唯一故障点。
            failure_point,
            // 尚未开始任何帧。
            begin_calls: 0,
            // 尚未结束任何帧。
            end_calls: 0,
            // 无操作画布不持有原生资源。
            canvas: NoopCanvas2D,
            // Restore 用例从已登记 owner 开始，其余用例等待 snapshot。
            has_backdrop: failure_point == BackdropFailurePoint::Restore,
            // 尚未执行任何 blur。
            blur_calls: Vec::new(),
            // 尚未提交正常树 FrameEncoder。
            encoded_frame_calls: 0,
            // 尚未捕获 clean snapshot。
            snapshot_calls: 0,
            // 尚未恢复 effect/clean。
            restore_calls: 0,
        }
    }
}

// 实现最小 RenderTarget，精确观察失败后的停止边界。
impl RenderTarget for FailingBackdropTarget {
    // 测试 target 不需要真实初始化。
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        // 初始化恒定成功。
        Ok(())
    }

    // 测试 target 不拥有需要销毁的原生资源。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // teardown 恒定成功。
        Ok(())
    }

    // 测试 target 不保存尺寸状态。
    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        // resize 恒定成功。
        Ok(())
    }

    // 记录场景管线是否越过了前置 backdrop 失败。
    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        // 每次进入 begin_frame 都留下可断言证据。
        self.begin_calls += 1;
        // 返回正常可绘制帧，避免引入第二个失败来源。
        RenderOutcome::FrameReady(DamageRegion::full())
    }

    // 记录场景管线是否错误提交了已失败的帧。
    fn end_frame(&mut self, _damage: &DamageRegion) -> RenderOutcome {
        // 每次进入 end_frame 都留下可断言证据。
        self.end_calls += 1;
        // 正常返回 present，若被调用将由计数暴露协议错误。
        RenderOutcome::Present(DamageRegion::full())
    }

    // 返回无副作用画布满足场景管线尺寸查询。
    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        // 借出 target 内唯一画布。
        &mut self.canvas
    }

    // 恢复测试需要进入 GPU-native overlay 路径。
    fn raster_pipeline(&self) -> RasterPipeline {
        // 三个用例统一使用 GPU-native，前置失败仍会在 begin_frame 前返回。
        RasterPipeline::GpuNative
    }

    // 在快照用例注入可恢复的设备丢失。
    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 每次进入 snapshot 都留下事务证据。
        self.snapshot_calls += 1;
        // 只在选定边界返回 typed failure。
        if self.failure_point == BackdropFailurePoint::Snapshot {
            // 使用生产恢复器识别的稳定错误码。
            return Err(Error::new(
                // 映射为 GraphicsFailure::DeviceLost。
                Errc::GraphicsDeviceLost,
                // 提供可诊断的测试错误文本。
                "test overlay backdrop snapshot device lost",
            ));
        }
        // 其余用例登记一个可供 blur/restore 查询的测试 owner。
        self.has_backdrop = true;
        // 向场景管线报告快照成功。
        Ok(true)
    }

    // 测试 target 显式声明真实 blur 能力，允许场景管线进入事务。
    fn supports_backdrop_blur(&self) -> bool {
        // 本 mock 用调用记录模拟通用 RHI renderer。
        true
    }

    // 记录 typed effect 传播，并在指定用例注入设备丢失。
    fn blur_overlay_backdrop(&mut self, region: Rect, radius: f32) -> Result<bool, Error> {
        // 保存 ScenePipeline 实际下发的稳定值。
        self.blur_calls.push((region, radius));
        // 只在选定边界返回 typed failure。
        if self.failure_point == BackdropFailurePoint::Blur {
            // 使用生产恢复器识别的设备丢失分类。
            return Err(Error::new(
                // 保持与真实 RHI submit failure 相同的错误码。
                Errc::GraphicsDeviceLost,
                // 提供可定位的测试诊断。
                "test overlay backdrop blur device lost",
            ));
        }
        // 其余用例报告事务真实执行。
        Ok(true)
    }

    // 在恢复用例注入可恢复的表面丢失。
    fn restore_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 每次进入 restore 都留下事务证据。
        self.restore_calls += 1;
        // 只在选定边界返回 typed failure。
        if self.failure_point == BackdropFailurePoint::Restore {
            // 使用生产恢复器识别的稳定错误码。
            return Err(Error::new(
                // 映射为 GraphicsFailure::SurfaceLost。
                Errc::GraphicsSurfaceLost,
                // 提供可诊断的测试错误文本。
                "test overlay backdrop restore surface lost",
            ));
        }
        // 其余用例保持恢复成功。
        Ok(true)
    }

    // 在释放用例注入不可静默忽略的资源错误。
    fn release_overlay_backdrop(&mut self) -> Result<(), Error> {
        // 只在选定边界返回 typed failure。
        if self.failure_point == BackdropFailurePoint::Release {
            // 使用统一 OOM 分类可识别的稳定错误码。
            return Err(Error::new(
                // 映射为 GraphicsFailure::OutOfMemory。
                Errc::InsufficientResources,
                // 提供可诊断的测试错误文本。
                "test overlay backdrop release resource failure",
            ));
        }
        // Restore 故障用例保留既有模拟 owner，以精确进入恢复失败边界。
        if self.failure_point != BackdropFailurePoint::Restore {
            // 其余用例按真实 release 清除 owner。
            self.has_backdrop = false;
        }
        // 其余用例保持释放成功。
        Ok(())
    }

    // 恢复用例模拟已经登记的同代 backdrop owner。
    fn has_overlay_backdrop(&self) -> bool {
        // 返回 snapshot/release 更新后的 owner 事实。
        self.has_backdrop
    }

    // 模拟正常树 FrameEncoder 已完整写入 retained texture，但尚未 present。
    fn try_execute_encoded_frame(
        // 借用测试 target owner。
        &mut self,
        // 命令内容由场景管线生成，本测试只观察边界次数。
        _encoder: &crate::draw::painting::FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        // 记录唯一中间提交。
        self.encoded_frame_calls += 1;
        // 明确报告完整执行。
        Ok(EncodedFrameExecution::Executed)
    }
}

// 提供一个正常根树与一个 root-level overlay 子节点。
struct RefreshScene {
    // 保存当前帧 typed effect。
    effect: Option<crate::draw::OverlayBackdropEffect>,
    // 决定正常树是否故意产生 CPU 光栅分段。
    cpu_raster_normal: bool,
    // 决定测试浮层是否拥有干净背景快照能力。
    requires_backdrop: bool,
}

// 两节点场景精确触发 normal_tree_dirty 与 overlay membership。
impl ScenePaint for RefreshScene {
    // 返回正常根节点。
    fn root_id(&self) -> Option<NodeId> {
        // 场景始终有效。
        Some(ROOT_NODE)
    }

    // 使用固定场景代际。
    fn tree_version(&self) -> u64 {
        // 与共享帧输入保持一致。
        7
    }

    // 正常树刷新使用全帧 dirty。
    fn dirty_region(&self) -> DirtyRegion {
        // 避免局部剪枝影响事务断言。
        DirtyRegion::full()
    }

    // 两个节点均可见。
    fn node_visible(&self, _id: NodeId) -> bool {
        // 不测试可见性门禁。
        true
    }

    // 正常根覆盖测试 surface，overlay 使用相同逻辑范围。
    fn node_frame(&self, _id: NodeId) -> Rect {
        // 使用确定的 64x48 逻辑区域。
        Rect::new(0.0, 0.0, 64.0, 48.0)
    }

    // 只有正常根声明 dirty。
    fn node_dirty(&self, id: NodeId) -> bool {
        // overlay 自身无需触发 refresh。
        id == ROOT_NODE
    }

    // overlay 在正常根之上。
    fn node_z_index(&self, id: NodeId) -> i32 {
        // 使用稳定的两层排序。
        i32::from(id == OVERLAY_NODE)
    }

    // 正常根包含唯一 overlay 子节点。
    fn node_children(&self, id: NodeId) -> &[NodeId] {
        // 保存稳定静态子节点切片。
        static OVERLAY_CHILD: [NodeId; 1] = [OVERLAY_NODE];
        // 只有根节点公开子关系。
        if id == ROOT_NODE {
            // 返回 overlay 子节点。
            &OVERLAY_CHILD
        } else {
            // overlay 没有后代。
            &[]
        }
    }

    // 只有第二节点属于 overlay。
    fn node_is_overlay(&self, id: NodeId) -> bool {
        // 正常根保持 normal tree 身份。
        id == OVERLAY_NODE
    }

    // 把模态/局部浮层分类显式交给场景管线。
    fn node_requires_overlay_backdrop(&self, id: NodeId) -> bool {
        // 只有指定的 overlay 节点参与背景快照事务。
        id == OVERLAY_NODE && self.requires_backdrop
    }

    // 返回本帧唯一 typed effect。
    fn overlay_backdrop_effect(&self) -> Option<crate::draw::OverlayBackdropEffect> {
        // Copy 值直接交给场景管线。
        self.effect
    }

    // 本测试不使用子树裁剪。
    fn children_clip(&self, _id: NodeId, _frame: Rect) -> Option<Rect> {
        // 保持无裁剪语义。
        None
    }

    // dirty rect 等于节点 frame。
    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        // 原样返回输入几何。
        frame
    }

    // 本测试没有滚动容器。
    fn scroll_offset(&self, _id: NodeId) -> Option<(f32, f32)> {
        // 不产生 scroll copy。
        None
    }

    // 本测试没有焦点节点。
    fn focused_node(&self) -> Option<NodeId> {
        // 返回空焦点。
        None
    }

    // 两个节点都不参与焦点导航。
    fn node_focusable(&self, _id: NodeId) -> bool {
        // 关闭焦点能力。
        false
    }

    // 本测试不执行命中。
    fn hit_test(&self, _pos: Point) -> Option<NodeId> {
        // 返回无命中。
        None
    }

    // 只声明 overlay 的父节点。
    fn parent(&self, id: NodeId) -> Option<NodeId> {
        // overlay 归属正常根。
        (id == OVERLAY_NODE).then_some(ROOT_NODE)
    }

    // 默认使用空绘制，降级用例则在正常树产生一个软件椭圆。
    fn paint(&self, id: NodeId, _frame: Rect, ctx: &mut PaintContext<'_>) {
        // 只在指定用例的正常根节点下发 CPU 光栅操作。
        if self.cpu_raster_normal && id == ROOT_NODE {
            // FrameRecordingCanvas 对椭圆使用可审计的 CPU segment，直接 GPU canvas 仍可正常绘制。
            ctx.fill_ellipse(Rect::new(4.0, 4.0, 12.0, 8.0), crate::draw::Color::blue());
        }
    }
}

// 提供只含一个可选 overlay 根节点的最小场景。
struct StubScene {
    // 空根用于验证浮层离场 release，非空根用于 snapshot/restore。
    root: Option<NodeId>,
    // 保存当前帧已经解析的 typed effect。
    effect: Option<crate::draw::OverlayBackdropEffect>,
}

// 实现最小 ScenePaint，确保测试只观察 backdrop 生命周期。
impl ScenePaint for StubScene {
    // 返回用例选择的根节点。
    fn root_id(&self) -> Option<NodeId> {
        // 复制稳定的可选节点身份。
        self.root
    }

    // 使用固定树代际便于断言输出。
    fn tree_version(&self) -> u64 {
        // 所有用例共享第七代场景。
        7
    }

    // 场景自身声明全帧 dirty。
    fn dirty_region(&self) -> DirtyRegion {
        // 避免局部 damage 干扰恢复用例。
        DirtyRegion::full()
    }

    // 唯一根节点始终可见。
    fn node_visible(&self, _id: NodeId) -> bool {
        // 测试不覆盖可见性裁剪。
        true
    }

    // 提供正尺寸根 frame 供 recorder 建立参考 surface。
    fn node_frame(&self, _id: NodeId) -> Rect {
        // 使用简单十像素正方形。
        Rect::new(0.0, 0.0, 10.0, 10.0)
    }

    // overlay 根本身不代表正常树脏。
    fn node_dirty(&self, _id: NodeId) -> bool {
        // 保持 backdrop 可复用条件。
        false
    }

    // 唯一节点不需要额外层级排序。
    fn node_z_index(&self, _id: NodeId) -> i32 {
        // 使用默认层级零。
        0
    }

    // 唯一节点没有子节点。
    fn node_children(&self, _id: NodeId) -> &[NodeId] {
        // 返回稳定的空切片。
        &[]
    }

    // 非空根明确声明为 overlay。
    fn node_is_overlay(&self, _id: NodeId) -> bool {
        // 仅存在于 snapshot/restore 用例的根就是浮层。
        self.root.is_some()
    }

    // 向 ScenePipeline 提供当前帧唯一 effect 计划。
    fn overlay_backdrop_effect(&self) -> Option<crate::draw::OverlayBackdropEffect> {
        // Copy 值避免测试场景引入共享可变状态。
        self.effect
    }

    // 测试不使用子树裁剪。
    fn children_clip(&self, _id: NodeId, _frame: Rect) -> Option<Rect> {
        // 保持无裁剪语义。
        None
    }

    // dirty rect 等于节点 frame。
    fn dirty_rect(&self, _id: NodeId, frame: Rect) -> Rect {
        // 原样返回输入范围。
        frame
    }

    // 测试场景没有滚动容器。
    fn scroll_offset(&self, _id: NodeId) -> Option<(f32, f32)> {
        // 不引入 scroll copy。
        None
    }

    // 测试场景没有焦点节点。
    fn focused_node(&self) -> Option<NodeId> {
        // 返回空焦点。
        None
    }

    // 唯一节点不参与焦点导航。
    fn node_focusable(&self, _id: NodeId) -> bool {
        // 关闭焦点能力。
        false
    }

    // 测试场景不响应命中测试。
    fn hit_test(&self, _pos: Point) -> Option<NodeId> {
        // 返回无命中。
        None
    }

    // 唯一根节点没有父节点。
    fn parent(&self, _id: NodeId) -> Option<NodeId> {
        // 返回空父级。
        None
    }

    // backdrop 失败应发生在真实 paint 之前。
    fn paint(&self, _id: NodeId, _frame: Rect, _ctx: &mut PaintContext<'_>) {
        // 故意不记录任何绘制命令。
    }
}

// 构造共享的非视觉帧输入，避免每个断言重复协议字段。
fn frame_input<'a>(
    // 借用当前用例的 dirty region。
    dirty_region: &'a DirtyRegion,
    // 借用空字体服务。
    font_service: &'a FontService,
    // 借用空图片服务。
    image_service: &'a ImageService,
) -> FrameRenderInput<'a> {
    // 返回已完成首帧后的 overlay 更新输入。
    FrameRenderInput {
        // 允许场景在 begin_frame 前捕获上一帧 retained 内容。
        rendered_first: true,
        // 使用调用方持有的 dirty region。
        dirty_region,
        // 与 StubScene 的固定代际一致。
        tree_version: 7,
        // 本批不测试滚动搬移。
        scroll_move: None,
        // 空服务使用零号占位字体句柄。
        font: FontHandle::new(0),
        // 传入空字体服务。
        font_service,
        // 传入空图片服务。
        image_service,
        // debug 模式会禁用 backdrop，因此保持关闭。
        debug_mode: false,
        // 本批不测试 hover 调试。
        hover_pos: None,
        // 本批不采集性能指标。
        metrics: None,
    }
}

// 高度重叠的多块脏区应合并为一次包围盒绘制，避免重复遍历场景。
#[test]
fn dense_dirty_region_coalesces_to_one_bounded_rect() {
    let mut region = DirtyRegion::empty();
    for x in [0.0, 4.0, 8.0, 12.0] {
        region.add_rect(Rect::new(x, 0.0, 24.0, 20.0));
    }

    let coalesced = coalesce_dense_dirty_region(region);

    assert_eq!(coalesced.rects(), &[Rect::new(0.0, 0.0, 36.0, 20.0)]);
    assert!(coalesced.clear_required);
}

// 稀疏小块的包围盒额外面积过大，应继续使用离散脏区。
#[test]
fn sparse_dirty_region_keeps_split_rects() {
    let mut region = DirtyRegion::empty();
    for (x, y) in [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)] {
        region.add_rect(Rect::new(x, y, 4.0, 4.0));
    }
    let expected = region.clone();

    assert_eq!(coalesce_dense_dirty_region(region), expected);
}

// 验证浮层离场释放失败会在任何新帧动作前终止。
#[test]
fn overlay_backdrop_release_failure_stops_before_begin_frame() {
    // 创建只在 release 边界失败的 target。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::Release);
    // 空根表示所有 overlay 已离场。
    let scene = StubScene {
        // 空根表示所有 overlay 已离场。
        root: None,
        // 离场场景没有效果请求。
        effect: None,
    };
    // 全帧 dirty 确保若错误被吞掉就会进入 begin_frame。
    let dirty = DirtyRegion::full();
    // 创建无需加载真实字体的服务。
    let fonts = FontService::new();
    // 创建无需加载真实图片的服务。
    let images = ImageService::new();
    // 执行被测场景边界。
    let output = ScenePipeline::new().render_frame(
        // 传入故障 target。
        &mut target,
        // 传入空 overlay 场景。
        &scene,
        // 传入共享帧参数。
        frame_input(&dirty, &fonts, &images),
    );
    // 资源不足必须保留为 OOM 分类。
    assert!(matches!(
        // 检查场景管线最终结果。
        output.outcome,
        // 不允许退化为 Idle 或普通 full redraw。
        RenderOutcome::Failed(GraphicsFailure::OutOfMemory(_))
    ));
    // 前置释放失败后不得开始新帧。
    assert_eq!(target.begin_calls, 0);
    // 未开始的帧更不得进入最终提交。
    assert_eq!(target.end_calls, 0);
}

// 验证快照设备丢失不会被解释为不支持 backdrop。
#[test]
fn overlay_backdrop_snapshot_device_lost_reaches_frame_failure() {
    // 创建只在 snapshot 边界失败的 target。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::Snapshot);
    // 非空根触发 overlay 快照逻辑。
    let scene = StubScene {
        // 使用固定 overlay 根节点。
        root: Some(ROOT_NODE),
        // 快照失败用例不需要额外效果。
        effect: None,
    };
    // 全帧 dirty 允许后续帧动作具备可观察性。
    let dirty = DirtyRegion::full();
    // 创建无需加载真实字体的服务。
    let fonts = FontService::new();
    // 创建无需加载真实图片的服务。
    let images = ImageService::new();
    // 执行被测场景边界。
    let output = ScenePipeline::new().render_frame(
        // 传入故障 target。
        &mut target,
        // 传入 overlay 场景。
        &scene,
        // 传入共享帧参数。
        frame_input(&dirty, &fonts, &images),
    );
    // 设备丢失必须抵达恢复器可识别的失败分类。
    assert!(matches!(
        // 检查场景管线最终结果。
        output.outcome,
        // 不允许降级为 Ok(false) 后继续整树重绘。
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
    ));
    // 快照失败发生在 begin_frame 之前。
    assert_eq!(target.begin_calls, 0);
    // 快照失败后不得进入最终提交。
    assert_eq!(target.end_calls, 0);
}

// 验证 begin_frame 后的恢复失败仍会阻止 paint/end/present。
#[test]
fn overlay_backdrop_restore_surface_lost_stops_before_end_frame() {
    // 创建只在 restore 边界失败的 target。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::Restore);
    // 非空根触发 GPU-native overlay 恢复逻辑。
    let scene = StubScene {
        // 使用固定 overlay 根节点。
        root: Some(ROOT_NODE),
        // 恢复失败用例不需要额外效果。
        effect: None,
    };
    // 全帧 dirty 使 overlay backdrop 具备恢复资格。
    let dirty = DirtyRegion::full();
    // 创建无需加载真实字体的服务。
    let fonts = FontService::new();
    // 创建无需加载真实图片的服务。
    let images = ImageService::new();
    // 执行被测场景边界。
    let output = ScenePipeline::new().render_frame(
        // 传入故障 target。
        &mut target,
        // 传入 overlay 场景。
        &scene,
        // 传入共享帧参数。
        frame_input(&dirty, &fonts, &images),
    );
    // 表面丢失必须抵达恢复器可识别的失败分类。
    assert!(matches!(
        // 检查场景管线最终结果。
        output.outcome,
        // 不允许降级为普通整树重绘并 present。
        RenderOutcome::Failed(GraphicsFailure::SurfaceLost(_))
    ));
    // restore 位于 begin_frame 之后，因此应恰好开始一次帧。
    assert_eq!(target.begin_calls, 1);
    // restore 失败后不得进入 end_frame 或 present。
    assert_eq!(target.end_calls, 0);
}

// 验证 UI typed effect 会在 begin_frame 前驱动唯一 blur，且失败不会被 mask fallback 吞掉。
#[test]
fn overlay_backdrop_blur_failure_stops_before_begin_frame() {
    // 创建只在 blur 边界失败的 target。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::Blur);
    // 创建有效逻辑区域与半径。
    let effect = crate::draw::OverlayBackdropEffect::new(
        // 使用非原点区域证明值按原样传播。
        Rect::new(2.0, 3.0, 40.0, 24.0),
        // 使用非默认半径证明显式值不会被替换。
        9.0,
    )
    // 测试输入必须满足 typed 契约。
    .expect("test backdrop effect should be valid");
    // 非空 overlay 根请求真实 blur。
    let scene = StubScene {
        // 使用固定 overlay 根节点。
        root: Some(ROOT_NODE),
        // 下发已经解析的唯一计划。
        effect: Some(effect),
    };
    // 全帧 dirty 保持失败前置边界可观察。
    let dirty = DirtyRegion::full();
    // 创建无需真实字体的服务。
    let fonts = FontService::new();
    // 创建无需真实图片的服务。
    let images = ImageService::new();
    // 执行 snapshot→blur 前置事务。
    let output = ScenePipeline::new().render_frame(
        // 传入故障 target。
        &mut target,
        // 传入带 typed effect 的 overlay 场景。
        &scene,
        // 传入共享帧参数。
        frame_input(&dirty, &fonts, &images),
    );
    // blur 的设备丢失必须抵达恢复层分类。
    assert!(matches!(
        // 检查最终失败结果。
        output.outcome,
        // 不允许静默降级并继续 present。
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
    ));
    // snapshot 后只能下发一次聚合 blur。
    assert_eq!(target.blur_calls, vec![(effect.region(), effect.radius())]);
    // 前置 blur 失败后不得 acquire/begin 新帧。
    assert_eq!(target.begin_calls, 0);
    // 更不得结束或呈现失败帧。
    assert_eq!(target.end_calls, 0);
}

// 验证正常树变化通过一次中间提交重建 clean/effect，最终只 begin/end/present 一次。
#[test]
fn overlay_backdrop_refresh_rebuilds_clean_source_before_single_present() {
    // 创建不注入失败的 retained GPU mock。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::None);
    // 创建有效独立区域与显式半径。
    let effect = crate::draw::OverlayBackdropEffect::new(
        // 使用部分区域证明 typed 值原样进入 blur。
        Rect::new(4.0, 5.0, 32.0, 20.0),
        // 使用非默认半径。
        7.0,
    )
    // 测试 effect 必须有效。
    .expect("refresh effect should be valid");
    // 正常根 dirty，overlay 子节点保持可见。
    let scene = RefreshScene {
        // 成功事务使用合法原生正常树。
        effect: Some(effect),
        // 不注入 CPU 光栅分段。
        cpu_raster_normal: false,
        // 显式要求建立干净背景。
        requires_backdrop: true,
    };
    // refresh 从完整 dirty 区域开始。
    let dirty = DirtyRegion::full();
    // 创建空字体服务。
    let fonts = FontService::new();
    // 创建空图片服务。
    let images = ImageService::new();
    // 构造首帧输入以覆盖 overlay 初始即打开的真实窗口路径。
    let mut input = frame_input(&dirty, &fonts, &images);
    // 首帧尚无可复用历史 surface，必须由中间正常树提交建立 clean source。
    input.rendered_first = false;
    // 执行两阶段 retained 事务。
    let output = ScenePipeline::new().render_frame(
        // 传入成功 target。
        &mut target,
        // 传入 normal+overlay 场景。
        &scene,
        // 传入首帧共享输入。
        input,
    );
    // 最终帧必须进入 backend-managed 或外部 presenter 的成功提交边界。
    assert!(matches!(
        // 测试 target 默认使用外部 presenter 能力，因此会返回 PresentPending。
        output.outcome,
        // 两种生产提交模式都代表唯一最终 present 已准备完成。
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));
    // 正常树只进行一次无 present 的 FrameEncoder 中间提交。
    assert_eq!(target.encoded_frame_calls, 1);
    // 新 clean snapshot 只捕获一次。
    assert_eq!(target.snapshot_calls, 1);
    // 新 effect 只从 clean 派生一次。
    assert_eq!(target.blur_calls, vec![(effect.region(), effect.radius())]);
    // effect/clean 只恢复一次。
    assert_eq!(target.restore_calls, 1);
    // 整个事务只 begin 一次。
    assert_eq!(target.begin_calls, 1);
    // 整个事务只 end/present 一次。
    assert_eq!(target.end_calls, 1);
}

// 验证正常树含 CPU 光栅分段时放弃 backdrop 优化，但仍完成当前 GPU 帧。
#[test]
fn overlay_backdrop_refresh_falls_back_for_cpu_raster_normal_tree() {
    // 创建不注入资源失败的 retained GPU mock。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::None);
    // 创建有效效果请求以触发首帧 backdrop refresh。
    let effect = crate::draw::OverlayBackdropEffect::new(
        // 使用稳定部分区域。
        Rect::new(4.0, 5.0, 32.0, 20.0),
        // 使用有效模糊半径。
        7.0,
    )
    // 测试 effect 必须有效。
    .expect("fallback effect should be valid");
    // 构造会在 API-neutral recorder 中产生 CPU segment 的正常树。
    let scene = RefreshScene {
        // 保留同一 typed effect。
        effect: Some(effect),
        // 显式打开 CPU 光栅注入。
        cpu_raster_normal: true,
        // 显式要求建立干净背景。
        requires_backdrop: true,
    };
    // 使用完整 dirty 区域模拟初次展示浮层。
    let dirty = DirtyRegion::full();
    // 创建空字体服务。
    let fonts = FontService::new();
    // 创建空图片服务。
    let images = ImageService::new();
    // 构造首帧输入。
    let mut input = frame_input(&dirty, &fonts, &images);
    // 首帧尚无可复用的历史表面。
    input.rendered_first = false;
    // 保存管线以审计降级状态。
    let mut pipeline = ScenePipeline::new();
    // 执行含非原生正常树的首帧。
    let output = pipeline.render_frame(
        // 传入可成功直绘的 GPU target。
        &mut target,
        // 传入正常树与 overlay 场景。
        &scene,
        // 移交首帧输入。
        input,
    );
    // 降级不得把能力缺口升级为窗口帧失败。
    assert!(matches!(
        // 读取真实帧结果。
        output.outcome,
        // 测试 target 会在一次 begin/end 后报告 Present。
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));
    // 非原生编码器必须在进入 backend 前被识别。
    assert_eq!(target.encoded_frame_calls, 0);
    // 降级不得从未提交的正常树捕获快照。
    assert_eq!(target.snapshot_calls, 0);
    // 降级帧仍只开始一次最终帧。
    assert_eq!(target.begin_calls, 1);
    // 降级帧仍只结束并呈现一次。
    assert_eq!(target.end_calls, 1);
    // 当前 overlay 生命周期必须禁止重试同一 retained 优化。
    assert!(pipeline.overlay_backdrop_blocked);
}

// 非模态局部浮层不应为页面切换建立整窗背景快照。
#[test]
fn local_overlay_skips_backdrop_refresh_transaction() {
    // 创建可记录所有背景资源事务的 retained GPU mock。
    let mut target = FailingBackdropTarget::new(BackdropFailurePoint::None);
    // 正常根与局部 overlay 都存在，但局部浮层不需要干净背景 owner。
    let scene = RefreshScene {
        // 局部浮层未声明 blur 效果。
        effect: None,
        // 正常树可直接走 GPU-native 绘制。
        cpu_raster_normal: false,
        // Message/Notification 等局部浮层不申请背景快照。
        requires_backdrop: false,
    };
    // 页面切换仍可要求完整表面重建。
    let dirty = DirtyRegion::full();
    // 创建空字体与图片服务。
    let fonts = FontService::new();
    let images = ImageService::new();
    // 模拟首次展示该页面，验证不会进入两阶段 refresh。
    let mut input = frame_input(&dirty, &fonts, &images);
    input.rendered_first = false;

    let output = ScenePipeline::new().render_frame(&mut target, &scene, input);

    assert!(matches!(
        output.outcome,
        RenderOutcome::Present(_) | RenderOutcome::PresentPending(_)
    ));
    // 正常树与局部浮层应在唯一最终帧直接绘制。
    assert_eq!(target.encoded_frame_calls, 0);
    assert_eq!(target.snapshot_calls, 0);
    assert_eq!(target.restore_calls, 0);
    assert_eq!(target.begin_calls, 1);
    assert_eq!(target.end_calls, 1);
}
