# 实现落地计划

← [Main](Main.md) · 决策 [#154](decisions.md#d154) [#157](decisions.md#d157)

> 设计规格 ahead of code 时的 **分阶段接线顺序** 与 **按文件路径的落地清单**。进度对照 → [Main · 实现进度总览](Main.md#实现进度总览)；差距明细 → [demand-driven · 实现差距](systems/demand-driven.md#实现差距)。

## 索引

| 章节 | 决策 |
|------|------|
| [分阶段路线图](#分阶段路线图) | #154 |
| [P0 落地清单](#p0-落地清单) | #157 |
| [维护](#维护) | — |

**关联**：[demand-driven](systems/demand-driven.md) · [application](systems/application.md) · [view-reactive](systems/view-reactive.md) · [decisions](decisions.md)

---

## 分阶段路线图

设计（#154）— 建议 **分阶段落地**顺序；每阶段须满足 #105 零闲置验收后再进下一阶段。与 [实现进度总览](Main.md#实现进度总览) 对照：已落地阶段打勾，未落地按此顺序推进。

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

- **不**为兼容保留 100ms 探活或每轮 `tick_effects`；P0 完成前勿叠 P2+ API。
- P1 `reconcile` 可先 **单窗** 接线，再扩 P4 多 session。
- P5 与 P1 **可交错**（`ComponentId` 宜尽早，利于 #123）。
- 每阶段对应 [demand-driven · 实现差距](systems/demand-driven.md#实现差距) 行清零或缩减。

> **实现注记**：P0 核心零闲置接线已落地：`ActiveWorkRegistry`、无 deadline 注册项、`WindowSession` 壳、单窗三态写回、DeepIdle 门控、Registry deadline wait、到期 `Timer` / `AppTimer` 消费、Tooltip 内置 timer 托管、WidgetAnimation 下一帧 deadline、`Spin` / `ProgressBar` indeterminate / Dropdown fade / Select fade / AutoComplete fade / TreeSelect fade / Cascader fade / ColorPicker fade / Tooltip fade / Popover fade / Popconfirm fade / Modal / Drawer / Collapse 内置动画源、IME composition session 托管、AppTimer 队列、MainThreadQueue、`AppHandle` 与 `.on_start` 已落地（无固定 100ms 探活，DeepIdle 不跑 `tick_effects`，Active 帧仅在 Effect pending 时 tick）；`AppHandle` 的 Timer / `post_to_ui` / `update_view` 已经按 `window_id` 路由；`WindowConfig` / `open_window` 请求层、native 副窗创建、独立 `WindowSession` bootstrap、`.on_window_start`、副窗 MainThreadQueue / `update_view` reconcile 消费、副窗事件按 `window_id` 路由、副窗运行期 frame drain 与副窗 deadline wait 已接；单窗 `pending_root` 与响应式 `State` 批次 reconcile 已接入主循环；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake/Linux Wayland 后端。当前剩余差距以 [Main · 实现进度总览](Main.md#实现进度总览) 与 [demand-driven · 实现差距](systems/demand-driven.md#实现差距) 为准。

---

## P0 落地清单

设计（#157）— **P0 完成前勿叠 P2+ API**（#154）。按文件路径的接线表；验收：无事件无 present、DeepIdle **不** `tick_effects`（#105 #106）。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/app/active_work_registry.rs`（已建） | `ActiveWorkRegistry`：`register` / `unregister` / `next_deadline` / `drain_due`；AppTimer 与 Animation 下一帧 deadline 已适配 | #115 |
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

## 维护

- 阶段划分或原则变更 → 同步 [decisions.md](decisions.md) #154（或 #160+ 新决策）与本文件。
- 新增/完成某阶段文件级任务 → 更新本文件对应表 + [Main · 实现进度总览](Main.md#实现进度总览) 行。
- P1+ 落地清单随阶段推进 **在本文件追加章节**（如 `## P1 落地清单`），勿回写 `systems/*.md` 正文。
