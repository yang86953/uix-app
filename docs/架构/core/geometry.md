# geometry 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 core 基础契约中的目标 `geometry` 边界及其组件契约。core 的 System 定级仍由[系统列表](../系统列表.md#core基础契约system-定级待定)持有。依赖：无。导出：所有上层共享的 logical geometry。

> **当前实现线索**：主要位于 `src/core/geometry/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Point` | struct | 二维 logical 位置 |
| `Size` | struct | 宽高 |
| `Constraints` | struct | measure 的 min/max 约束 |
| `Rect` | struct | frame、clip、hit-test、damage 的共同矩形 |
| `EdgeInsets` | struct | margin、border、padding 的四边值 |

## 组件：Constraints

布局从父约束向下传递，组件返回收敛后的有限非负 Size。无界轴是约束语义，不能把无界哨兵写入实际 frame。

## 组件：Rect

Rect 不携带物理像素或 OS surface 语义。platform/graphics 在边界换算 DPR，ui 的布局、命中、语义和 damage 使用同一 logical Rect。

## 不变量

- geometry 只定义无资源所有权的值与运算，不持有窗口、surface、组件或缓存生命周期。
- 非有限、负尺寸和反向约束必须在建立边界时拒绝或归一化；不得把哨兵值写入实际 frame。
- logical/physical、局部/窗口和内容/边框空间的转换必须显式发生，不能复用同一数值暗示不同坐标系。
