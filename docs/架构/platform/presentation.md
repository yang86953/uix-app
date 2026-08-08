# presentation 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `presentation` 模块，权威持有原生 surface、图形 recipe、thin RHI provider、presenter 与提交能力。依赖：[windowing](windowing.md)、core。导出：供 graphics [backend](../graphics/backend.md) bootstrap/执行使用的平台图形边界。

> **设计状态**：🔄 迁移中（2026-08-03）。`GraphicsDevice`、`GraphicsSurface`、事实型 `GraphicsCapabilities` 和 surface generation 已在 `src/native/present/rhi.rs` 建立；D3D11 与可选 `opengles` feature 下的 WGL/EGL OpenGL ES adapter 已接通资源/采样器/pipeline、pass、solid/textured（含 SrcOver/Additive）/gradient/coverage/MSDF/shape/仿射 shadow/单方向 blur draw 与 surface 生命周期，Picture texture 由同一 RHI owner 管理，MSDF 已加入有界跨帧 RGBA8 texture cache；`FramePlan` 的局部 `ClearRect`、同纹理 `TextureMove`、主 surface native→soft sampled 合成、Picture native→soft 离屏合成和 surface generation 代际检查也已在两个 adapter 接通，并由生产 capability profile 显式启用 retained framebuffer。共享 CPU rasterizer 另已提供带 save/restore 的路径裁剪 mask，供 hybrid soft staging 保持 Canvas2D clip 语义。现有 `IGraphicsContext` 仍是生产兼容门面，尚未被所有原生 adapter 替换；完成状态只以对应自动测试为准。

> **当前实现线索**：目标接口位于 `src/native/present/rhi.rs`，兼容接口位于 `src/native/present/`，构造位于 `src/native/factory/`，presenter 位于 `src/native/presentation/`；D3D11 surface 迁移位于 `src/native/presentation/graphics/d3d11/platform/context/rhi.rs`，OpenGL ES surface host 位于 `src/native/presentation/graphics/opengl/rhi_host.rs`，两者的 device 资源与状态分别位于对应 raster/context 子目录，构造门禁位于 `src/native/factory/registry.rs`。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `platform::graphics::GraphicsBackend` | public value | 只表达当前构建已启用的具体 GPU API |
| `GraphicsApi` / `GraphicsSelection` | crate-private value / strategy | registry 的具体 API 身份，以及 `Automatic` / `Explicit` 启动策略 |
| `GraphicsRecipe` | value | 平台、feature、API 与 fallback 候选 |
| `GraphicsDevice` | thin RHI interface | GPU 资源、pipeline、pass、draw/copy、submit 与 device 维护 |
| `GraphicsSurface` | surface interface | acquire、resize、present、surface generation 与呈现状态 |
| `GraphicsCapabilities` | value | RHI 原语、retained、occlusion 与 present 的事实能力 |
| `GraphicsContext` | 迁移期门面 | 当前组合 device/surface 与高层 raster；目标拆分后移除高层操作 |
| `Presenter` | 提交接口 | CPU/GPU 结果到原生窗口的最终 present |
| `SurfaceToken` | generation value | surface 重建与迟到 callback 隔离 |

## 组件：GraphicsRecipe

registry 只陈述可构造候选；graphics 决定选择和恢复策略。显式 API 请求不偷换其他 API，自动模式可按固定候选顺序降级。

公开选择面只有 `platform::graphics::GraphicsBackend`，且不包含 `Auto`。公开 builder 在边界处把具体 API 转换为 crate-private `GraphicsApi`；只有省略 builder、空配置或 `auto` 配置才构造私有 `GraphicsSelection::Automatic`。显式策略只保留同一 API 的 registry 行，自动策略只接纳 Active recipe 并保持 GPU-native 优先；CPU fallback 由 App 在 GPU 候选耗尽后整体执行，不进入任何设备/API 枚举。

## 组件：GraphicsDevice / GraphicsSurface

`GraphicsDevice` 只提供 buffer、texture、sampler、pipeline、render target、pass、draw/copy、submit 和 device 恢复所需的最小原语；不得提供 `draw_glyphs`、`draw_rounded_rect`、`draw_picture` 等 UI 操作。固定 pipeline 语义、batch、atlas 与 effect pass 由 graphics/backend 持有。

`GraphicsSurface` 独立持有窗口 surface、swapchain、尺寸、DPR 相关像素 extent 与 generation。device 和 surface 可以由首个 adapter 在同一 owner-thread 对象中组合，但资源寿命、错误分类与重建范围必须保持可区分，也不把跨窗口 device 共享设为首版前置条件。

原生 adapter 负责 API/OS 专属的 adapter/device/surface 创建、资源映射、命令编码、同步、acquire/present 和错误翻译；不得决定 Picture 缓存、字形 atlas、路径细分或 UI fallback。

## 组件：GraphicsCapabilities

capability 只陈述可验证的底层事实，例如 sampled texture、render-to-texture、scissor、texture copy、retained framebuffer、partial present 与 occlusion。逐 UI 操作支持由这些事实和通用 GPU Renderer 推导，不由 adapter 维护平行的 `draw_*` 布尔表。

## 组件：Presenter

`Presenter` 是 renderer 面向的统一最终提交门面：GPU 路径委托 `GraphicsSurface::present`，CPU 路径执行平台像素上传/合成；同帧只能由一个所有者调用。CPU 写入 retained pixels 或 GPU submit 均不等于提交成功；最终 OS present 返回 typed Result，失败帧不能被标记为成功。

## 模块不变量

platform 只拥有原生 device/surface、低层 RHI 实现与提交能力，不拥有 UI 绘制命令、路径/字形算法、字体/图片 atlas、Picture 策略或 WidgetTree。raw native handle 不得越过 thin RHI 边界。
