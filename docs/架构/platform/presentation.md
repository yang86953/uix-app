# presentation 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `presentation` 模块，权威持有原生 surface、图形 recipe、thin RHI provider、presenter 与提交能力。依赖：[windowing](windowing.md)、core。导出：供 graphics [backend](../graphics/backend.md) bootstrap/执行使用的平台图形边界。

> **设计状态**：🔄 迁移中。FramePlan、thin RHI、D3D11 与 OpenGL ES 的 retained GPU 路径已经落地；本轮已物理移除统一兼容 context 门面。剩余迁移集中在未闭合 effect 语义与真实环境矩阵，完成状态只以对应自动测试和所有者验收为准。

> **当前实现线索**：共享生命周期与 recipe 专用契约位于 `src/native/present/traits.rs`，薄 RHI 位于 `src/native/present/rhi.rs`，类型化 candidate/owner 位于 `src/native/present/`，构造与线程绑定位于 `src/native/factory/`。D3D11、D3D12、WGL/EGL、Vulkan 与 Metal adapter 分别在各自 platform context 内实现所需 trait。

> **类型化 recipe 生命周期**：`GraphicsContextLifecycle` 只定义原子 `PresentSurface` 快照与 checked shutdown；`GpuRecipeContext: GraphicsContextLifecycle` 额外拥有不可拆分的 thin RHI 与 GPU surface resize，`PixelUploadSurface: GraphicsContextLifecycle` 额外拥有上传 surface resize 与最终 pixels 提交。三个 trait 都没有运行期 `Option` 能力查询。

> **candidate 与 registry 边界**：adapter 创建事务必须直接选择 `GraphicsContextCandidate::gpu` 或 `GraphicsContextCandidate::pixel_upload`，并同时交付同源 `GraphicsContextCaps`。registry 校验精确 row、静态 recipe 轴与 `GraphicsRecipeContext::{Gpu, PixelUpload}` 分支一致；错配在返回前 checked shutdown。GPU baseline 与固定 pipeline probe 只对 GPU 分支执行，PixelUpload 分支不得查询或伪造 RHI。

> **线程亲和边界**：registry 校验后按具体 trait object 建立 `ThreadBoundGraphicsContext<T>`。wrapper 通过 `!Send + !Sync` 标记和 owner-thread 检查保护原生资源；resize 仅在底层成功后刷新完整 `PresentSurface`。错误线程 Drop 不触碰原生 API，checked shutdown failure 保持 typed 返回或结构化日志。

> **recipe owner 边界**：`GraphicsRecipeOwner` 同时匹配类型化 context 分支和静态 raster × present 轴，再构造直接持有 `Box<dyn GpuRecipeContext>` 的 `GpuRecipeOwner`，或直接持有 `Box<dyn PixelUploadSurface>` 的 `PixelUploadRecipeOwner`。构造后不存在可选视图丢失、能力重查或统一门面回退。

> **会话与窗口所有权**：native factory 在 context 离开 platform System 前返回已验证 `GraphicsRecipeOwner`；bootstrap、recovery 和 renderer 只传递该枚举。窗口只持有 native surface 与 presenter，不持有或关闭图形 context，因此 renderer/recovery 生命周期是唯一 teardown owner。

> **静态与动态事实**：`GraphicsContextCaps` 只固定 backend、raster/present、coherency 与 occlusion；drawable extent、DPR、transform 与 generation 只来自单次 `PresentSurface` 快照。thread-bound wrapper 不缓存静态 capability，recipe owner 不在运行期重新推断 recipe。

> **RHI 与最终提交边界**：逐 UI draw/clear/offscreen/upload、二阶段 initialize、平台 current、通用 resize、统一 present 与 readback 均不属于共享 lifecycle。生产 GPU 只通过 `GraphicsDevice` / `GraphicsSurface` 执行资源、probe、resize、readback 与最终 present；CPU × PixelUpload 只通过 `PixelUploadSurface` 上传并提交。平台 adapter 不拥有 FramePlan、fallback、Picture 或 effect 策略；draw 侧 Picture create/destroy/begin/flush/end/blit 只允许 checked `Result` 边界，资源失败不能用 `Option`、bool 或 void 门面推迟或吞掉。`try_create_offscreen` 的 `Ok(None)` 只陈述无效尺寸或不支持，thin RHI 返回的 device/surface/OOM 分类原样进入 graphics recovery。

