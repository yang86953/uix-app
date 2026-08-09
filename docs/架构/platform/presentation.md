# presentation 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `presentation` 模块，权威持有原生 surface、图形 recipe、thin RHI provider、presenter 与提交能力。依赖：[windowing](windowing.md)、core。导出：供 graphics [backend](../graphics/backend.md) bootstrap/执行使用的平台图形边界。

> **设计状态**：🔄 迁移中（2026-08-09）。`GraphicsDevice`、`GraphicsSurface`、事实型 `GraphicsCapabilities` 和 surface generation 已在 `src/native/present/rhi.rs` 建立；D3D11 与可选 `opengles` feature 下的 WGL/EGL OpenGL ES adapter 已接通资源/采样器/pipeline、pass、solid/textured（含 SrcOver/Additive）/gradient/coverage/MSDF/shape/仿射 shadow/单方向 blur draw 与 surface 生命周期，Picture texture 只由同一 RHI owner 管理，原生 adapter 的平行 offscreen texture/FBO 资源族已经移除，MSDF 已加入有界跨帧 RGBA8 texture cache；`FramePlan` 的局部 `ClearRect`、同纹理 `TextureMove`、主 surface native→soft sampled 合成、Picture native→soft 离屏合成和 surface generation 代际检查也已在两个 adapter 接通，并由生产 capability profile 显式启用 retained framebuffer。共享 CPU rasterizer 另已提供带 save/restore 的路径裁剪 mask，供 hybrid soft staging 保持 Canvas2D clip 语义。现有 `IGraphicsContext` 只保留生产生命周期、组合 thin RHI 与最终 present 等迁移门面；逐 UI `draw_*` ABI、二阶段 `initialize`、`make_current`、`swap_buffers` 与 readback 已从公共 trait、thread-bound wrapper 和原生 adapter 实现移除，surface readback 改由事实 capability 约束的可选 thin RHI 操作承接；所有 context 构造成功即处于可用状态，每帧 owner-context 准备由 thin RHI 设备维护承担。完成状态只以对应自动测试为准。

> **当前实现线索**：目标接口位于 `src/native/present/rhi.rs`，兼容接口位于 `src/native/present/`，构造位于 `src/native/factory/`，presenter 位于 `src/native/presentation/`；D3D11 surface 迁移位于 `src/native/presentation/graphics/d3d11/platform/context/rhi.rs`，OpenGL ES surface host 位于 `src/native/presentation/graphics/opengl/rhi_host.rs`，两者的 device 资源与状态分别位于对应 raster/context 子目录，构造门禁位于 `src/native/factory/registry.rs`。

> **离屏模糊边界**：`IGraphicsContext` 不再声明 `blur_offscreen_target`，D3D11 adapter 也不再持有高层 blur 方法、专属 scratch owner 或私有核计算。两个生产 adapter 只保留 `BLUR_PASS` 固定 shader 与底层资源/draw 原语；Picture 的双 pass、region、权重、资源清理和提交顺序全部由 graphics backend 决定。

