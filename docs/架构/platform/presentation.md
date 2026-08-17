# presentation 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform System 的原生 surface、图形 recipe、thin RHI、presenter 与提交能力。基础依赖：core；[windowing](windowing.md)产生的窗口 owner 由 platform System 编排交付，Module 间不直接持有实例。导出：供 graphics [backend](../graphics/backend.md) bootstrap 和执行使用的平台图形边界。
>
> **当前实现线索**：共享生命周期、thin RHI、类型化 candidate/owner、registry 与线程绑定位于 `src/native/present/` 和 `src/native/factory/`；原生 Adapter 位于 `src/native/presentation/graphics/` 及各平台 context。路径只用于定位迁移，不构成公开 API；实时差距与环境矩阵由 Vikunja 持有。

## 责任边界

presentation 只拥有原生 device/surface、低层资源与命令原语、线程亲和和最终 OS present。它不拥有 FramePlan、Picture/effect 策略、路径/字形算法、字体/图片 atlas、WidgetTree 或 UI fallback。

```text
platform window owner
  → 类型化 GraphicsRecipe candidate
  → checked registry / owner-thread 绑定
  → GraphicsRecipeOwner
  → graphics backend 使用 thin RHI 或 PixelUploadSurface
  → Presenter 建立最终提交结果
```

逐 UI `draw_*`、统一“万能 context”、二阶段初始化和默认成功的空实现不属于目标契约。graphics 只能通过本页公开的 owned value 与窄接口消费平台能力。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `platform::graphics::GraphicsBackend` | public value | 表达当前构建启用的具体 GPU API |
| `GraphicsApi` / `GraphicsSelection` | private value/strategy | registry 的 API 身份和 Automatic / Explicit 选择策略 |
| `GraphicsRecipe` | value | 平台、feature、API、raster 与 present 的完整候选 |
| `GraphicsContextCandidate` | construction value | 原子交付 GPU 或 PixelUpload context、caps 与来源 |
| `GraphicsRecipeOwner` | owned enum | 向 graphics 交付唯一 `GpuRecipeOwner` 或 `PixelUploadRecipeOwner` |
| `GraphicsContextLifecycle` | internal interface | `PresentSurface` 快照与 checked shutdown |
| `GpuRecipeContext` | internal interface | 不可拆分的 thin RHI、GPU surface 与当前 image 身份 |
| `PixelUploadSurface` | internal interface | CPU 像素 resize、上传与最终提交 |
| `GraphicsDevice` / `GraphicsSurface` | thin RHI interfaces | GPU 资源、pass、submit、resize、probe、present 与可选 readback |
| `GraphicsCapabilities` | value | 由实际原语和 probe 支撑的事实能力 |
| `Presenter` | submit interface | 建立本帧最终 OS present 结果 |
| `SurfaceToken` / `PresentSurface` | generation/snapshot values | 隔离重建、迟到 callback，并原子描述 extent、DPR 与 transform |

## GraphicsRecipe 与选择

registry 只陈述可构造候选；graphics System 决定使用、恢复和 fallback 策略。显式 API 请求只尝试该 API 的完整 recipe，不偷换其他 API；Automatic 按固定候选顺序选择已启用且通过验证的 recipe，GPU 候选耗尽后才由上层决定是否整体使用 CPU。

`GraphicsContextCandidate` 必须在一次构造事务中选择 GPU 或 PixelUpload 分支，并交付同源 capabilities。registry 在发布前核对平台、feature、API、raster/present 轴和分支：错配执行 checked shutdown 并返回 typed error。GPU baseline/probe 只对 GPU 分支运行；PixelUpload 不查询或伪造 RHI。

构造成功表示 context 已绑定 surface、处于 owner thread 且可进入第一帧。recipe 身份和构造期静态能力在 owner 内冻结；恢复仍使用相同类型化构造流程，不从 backend 名称或临时布尔查询重新拼装 recipe。

## 线程、所有权与生命周期

- registry 验证后建立 `!Send + !Sync` 的线程绑定 owner，并在每次原生调用前校验 owner thread。
- native factory 在 context 离开 platform System 前返回已验证 `GraphicsRecipeOwner`；窗口只持有 native window/surface 关系，不成为图形 context 的第二 teardown owner。
- resize 只在底层成功后原子发布新的 `PresentSurface` 和 generation；失败保留旧快照及 typed 原因，不能部分刷新 extent、DPR 或 transform。
- 错误线程 Drop 不调用原生 API。正常关闭必须在 owner thread 停止新操作、隔离 callback、checked shutdown surface/device；失败交给最终责任边界观察。
- recipe owner、surface token、present image 和 callback source 使用同一 generation。旧代帧、资源或 callback 只能 stale，不能重定向到新 surface。

## GraphicsDevice 与 GraphicsSurface

