# presentation 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `presentation` 模块，权威持有原生 surface、图形 recipe、thin RHI provider、presenter 与提交能力。依赖：[windowing](windowing.md)、core。导出：供 graphics [backend](../graphics/backend.md) bootstrap/执行使用的平台图形边界。

> **设计状态**：🔄 迁移中（2026-08-09）。`GraphicsDevice`、`GraphicsSurface`、事实型 `GraphicsCapabilities` 和 surface generation 已在 `src/native/present/rhi.rs` 建立；D3D11 与可选 `opengles` feature 下的 WGL/EGL OpenGL ES adapter 已接通资源/采样器/pipeline、pass、solid/textured（含 SrcOver/Additive）/gradient/coverage/MSDF/shape/仿射 shadow/单方向 blur draw 与 surface 生命周期，Picture texture 只由同一 RHI owner 管理，原生 adapter 的平行 offscreen texture/FBO 资源族已经移除，MSDF 已加入有界跨帧 RGBA8 texture cache；`FramePlan` 的局部 `ClearRect`、同纹理 `TextureMove`、主 surface native→soft sampled 合成、Picture native→soft 离屏合成和 surface generation 代际检查也已在两个 adapter 接通，并由生产 capability profile 显式启用 retained framebuffer。共享 CPU rasterizer 另已提供带 save/restore 的路径裁剪 mask，供 hybrid soft staging 保持 Canvas2D clip 语义。现有 `IGraphicsContext` 只保留静态 recipe 能力、组合 thin RHI、recipe 专用视图、live surface 元数据与 checked shutdown；逐 UI `draw_*` ABI、二阶段 `initialize`、`make_current`、`swap_buffers`、统一 `present`、通用 `resize`、RHI surface resize、readback 与 `native_raster_caps` 已从公共 trait 移除。GPU backend 从同一次 thin RHI 能力快照校验基线并派生 renderer 投影，GPU resize 只借用专用 `RhiSurfaceLifecycle`，由 D3D11/WGL/EGL owner 进入唯一 `GraphicsSurface::resize`，thread-bound wrapper 仅在成功后刷新完整 `PresentSurface`；CPU PixelUpload resize/提交由专用 `PixelUploadSurface` 承接，未接线的 Wayland 平行 GPU presenter 与 `SwapchainPresentation` 视图已删除，生产 GPU 最终提交只由 thin RHI `GraphicsSurface::present` 持有，surface readback 改由事实 capability 约束的可选 thin RHI 操作承接；所有 context 构造成功即处于可用状态，每帧 owner-context 准备由 thin RHI 设备维护承担。完成状态只以对应自动测试为准。

> **当前实现线索**：目标接口位于 `src/native/present/rhi.rs`，兼容接口位于 `src/native/present/`，构造位于 `src/native/factory/`，presenter 位于 `src/native/presentation/`；D3D11 surface 迁移位于 `src/native/presentation/graphics/d3d11/platform/context/rhi.rs`，OpenGL ES surface host 位于 `src/native/presentation/graphics/opengl/rhi_host.rs`，两者的 device 资源与状态分别位于对应 raster/context 子目录，构造门禁位于 `src/native/factory/registry.rs`。

> **GPU recipe owner 边界**：生产 `GpuBackend` 不再直接持有 `Box<dyn IGraphicsContext>`，而是由 draw factory 先构造 `GpuRecipeOwner`。该 owner 在进入 backend 前验证 GPU-native × swapchain 与组合 thin RHI，并把运行期视图丢失统一映射为 typed state error；FramePlan/RHI 代码只借用其必需 Result 契约，不再把兼容期 `Option` 查询解释为可降级能力。

> **PixelUpload recipe owner 边界**：统一 renderer 的 CPU PixelUpload presentation 不再直接持有 `Box<dyn IGraphicsContext>`，而是在分派 recipe 后先构造 `PixelUploadRecipeOwner`。该 owner 验证 CPU × PixelUpload 与专用 surface，并统一承接 resize、最终 pixels 提交、`PresentSurface` 和 checked shutdown；presentation 不再直接查询可选 `pixel_upload_surface()`。

> **会话装配边界**：`RenderSession` 与通用 backend kind factory 不再持有或暂存 `IGraphicsContext`。生产 GPU bootstrap 与恢复只能从 `Renderer::from_context` 进入 native recipe factory，在完成 GPU recipe、thin RHI 与专用 owner 校验后直接注入会话；没有完整 recipe 的通用 GPU 构造或运行时切换返回稳定 typed error，不建立第二条 native 资源生命周期。

