# backend 模块

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统的目标 `backend` 模块及 CPU/GPU 执行契约，权威持有“通用 UI GPU Renderer + 薄原生 RHI”的分层。依赖：[painting](painting.md)、[platform/presentation](../platform/presentation.md)。导出：renderer 内部 `RenderBackend`。

> **设计状态**：🔄 迁移中。通用 FramePlan、retained RHI、D3D11 与 OpenGL ES 执行路径已经落地；本轮完成了 native graphics context 的类型化 recipe 收口。剩余工作集中在 effect 调度与真实环境矩阵，不把机器测试等同于所有者视觉验收。

> **SMC 所有权**：graphics/backend System 拥有 FramePlan、fallback、Picture/effect 与 renderer capability 投影；platform/presentation System 只拥有原生 device/surface、thin RHI 实现、线程亲和与最终提交能力。两侧只通过 `GraphicsRecipeOwner`、`GpuRecipeContext`、`PixelUploadSurface`、`GraphicsDevice` 与 `GraphicsSurface` 交互。

> **类型化构造**：adapter 创建事务直接选择 `GraphicsContextCandidate::gpu` 或 `GraphicsContextCandidate::pixel_upload` 并交付同源 `GraphicsContextCaps`。registry 校验 registry row、静态 recipe 轴与 `GraphicsRecipeContext` 分支一致；错配 checked shutdown。GPU 分支再执行基线与固定 pipeline probe，PixelUpload 分支不触碰 RHI。

> **owner 收口**：native factory 在 context 离开 platform 前构造 `GraphicsRecipeOwner::{Gpu, PixelUpload}`。`GpuRecipeOwner` 直接持有 `Box<dyn GpuRecipeContext>`，`PixelUploadRecipeOwner` 直接持有 `Box<dyn PixelUploadSurface>`；没有运行期 Option 能力查询、视图丢失或统一门面回退。

> **renderer 装配**：bootstrap 与 recovery 共用 `assemble_renderer`，只把已验证 recipe owner 交给 `Renderer::from_recipe_owner`。RenderSession、draw/backend 与 draw/renderer 不暂存原生 context，也不建立第二条 native 生命周期。

> **surface 生命周期**：GPU resize 通过同一 `GpuRecipeContext` 进入唯一 `GraphicsSurface::resize`；CPU PixelUpload resize/提交只通过 `PixelUploadSurface`。thread-bound wrapper 仅在底层成功后刷新完整 `PresentSurface`，旧代 retained/overlay 资源继续在 generation 推进前检查式释放。

> **能力与提交**：生产 GPU backend 从同一次 `GraphicsCapabilities` 快照验证 GPU baseline 并派生 `NativeRasterCaps`；逐 UI capability 不在 adapter 平行声明。局部重绘只在 retained 主颜色目标和 `TrackedSwapchain` image 身份同时成立时开放；GPU 最终提交只由 `GraphicsSurface::present` 持有，CPU PixelUpload 最终提交只由 `PixelUploadSurface::present_pixels` 持有。

> **坐标与行序契约**：通用逻辑层只使用左上原点逻辑坐标，不得感知平台行序；thin RHI 的坐标与行序契约由 [platform/presentation](../platform/presentation.md) 权威持有（左上原点输入、readback top-left 输出、目标行序差异由 adapter 内消化），本模块作为消费方引用。graphics 侧始终把 texture `v=0` 视为顶部：CPU 软渲染/upload 的像素数据保持 top-left 直通，UV 跟随顶点；Picture、retained surface、sampled 合成和 blur 的每个 texture pass 都保持同一 top-left 存储，不依赖偶数次翻转抵消；机械拷贝（GPU 快照 copy、滚动 memmove）逐 texel 对位、与行序无关。OpenGL adapter 按 render target 在 draw 时选择 texture 与 native surface 的 Y 映射，并在 surface readback 内完成区域坐标换算和行反转；通用 graphics 不叠加平台翻转。

> **当前实现线索**：通用代码位于 `src/draw/backend/`，thin RHI 契约位于 `src/native/present/rhi.rs`，类型化 recipe 生命周期位于 `src/native/present/traits.rs`，candidate/owner 位于 `src/native/present/`，registry 与线程绑定在 `src/native/factory/`，生产 adapter 位于 `src/native/presentation/graphics/`。

## fallback 边界

