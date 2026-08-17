# renderer 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics System 的 `renderer` Module 及帧会话、提交结果与恢复契约。基础依赖：[platform/presentation](../platform/presentation.md)；[scene](scene.md)、[backend](backend.md)和[resources](resources.md)的协作由 graphics System 编排，Module 间不直接持有实例。导出：`Renderer` 与 app 使用的 `RenderTarget`。

> **当前实现线索**：主要位于 `src/draw/renderer/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Renderer` | struct | 持有已选择 backend 的唯一具体渲染器 |
| `RenderSession` | struct | owner-thread 帧与 backend 生命周期 |
| `RenderTarget` | trait | resize、帧、present 和维护边界 |
| `ScenePipeline` | struct | LayerTree 到一帧执行输入/输出 |
| `FrameRenderInput` / `FrameRenderOutput` | struct | 场景执行数据 |
| `RenderOutcome` / `GraphicsFailure` | enum | 提交结果与 typed 调度分类 |
| `InvalidationQueue` / `Invalidation` | struct/enum | 节点 Paint/Layout/Composite 失效 |
| `GraphicsRecovery` / `RecoveryDriver` | struct | backend/surface 恢复协调 |
| `RenderMetrics` | struct | 成功帧和失效来源指标 |

运行时 backend 切换由 `RenderSession` 作为唯一 owner 执行检查式事务：候选构造失败不触碰当前 backend；当前 backend 的 `try_shutdown` 成功是 owner 切换提交点，失败时返回原 typed error 并保留原 backend、extent 与 capability。提交后先安装候选并保留 `FullRedraw` 要求，再恢复同一 extent；若候选 resize 失败，新 backend 仍作为唯一 owner 留在会话中供恢复、重试或 checked shutdown，不能伪装成已回滚到旧 backend。

遮挡本身是健康的 surface 可用性状态，不触发 backend 重建。若同一 surface 连续三次出现无数据 `test_present` 返回 `Presentable`、紧随其后的真实 Present 却仍返回 `GraphicsOccluded`，`RecoveryDriver` 才把该协议矛盾升级为 `GraphicsSurfaceLost`；该有界阈值表示当前显示输出与硬件 swapchain 不兼容，因此下一帧直接执行一次 `Software` 恢复，不再换用另一条 GPU recipe。probe 仍返回 `Occluded` 或后续 Present 成功都会清零矛盾计数。

## 组件：Renderer

CPU/GPU 是 Renderer 内部 backend 选择，不是两套组件绘制 API。同窗一帧只有一条有序命令序列和至多一次最终 present。

Renderer 只面向 `RenderBackend`：CPU 路径执行 canonical 语义，GPU 路径由[通用 GPU Renderer](backend.md)统一降级为 `FramePlan` / `DrawPacket`。Renderer 不选择原生 pipeline、不持有 RHI 资源，也不按 D3D11、Vulkan、Metal 等 API 分叉场景调度。

## 组件：ScenePipeline

ScenePipeline 合并失效、选择 full/partial/composite 策略并驱动 scene；backend/capability 替换后必须重新读取能力，不能沿用旧提交协议。

## 组件：GraphicsRecovery

surface lost/outdated、occluded/would-block、device lost 和 OOM 保持不同分类。graphics 拥有资源重建，app 按窗口登记重试/休眠 deadline；失败帧保留 dirty。

## 所有权、生命周期与模块不变量

- 每个 `RenderTarget` / surface generation 只由一个 owner-thread `RenderSession` 拥有；`Renderer` 句柄不能制造第二个 backend owner，也不能跨窗口复用线程亲和资源。
- resize、backend 替换、surface 重建和 shutdown 都是检查式状态转换；失败返回 typed error，并保留文档声明的唯一可恢复状态，不静默回退或伪造成功。
- 最终 OS/GPU present 成功后才消费 damage 和记录成功指标；retained dirty 不得在不可呈现状态形成 busy loop。
- 关闭先停止新帧、取消/隔离晚到 callback，再 checked shutdown backend 与 surface；shutdown 失败仍须被最终责任边界观察，不能通过 drop 吞掉。
- `RenderOutcome` 只陈述本次提交结果；恢复已请求、deadline 已登记或 backend 已构造都不表示画面已经呈现。
