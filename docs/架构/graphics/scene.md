# scene 模块

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统的目标 `scene` 模块及场景/Picture 合成契约。依赖：[painting](painting.md)。导出：renderer 消费的 LayerTree。

> **当前实现线索**：主要位于 `src/draw/scene/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `LayerTree` / `LayerNode` | struct/enum | clip、transform、opacity、Picture 的场景层级 |
| `Picture` | struct | 可缓存的子场景绘制结果 |
| `RenderObjectTree` / `RenderObjectEntry` | struct | 场景对象与缓存身份 |
| `ScenePaint` | trait | UI 树向场景录制的桥 |
| `PicturePolicy` | enum | Picture 缓存/重放策略 |

## 组件：LayerTree

LayerTree 合成普通树、滚动变换和 overlay；scene 变化、surface generation、主题或 transform 变化会失效相应缓存。

## 组件：Picture

Picture 只在 clip、transform、opacity、blend 与资源 generation 均兼容时复用。已含旧 overlay/主题/尺寸的像素不能重新当作干净背景。
