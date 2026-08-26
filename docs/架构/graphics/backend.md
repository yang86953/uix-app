# backend 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics System 的 CPU/GPU 执行、能力协商、资源代际与降级契约。基础依赖：[platform/presentation](../platform/presentation.md)；[painting](painting.md)、[scene](scene.md)与[renderer](renderer.md)的输入由 graphics System 编排，Module 间不直接持有实例。导出：renderer 内部 `RenderBackend`。
>
> **当前实现线索**：通用执行位于 `src/draw/backend/`；API 无关 thin RHI 与共用机制位于 `src/platform/presentation/rhi/`；类型化 recipe、registry、线程绑定与原生 Adapter 位于 platform System 的私有实现 `src/native/`。永久源码依赖规则见[图形源码边界](source-boundary.md)。路径只用于定位迁移，不构成公开 API。

## 当前实现状态

| 状态 | 后端/范围 | 当前实现与公开边界 |
|---|---|---|
| **已实现** | 共享规范 | Drawing 唯一持有 11 类 `PipelineKind` 的 canonical 语义，三套 Adapter 同时消费同一 `RhiSurfaceLifecycle`；这些是私有实现，不建立项目测试。 |
| **已实现** | Vulkan | Linux、Windows、macOS 三个生产 registry 中 Vulkan 均为唯一最高优先级 `100`；公开使用方通过 `GraphicsBackend` 与 `Platform` 门面选择和查询。多窗口所有权细节见 [Vulkan 多窗口共享设备合同](vulkan-multi-window.md)。 |
| **已实现** | OpenGL ES | Linux EGL/OpenGL ES 实现共享 Surface 创建、resize、失效与恢复生命周期。OpenGL 仍是显式兼容候选，不改变 Vulkan-first 生产优先级。 |
| **已实现** | D3D11 实现边界 | Windows D3D11 已具备生产 draw/submit、Blur 与 Surface 生命周期实现；内部 HWND、readback、RHI 和链接结构不属于公开 API，不建立项目测试。 |
| **待验收** | D3D11 真实 Windows 公开运行 | 真实 Windows x64 的公开 `GraphicsBackend::Direct3D11` 使用路径尚未完成；该项是 `0.0.1` 发布门禁，不能由交叉链接或内部门禁替代。实时阻塞与环境证据由 [Gitea Issue #10](http://100.79.245.29:3000/admin/uix-app/issues/10) 持有。 |

上述“已实现”只记录仓库当前固定实现；公开 API 测试状态与环境结果由 Gitea 维护，内部图形执行不形成测试矩阵。

### Windows 公开 API 验收条件

当前发布门禁必须由外部应用在真实 Windows x64 桌面会话中，只通过公开 `App`、`GraphicsBackend` 与 `Platform` API 选择 D3D11，并观察公开成功结果或 typed failure。不得启用 parity/test-harness feature，不得访问 HWND、RHI、FramePlan、readback、故障注入或其他私有入口。在公开入口实际执行并记录环境前继续保持 **待验收**。

## 责任边界

backend 把 backend-neutral 的帧计划执行到 CPU 像素目标或 GPU thin RHI。UI 图元、painter order、clip、transform、opacity、blend、Picture 和 effect 的规范语义只在 graphics System 定义一次；原生 Adapter 只实现资源、命令、surface 和 present 原语，不复制逐 UI 组件或图元算法。

```text
painting / scene 的规范输入
  → graphics System 编排 FramePlan
  → RenderBackend（CPU 或通用 GPU Renderer）
  → platform thin RHI / PixelUploadSurface
  → 唯一最终 present
```

renderer、scene、painting、UI 和应用不得取得 raw GPU/native handle。platform 也不得反向解释 Theme、Widget、PaintOp 或 UIX 属性。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `RenderBackend` | internal trait | 执行帧、资源、Picture/effect、提交与 checked shutdown |
| `GpuBackend` | struct | 把规范图形语义降低为统一 `FramePlan` 并驱动 thin RHI |
| `SoftwareRasterizer` | struct | 以同一规范语义产生 CPU 像素 |
| `FramePlan` / pass / draw packet | values | 有序、backend-neutral 的设备执行计划 |
| `RhiRendererFrame` / `RhiRendererPass` | internal owner / value | 由一次 Surface 或 Offscreen 执行唯一持有计划，把 producer 的目标无关命令包绑定到合法目标并消费一次执行资格 |
| `NativeRasterCaps` | value | 从一次实际 `GraphicsDeviceCapabilities` 快照派生的 renderer 能力 |
| `SoftFallbackTile` | staging value | 语义等价且有界的局部 CPU fallback 输入 |
| Picture/effect planner | internal components | 编排离屏资源、采样合成、blur 和清理事务 |

## 消费的 platform 契约

| 契约 | 责任 |
|---|---|
| `GraphicsRecipeOwner::{Gpu, PixelUpload}` | 类型化交付唯一原生 recipe owner，不使用运行期 `Option` 猜测能力 |
| `GpuRecipeContext` / `PixelUploadSurface` | 分离 GPU RHI 与 CPU 像素上传生命周期 |
| `GraphicsDevice` / `GraphicsSurface` | 资源、pass、submit、resize、present 与可选 readback 原语 |
| `PresentSurface` | 同一时点的 extent、DPR、transform 与 generation 原子快照 |
| `GraphicsDeviceCapabilities` / `GraphicsSurfaceCapabilities` | 分别陈述设备原语，以及 Surface 呈现一致性与可选原语，不从平台名称推断 |

这些是跨 System 公开契约。backend 不取得 platform 私有 Adapter、registry 或原生句柄。

## 构造与能力协商

Adapter 创建事务直接交付 GPU 或 PixelUpload candidate，并保证 recipe 类型、能力快照和 surface 来源一致。GPU recipe 的呈现一致性只能从实际 `GraphicsSurfaceCapabilities` 冻结；owner 会在交付 Drawing 前拒绝静态快照与实际 Surface 漂移。registry 与 backend 在发布 owner 前完成以下检查：

1. candidate 类型与静态 recipe 轴一致；
2. GPU candidate 满足统一 retained RHI 基线和固定 pipeline probe；
3. PixelUpload candidate 只验证 CPU 提交所需能力，不触碰 RHI；
4. 任一错配执行 checked shutdown，并返回保留原因链的 typed error。

构造成功即表示 context 已绑定目标 surface 并可进入第一帧，不再存在二阶段 `initialize` 或“默认成功”的空实现。能力快照绑定 surface/device generation；resize、设备替换或恢复后必须重新读取，不能缓存平台名或旧布尔值推断新能力。

baseline 缺失是构造/恢复失败。可选效果可以返回明确“不支持”并采用已声明的等价降级；编译成功、API 存在或 Adapter 声称支持都不能代替运行契约。

## 所有权与生命周期

```text
WindowSession
  → RenderSession
    → 唯一 RenderBackend
      → 唯一 GraphicsRecipeOwner
        → device / surface / generation-scoped resources
```

- 所有线程亲和对象由 owner-thread 独占；cloneable handle 只携带稳定身份和 generation，不制造第二个 owner。
- GPU resize 只经同一 `GpuRecipeContext` 进入 `GraphicsSurface::resize`；PixelUpload resize 只经 `PixelUploadSurface`。底层成功后才原子发布新的 `PresentSurface`。
- 旧 generation 的 retained target、Picture、effect、callback 和帧计划必须在新代使用前 checked release 或明确隔离；晚到结果不能写入新 owner。
- 关闭先停止新帧和资源创建，再隔离 callback、结束活动离屏事务、释放资源、surface 与 device。destroy/shutdown 失败保持 typed error，由最终责任边界观察，不能通过 Drop 伪装成功。

## 帧执行与提交

- 同窗一帧只有一条有序计划、至多一次主 surface acquire 和一次最终 present。每次 Surface 或 Offscreen 执行都由唯一 `RhiRendererFrame` 持有并执行自己的 `FramePlan`；producer 只能生成目标无关的 `RhiRendererPass` 命令包，不能直接构造真实 pass、持有计划或调用底层执行器。
- `RhiRendererFrame` 的封闭角色决定默认目标：Surface 角色只能写入本次 acquire 的 surface，Offscreen 角色默认写入构造时冻结的最终 texture。只有 Offscreen 角色可在计划变化前通过门禁加入显式纹理目标；Surface 角色尝试注入离屏目标必须返回 typed `InvalidArgument`，且不得改变计划。
- `GraphicsDevice::submit` 返回的 `SubmissionHandle` 是组合 context 的类型化事务身份；最终 `GraphicsSurface::present` 只接受同一 context 最近一次成功提交。身份签发与校验由共享 RHI 状态机定义，Adapter 不得忽略参数或维护另一套计数规则。
- render-pass 生命周期由共享 `RhiPassState` 原子拥有：活动目标、物理 extent、scissor 与 sampled texture/sampler 绑定随 begin/end 共同建立和清除。槽位限制、pass 内外命令位置及 render target 反馈环在这里统一拒绝；Adapter 只保留 framebuffer/RTV 等原生编码对象，不得维护平行 `pass_open` 或绑定镜像。
- texture copy/move 的格式、非空区域、checked 边界和资源关系由共享传输契约一次验证；普通 copy 只允许不同的同格式可渲染颜色纹理，同资源区域搬移必须走具有 scratch/memmove 语义的 move。两端坐标始终是左上原点，Adapter 不得通过私有翻转或饱和运算改写它。
- 唯一 `FramePlan` 执行器在任何原生命令前依次调用 `GraphicsDevice::activate` 与 `GraphicsDevice::maintain`。前者只建立 owner-context 可用性，OpenGL 在此恢复 current context；后者只检查设备健康。surface 与 offscreen 路径都不能依赖上层调用顺序或另一个窗口遗留的 current 状态。
- painter order、clip、transform、opacity 和 blend 切换形成明确 barrier；优化、合批与 fallback 不能跨越目标相关操作重排。
- retained 主颜色目标可靠时，Drawing 可以只更新 dirty rect，并在 Device 明确实现 `texture_region_move` 时执行滚动搬移；该能力不依赖 swapchain image 历史。最终 partial present 仍要求 Surface 证明 per-image coherency，否则 presenter 把同一 damage 真相升级为完整 surface 提交，不能反向迫使 retained 内容整帧重绘。
- device submit 成功、CPU 像素已写入、帧已编码或 present 已调用都不等于画面已呈现；只有平台返回最终 present 成功才消费 damage、推进成功历史和发布 presented revision。
- 空帧、不可呈现、遮挡、would-block 与失败是不同结果。不可呈现状态保留 dirty，但不得由 retained dirty 形成 busy loop。

## 坐标、像素与行序

通用 graphics 使用左上原点 logical 坐标，并把 texture `v=0` 解释为顶部。CPU 像素、Picture、retained texture、sampled 合成、blur 和机械 copy 均保持同一 top-left 语义；不得依赖“偶数次翻转”偶然抵消。

native surface 的坐标与行序差异由 [platform/presentation](../platform/presentation.md) 的 Adapter 消化。readback 若受支持，必须返回 top-left 结果并在 Adapter 内完成区域换算和行反转；graphics 不再叠加平台特例。

全部 extent、DPR、transform、颜色和采样参数必须有限且在资源预算内。亚像素几何保留原始精度，不能为了适配整数 fast path 而静默取整；无法等价表达时选择受控 fallback 或返回 typed error。

## CPU 与 GPU fallback

CPU backend 是完整 renderer，不是各原生 Adapter 的补丁集合。帧内 GPU→CPU fallback 只在以下条件同时满足时允许：

- 像素语义、painter order、clip、transform、opacity 与 blend 可保持；
- staging 区域和内存有硬上限；
- CPU 结果能作为普通 sampled 输入回到当前有序计划；
- fallback 不绕过 capability、generation、damage 或最终 present 契约。

GPU baseline 操作不能依赖常态 CPU fallback。无法保持语义、资源不足或设备状态不安全时返回 typed failure，由 renderer 的恢复策略选择下一帧重试、surface 重建或整 backend 替换。可选 backdrop blur 不支持时可以保留纯色 mask，但不能伪报 blur 成功。

## Picture、offscreen 与 effect

- Picture/offscreen texture 有唯一 owner 和 generation。创建返回的“不支持/无效空尺寸”与实际分配失败必须可区分；destroy 只有资源释放完成后才成功，失败时保留 owner 供恢复或关闭重试。
- begin/flush/end/blit 都是 checked 边界。主 surface present 前若仍有活动 Picture，必须先成功结束；未实现操作返回 typed `NotImplemented`，不能 no-op 后报告成功。
- 无像素贡献的空源、空目标或全透明 blit 是 canonical no-op；非空越界源、无效变换或不能保持语义的输入返回 typed error。
- backdrop blur 同时维护未模糊 clean texture 和从其派生的 effect texture；策略、半径、区域、主题、surface 或 generation 变化时从 clean 重新派生，不能累计模糊旧结果。
- blur 的 source→scratch→target 两个 pass 由同一个 Offscreen `RhiRendererFrame` 持有：水平 pass 经角色门禁绑定 scratch，垂直 pass 写入构造时冻结的最终 target，完整计划只执行一次 submit，且不取得主 surface、不额外 present。scratch 在主阶段成功或失败后都执行检查式释放；主阶段失败优先返回原始错误，主阶段成功而清理失败时返回清理错误。任一步失败保留 invalidation，并停止当前帧最终提交。

## 恢复与失败分类

`SurfaceLost`、`DeviceLost`、`Occluded`、`WouldBlock`、OOM、`NotImplemented` 与来源违规保持不同 typed 分类。backend 只陈述执行失败与可恢复状态，graphics renderer 拥有恢复算法，app 只登记窗口 deadline 和调度机会，diagnostics 只观察/协调。

恢复和 bootstrap 共用同一类型化装配入口。候选 backend 在完整验证前不能替换当前 owner；替换提交后若 resize 失败，新 owner 保持唯一并进入恢复/关闭路径，不能伪装回滚到已释放旧 owner。重试必须有次数或 deadline 上限，不允许每帧无条件重建。

## 数据与安全边界

- shader、pipeline 和 draw packet 由框架固定生成，不把任意用户 shader、原生命令或 GPU 指针作为 UI API。
- 图片、字体、路径和 RichText 等不可信内容在 resources/painting 边界完成大小与格式约束；backend 仍须检查缓冲长度、offset、row pitch、索引范围和整数溢出。
- surface readback 是显式可选能力，可能包含敏感界面像素；只有授权的宿主流程才能请求，结果不得自动写日志、上传或跨窗口共享。
- 原生 FFI 与 unsafe 操作收敛在 Adapter，逐次证明线程、句柄、长度、对齐和生命周期前置条件；unsafe 函数体本身不视为隐式授权。

## 公开 API 测试边界

- 只测试使用方能够从 `App`、`GraphicsBackend`、`Platform` 和公开 Drawing 类型观察的构造、选择、结果与 typed failure。
- painter lowering、FramePlan、资源事务、generation、damage、present、恢复状态机、Adapter、RHI、shader、像素 readback、GPU parity 和驱动差异都是内部实现，不建立项目测试。
- 真实环境运行或人工视觉检查可以作为发布操作记录，但不属于项目测试，也不得据此扩大测试面。

## 模块不变量

- UI 图形语义只有一个生产定义；native Adapter 只提供 thin RHI / PixelUpload 原语。
- 同一 surface generation 只有一个 backend owner、一条帧计划和一个最终 present 责任边界。
- 每次 Surface 或 Offscreen 执行只有一个 `RhiRendererFrame` 计划 owner；pass producer 不拥有目标选择、计划追加或底层执行资格。
- capability 来自实际原语和 probe，不能由平台名、编译 feature 或旧缓存推断。
- 任一 fallback、恢复或优化都不能改变像素语义、跳过 typed failure、消费失败帧 damage 或建立第二条生命周期。

## 已知设计债务与待跟进

以下为图形后端评审确认的设计债务，暂不影响 0.0.1 图形门禁，但需在对应改动前收敛。

- **soft fallback 使用量不可观测**：`RenderMetrics` 只统计 layout/paint/present/idle，没有 soft 路径（CPU 栅格化段）的使用率统计。`NativeGpuCanvas2D` 在 `!retained_color_target || !native_blend` 等条件下整段落入 CPU 软栅格化，若某平台能力缺失导致常态触发，性能退化无指标暴露。待跟进：为 renderer metrics 增加 soft 像素/软段帧计数，soft 占比超标时可观测告警。
- **macOS Metal 占位条目的 recipe 声明待修正**：`registry_macos.rs` 中 Metal 条目标注 `RasterMode::Cpu × PresentMode::PixelUpload`，与 Metal 作为 GPU API 的预期形态（应为 `GpuNative × Swapchain`，经 CAMetalLayer）矛盾。当前状态 `Disabled` 无实际影响，但启用 Metal 前必须先修正 recipe 轴，避免误导实现。
- **PixelUpload 完整实现暂无活跃消费者**：`thread_bound.rs` 的 CPU 像素上传链、contract 与 macOS cocoa 引用均齐备，但当前任何 Active 注册表条目都不使用它（macOS 生产实际走 Vulkan/MoltenVK）。该路径的启用条件（如无 GPU 环境回退或 CI 兜底）未写入设计文档，存在成为死代码的维护成本。待跟进：在架构文档明确其启用条件与验收入口。
