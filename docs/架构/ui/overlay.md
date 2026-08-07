# overlay 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的窗口内浮层栈、层叠、焦点陷阱和进退场。依赖：[component](component.md)、[event](event.md)、[animation](animation.md)。导出：所有内置与自定义浮层共享的栈契约；公开用法见[使用 · 反馈](../../使用/反馈.md)。
>
> **当前实现线索**：栈与 entry 位于 `src/ui/overlay/mod.rs`，组件树在 `src/ui/component/widget/tree_layout/layout.rs` 重建登记，焦点陷阱位于 `src/ui/foundation/focus_trap.rs`，具体 placement 由各浮层组件持有。

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
- 组件树重建登记时通过 `WidgetRender::overlay_entry_for_surface` 显式传入当前根表面；默认实现向后兼容旧入口，依赖窗口边界的组件不得只凭触发器 frame 复用上一帧 surface 下的绝对矩形。
- owner 移除、隐藏、换根或 generation 失效时，managed entry 自动清理。

`f41fede8` 为该表面契约建立 Popover/Popconfirm 回归：触发器 frame 不变而 surface 缩小时，布局阶段的 OverlayStack bounds 与随后绘制共同消费新表面；旧缓存横坐标 150px/200px 分别收敛为 20px/60px。两项聚焦契约、布局 24 项、反馈门面 4 项与完整库 151 项测试通过。

`5fefe2ed` 把同一契约扩展到共享提示气泡原语：Tooltip 与 Slider 的提示登记显式接收当前 surface，作者方向与主轴反向候选按越界量择优，再将最终尺寸和坐标约束到窗口内；绘制箭头使用解析后的真实方向，文字裁在最终气泡中。两项原语契约、两项组件集成契约与完整库 155 项测试通过。

`d288e6c5` 进一步闭合 Select：布局从 `WidgetTree` 根 frame、绘制从 `PaintContext`、登记从 `overlay_entry_for_surface` 取得同帧 surface，共享解析器统一横向收敛、上下翻转与可用高度缩减；自定义选项布局、滚动、绘制、dirty、命中和 bounds 均复用最终矩形，过滤前后方向不同的 dirty 同时覆盖两侧。四项聚焦契约与完整库 159 项测试通过。

`c386f992` 以同一原则闭合 Cascader，但把多列作为组件私有约束处理：共享解析器先把自然总宽限制到 surface，再按可见列数等分最终列宽；纵向翻转或缩高后的实际视口同时驱动列滚动、搜索滚动和键盘显露。登记与绘制记录同帧 surface，事件、绘制裁剪、dirty、命中与 bounds 复用相对弹层缓存。四项聚焦契约与完整库 163 项测试通过。

`250ba978` 将同一约束延伸到 TreeSelect：组件缓存同帧 surface、触发器绝对锚点、相对弹层与跨帧绝对脏区；树节点展开或过滤改变可见行数时，仍以同一表面和锚点重算。事件命中、虚拟滚动、键盘显露、绘制裁剪、dirty、OverlayStack bounds 统一使用翻转或缩高后的实际视口，关闭动画和表面变化也保留旧弹层覆盖。五项聚焦契约与完整库 168 项测试通过。AutoComplete、Mentions 与日期类选择弹层仍需逐项审计，不能仅因 Select/Cascader/TreeSelect 已闭合就视为全部 overlay placement 完成。

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
