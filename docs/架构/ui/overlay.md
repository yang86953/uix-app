# overlay 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的窗口内浮层栈、层叠、焦点陷阱和进退场。依赖：[component](component.md)、[event](event.md)、[animation](animation.md)。导出：所有内置与自定义浮层共享的栈契约；公开用法见[使用 · 反馈](../../使用/反馈.md)。
>
> **当前实现线索**：相关实现暂位于 `src/ui/overlay.rs`、`src/ui/foundation/focus_trap.rs` 和 widgets 中的具体浮层组件；重构后栈机制归本模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `OverlayStack` | struct | 每窗 entry 栈、层级、命中和 focus trap 查询 |
| `OverlayEntry` | struct | owner、kind、bounds、z-index、modal/dismiss/focus 行为 |
| `OverlayId` | value | 栈内稳定身份 |
| `OverlayKind` | enum | Modal、Drawer、Popover、Tooltip、ContextMenu、Message、Notification、Custom |
| `FocusTrap` | foundation 机制 | 最上层模态子树内的焦点循环与恢复 |
| `TransitionPlayer` | animation struct | enter/leave 视觉生命周期 |

## 组件：OverlayStack / OverlayEntry

- 每个 WidgetTree 只有一个 OverlayStack；后打开或更高 z-index 的 entry 先命中。
- overlay 命中先递归 owner 的浮层子树，再回退 owner/mask；底层树在 modal 遮罩下不可接收事件。
- bounds 使用窗口 logical 坐标并收敛到当前 surface；placement、绘制、damage 与 hit-test 共用同一几何。
- owner 移除、隐藏、换根或 generation 失效时，managed entry 自动清理。

## 焦点

- 模态浮层打开后把焦点移入第一个可聚焦节点，Tab/Shift+Tab 在 trap 内循环。
- 关闭后焦点尽量回到仍有效的触发节点。
- 多层浮层只由最上层有效 trap 约束；Escape/外部点击遵守 entry 的 dismiss 配置。
- 离场开始即停止新交互，但可保留视觉节点到动画完成。

## enter / leave

entry membership 改变时同步 OverlayStack。进出场只在 TransitionPlayer active 时登记动画帧；holding 阶段用 timer deadline 或纯静态状态，不维持固定 16ms tick。

Message/Notification 的每项可以有独立稳定 ID 和 `Entering → Holding → Leaving` 状态，但最终都归所属窗口调度，队列为空时删除 entry 和相关 timer。

## 绘制与缓存

全屏 overlay 可能把 damage 提升为 FullComposite。backend 能安全复用干净背景时，可缓存 overlay 打开前的 retained 内容并只重绘浮层；普通树、尺寸、主题、debug 状态或 overlay membership 变化后必须失效，不能从已含遮罩的帧重新捕获“背景”。

## 不变量

- OverlayStack 不单独创建线程或跨窗 wake。
- `Custom` entry 没有隐式 modal/dismiss/focus 行为，调用方必须显式配置。
- 浮层内容是普通 View 子树，仍使用同一 layout/event/paint/semantics 管线。