CPU backend 继续作为完整、可验证的 renderer，而不是每个 native adapter 的补丁集合。帧内 GPU→CPU fallback 只有在像素语义、painter order、clip/opacity/blend 与最终提交协议均可保持时才允许；否则返回 typed failure，由 renderer 的恢复策略选择整后端重建或下一帧重试。连续 Additive 填充、描边、渐变、raw image、glyph coverage 与 box shadow 可以利用逐通道饱和加法的结合律，在透明 scratch 中累积源贡献，再以不透明 opacity 封装为紧边界 Additive sampled tile；描边轮廓必须先在本地空间解析 width、cap、join 与 miter，再执行 offset 后的完整仿射；线性、径向渐变、raw image、glyph 与定向/环境阴影必须把设备像素中心逆映射到 offset 后的局部空间求值，图片与 coverage 保持 point sampling，字形颜色按 opacity 和 coverage 各调制一次，阴影 offset、corner radius、blur 与 coverage 曲线在 transform 前按局部语义解析。blend 切换与 `save`/`restore` 状态切换必须形成 painter barrier，变换、裁剪和 opacity 只能烘焙一次。路径 coverage clip 仅在可强制使用 source scratch 的 Additive 状态下准入。Picture/offscreen blur 在 Picture texture 空间裁剪 region，RHI 按 source→scratch→原 Picture 执行两个同核正交 pass；它不获取或呈现主 surface，scratch 在成功和提交失败后都检查式释放。blur 后的 Picture sampled quad 从当前目标继承有限 opacity 与 SrcOver/Additive；CPU 主表面合成会临时建立 identity/zero-offset 的 surface-space 几何，再恢复调用方状态。overlay 干净背景快照与恢复分别执行 retained→backdrop 和 backdrop→retained 的全幅 texture copy，二者只 submit device 命令、不 acquire/present；创建或 submit 失败不登记快照，resize、legacy 降级、shutdown 与显式 release 均检查式回收该纹理。快照、模糊、恢复或释放中的 create/copy/draw/submit/destroy/device-maintenance 失败通过 `RenderBackend` 与 `RenderTarget` 保持 typed error；`RecoveryDriver` 统一登记无帧事务失败，场景接线后仍由既有有界恢复接管。只有不支持、首帧无 retained 内容或代际不匹配保留 `Ok(false)` 语义。通用 GPU renderer 可以对同代 backdrop 在主 surface DPR 下执行 backdrop→scratch→backdrop 的区域双 pass blur，且不 acquire/present；当前仍不由 renderer 擅自决定组件启用策略、半径或区域失效规则。

CPU 与 soft fallback 的线性、径向渐变只由 `SoftwareRasterizer` 在完整 transform、clip、opacity 与 blend 状态下执行；零调用且缺少这些状态契约的无状态平行渐变模块已经移除。

Picture/offscreen 的 create/destroy/paint/blit/blur 全部围绕唯一 RHI texture owner 执行；公共 `OffscreenTargetId`、`旧统一 context 门面` 的 create/destroy/bind/blit 入口，以及 D3D11/OpenGL ES 的平行 texture/FBO、绑定、采样和释放状态均已移除。这样两个生产 adapter 只执行同一个通用 Picture 计划、高斯核、区域裁剪、scratch 生命周期和 sampled 合成，adapter 仅保留薄 RHI 资源与底层 draw 编码。

Picture 资源与绘制事务在 draw 侧只公开检查式边界：`try_create_offscreen` 返回 `Result<Option<ImageHandle>, Error>`，其中无效尺寸或不支持为 `Ok(None)`，CPU 像素分配、owner-thread context 与 RHI texture 创建失败保持 typed error；`try_destroy_offscreen` 只有在资源释放完成后返回成功，失败时保留 owner 供恢复或 shutdown 重试，替换资源的补偿释放失败追加到原错误原因链。绘制阶段只公开 `try_begin_offscreen_paint`、`try_flush_offscreen_paint`、`try_end_offscreen_paint` 与 `try_blit_offscreen_src`；`RenderBackend`、`RenderTarget`、`Renderer` 与 `RecoveryDriver` 不再保留 Option/void/bool 未检查兼容入口。未实现的默认 destroy/begin/flush/end/blit 返回 typed `NotImplemented`，不能以无操作或漏绘报告成功；GPU 最终 present 若发现 active Picture，也必须先通过 checked end，失败时停止主 surface 提交。

