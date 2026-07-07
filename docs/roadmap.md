# 实现落地计划

← [Main](Main.md) · 决策 [#154](decisions.md#d154) [#157](decisions.md#d157)

> 设计规格 ahead of code 时的 **分阶段接线顺序** 与 **按文件路径的落地清单**。进度对照 → [Main · 实现进度总览](Main.md#实现进度总览)；差距明细 → [demand-driven · 实现差距](systems/demand-driven.md#实现差距)。

## 索引

| 章节 | 决策 |
|------|------|
| [分阶段路线图](#分阶段路线图) | #154 |
| [P0 落地清单](#p0-落地清单) | #157 |
| [P1 落地清单](#p1-落地清单) | #118 #123 #143 #153 |
| [P2 落地清单](#p2-落地清单) | #132–#140 |
| [P3 落地清单](#p3-落地清单) | #107 #109 #122 #129 |
| [P4 落地清单](#p4-落地清单) | #116 #148 #149 #150 |
| [P5 落地清单](#p5-落地清单) | #145–#152 #147 |
| [维护](#维护) | — |

**关联**：[demand-driven](systems/demand-driven.md) · [application](systems/application.md) · [view-reactive](systems/view-reactive.md) · [decisions](decisions.md)

---

## 分阶段路线图

设计（#154）— 本文保留 **分阶段落地**的历史接线清单；当前实现状态以 [实现进度总览](Main.md#实现进度总览) 为准，剩余差距以 [demand-driven · 实现差距](systems/demand-driven.md#实现差距) 为准。

```text
P0 基础零闲置          P1 热更新           P2 App 运行时 API      P3 渲染窄路径
├─ 三态主循环 #106     ├─ reconcile 接入   ├─ Timer #132          ├─ PicturePolicy #122
├─ Registry #115       ├─ 帧内合并 #118    ├─ post_to_ui #133     ├─ Composite #107
├─ wait_until #127     ├─ reconcile合并#153├─ MainThreadQueue #137  └─ PointerMove #109
└─ Effect DeepIdle     ├─ handler重绑#123  ├─ on_start #140
                       └─ StateSlotId #143 └─ TestClock #139

P4 多窗                P5 组件 Handle 体系
├─ WindowSession #116  ├─ AppState #145
├─ open_window #148    ├─ Snapshot #146 #151 #152
├─ update_view #149    ├─ Handle emit #147
└─ State fan-out #150  └─ ComponentId #101（可与 P1 交错）
```

| 阶段 | 目标 | 关键决策 | 验收 |
|------|------|----------|------|
| **P0** | DeepIdle 真休眠 | #106 #115 #127 #105 | 无事件无 present；无 tick Effect |
| **P1** | 热更新不上全量 build | #118 #153 #123 #143 | State 批次单次 reconcile；handler 智能重绑 |
| **P2** | App 公开定时/投递 | #132–#137 #140 #139 | Timer cancel 回 DeepIdle；TestClock 测 drain / wait_until |
| **P3** | L2 最小脏区 | #122 #129 #107 #109 | 滚动 Composite；Picture 自适应 |
| **P4** | 多窗编排 | #116 #148 #149 #150 | 副窗独立 tree；State fan-out 仅 wake 有关窗 |
| **P5** | Handle 零维护 | #145–#152 #147 | mount 自动 snapshot；emit 走 dispatch_semantic |

**原则**：

- **不**为兼容保留 100ms 探活或每轮 `tick_effects`；新增 API / 主循环路径继续受 #105 零闲置约束。
- P1 `reconcile` 可先 **单窗** 接线，再扩 P4 多 session。
- P5 与 P1 **可交错**（`ComponentId` 宜尽早，利于 #123）。
- 每阶段对应 [demand-driven · 实现差距](systems/demand-driven.md#实现差距) 行清零或缩减。

> **实现注记**：P0 核心零闲置接线已落地：`ActiveWorkRegistry`、无 deadline 注册项、`WindowSession` 壳、单窗三态写回、DeepIdle 门控、Registry deadline wait、到期 `Timer` / `AppTimer` 消费、Tooltip widget scoped timer route 托管、WidgetAnimation `Animation(id)` 下一帧 deadline、`Spin` / `ProgressBar` indeterminate / Dropdown fade / Select fade / AutoComplete fade / TreeSelect fade / Cascader fade / ColorPicker fade / Tooltip fade / Popover fade / Popconfirm fade / Modal / Drawer / Collapse 内置动画源、IME composition session 托管、AppTimer 队列、MainThreadQueue、`AppHandle` 与 `.on_start` 已落地（无固定 100ms 探活，DeepIdle 不跑 `tick_effects`，Active 帧仅在 Effect pending 时 tick，隐藏窗口不 layout/render 且保留 pending dirty）；`AppHandle` 的 Timer / `post_to_ui` / `update_view` 已经按 `window_id` 路由；`WindowConfig` / `open_window` 请求层、native 副窗创建、独立 `WindowSession` bootstrap、`.on_window_start`、副窗 MainThreadQueue / `update_view` reconcile 消费、副窗事件按 `window_id` 路由、副窗运行期 frame drain 与副窗 deadline wait 已接；单窗 `pending_root` 与响应式 `State` 批次 reconcile 已接入主循环；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake/Linux Wayland 后端；平台窗口可选能力已改为 `WindowOps -> Result<()>`，未支持能力不再误写共享状态。当前剩余差距以 [Main · 实现进度总览](Main.md#实现进度总览) 与 [demand-driven · 实现差距](systems/demand-driven.md#实现差距) 为准。

---

## P0 落地清单

设计（#157）— 历史 P0 接线表；验收仍是无事件无 present、DeepIdle **不** `tick_effects`（#105 #106）。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/app/active_work_registry.rs`（已建） | `ActiveWorkRegistry`：`register` / `unregister` / `next_deadline` / `drain_due`；AppTimer、widget scoped Timer 与 `Animation(id)` 下一帧 deadline 已适配 | #115 |
| `src/app/window_session.rs`（已建） | `WindowSession`：三态、`registry`、`view_factory`、`pending_root`、`reconcile_pending`；单窗 loop 已写回三态 | #106 #116 #153 #155 #156 |
| `src/app/event_loop/event_loop.rs` | 拆 `run_app_loop` + `run_active_frame`；已先在单窗 loop **移除** `idle_count` + `wait_timeout(100ms)` 探活 | #106 #127 |
| 同上 | DeepIdle：无 Registry deadline 时走 `wait_event` 并 **跳过** layout/render/`tick_effects`；有 deadline 时走单次 `wait_timeout(remaining)` | #105 #117 #127 |
| 同上 | RegisteredActive：已接 `next_deadline` / `drain_due` 骨架、无 deadline 注册项、到期 `Timer` → `SystemEvent::Timer` 消费、AppTimer 主线程回调执行、Tooltip pending timer 自动托管、WidgetAnimation 下一帧 deadline、`Spin` / `ProgressBar` indeterminate / Dropdown fade / Select fade / AutoComplete fade / TreeSelect fade / Cascader fade / ColorPicker fade / Tooltip fade / Popover fade / Popconfirm fade / Modal / Drawer / Collapse 内置动画源与 IME composition session 托管，并写回 session 状态 | #115 #117 |
| 同上 | Active：UiEvent → dispatch → due work → `MainThreadQueue::drain` → 单窗 pending_root / State 批次 reconcile 已接 | #106 #118 #137 #153 |
| 同上 | `tick_effects` 已门控到 Active 帧且收窄到 Effect pending；WidgetAnimation 下一帧 deadline 已接 | #105 |
| `src/app/shell/application.rs` | `run_gui` 已构造主窗 `WindowSession` + Registry + root factory，并在同一 loop 内编排副窗 session；主窗 `AppHandle` 与 `.on_start` 已在首帧前注入 | #116 #134 #140 #155 |
| `src/ui/core/widget/tree_dirty.rs` | 保持 `tick_effects` 实现并提供 Effect pending 查询；由 loop **门控**调用时机（不在此加轮询） | #105 |
| `src/native/traits/event/mod.rs` | 确认 `wait_timeout` 契约满足 `wait_until`（#127）；**不**要求新 native API | #127 |
| `src/tests/app/event_loop/`（已扩） | DeepIdle：无事件 N 轮 → assert 无额外 `record_present`、无固定 timeout 探活；TestClock 覆盖 App drain_due / wait_until | #105 #139 |

```text
接线顺序（建议）:
  1. Registry 类型 + 单元测试
  2. WindowSession 壳 + application 传入
  3. event_loop 三态 + wait_until 替换 100ms（单窗与副窗 deadline wait、外部 wake 已接）
  4. 门控 tick_effects
  5. 零闲置验收测试
```

P1 的单窗 `reconcile_pending` / `pending_root`（#153）、响应式 `State` 批次置位与 `view_factory`（#155–#156）已接入主循环；副窗 `MainThreadQueue` / `update_view` root reconcile 消费已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake/Linux Wayland 后端 — 见 [view-reactive · view_factory](systems/view-reactive.md#view_factory-生命周期)。

---

## P1 落地清单

目标：热更新不上全量 build；State 批次在帧内合并为一次 reconcile，handler 只在签名变化时重绑。

状态：主体接线已落地；同类型组件 patch 继续按静态配置同步、运行态保留的规则扩展。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/ui/view/adapter.rs` | `reconcile` 复用同类型 / key 匹配节点，patch 样式、handler 与静态配置 snapshot；类型不同才卸载子树 | #118 #153 |
| `src/ui/event.rs` | `HandlerSignature` / `handler_generation` / capture fingerprint 比较；State / WindowId capture 变化才重注册 handler；无 generation / fingerprint 的普通 handler 保守重绑 | #123 #135 #138 #159 #160 |
| `src/ui/foundation/state.rs` | `StateSlotId` 单调稳定；State set/update fan-out 到 reconcile site 与 paint site | #143 #150 |
| `src/app/window_session.rs` | `pending_root` 与 `reconcile_pending` 挂在 session；帧内消费一次，`pending_root` 优先于 factory rebuild | #153 #155 #156 |
| `src/app/event_loop/event_loop.rs` | Active 帧中 drain due / post_to_ui / State 批次后、layout 前执行 reconcile；无 pending 时不唤醒 | #105 #118 |
| `src/tests/ui/view/adapter.rs`、`src/tests/app/event_loop/` | 覆盖 keyed reuse、handler generation、State 批次 reconcile 与 DeepIdle 不额外工作 | #118 #123 #153 |

## P2 落地清单

目标：App 公开 Timer、主线程投递、启动回调与 TestClock，且 cancel / drain 后回到 DeepIdle。

状态：Timer、post_to_ui、MainThreadQueue、AppHandle、on_start / on_window_start 与 TestClock 主体接线已落地。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/app/app_timer.rs` | `run_after` / `run_interval` / `TimerHandle`；cancel 移除 pending timer，interval 以触发时间重排 | #132 |
| `src/app/main_thread_queue.rs` | FIFO drain；drain 中追加的任务同轮继续执行；按 window session 隔离 | #137 |
| `src/app/session_runtime.rs` | `AppRuntime` 按 `WindowId` 路由 timer、post_to_ui、update_view，并在成功注册后 wake event loop | #132 #133 #141 |
| `src/app/app_handle.rs` | `AppHandle` cloneable；关闭后投递 / timer / update_view 惰性丢弃或返回 inert handle | #132 #133 #134 |
| `src/app/shell/application.rs` | `.on_start` / `.on_window_start` 在 session 首帧前注入 `AppHandle`；`AppClock` / `TestClock` 测试入口 | #139 #140 |
| `src/tests/app/` | 覆盖 timer deadline、cancel、post_to_ui 顺序、TestClock drain_due 与关闭后无 wake | #132 #137 #139 |

## P3 落地清单

目标：渲染只为真实变化工作；滚动走 Composite memmove，Picture 缓存由元数据与运行时信号共同决定。

状态：Composite strip、PicturePolicy、PointerMove 窄路径与空 dirty idle 主体接线已落地。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/draw/pipeline/invalidation.rs` | `Invalidation::Paint/Layout/Composite` 合并与 dirty region 查询；Layout 不隐式 Paint | #107 |
| `src/draw/pipeline/render_frame.rs` | 空 dirty + 无 scroll_move 直接返回 Idle；首帧才强制 full redraw | #105 #107 |
| `src/draw/compositor/layer_tree.rs` | `PicturePolicy` 阈值、运行时信号降级、node_id+bounds cache 复用、z-order 构建 | #122 #129 |
| `src/ui/core/widget/tree_events.rs` | PointerMove 边界窄路径：drag / pressed 全 dispatch，hover hit frame 内跳过 hit_test，opt-in 才连续 dispatch | #109 #121 |
| `src/ui/widgets/other/scroll_view/` | Wheel、键盘、拖拽与程序化滚动写 Composite exposed strip；layout 使用自然坐标 | #107 |
| `src/tests/draw/`、`src/tests/ui/widgets/other/scroll_view/` | 覆盖 Picture 阈值、空 dirty idle、Composite strip 与滚动来源一致性 | #107 #122 #129 |

## P4 落地清单

目标：多窗共享 AppState / Theme，但每窗独立 session、队列、三态与 deadline；State fan-out 只 wake 相关窗。

状态：WindowSession、多窗 open_window / update_view、session 路由与 State fan-out 主体接线已落地。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/app/window_session.rs` | 每窗持有独立 WidgetTree、Registry、MainThreadQueue、pending_root、AppTimer 与 loop_state | #116 |
| `src/app/shell/application.rs` | `WindowConfig` / `open_window` 请求 drain；副窗 native 创建与独立 session bootstrap | #148 |
| `src/app/session_runtime.rs` | session 路由表；副窗关闭清理 timer、queue、pending open request 与 handle alive 状态 | #134 #141 #148 |
| `src/app/event_loop/event_loop.rs` | 主窗事件与副窗事件按 `window_id` 路由；副窗 frame drain 跳过 DeepIdle 窗 | #110 #116 |
| `src/ui/foundation/state.rs` | 多 paint / reconcile site fan-out；重复 site 原地更新，set 只 wake 绑定 session | #150 |
| `src/tests/app/window_session.rs`、`src/tests/app/shell/application.rs` | 覆盖副窗事件路由、独立 deadline、shared State fan-out 与关闭清理 | #116 #148 #150 |

## P5 落地清单

目标：ComponentHandle 与 AppState 零维护；mount / unmount 自动注册 snapshot，emit 统一走语义派发。

状态：AppState snapshot registry、ComponentHandle getter / invalidate / emit、SnapshotSource 与 mount/unmount 注册主体接线已落地。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/ui/app_state.rs` | snapshot registry、semantic queue、handle lookup、owner-thread 校验与失效队列引用 | #145 #147 |
| `src/ui/component_handle.rs` | live handle 与 lookup handle 的 `id`、`snapshot`、getter、`invalidate`、`emit` | #145 #147 |
| `src/ui/component_snapshot.rs` | `ComponentConfigSnapshot`、`SnapshotFields`、内置组件静态配置提取与 custom snapshot | #146 #151 #152 |
| `src/ui/core/widget/tree_core.rs` | mount/unmount/register/unregister snapshot；root replace 清理 stale snapshot 与 manager override | #145 #146 |
| `src/ui/core/widget/tree_dirty.rs` | State paint binding 与 AppState lookup handle invalidate 均走窄 Paint | #105 #145 #147 |
| `src/tests/ui/event.rs`、`src/tests/ui/component_snapshot.rs`、`src/tests/ui/core/widget/tree_core.rs` | 覆盖 generation key、snapshot getter、lookup emit drain、unmount 不可读与窄 Paint | #145–#152 |

---

## 维护

- 阶段划分或原则变更 → 同步 [decisions.md](decisions.md) #154（或 #162+ 新决策）与本文件。
- 新增/完成某阶段文件级任务 → 更新本文件对应表 + [Main · 实现进度总览](Main.md#实现进度总览) 行。
- 新阶段落地清单随阶段推进 **在本文件追加章节**；系统行为变更仍须同步对应 `systems/*.md` 正文。
