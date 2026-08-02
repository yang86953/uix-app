# renderer 模块

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统的目标 `renderer` 模块及帧会话、提交结果与恢复契约。依赖：[scene](scene.md)、[backend](backend.md)、[resources](resources.md)、[platform/presentation](../platform/presentation.md)。导出：`Renderer` 与 app 使用的 `RenderTarget`。

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

## 组件：Renderer

CPU/GPU 是 Renderer 内部 backend 选择，不是两套组件绘制 API。同窗一帧只有一条有序命令序列和至多一次最终 present。

## 组件：ScenePipeline

ScenePipeline 合并失效、选择 full/partial/composite 策略并驱动 scene；backend/capability 替换后必须重新读取能力，不能沿用旧提交协议。

## 组件：GraphicsRecovery

surface lost/outdated、occluded/would-block、device lost 和 OOM 保持不同分类。graphics 拥有资源重建，app 按窗口登记重试/休眠 deadline；失败帧保留 dirty。

## 模块不变量

最终 OS/GPU present 成功后才消费 damage 和记录成功指标；retained dirty 不得在不可呈现状态形成 busy loop。