`旧统一 context 门面::blit_soft_fallback_tile` 及 D3D11、D3D12、OpenGL adapter 内对应的私有上传纹理、pipeline 和转发已经移除；紧边界 `SoftFallbackTile` 只作为 draw backend 的 staging 描述存在。`clear_render_target`、`clear_rects`、`bind_swapchain_target` legacy 门面和对应逐 UI capability 也已删除，局部与整面清理由通用 `FramePlan` 的 `Clear` / `ClearRect` 在 retained RHI target 上执行；adapter 内部仅保留 acquire、pass 和最终 present 所需的低层 target 恢复。solid/stroke rect、glyph、linear/radial gradient、sector、solid mesh、box shadow 与 image blit 的 `旧统一 context 门面::draw_*` ABI、thread-bound 转发和原生 context wrapper 同样已经移除；`NativeRasterCaps` 是 graphics backend 从薄 RHI `GraphicsCapabilities` 投影出的 renderer 内部值，只陈述 retained framebuffer 与 RHI Additive 事实，逐图元 pipeline 由固定 RHI probe 验证。主 `FrameEncoder` 的 Additive、scroll、图片与 CPU segment 现在必须整条无损 lower 到 retained RHI，其中 CPU segment 作为 sampled texture 合成；前置 damage 先以同代纹理上的 `ClearRect` 提交，任何缺失能力都返回 typed failure，不再读回 CPU 后整面 replace。旧 `旧统一 context 门面::blit_soft_fallback`、`旧统一 context 门面::upload_surface_pixels`、逐命令 legacy frame 执行器及各 adapter 的整面上传实现也已经移除。主 surface 的最终 present 和 Picture/effect 前有序边界只接受 retained RHI 完整提交：空新帧以透明 dummy pass 初始化 retained texture，无新绘制时重新采样既有 retained 内容；未覆盖的非空队列返回 typed failure，不再销毁 retained texture 后调用逐 UI adapter 或 direct present swapchain。CPU presenter 的 `PixelBuffer` present 保持独立。

页面销毁或切换阶段产生的空源、空目标或全透明 `PictureBlit` 是无像素贡献的 canonical no-op：录制入口不再把它加入新命令流，retained RHI lowering 仍能安全消费历史命令流中的同类记录。非空越界源和无法保持像素语义的输入继续返回 typed failure。

GPU 基线内的操作不能依赖常态 CPU fallback。可选效果可以显式声明等价降级，但不能静默改变视觉结果。

## 当前映射与目标差距