> **坐标与行序契约**：thin RHI 输入坐标统一左上原点，viewport、scissor、顶点、UV 与 `PresentDamage` 跨平台语义一致（`RhiScissor` 保持左上原点约定）；帧缓冲行序差异只发生在呈现层，由各 adapter 内部消化，不得泄漏到通用层。平台机理——WGL 的 DIB 为 bottom-up 行序，顶点阶段固定保留 `ndc.y = -ndc.y`，scissor 必须换算为 GL 左下原点（height − y − h）；Wayland EGL 的 wl_buffer 为 top-left 行序，经 `flip_y` 标志在 shader 编译期去掉顶点 Y 翻转，其呈现行序反向与 GL 坐标反向恰好双重抵消，scissor 可直接使用 RHI 坐标；D3D11 backbuffer 为 top-left 行序，顶点阶段同样固定保留 `ndc.y = -ndc.y`（top-down 逻辑到 NDC y-up 的标准转换），scissor 原生 top-left 直接使用。`read_surface_pixels` 的 readback 输出统一为 top-left 行序（row 0 = 窗口顶部）：D3D11 staging 拷贝与 EGL 的 glReadPixels（row 0 = GL y=0 行 = 窗口顶部）天然满足，bottom-up 平台（WGL）的 adapter 必须在返回前完成垂直翻转。像素搬运路径中，机械拷贝（快照 copy、滚动 memmove）逐 texel 对位、与行序无关；blur 的 NDC quad 采样在 GL 上每个 pass 各翻转一次，靠两垂直 pass 抵消保持行序自洽（该机制与平台无关，EGL 同样依赖），属脆弱结构，新增单 pass 效果前必须显式处理。CPU 软渲染/upload 的像素数据统一为 top-left 行序，纹理上传保持数据直通。当前 `flip_y` 已覆盖 OpenGL 绘制路径；WGL readback 尚未收敛：当前实现未做垂直翻转，输出仍为 bottom-up（与 top-left 契约不符），且区域读回还需把左上原点 y 换算为 GL 左下原点（height − y − h，与 scissor 同式）；现有唯一调用点（backdrop 全幅快照）y 换算恒为 0，缺口表现为纯行序问题。属于待办契约缺口。graphics/backend 消费本契约，不在通用层叠加翻转。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `platform::graphics::GraphicsBackend` | public value | 只表达当前构建已启用的具体 GPU API |
| `GraphicsApi` / `GraphicsSelection` | crate-private value / strategy | registry 的具体 API 身份，以及 `Automatic` / `Explicit` 启动策略 |
| `GraphicsRecipe` | value | 平台、feature、API 与 fallback 候选 |
| `GraphicsDevice` | thin RHI interface | GPU 资源、pipeline、pass、draw/copy、submit 与 device 维护 |
| `GraphicsSurface` | surface interface | acquire、resize、present、idle present probe、surface generation 与呈现状态 |
| `GraphicsCapabilities` | value | RHI 原语、retained、occlusion 与 present 的事实能力 |
| `GraphicsContext` | 迁移期门面 | 当前只组合 live surface、checked 生命周期与 recipe 专用视图；静态 recipe 能力由 adapter candidate 移交并固化在 owner，不再持有 renderer capability 投影或统一最终提交 |
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

`Presenter` 是 renderer 面向的统一最终提交门面：生产 GPU backend 委托 `GraphicsSurface::present`，CPU × PixelUpload 委托 `PixelUploadSurface::present_pixels`；共享 lifecycle 不提交 payload，也不暴露 external swapchain 视图。同帧只能由一个 recipe owner 调用最终 OS present。CPU 写入 retained pixels 或 GPU submit 均不等于提交成功；最终 present 返回 typed Result，失败帧不能被标记为成功。

## 模块不变量

platform 只拥有原生 device/surface、低层 RHI 实现与提交能力，不拥有 UI 绘制命令、路径/字形算法、字体/图片 atlas、Picture 策略或 WidgetTree。raw native handle 不得越过 thin RHI 边界。
