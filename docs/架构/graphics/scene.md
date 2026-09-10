# scene 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 graphics System 的 `scene` Module 及场景/Picture 合成契约。[painting](painting.md)输出由 graphics System 编排交付，Module 间不直接持有实例。导出：graphics System 交给 renderer 的 `LayerTree`。

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

LayerTree 合成普通树、滚动变换和 overlay；由所属窗口的 scene 会话独占当前可变树，发布给 renderer 的版本必须不可变。scene 变化、surface generation、主题、资源 generation 或 transform 变化会失效相应缓存；旧 generation 的 Layer/Picture 不能进入新 surface。

## 组件：Picture

Picture 只在 clip、transform、opacity、blend 与资源 generation 均兼容时复用。已含旧 overlay/主题/尺寸的像素不能重新当作干净背景。

## 所有权与不变量

- `RenderObjectTree` 的身份和缓存由所属窗口拥有；稳定 ID 只用于匹配，不延长 UI 节点、Picture 或资源生命周期。
- `ScenePaint` 只是 UI 到 graphics 的窄适配契约，不得反向暴露 `WidgetTree`、状态容器或事件处理器给 scene。
- scene diff 失败必须保留上一份已发布场景或显式使本帧失败，不能发布半更新树。
- 缓存命中只表示场景输入兼容，不表示目标 surface 可呈现；最终提交事实仍由 renderer/presentation 建立。
