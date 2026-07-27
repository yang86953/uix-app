# backend 模块

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统的目标 `backend` 模块及 CPU/GPU 执行契约。依赖：[painting](painting.md)、[platform/presentation](../platform/presentation.md)。导出：renderer 内部 `RenderBackend`。

> **当前实现线索**：主要位于 `src/draw/backend/` 与 platform 的原生图形适配。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `RenderBackend` | trait | 帧执行、surface 能力和资源边界 |
| `BackendKind` / `BackendCapabilities` | enum/struct | backend 分类与支持能力 |
| CPU backend / rasterizer | 子模块 | retained pixels、路径/字形/图像软件光栅化 |
| GPU backend | 子模块 | native command、batch、texture/atlas 与受控 fallback |
| backend factory | 子模块 | 由 renderer bootstrap 选择实现 |

## 组件：RenderBackend

backend 只执行已编码绘制语义，不决定 Widget 布局、主题或窗口调度。GPU 未实现操作只有在语义等价时 fallback；否则 typed 失败。

## 模块不变量

CPU 写入 retained buffer 不等于 present 成功；最终提交仍由 platform surface/presenter 返回 Result。