- 已有唯一 `RenderBackend` 抽象、通用 `GpuBackend`、有序 `FramePlan`、薄 RHI 契约和 CPU backend。
- `旧统一 context 门面` 的逐 UI draw/clear/offscreen/upload 定义、adapter wrapper、二阶段 `initialize`、`make_current`、`swap_buffers`、通用 `resize`、RHI surface resize、分离 `width`/`height`/DPR 查询、readback、静态 `caps`、由 caps 重复派生的 backend/recipe 布尔查询、`native_raster_caps` 及默认 `present` 回退已移除；native factory 的构造成功即表示 context 已绑定 surface 并可用，RenderSession 每帧通过语义型 `prepare_frame` 进入 thin RHI 设备维护。零调用的单 backend 内部 raw-surface 工厂链、backend-only 候选投影和自动创建空故障队列的 platform/recipe 构造入口也已删除，生产 bootstrap 与恢复只按完整 `GraphicsRecipe` 精确选行并携带 runtime-scoped 故障队列。live drawable extent、DPR、transform 与 generation 只由显式 `PresentSurface` 原子快照提供，thread-bound wrapper 也只缓存并在成功生命周期变更后整体刷新该快照；`GraphicsContextCaps` 不再混入动态 DPR，也不再进入 thread-bound wrapper。生产 `GpuBackend` 从一次 `GraphicsCapabilities` 快照同时校验完整 GPU-only retained RHI baseline，并派生只含 retained framebuffer 和 RHI Additive 事实的 `NativeRasterCaps` renderer 投影；thread-bound wrapper 与 D3D11/OpenGL ES context 不再缓存或硬编码第二份能力。initialize 不再二次 resize，显式 GPU resize 只借用原子 `GpuRecipeContext` 并进入 `GraphicsSurface`；D3D11/WGL/EGL owner 复用同一 DPR/extent 校验，thread-bound wrapper 在成功后才整体刷新 `PresentSurface`。CPU × PixelUpload recipe 则在构造时验证独立 `PixelUploadSurface`，不再让 GPU adapter 包装同名生命周期入口。同步 surface readback 现在是 `GraphicsCapabilities::surface_readback` 声明的可选 `GraphicsSurface` 操作，仅 D3D11/OpenGL ES 生产 adapter 启用；Vulkan GFX-R5 与 D3D12 测试期内部诊断不构成通用能力。D3D11/OpenGL ES 无消费者的旧 rect/glyph/gradient/mesh/shadow/image pipeline 资源已经删除，D3D12 的测试期逐 UI raster pipeline 也已退出。draw backend 已无 direct swapchain legacy consumer；`PresentFrame` 与统一最终提交已退出 `旧统一 context 门面`，PixelUpload 通过专用视图提交，生产 GPU 只通过 thin RHI 最终提交，未接线的 Wayland 平行 presenter 已删除。其余迁移工作是继续收窄 `旧统一 context 门面` 的 recipe 视图并把未闭合 effect 调度收回通用 GPU Renderer。
- 已遮挡 swapchain 的 idle present probe 也已退出 `旧统一 context 门面`：`GpuBackend` 只借用组合 thin RHI 的 `GraphicsSurface::test_present`，D3D11 在该 surface 实现内执行 `DXGI_PRESENT_TEST`，PixelUpload 明确返回不适用。探测不提交帧数据，也不改变正常帧 present 的唯一所有者。
- D3D11 与 OpenGL ES 已接入资源、pass、draw/copy、retained framebuffer、surface resize、submit/present、错误映射和恢复边界；主 surface 未覆盖语义返回明确 typed failure。D3D11 在实际创建 `FLIP_SEQUENTIAL` 双缓冲并取得 `IDXGISwapChain3` 时冻结为 `TrackedSwapchain`，graphics backend 按当前 image index 合并错过的成功历史，以同一组物理矩形裁剪 retained-to-swapchain 绘制并调用 `Present1`；创建期接口或交换链失败回退 `DISCARD`、`FullOnly` 与完整重绘。OpenGL ES 当前继续保持 `FullOnly`。
- 剩余差距包括 backdrop blur 的 UI 策略、半径与区域失效接线，以及 Picture/offscreen blur 与 overlay backdrop 跨 DPI、真实 GPU 和操作系统的运行时矩阵覆盖；图形装配与共享 GPU 生命周期已经只传递已验证的原子 recipe owner。

## 当前测试