> **窗口所有权边界**：`PlatformWindow::graphics_context`、共享窗口的 staged GPU context 与测试门面已经删除。窗口只持有 native surface 与 presenter；图形 context 从 bootstrap 开始由 renderer/recovery owner 唯一管理，窗口关闭不再执行第二次 checked shutdown。

> **present recipe 值域**：`PresentMode` 只描述 live `IGraphicsContext` 的实际提交配方，即 GPU `Swapchain` 与 CPU `PixelUpload`。没有 context 的纯 CPU app presenter 不再伪装成 native context recipe，零构造的 `CpuPresenter` 枚举值已经删除。

> **离屏模糊边界**：`IGraphicsContext` 不再声明 `blur_offscreen_target`，D3D11 adapter 也不再持有高层 blur 方法、专属 scratch owner 或私有核计算。两个生产 adapter 只保留 `BLUR_PASS` 固定 shader 与底层资源/draw 原语；Picture 的双 pass、region、权重、资源清理和提交顺序全部由 graphics backend 决定。

> **离屏资源边界**：`OffscreenTargetId` 与 `IGraphicsContext` 的 create/destroy/bind/blit offscreen 方法已经移除，thread-bound 门面不再转发这些高层操作；D3D11 context 和 OpenGL ES raster 也不再保存平行的离屏槽位、RTV/SRV/FBO、绑定标记或 legacy sampled-blit shader。Picture 创建失败或 queue 无法无损 lower 时由 graphics backend 返回 typed failure，platform 不选择 UI fallback。
>
> **软回退上传边界**：`IGraphicsContext::blit_soft_fallback` 整面透明混合入口、`blit_soft_fallback_tile` compact tile 入口及 D3D11/D3D12/OpenGL 对应实现和私有上传资源已经移除；紧边界 tile 只在 draw backend 内描述 staging，生产 soft fallback 只通过通用 renderer 的 retained RHI sampled segment 执行。主 `FrameEncoder` 现在与 Picture 一样要求整条无损 RHI lowering，前置 damage 由 retained texture `ClearRect` 执行；`upload_surface_pixels` 兼容入口及 adapter 整面 replace 实现也已移除。最终 present 与 Picture/effect 前的主 surface 顺序边界不再放弃 retained texture 或调用逐 UI adapter：空新帧在 retained target 内透明初始化，idle 帧重新采样既有 retained 内容，未覆盖语义返回 typed failure。CPU presenter 的正式 `PixelBuffer` present 仍保持独立，不属于该兼容分叉。
>
> **主表面清理边界**：`IGraphicsContext` 不再声明 `clear_render_target`、`clear_rects` 或 `bind_swapchain_target`，thread-bound 门面与 D3D11、D3D12、WGL/EGL 的 legacy wrapper 也已移除；`NativeRasterCaps` 不再用逐 UI clear 布尔值描述能力。整面和局部清理由通用 `FramePlan` 在 retained RHI texture 上编码，adapter 只在 acquire、pass 与最终 present 内部恢复真实 swapchain target。
>
> **逐图元绘制边界**：`IGraphicsContext` 不再声明 solid/stroke rect、glyph、linear/radial gradient、sector、solid mesh、box shadow 或 image blit 等 `draw_*` 方法，owner-thread wrapper 与各原生 context 也不再转发这些高层 batch。D3D11/OpenGL ES 对应的无消费者 shader、buffer、atlas 与图片上传 owner 已物理删除，D3D12 的测试期逐 UI raster pipeline 也已删除；adapter 仅保留薄 RHI packet 所需的固定 shader 与编码。`NativeRasterCaps` 是 graphics backend 从 `GraphicsCapabilities` 派生的 renderer 投影，只保留 retained framebuffer 与 RHI Additive 两项事实；逐图元 pipeline 是否可用由 registry 的固定 RHI probe 在首帧前一次性验证。
>
> **按 recipe 呈现边界**：`PresentFrame` 载荷并集与 `IGraphicsContext::present`、`present_pixels`、`present_image` 已物理移除。CPU × PixelUpload 只能经 `PixelUploadSurface::present_pixels` 上传并提交 retained pixels；D3D11 与 WGL/EGL 生产 GPU backend 的最终提交只由 thin RHI `GraphicsSurface::present` 持有。零构造调用的 Wayland `GpuPresenter`、`SwapchainPresentation` 及其 thread-bound/EGL 平行实现已经删除；D3D12 测试 context 与 fake 也不伪造无消费者的兼容 present。
>
> **构造生命周期边界**：`IGraphicsContext` 不再声明二阶段 `initialize`，thread-bound wrapper 与 D3D11/D3D12/Vulkan/Metal/WGL/EGL 实现也不再转发或提供空操作。registry 只接收已经完成 surface 绑定和初始尺寸归一化的 context；构造失败直接返回 typed error，成功值可立即接受 resize、RHI probe 与呈现。仓库内部零调用的单 `GraphicsApi` raw-surface 工厂链、backend-only 候选投影与自动创建空故障队列的 platform/recipe 构造入口已经删除，生产构造与恢复只通过完整 `GraphicsRecipe` 和运行时 `PendingFailureQueue` 进入精确 registry 行。
>
> **设备准备边界**：`IGraphicsContext` 与 `RenderBackend` 不再声明平台语义的 `make_current`。`RenderSession::begin_frame` 只调用语义型 `prepare_frame`，GPU 实现把它收敛到 `GraphicsDevice::maintain`；OpenGL adapter 在该薄 RHI 维护入口内恢复 WGL/EGL owner context，D3D11 在同一入口执行设备健康检查。无 acquire 的 overlay texture copy 事务也复用这一设备准备边界。
>
> **resize 边界**：生产 `GpuBackend` 只接收 native factory 已准备并通过 probe 的 GPU-only thin RHI context。初始化只同步 drawable 元数据；后续 resize 释放旧代 retained/overlay 资源后借用 `RhiSurfaceLifecycle` 调用唯一 RHI surface resize，任何 typed failure 都直接进入恢复层。`IGraphicsContext` 不再声明通用 `resize` 或 RHI resize，只返回 recipe 专用视图；thread-bound wrapper 先转发 owner-thread resize，成功后才整体刷新 `PresentSurface`。CPU × PixelUpload recipe 在构造时必须暴露独立 `PixelUploadSurface`，Vulkan/Metal 只通过该契约重建上传 surface，GPU adapter 不实现它。
>
> **live surface 元数据边界**：`GraphicsContextCaps` 只描述 backend、raster/present recipe、coherency 与遮挡能力，不携带动态 DPR。`IGraphicsContext` 不再分离暴露 `width`、`height` 或 DPR；所有 context 必须一次返回完整 `PresentSurface`，GPU backend、PixelUpload runtime 与诊断场景从同一快照派生 drawable extent 和 DPR，不能拼装可能跨 generation 的状态。thread-bound wrapper 也只缓存并在成功生命周期变更后整体刷新该快照。
>
> **recipe 查询边界**：`IGraphicsContext` 不再声明由 `GraphicsContextCaps` 重复派生的 `graphics_backend`、`supports_gl_proc_address` 或 `supports_pixel_present`。需要 adapter 身份的 typed error 一次读取 `caps().backend`；是否可借用 thin RHI 或 `PixelUploadSurface` 仍由对应专用视图决定，不能通过无消费者布尔查询猜测 recipe。
>
> **renderer 能力投影边界**：`IGraphicsContext::native_raster_caps` 已移除，thread-bound wrapper 不再缓存该值，D3D11、WGL 与 EGL adapter 也不再硬编码平行 profile。生产 GPU backend 只从一次 `GraphicsDevice::capabilities` 快照验证完整 GPU 原语与 retained 事实，并把 retained framebuffer、Additive 两项 renderer 所需事实投影为 `NativeRasterCaps`；固定 probe 仍是能力声明可执行性的权威验证。
>
> **idle present probe 边界**：`IGraphicsContext` 与 thread-bound wrapper 不再声明或转发 `test_present`。无帧遮挡退出探测属于 `GraphicsSurface` 生命周期：D3D11 在该接口内执行 `Present(0, DXGI_PRESENT_TEST)` 并保留 `Presentable` / `Occluded` / typed error 映射；GPU backend 只借用组合 thin RHI surface，CPU PixelUpload 明确拒绝不适用的 probe。
>
> **surface readback 边界**：`IGraphicsContext` 与 thread-bound wrapper 不再声明或转发 `read_pixels`。同步窗口 surface 回读是 `GraphicsCapabilities::surface_readback` 声明的可选 `GraphicsSurface::read_surface_pixels` 操作；D3D11 和 OpenGL ES adapter 如实启用，缺少能力时返回 typed failure。Vulkan GFX-R5 与 D3D12 测试期保留的内部诊断辅助不属于通用 thin RHI surface 能力。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `platform::graphics::GraphicsBackend` | public value | 只表达当前构建已启用的具体 GPU API |
| `GraphicsApi` / `GraphicsSelection` | crate-private value / strategy | registry 的具体 API 身份，以及 `Automatic` / `Explicit` 启动策略 |
| `GraphicsRecipe` | value | 平台、feature、API 与 fallback 候选 |
| `GraphicsDevice` | thin RHI interface | GPU 资源、pipeline、pass、draw/copy、submit 与 device 维护 |
| `GraphicsSurface` | surface interface | acquire、resize、present、idle present probe、surface generation 与呈现状态 |
| `GraphicsCapabilities` | value | RHI 原语、retained、occlusion 与 present 的事实能力 |
| `GraphicsContext` | 迁移期门面 | 当前组合静态 recipe 能力、live surface、生命周期、thin RHI 与 recipe 专用视图；不再持有 renderer capability 投影或统一最终提交 |
| `Presenter` | 提交接口 | CPU/GPU 结果到原生窗口的最终 present |
| `SurfaceToken` | generation value | surface 重建与迟到 callback 隔离 |

