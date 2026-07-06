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
| **P2** | App 公开定时/投递 | #132–#137 #140 #139 | Timer cancel 回 DeepIdle；FakeClock 测 drain |
| **P3** | L2 最小脏区 | #122 #129 #107 #109 | 滚动 Composite；Picture 自适应 |
| **P4** | 多窗编排 | #116 #148 #149 #150 | 副窗独立 tree；State fan-out 仅 wake 有关窗 |
| **P5** | Handle 零维护 | #145–#152 #147 | mount 自动 snapshot；emit 走 dispatch_semantic |

**原则**：

- **不**为兼容保留 100ms 探活或每轮 `tick_effects`；P0 完成前勿叠 P2+ API。
- P1 `reconcile` 可先 **单窗** 接线，再扩 P4 多 session。
- P5 与 P1 **可交错**（`ComponentId` 宜尽早，利于 #123）。
- 每阶段对应 [demand-driven · 实现差距](systems/demand-driven.md#实现差距) 行清零或缩减。

> **实现注记**：当前整体处于 **P0 未完备**（仍 100ms timeout + 每轮 tick_effects）；`reconcile` 函数已存在但未接主循环（P1 局部就绪）。接线顺序见 [P0 落地清单](#p0-落地清单)（#157）。

---

## P0 落地清单

设计（#157）— **P0 完成前勿叠 P2+ API**（#154）。按文件路径的接线表；验收：无事件无 present、DeepIdle **不** `tick_effects`（#105 #106）。

| 文件 / 模块 | 动作 | 决策 |
|-------------|------|------|
| `src/app/active_work_registry.rs`（**新建**） | `ActiveWorkRegistry`：`register` / `unregister` / `next_deadline` / `drain_due`；Animation / AppTimer 适配 | #115 |
| `src/app/window_session.rs`（**新建**） | `WindowSession`：三态、`registry`、`view_factory`（占位，P1 接 reconcile 字段） | #106 #116 #155 |
| `src/app/event_loop/event_loop.rs` | 拆 `run_app_loop` + `run_active_frame`；**移除** `idle_count` + `wait_timeout(100ms)` 探活 | #106 #127 |
| 同上 | DeepIdle：`wait_event` / `wait_until(registry.next_deadline)`；**跳过** layout/render/`tick_effects` | #105 #117 |
| 同上 | RegisteredActive：`drain_due` + 窄 tick；无 UiEvent 时 **不**全帧 dispatch | #115 #117 |
| 同上 | Active：UiEvent → dispatch →（P1：`drain_queue` / reconcile）→ layout → render | #106 #137 |
| 同上 | `tick_effects` **仅** Active 且（有 Effect pending **或** Registry 有 animation 条目） | #105 |
| `src/app/shell/application.rs` | `run_gui` 构造 `WindowSession` + Registry；传入 loop 而非裸 `WidgetTree` | #116 |
| `src/ui/core/widget/tree_dirty.rs` | 保持 `tick_effects` 实现；由 loop **门控**调用时机（不在此加轮询） | #105 |
| `src/native/traits/event/mod.rs` | 确认 `wait_timeout` 契约满足 `wait_until`（#127）；**不**要求新 native API | #127 |
| `src/tests/app/event_loop/`（**扩**） | DeepIdle：无事件 N 秒 → assert 无 `record_present`、无 `tick_effects` 调用 | #105 #139 |

```text
接线顺序（建议）:
  1. Registry 类型 + 单元测试
  2. WindowSession 壳 + application 传入
  3. event_loop 三态 + wait_until 替换 100ms
  4. 门控 tick_effects
  5. 零闲置验收测试
```

P1 起在同一 `WindowSession` 上追加 `reconcile_pending` / `pending_root`（#153）与 `view_factory` 接线 — 见 [view-reactive · view_factory](systems/view-reactive.md#view_factory-生命周期)。

---

## 维护

- 阶段划分或原则变更 → 同步 [decisions.md](decisions.md) #154（或 #158+ 新决策）与本文件。
- 新增/完成某阶段文件级任务 → 更新本文件对应表 + [Main · 实现进度总览](Main.md#实现进度总览) 行。
- P1+ 落地清单随阶段推进 **在本文件追加章节**（如 `## P1 落地清单`），勿回写 `systems/*.md` 正文。