- Windows 真窗测试覆盖 D3D11 与 OpenGL ES/WGL 的 RHI surface、`FramePlan`、present、resize 和兼容遍历。
- `test-harness` 测试覆盖 DeviceLost、SurfaceLost、teardown/rebuild 与恢复后交互。
- mock RHI 测试覆盖 pass 顺序、一次最终 present、受控 SurfaceLost，以及 resize 后旧代计划拒绝与新代计划恢复；失败帧不消费 damage。
- tracked present 测试覆盖双 image 历史修复、失败/遮挡帧不提交 history、surface generation 重建强制全帧、draw scissor 与 Present1 dirty rect 同源，以及非法或超量矩形完整提交回退。
- Picture/offscreen blur 测试覆盖逻辑 region 不重复应用主 surface DPR、source→scratch→原 Picture 双 pass、同核水平/垂直 uniform、区域裁剪、单次 submit、无 acquire/present、失败清理与空区域无资源 no-op；sampled lowering 另覆盖目标 opacity、drawable 比例与 Additive 保真，CPU 参考覆盖真实 blur 后的目标相关 Additive 像素、clip 和状态恢复。
- Picture owner 测试覆盖缺失 RHI renderer、无效 extent 与有效单一 owner 门禁；D3D11 默认构建和源码门禁同时证明旧高层 blur、`OffscreenTargetId`、原生 texture/FBO、专属 scratch 与 legacy sampled blit 已退出生产依赖图。
- OpenGL ES draw、shader 编译与共享 Win32 HDC 边界使用模块级 `unsafe_op_in_unsafe_fn = deny` 门禁；每个 glow/Win32 底层调用都在保持 owner-thread、原生句柄和资源表前置责任的显式 `unsafe` 操作中执行，Rust 2024 不再把 unsafe 函数体本身视为隐式授权。
- Linux EGL 路径同样使用模块级 `unsafe_op_in_unsafe_fn = deny` 门禁（`platform/linux.rs` 与 `opengl/platform/egl.rs`）；Wayland surface 描述符解引用、EGL/wl_egl FFI 调用都在显式 `unsafe` 操作中执行，Rust 2024 不再把 unsafe 函数体本身视为隐式授权。khronos-egl v6 的 static 绑定不携带 `#[link]`，且项目刻意使用 `no-pkg-config`，因此 `egl.rs` 内显式声明 `#[link(name = "EGL")]` 以保证 egl* 符号进入最终链接。
- overlay backdrop recording 测试覆盖 retained→backdrop 快照与 backdrop→retained 恢复的方向、全幅物理 extent、唯一 device submit、无 acquire/present，以及创建失败无半成品和 submit 失败检查式销毁；blur 测试另覆盖逻辑区域只应用一次主 surface DPR、extent 裁剪、backdrop→scratch→backdrop 顺序、单次 submit、无 acquire/present 与 scratch 检查式销毁；场景管线测试覆盖 snapshot DeviceLost、restore SurfaceLost 与 release OOM 的 typed 分类传播，并证明失败后不继续 begin/end/present。
- 通用 canvas 单元测试覆盖 soft fallback 在 SrcOver/Additive 交替时的分段顺序与成功提交后的 staging 消费。
- 通用 canvas 与 capability 单元测试覆盖 `GraphicsCapabilities` 到 `NativeRasterCaps` 的唯一投影、生产 retained RHI profile 的 Additive shape 直达入队、事实能力门禁及无 retained/Additive 能力时的 soft 回退；源码契约锁定 `旧统一 context 门面`、thread-bound wrapper 与生产 adapter 不再恢复平行 raster capability 查询。
- FrameRecordingCanvas、FrameEncoder 与 NativeGpuCanvas2D 单元测试覆盖 Additive 矩形/圆的填充与描边命令事实、整数矩形裁剪、正负整数像素 offset、纯平移整数 transform、含非统一圆角角位重排的八种单位正交 transform、有限全局 opacity 的 premultiplied 颜色折叠、目标相关 CPU 参考像素、固定 Native shape 无法表达时矩形/圆/椭圆/扇形/路径填充、矩形/圆/路径/直线描边及线性/径向渐变的仿射 sampled soft 分段、raw image 的直接 Additive textured quad 与仿射 sampled soft 分段、glyph coverage 与定向/环境 box shadow 的仿射 sampled soft 分段、描边 width/cap/join/miter、渐变/图片/字形/阴影局部逆映射、图片源 crop、point sampling、字形与阴影 coverage、路径 clip mask、连续饱和累积、跨源图元合批、紧边界 tile、SrcOver 与 `restore` painter barrier、能力/轴对齐门禁、blend 批隔离，以及带物理 scissor 的 `AdditiveShape` lowering。
- 缺少运行环境的组合记为未测试，不生成独立完成记录。

## 迁移约束与测试

迁移不绑定版本、日期或执行顺序；完成状态只以相应自动测试通过为准：

- D3D11 作为参考 adapter 完整执行薄 RHI，且 adapter 内不新增逐 UI 操作入口；
- 至少第二个原生 API 复用同一 `GpuRenderer`、`FramePlan`、atlas、offscreen 与 effect 调度，新增 adapter 不复制 UI raster 算法；
- bootstrap capability gate 测试覆盖首帧前拒绝缺失 GPU 基线的实现；
- 矩形、路径、字形、图片、Picture、opacity、additive、离屏、模糊、resize、surface lost 与 device lost 由对应测试覆盖；
- mock RHI 测试覆盖 pass 顺序、资源代际、一次最终 present 与失败帧不消费 damage。

## 模块不变量

- renderer、scene、painting 与 widget 不取得 raw GPU/native handle。
- UI 图形语义在通用 GPU Renderer 中只有一个生产实现；native adapter 不定义平行的 `draw_*` 语义层。
- capability 必须由实际可执行原语和 bootstrap probe 支撑，不能用编译成功代替运行契约。
- CPU 写入 retained buffer 或 GPU submit 成功均不等于 present 成功；最终提交结果由 platform surface/presenter 返回。