## 组件：GraphicsRecipe

registry 只陈述可构造候选；graphics 决定选择和恢复策略。显式 API 请求不偷换其他 API，自动模式可按固定候选顺序降级。内部构造不得把 recipe 降为单 backend 查找或候选投影，因为同一 API 可以拥有多个 raster × present 行；bootstrap、平台创建与运行时恢复必须携带完整 `GraphicsRecipe` 和同一作用域的 callback 故障队列，不能在兼容入口内临时创建空队列。

公开选择面只有 `platform::graphics::GraphicsBackend`，且不包含 `Auto`。公开 builder 在边界处把具体 API 转换为 crate-private `GraphicsApi`；只有省略 builder、空配置或 `auto` 配置才构造私有 `GraphicsSelection::Automatic`。显式策略只保留同一 API 的 registry 行，自动策略只接纳 Active recipe 并保持 GPU-native 优先；CPU fallback 由 App 在 GPU 候选耗尽后整体执行，不进入任何设备/API 枚举。

## 组件：GraphicsDevice / GraphicsSurface

`GraphicsDevice` 只提供 buffer、texture、sampler、pipeline、render target、pass、draw/copy、submit 和 device 恢复所需的最小原语；不得提供 `draw_glyphs`、`draw_rounded_rect`、`draw_picture`、`create/bind/blit_offscreen_target`、`blur_offscreen_target` 等 UI 操作。固定 pipeline 语义、batch、atlas 与 effect pass 由 graphics/backend 持有。

