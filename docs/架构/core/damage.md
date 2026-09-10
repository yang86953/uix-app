# damage 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 core 基础契约中的目标 `damage` 边界及其组件契约。core 的 System 定级仍由[系统列表](../系统列表.md#core基础契约system-定级待定)持有；本文不以文件名预先完成定级。依赖：[geometry](geometry.md)。导出：ui 失效、graphics 策略和 app 提交共享的区域模型。

> **当前实现线索**：主要位于 `src/core/damage/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `DirtyRegion` | struct | 节点/逻辑阶段的脏区合并 |
| `DamageRegion` | struct | surface 更新区域 |
| `PresentDamage` | enum | 全帧、局部、滚动等提交表达 |
| `PresentCoherency` / `PresentTransform` | enum | retained 内容和复制变换契约 |
| `PresentSurface` / `PresentImage` | struct | 提交表面与图像描述 |
| `PresentDamagePlan` | struct | 本次 present 的可执行计划 |
| `PresentDamageTracker` | struct | 跨帧跟踪并在成功后消费 damage |

## 组件：PresentDamageTracker

它把多来源失效合并为当前 surface 可执行的计划；surface generation、尺寸或 coherency 变化可把局部 damage 提升为全帧。`TrackedSwapchain` 按当前 `PresentImage` 合并该 image 自上次成功提交后错过的历史区域，`draw_damage` 与 `present_damage` 必须来自同一修复集合。只有最终 native present 成功后才能提交 tracker 状态；遮挡、device lost、OOM、非法调用或其它失败均不得推进 image history。

## 模块不变量

damage 描述“更新哪里”，不决定“如何绘制”；失败帧保留未提交区域，多个滚动视口不能互相覆盖为最后一个 delta。

这些值本身不拥有 surface、renderer 或窗口生命周期。跨帧 `PresentDamageTracker` 由实际提交责任方实例化和拥有，其存活期不得超过对应 surface generation。