> **离屏资源边界**：`OffscreenTargetId` 与 `IGraphicsContext` 的 create/destroy/bind/blit offscreen 方法已经移除，thread-bound 门面不再转发这些高层操作；D3D11 context 和 OpenGL ES raster 也不再保存平行的离屏槽位、RTV/SRV/FBO、绑定标记或 legacy sampled-blit shader。Picture 创建失败或 queue 无法无损 lower 时由 graphics backend 返回 typed failure，platform 不选择 UI fallback。
>
> **软回退上传边界**：`IGraphicsContext::blit_soft_fallback` 整面透明混合入口、`blit_soft_fallback_tile` compact tile 入口及 D3D11/D3D12/OpenGL 对应实现和私有上传资源已经移除；紧边界 tile 只在 draw backend 内描述 staging，生产 soft fallback 只通过通用 renderer 的 retained RHI sampled segment 执行。主 `FrameEncoder` 现在与 Picture 一样要求整条无损 RHI lowering，前置 damage 由 retained texture `ClearRect` 执行；`upload_surface_pixels` 兼容入口及 adapter 整面 replace 实现也已移除。最终 present 与 Picture/effect 前的主 surface 顺序边界不再放弃 retained texture 或调用逐 UI adapter：空新帧在 retained target 内透明初始化，idle 帧重新采样既有 retained 内容，未覆盖语义返回 typed failure。CPU presenter 的正式 `PixelBuffer` present 仍保持独立，不属于该兼容分叉。
>
> **主表面清理边界**：`IGraphicsContext` 不再声明 `clear_render_target`、`clear_rects` 或 `bind_swapchain_target`，thread-bound 门面与 D3D11、D3D12、WGL/EGL 的 legacy wrapper 也已移除；`NativeRasterCaps` 不再用逐 UI clear 布尔值描述能力。整面和局部清理由通用 `FramePlan` 在 retained RHI texture 上编码，adapter 只在 acquire、pass 与最终 present 内部恢复真实 swapchain target。
>
> **逐图元绘制边界**：`IGraphicsContext` 不再声明 solid/stroke rect、glyph、linear/radial gradient、sector、solid mesh、box shadow 或 image blit 等 `draw_*` 方法，owner-thread wrapper 与各原生 context 也不再转发这些高层 batch。D3D11/OpenGL ES 对应的无消费者 shader、buffer、atlas 与图片上传 owner 已物理删除，D3D12 的测试期逐 UI raster pipeline 也已删除；adapter 仅保留薄 RHI packet 所需的固定 shader 与编码。`NativeRasterCaps` 只保留 retained framebuffer 与 RHI Additive 两项事实；逐图元 pipeline 是否可用由 registry 的固定 RHI probe 在首帧前一次性验证。
>
> **统一呈现边界**：`IGraphicsContext` 不再声明 `swap_buffers`，也不再用默认 `present` 隐式串联 `make_current` 与交换操作。每个 native context 必须显式匹配 `PresentFrame::Swapchain` 或 `PresentFrame::PixelBuffer`，不符合所选 recipe 的 payload 返回 typed error；Wayland `GpuPresenter` 同样构造 `PresentFrame::Swapchain` 后进入唯一兼容 present 入口。D3D11/OpenGL ES 的薄 RHI 最终提交继续直接调用 `GraphicsSurface::present`，adapter 内部的 DXGI/WGL/EGL 原生交换函数只属于该低层实现。
>
> **构造生命周期边界**：`IGraphicsContext` 不再声明二阶段 `initialize`，thread-bound wrapper 与 D3D11/D3D12/Vulkan/Metal/WGL/EGL 实现也不再转发或提供空操作。registry 只接收已经完成 surface 绑定和初始尺寸归一化的 context；构造失败直接返回 typed error，成功值可立即接受 resize、RHI probe 与呈现。
>
> **设备准备边界**：`IGraphicsContext` 与 `RenderBackend` 不再声明平台语义的 `make_current`。`RenderSession::begin_frame` 只调用语义型 `prepare_frame`，GPU 实现把它收敛到 `GraphicsDevice::maintain`；OpenGL adapter 在该薄 RHI 维护入口内恢复 WGL/EGL owner context，D3D11 在同一入口执行设备健康检查。无 acquire 的 overlay texture copy 事务也复用这一设备准备边界。
>
> **GPU resize 边界**：生产 `GpuBackend` 只接收 native factory 已准备并通过 probe 的 GPU-only thin RHI context。初始化只同步 drawable 元数据；后续 resize 释放旧代 retained/overlay 资源后调用 RHI surface resize，任何 typed failure 都直接进入恢复层，不再切换到 `IGraphicsContext::resize`。兼容 resize 目前只为尚未迁移的 pixel-upload presenter 保留。
>
> **surface readback 边界**：`IGraphicsContext` 与 thread-bound wrapper 不再声明或转发 `read_pixels`。同步窗口 surface 回读是 `GraphicsCapabilities::surface_readback` 声明的可选 `GraphicsSurface::read_surface_pixels` 操作；D3D11 和 OpenGL ES adapter 如实启用，缺少能力时返回 typed failure。Vulkan GFX-R5 与 D3D12 测试期保留的内部诊断辅助不属于通用 thin RHI surface 能力。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `platform::graphics::GraphicsBackend` | public value | 只表达当前构建已启用的具体 GPU API |
| `GraphicsApi` / `GraphicsSelection` | crate-private value / strategy | registry 的具体 API 身份，以及 `Automatic` / `Explicit` 启动策略 |
| `GraphicsRecipe` | value | 平台、feature、API 与 fallback 候选 |
| `GraphicsDevice` | thin RHI interface | GPU 资源、pipeline、pass、draw/copy、submit 与 device 维护 |
| `GraphicsSurface` | surface interface | acquire、resize、present、surface generation 与呈现状态 |
| `GraphicsCapabilities` | value | RHI 原语、retained、occlusion 与 present 的事实能力 |
| `GraphicsContext` | 迁移期门面 | 当前组合 device/surface、生命周期、thin RHI 借用与显式 `PresentFrame`；目标拆分后继续收窄 |
| `Presenter` | 提交接口 | CPU/GPU 结果到原生窗口的最终 present |
| `SurfaceToken` | generation value | surface 重建与迟到 callback 隔离 |