`GraphicsSurface` 独立持有窗口 surface、swapchain、尺寸、DPR 相关像素 extent、generation 与不提交帧数据的遮挡退出探测。device 和 surface 可以由首个 adapter 在同一 owner-thread 对象中组合，但资源寿命、错误分类与重建范围必须保持可区分，也不把跨窗口 device 共享设为首版前置条件。

原生 adapter 负责 API/OS 专属的 adapter/device/surface 创建、资源映射、命令编码、同步、acquire/present 和错误翻译；不得决定 Picture 缓存、字形 atlas、路径细分或 UI fallback。

## 组件：GraphicsCapabilities

capability 只陈述可验证的底层事实，例如 sampled texture、render-to-texture、scissor、texture copy、retained framebuffer、partial present 与 occlusion。逐 UI 操作支持由这些事实和通用 GPU Renderer 推导，不由 adapter 维护平行的 `draw_*` 布尔表。

`retained_framebuffer` 与 renderer 的 `partial_redraw` 必须保持分层：前者只描述 platform adapter 能否保存跨帧颜色纹理，后者还要求所有绘制与兼容回退都不会绕过该纹理。迁移期 platform 继续如实报告 retained 能力，由 graphics backend 在存在直接 swapchain 回退时保守关闭局部重绘。

## 组件：Presenter

`Presenter` 是 renderer 面向的统一最终提交门面：生产 GPU backend 委托 `GraphicsSurface::present`，CPU × PixelUpload 委托 `PixelUploadSurface::present_pixels`；`IGraphicsContext` 不再直接提交任何 payload，也不再暴露未接线的 external swapchain 视图。同帧只能由一个 recipe owner 调用最终 OS present。CPU 写入 retained pixels 或 GPU submit 均不等于提交成功；最终 present 返回 typed Result，失败帧不能被标记为成功。

## 模块不变量

platform 只拥有原生 device/surface、低层 RHI 实现与提交能力，不拥有 UI 绘制命令、路径/字形算法、字体/图片 atlas、Picture 策略或 WidgetTree。raw native handle 不得越过 thin RHI 边界。
