# animation 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的值插值、过渡、动画源与生命周期。依赖：[component](component.md)、[reactive](reactive.md)。导出：动画播放器、下一 deadline 和活动源契约，供 app 调度；公开用法见[使用 · 动画](../../使用/动画.md)。
>
> **当前实现线索**：相关实现暂位于 `src/ui/animation/`（traits.rs 同目录）；帧调度位于 app，但依赖方向必须是 app 消费 ui 导出的活动源，ui 不反向依赖 app。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Animatable` | trait | 值插值与差值契约 |
| `Animation<T>` | struct | from/to、duration、easing、播放控制 |
| `Animated<T>` | struct | State + 播放器 + 逐窗 source 身份 |
| `Easing` | enum | 线性、二/三/四次、Back、Bounce、Elastic、Bezier |
| `Spring` / `SpringAnimation<T>` | struct | 解析式弹簧采样与静止判定 |
| `Keyframe` / `KeyframeAnimation<T>` | struct | 有序关键帧和分段 easing |
| `KeyframePlayback` | struct | delay、有限/无限轮次、方向与填充模式 |
| `AnimationConfig` / `TransitionPlayer` | struct | fade/slide/zoom 的 enter/leave 播放 |
| `AnimationGroup` / `AnimationGroupItem` | struct | parallel/sequence/stagger 的异构 source 编排 |
| `WidgetAnimation` | trait | 组件自身逐帧能力 |

## 组件：Animation / Animated

- `Animation<T>`、`SpringAnimation<T>`、`KeyframeAnimation<T>` 是纯播放器；调用方传入 `dt`。
- `Animated<T>` 把播放器结果发布到 State 并以稳定 source ID 登记到所属窗口；View 的 animated style 读取该值并沿普通 reconcile/paint 分类失效。

## UIX 声明降低

- `uix-derive` 拥有 `@keyframes`、固定位置 `animation` 简写和首批字段矩阵的编译期验证；它只生成 typed `Animated<T>`、`Keyframe<T>` 与 `KeyframePlayback`，不拥有时钟。
- 每个动画节点从最近文档根或 `Component` 作用域派生私有子作用域，各字段使用稳定编号保存 `State<Animated<T>>`；`For` 实例路径进入身份键，同身份 reconcile 复用 source。
- View 的 `*_animated` 方法是取值适配器，不创建播放器或第二套调度器；`.value()` 把 source 交给既有窗口捕获与活动源登记。
- 基础静态样式提供 fill-mode 恢复值。伪类与 `setStyle` 不得创建竞争播放生命周期，编译器给出定向 `transition` 诊断。

## 帧驱动

```text
事件/reconcile 发现 active source
  → ActiveWorkRegistry 登记
  → FrameScheduler 申请一次 frame opportunity
  → 使用单调 frame time 计算 dt
  → 只推进已登记 source / widget
  → 值变化标 Layout 或 Paint
  → 完成后 unregister
```

- 稳定动画帧不遍历整棵树查找动画。
- 同一窗口可以有多个 source，但只维护一个最近帧机会；不同窗口互不唤醒。
- 完成、stop、节点移除或 source drop 必须取消登记。
- 协调事务中的 source owner 替换只在最外层成功时发布；发布失败进入 fail-stop，后续不再推进动画，关闭时解除全部 owner 并取消帧调度。

## 暂停与时间

- Hidden/Minimized/ZeroExtent/Occluded 等不可呈现状态冻结视觉动画并取消视觉 deadline。
- 恢复首帧 `dt = 0` 重基，不补跑隐藏期间的视觉帧。
- 业务计时使用 AppTimer/Timer，不能借动画补算真实时间。
- duration 非正时在首次推进收敛到终态；非有限参数在构造/边界处拒绝或归一，不能产生永不结束的 busy loop。

## View 过渡

enter 只在首次 mount 启动；同类型同 key reconcile 不重启。leave 使节点进入有界 pending-removal：立即退出命中、语义、timer 和浮层管理，保留视觉/布局槽直到过渡完成；同 key 在完成前重新出现可取消离场并复用身份。

visual opacity/offset/scale 不改变 layout frame，但绘制、命中、语义 bounds 和 damage 共用同一 transform 链；width/height 动画则属于 Layout 失效。

## 动画组与错误

AnimationGroup 只编排已有 source，不创建第二份 State 或 frame registration。空组、重复 source、无限循环或无法证明有限总时长的组合返回 `AnimationGroupError`，不静默降级。