## 组件：GraphicsRecipe

registry 只陈述可构造候选；graphics 决定选择和恢复策略。显式 API 请求不偷换其他 API，自动模式可按固定候选顺序降级。

公开选择面只有 `platform::graphics::GraphicsBackend`，且不包含 `Auto`。公开 builder 在边界处把具体 API 转换为 crate-private `GraphicsApi`；只有省略 builder、空配置或 `auto` 配置才构造私有 `GraphicsSelection::Automatic`。显式策略只保留同一 API 的 registry 行，自动策略只接纳 Active recipe 并保持 GPU-native 优先；CPU fallback 由 App 在 GPU 候选耗尽后整体执行，不进入任何设备/API 枚举。

## 组件：GraphicsDevice / GraphicsSurface

`GraphicsDevice` 只提供 buffer、texture、sampler、pipeline、render target、pass、draw/copy、submit 和 device 恢复所需的最小原语；不得提供 `draw_glyphs`、`draw_rounded_rect`、`draw_picture`、`create/bind/blit_offscreen_target`、`blur_offscreen_target` 等 UI 操作。固定 pipeline 语义、batch、atlas 与 effect pass 由 graphics/backend 持有。

`GraphicsSurface` 独立持有窗口 surface、swapchain、尺寸、DPR 相关像素 extent 与 generation。device 和 surface 可以由首个 adapter 在同一 owner-thread 对象中组合，但资源寿命、错误分类与重建范围必须保持可区分，也不把跨窗口 device 共享设为首版前置条件。

原生 adapter 负责 API/OS 专属的 adapter/device/surface 创建、资源映射、命令编码、同步、acquire/present 和错误翻译；不得决定 Picture 缓存、字形 atlas、路径细分或 UI fallback。

## 组件：GraphicsCapabilities

capability 只陈述可验证的底层事实，例如 sampled texture、render-to-texture、scissor、texture copy、retained framebuffer、partial present 与 occlusion。逐 UI 操作支持由这些事实和通用 GPU Renderer 推导，不由 adapter 维护平行的 `draw_*` 布尔表。

`retained_framebuffer` 与 renderer 的 `partial_redraw` 必须保持分层：前者只描述 platform adapter 能否保存跨帧颜色纹理，后者还要求所有绘制与兼容回退都不会绕过该纹理。迁移期 platform 继续如实报告 retained 能力，由 graphics backend 在存在直接 swapchain 回退时保守关闭局部重绘。

## 组件：Presenter

`Presenter` 是 renderer 面向的统一最终提交门面：GPU 路径委托 `GraphicsSurface::present`，CPU 路径执行平台像素上传/合成；迁移期 `IGraphicsContext::present` 也只接受显式 `PresentFrame`，不存在独立 `swap_buffers` 旁路。同帧只能由一个所有者调用。CPU 写入 retained pixels 或 GPU submit 均不等于提交成功；最终 OS present 返回 typed Result，失败帧不能被标记为成功。

## 模块不变量

platform 只拥有原生 device/surface、低层 RHI 实现与提交能力，不拥有 UI 绘制命令、路径/字形算法、字体/图片 atlas、Picture 策略或 WidgetTree。raw native handle 不得越过 thin RHI 边界。
