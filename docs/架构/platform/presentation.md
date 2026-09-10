# presentation 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform System 的原生 surface、图形 recipe、thin RHI、presenter 与提交能力。基础依赖：core；[windowing](windowing.md)产生的窗口 owner 由 platform System 编排交付，Module 间不直接持有实例。导出：供 graphics [backend](../graphics/backend.md) bootstrap 和执行使用的平台图形边界。
>
> **当前实现线索**：API 无关 thin RHI 与共用机制位于 `src/platform/presentation/rhi/`；类型化 candidate/owner、registry、线程绑定及原生 Adapter 位于 platform System 的私有实现 `src/native/`。Vulkan GPU-native swapchain 是三平台唯一最高优先参考路径；OpenGL ES、D3D11、D3D12 与 Metal 已实现同一 RHI/Surface 生命周期，精确运行状态与暂缓项见 [graphics/backend 状态矩阵](../graphics/backend.md#当前实现状态)。路径只用于定位实现，不构成公开 API；后续新增环境矩阵由 Gitea 持有。

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
| `RhiSurfaceLifecycle` | internal state machine | 原子拥有 swapchain generation、重建事务及当前帧成功/重试语义 |
| `GraphicsDeviceCapabilities` / `GraphicsSurfaceCapabilities` | values | 分别陈述设备原语，以及 Surface 的呈现一致性与可选原语事实 |
| `Presenter` | submit interface | 建立本帧最终 OS present 结果 |
| `SurfaceToken` / `PresentSurface` | generation/snapshot values | 隔离重建、迟到 callback，并原子描述 extent、DPR 与 transform |

## GraphicsRecipe 与选择

registry 只陈述可构造候选；graphics System 决定使用、恢复和 fallback 策略。显式 API 请求只尝试该 API 的完整 recipe，不偷换其他 API；Automatic 按固定候选顺序选择已启用且通过验证的 recipe，GPU 候选耗尽后才由上层决定是否整体使用 CPU。

`GraphicsContextCandidate` 必须在一次构造事务中选择 GPU 或 PixelUpload 分支，并交付同源 capabilities。GPU recipe 中的 `PresentCoherency` 只能从实际 `GraphicsSurfaceCapabilities` 冻结，owner 在发布前再次核对两者；任何漂移都执行 checked shutdown 并返回 typed error。registry 还要核对平台、feature、API、raster/present 轴和分支。GPU baseline/probe 只对 GPU 分支运行；PixelUpload 不查询或伪造 RHI。

构造成功表示 context 已绑定 surface、处于 owner thread 且可进入第一帧。recipe 身份和构造期静态能力在 owner 内冻结；恢复仍使用相同类型化构造流程，不从 backend 名称或临时布尔查询重新拼装 recipe。

## 线程、所有权与生命周期

- registry 验证后建立 `!Send + !Sync` 的线程绑定 owner，并在每次原生调用前校验 owner thread。
- native factory 在 context 离开 platform System 前返回已验证 `GraphicsRecipeOwner`；窗口只持有 native window/surface 关系，不成为图形 context 的第二 teardown owner。
- resize 只在底层成功后原子发布新的 `PresentSurface` 和 generation；失败保留旧快照及 typed 原因，不能部分刷新 extent、DPR 或 transform。
- 错误线程 Drop 不调用原生 API。正常关闭必须在 owner thread 停止新操作、隔离 callback、checked shutdown surface/device；失败交给最终责任边界观察。
- recipe owner、surface token、present image 和 callback source 使用同一 generation。旧代帧、资源或 callback 只能 stale，不能重定向到新 surface。

`RhiSurfaceLifecycle` 的稳定状态转换为 `Uninitialized → Recreating → Active`，以及 `Active/Invalidated → Recreating → Active/Invalidated`；只有原生重建成功提交才发布新 extent 并推进 generation。窗口零尺寸不进入原生重建事务：逐窗调度器进入 `Suspended(ZeroExtent)`，保留 dirty，且 GPU backend 不创建 1×1 替身；恢复到正尺寸后才执行正常 resize。

- acquire 或 present 返回 `OUT_OF_DATE` 且未证明本帧已呈现时，Adapter 只映射为拒绝原因；受控重建成功后返回 `GraphicsSurfaceChanged`，当前 dirty 帧用新 token 重建 `FramePlan` 并立即重试，不触发整 recipe fallback。
- `SUBOPTIMAL` 表示当前 present 已接受：先登记本帧同步完成事实，再为后续帧重建并推进 generation；当前帧仍是成功 present，不能改写为失败或重复消费 damage。
- 原生重建失败进入 `Invalidated` 并保留旧 token 仅供诊断；后续只能开始新的受控重建。失败必须保留原生状态与重建错误来源链，不能继续 acquire 旧代际或静默丢帧。

## GraphicsDevice 与 GraphicsSurface

`GraphicsDevice` 只提供 buffer、texture、sampler、pipeline、render target、pass、draw/copy、submit 和设备维护所需的最小原语。不得提供 `draw_glyphs`、`draw_rounded_rect`、`draw_picture`、高层 offscreen blur 等 UI 语义；固定 pipeline、batch、atlas 与 effect 编排由 graphics/backend 持有。

`GraphicsSurface` 独立持有窗口 surface、swapchain、物理 extent、generation、当前 present image 和不提交帧数据的可呈现探测。device 与 surface 可以在同一 Adapter owner 中组合，但资源寿命、错误分类和重建范围必须可区分；首版不要求跨窗口共享 device。

组合 owner 通过共享 `RhiSubmissionSequence` 关联两个角色：Device 只从该状态机签发非零 `SubmissionHandle`，Surface 在触碰原生 Present 前只接受同一 owner 最近一次成功提交。OpenGL、D3D11、D3D12、Vulkan 与 Metal Adapter 只调用该共享规则，不得各自维护或跳过提交身份语义。

Device 内部通过共享 `RhiPassState` 管理 API 无关的 render-pass 生命周期。活动目标身份、物理 extent、scissor 和采样绑定必须原子建立与清除；pass 外绑定、非零槽位、目标自采样反馈环和未结束 pass 的 submit 必须在共享层得到同一拒绝结果。Adapter 只保存当前 framebuffer、RTV 等原生对象，并在 end 时显式解除输入与输出绑定，不依赖驱动替调用方解决资源冲突。

共享 FramePlan 允许同一 Buffer 在 painter order 中多次上传和绘制。立即执行 API 可以直接消费更新；延迟执行 API 必须在 Adapter 的统一帧资源层冻结每次 Draw 观察到的 Vertex、Index 与 Uniform 内容，并在覆盖提交的 fence 完成前保持这些快照有效。该机制只能集中实现一次，不得分散复制到各 pipeline 或 UI 操作分支。

每次 `FramePlan` 执行都在目标和结构验证成功后、第一条原生命令前依次调用 `GraphicsDevice::activate` 与 `GraphicsDevice::maintain`。`activate` 只建立原生 owner-context 可用性，OpenGL 在这里恢复对应 current context；`maintain` 只检查设备健康，D3D11 在这里映射 device-removed 状态。资源借用和 checked teardown 同样先 activate，但不得因健康检查失败而失去释放机会；离屏计划不得借用另一个窗口或前一事务遗留的隐式上下文。

GPU resize、present 和 readback 只经 `GraphicsSurface`；CPU 像素 resize/提交只经 `PixelUploadSurface`。共享 lifecycle 不提交帧 payload，也不暴露 external swapchain 视图。

## 静态能力与动态事实

- `GraphicsContextCaps` 只描述构造期稳定的 backend、recipe、支持轴，以及从实际 Surface 冻结的 coherency 快照；它不是第二个能力权威。
- drawable extent、DPR、transform、surface generation 与当前 image 都是动态事实，只能来自一次原子 `PresentSurface` / owner 查询。
- `GraphicsDeviceCapabilities` 只陈述 buffer、texture、sampler、pipeline、pass、scissor、copy、blend 等可执行设备原语；不得夹带 Renderer 派生策略或 Surface 状态。
- `GraphicsSurfaceCapabilities` 是 Surface 呈现事实的权威来源，直接陈述 `PresentCoherency` 与同步 readback 等可选原语。局部提交不能再复制为 `partial_present` 布尔值；candidate 中的静态 recipe 只能复制并校验这一事实。
- 遮挡退出探测由正常 Present 的 `GraphicsOccluded` 结果进入，并以 `PresentTestResult` 返回动态状态；窗口生命周期遮挡依赖原生可见性事件恢复，两者都不需要静态 `occlusion` 布尔值。
- UI 操作支持由 graphics System 从这些事实推导；Adapter 不维护平行 `draw_*` 能力表。
- Drawing 从 render-to-texture、sampled texture 与 copy 原语共同推导 `retained_color_target`，并只依据该保留事实与 `texture_region_move` 决定局部重绘和滚动搬移。最终 Surface 是否接收局部 damage 由 `PresentCoherency` 独立决定；`FullOnly` 只把最终呈现升级为完整 surface，不得反向关闭 retained texture 的局部复用。

能力在构造/恢复边界探测，不能在同一 generation 的帧内漂移。运行中真实失败仍由 typed error 表达，不用临时降级改写已发布能力含义。

## 坐标与行序

thin RHI 输入统一使用左上原点：viewport、scissor、顶点、UV、readback 区域与 `PresentDamage` 共享同一语义；`read_surface_pixels` 输出 row 0 也表示窗口顶部。

原生 API 的 NDC、scissor 原点和 framebuffer 行序差异只在 Adapter 内转换：

- texture target 与 window target 分别选择正确的 Y 映射，不修改通用 shader/场景语义；
- readback 在 Adapter 内换算区域并按需翻转行；
- CPU upload 数据保持 top-left 直通；
- texture copy 和滚动搬移先经过共享格式、非空区域、checked 边界与资源关系门禁，再逐 texel 对位；同资源重叠区域只走 scratch/memmove 语义；
- OpenGL 离屏 texture 的逻辑顶部就是原生第零行，copy 两端直接使用 top-left Y；只有 window surface 边界执行所需的原点转换，不通过重复翻转偶然抵消。

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

presentation 变更至少按风险验证：recipe/registry 分支一致性、错误线程调用拒绝、resize 原子发布、旧 generation 隔离、capability 与实际 probe 一致、一次最终 present、失败帧不消费 damage、readback 行序、局部 retained `TextureMove` 的真实执行与像素结果、checked shutdown，以及多个原生 Adapter 复用同一 thin RHI。执行证据由共享 FramePlan 统计，Adapter 不维护平台私有成功计数。真实 GPU、DPI、resize、遮挡和驱动矩阵的实时结果由 Gitea 持有；缺少环境时标为未验证。

## 模块不变量

- platform 只拥有原生资源、thin RHI 和最终提交能力，不拥有 UI 绘制与恢复策略。
- 一个 recipe owner 对应一个线程亲和生命周期和一个最终 present 责任边界。
- 静态能力与动态 surface 事实分离；旧快照不能证明新 generation 仍有效。
- 任一失败都保留 typed 语义，不能通过空实现、`Option`、bool、Drop 或隐式 fallback 伪装成功。