`GraphicsDevice` 只提供 buffer、texture、sampler、pipeline、render target、pass、draw/copy、submit 和设备维护所需的最小原语。不得提供 `draw_glyphs`、`draw_rounded_rect`、`draw_picture`、高层 offscreen blur 等 UI 语义；固定 pipeline、batch、atlas 与 effect 编排由 graphics/backend 持有。

`GraphicsSurface` 独立持有窗口 surface、swapchain、物理 extent、generation、当前 present image 和不提交帧数据的可呈现探测。device 与 surface 可以在同一 Adapter owner 中组合，但资源寿命、错误分类和重建范围必须可区分；首版不要求跨窗口共享 device。

GPU resize、present 和 readback 只经 `GraphicsSurface`；CPU 像素 resize/提交只经 `PixelUploadSurface`。共享 lifecycle 不提交帧 payload，也不暴露 external swapchain 视图。

## 静态能力与动态事实

- `GraphicsContextCaps` 只描述构造期稳定的 backend、recipe、交换链 coherency 与支持轴。
- drawable extent、DPR、transform、surface generation 与当前 image 都是动态事实，只能来自一次原子 `PresentSurface` / owner 查询。
- `GraphicsCapabilities` 只陈述 sampled texture、render-to-texture、scissor、copy、retained framebuffer、partial present、occlusion、readback 等可验证原语。
- UI 操作支持由 graphics System 从这些事实推导；Adapter 不维护平行 `draw_*` 能力表。
- `retained_framebuffer` 不等于 `partial_redraw`。局部提交还要求全部绘制经过 retained target，并且交换链能证明 per-image coherency；条件不足时固定为 FullOnly。

能力在构造/恢复边界探测，不能在同一 generation 的帧内漂移。运行中真实失败仍由 typed error 表达，不用临时降级改写已发布能力含义。

## 坐标与行序

thin RHI 输入统一使用左上原点：viewport、scissor、顶点、UV、readback 区域与 `PresentDamage` 共享同一语义；`read_surface_pixels` 输出 row 0 也表示窗口顶部。

原生 API 的 NDC、scissor 原点和 framebuffer 行序差异只在 Adapter 内转换：

- texture target 与 window target 分别选择正确的 Y 映射，不修改通用 shader/场景语义；
- readback 在 Adapter 内换算区域并按需翻转行；
- CPU upload 数据保持 top-left 直通；
- texture copy 和滚动搬移逐 texel 对位，不通过重复翻转偶然抵消。

graphics/backend 只消费这一规范契约，不按 D3D、OpenGL、Vulkan 或 Metal 名称叠加平台特例。

## Presenter 与提交真相

`Presenter` 是最终提交边界：GPU recipe 委托 `GraphicsSurface::present`，PixelUpload recipe 委托 `present_pixels`。同一帧只能由一个 recipe owner 调用一次最终 OS present。

device submit 成功、像素上传成功、present 调用已发起和画面实际成功是不同事实。最终 present 返回 typed result；失败、遮挡、would-block 或 stale generation 都不能标记为成功，也不能消费 graphics damage。取消、不可呈现与不支持必须使用可区分结果，不能压成布尔值。

## 错误、安全与数据边界

- Adapter 把 HRESULT、errno、API status 与设备原因转换为保留来源链的 typed error；OOM、SurfaceLost、DeviceLost、Occluded、WouldBlock、NotImplemented 与参数错误保持可区分。
- raw handle、映射指针、command buffer 和 swapchain image 不越过 thin RHI。FFI/unsafe 调用逐次证明线程、句柄、长度、对齐和生命周期前置条件。
- readback 可能包含敏感界面像素，只作为显式可选能力开放；platform 不默认记录、持久化、上传或跨窗口共享结果。
- extent、row pitch、buffer size、offset 和 damage rect 在进入原生 API 前验证有限范围及整数溢出；无效输入不传给驱动猜测。

## 验证责任

presentation 变更至少按风险验证：recipe/registry 分支一致性、错误线程调用拒绝、resize 原子发布、旧 generation 隔离、capability 与实际 probe 一致、一次最终 present、失败帧不消费 damage、readback 行序、checked shutdown，以及多个原生 Adapter 复用同一 thin RHI。真实 GPU、DPI、resize、遮挡和驱动矩阵的实时结果由 Vikunja 持有；缺少环境时标为未验证。

## 模块不变量

- platform 只拥有原生资源、thin RHI 和最终提交能力，不拥有 UI 绘制与恢复策略。
- 一个 recipe owner 对应一个线程亲和生命周期和一个最终 present 责任边界。
- 静态能力与动态 surface 事实分离；旧快照不能证明新 generation 仍有效。
- 任一失败都保留 typed 语义，不能通过空实现、`Option`、bool、Drop 或隐式 fallback 伪装成功。
