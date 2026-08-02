# presentation 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `presentation` 模块，权威持有原生 surface、图形 recipe、presenter 与提交能力。依赖：[windowing](windowing.md)、core。导出：供 graphics bootstrap/renderer 使用的平台呈现边界。

> **当前实现线索**：分布在 `src/native/factory/`、`src/native/graphics/`、`src/native/presenter.rs` 与各 OS backend。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `GraphicsRecipe` | value | 平台、feature、API 与 fallback 候选 |
| `GraphicsContext` | 资源接口 | device/surface、resize、begin/end frame |
| `GraphicsCapabilities` | value | raster、retained、occlusion 与 present 能力 |
| `Presenter` | 提交接口 | CPU/GPU 结果到原生窗口的最终 present |
| `SurfaceToken` | generation value | surface 重建与迟到 callback 隔离 |

## 组件：GraphicsRecipe

registry 只陈述可构造候选；graphics 决定选择和恢复策略。显式 API 请求不偷换其他 API，自动模式可按固定候选顺序降级。

## 组件：Presenter

CPU 写入 retained pixels 不等于提交成功；最终 OS present 返回 typed Result，失败帧不能被标记为成功。

## 模块不变量

platform 只拥有原生 surface/提交能力，不拥有 UI 绘制命令、字体缓存或 WidgetTree。
