# 测试系统

← [Main](../Main.md) · 系统 **#12** · 功能域：跨域

> 组件与集成测试策略；FakePlatform 驱动真实代码路径。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 总策略 | [测试策略](#测试策略) | #40 |
| 内存平台 | [FakePlatform](#fakeplatform) | #40 |
| 行为断言 | [语义断言](#语义断言) | #40 |
| 绘制断言 | [paint snapshot](#paint-snapshot) | #40 |
| 时间与 Timer | [测试时钟分层](#测试时钟分层) | #132 #127 #139 |
| 端到端 | [典型流程](#典型流程) | #40 |

**关联**：[event](event.md) · [platform](platform.md) · [rendering](rendering.md) · [application](application.md) · [demand-driven](demand-driven.md)

---

## 测试策略

裁决 [#40](../decisions.md#d40)：**FakePlatform + 语义断言 + paint snapshot**。

| 层级 | 手段 | 断言对象 |
|------|------|----------|
| 输入 | `FakePlatform` 注入 `UiEvent` | 走 `map_ui_event` → `dispatch_event` 生产路径 |
| 行为 | HandlerTable / State 副作用 | 语义事件、业务状态、focus/hover |
| 绘制 | paint snapshot | 帧缓冲或 DisplayList 哈希/像素对比 |
| 平台 | Fake 调用历史 | presenter damage、clipboard、text_input 等 |

原则：

- **零分叉**：测试用 `FakePlatform` 实现完整 `Platform` trait，不 stub 上层逻辑。
- **三层事件**：优先断言 **SemanticEvent** 与 handler 副作用；SystemEvent 仅作中间态。
- **局部绘制**：配合 `NullEngine` / `SoftwareEngine` + FakePresenter 记录 damage rects。
- 生产 `deny(clippy::unwrap_used)`；**测试 crate 除外**。
- **零闲置验收**（#105）：无 `UiEvent`、无标脏时 assert 无 `present`、无 `layout`（**L0**）；DeepIdle 下 assert 无 `tick_effects`、blocking wait（**L1**）。**Timer**：`run_interval` + `cancel` 后 assert 回 DeepIdle（#132）；到期行为见 [测试时钟分层](#测试时钟分层)。
- **豁免**（#113）：须 `decisions.md` **#158+** 公开条目 + 测试覆盖豁免边界；默认不豁免。

---

## FakePlatform

`native/test_harness/FakePlatform` — 内存 `Platform` 聚合，各 Fake 子系统记录调用历史。

| 能力 | 说明 |
|------|------|
| `FakeEventSource` | `inject` / `inject_all` FIFO 队列 |
| `FakeWindow` / `FakePresenter` | 呈现与 damage 历史（不通过 `Platform` trait 暴露） |
| `FakeTimer` | `ITimer` 平台定时器；`advance` 触发 **平台层** timer（见 [#139](../decisions.md#d139)） |
| 其余 Fake | clipboard、cursor、text_input… 同生产接口 |

详见 [platform · 测试平台](platform.md#测试平台)。

---

## 语义断言

事件路径与运行时一致（见 [event · 测试](event.md#测试)）：

```text
FakePlatform.inject(UiEvent)
    → map_ui_event (app)
    → WidgetTree::dispatch_event
    → SystemEvent → semantic_event → HandlerTable
```

| 断言类型 | 示例 |
|----------|------|
| Handler 触发 | `on_click` 闭包计数、State 变更 |
| 语义 payload | Click `{ button, pos, modifiers }`（#36） |
| 焦点 / 冒泡 | focus 转移、`stop_propagation` 阻后续 handler（#68） |
| 平台副作用 | `FakeClipboard` 历史、`text_input.start/stop` 调用序 |

Reconciler rebuild 后 handler **智能重绑**（#123、#135、#138）；未变则保留注册。测试勿假设跨 rebuild 的 handler 指针/闭包 identity 稳定；可断言 **同 handler_generation + 同 SemanticKind 集** 时 HandlerTable 保留。**ComponentId** key 复用仍有效；`WidgetId` 仅为 WidgetTree 内部同型别名。

---

## paint snapshot

绘制断言 complement 语义断言，覆盖 **render 输出** 与 **damage 区域**。

| 输入 | 手段 |
|------|------|
| 单 Widget / 小树 | `SoftwareEngine` + `FrameRenderer.render_frame` |
| 无 GPU | `NullEngine` 跳过 present，仍走 LayerTree / DisplayList |
| damage | `FakePresenter` 记录的 `PresentDamage` rects |

典型对比：逻辑帧缓冲像素 hash、或 golden file（CI 固定 scale/theme）。

失效类型与 present 规则见 [rendering · 管线与失效](rendering.md#管线与失效)（Layout alone 不 present）。

---

## 测试时钟分层

设计（#139）— **平台定时器** 与 **App ActiveWorkRegistry** 使用 **不同** 测试时钟，禁止混用。

```text
┌─────────────────────────────────────────────────────────┐
│  FakeTimer（已有，native/test_harness/fake_timer.rs）     │
│  ITimer 平台 API · pf.timer.advance(delta)              │
│  用途：平台子系统 / 未来 native 定时能力测试              │
│  ✗ 不驱动 App::run_after / ActiveWorkRegistry           │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  TestClock（已接，app/test_clock.rs）                   │
│  注入 AppTimerQueue / session loop 的 now() 源            │
│  test_clock.advance(delta) → drain_due(now)             │
│  用途：#132 App Timer、Animation Registry、wait_until    │
└─────────────────────────────────────────────────────────┘
```

### TestClock API（设计）

```rust
pub struct TestClock {
    origin: Instant,
    elapsed: Duration,
}

impl TestClock {
    pub fn now(&self) -> Instant { self.origin + self.elapsed }
    pub fn advance(&mut self, delta: Duration) { self.elapsed += delta; }
}

// 当前测试入口
fn run_window_session_loop_with_clock(
    platform: &mut FakePlatform,
    clock: &mut TestClock,
    steps: impl FnMut(&mut TestClock),
);
```

| 场景 | 时钟 | 做法 | 断言 |
|------|------|------|------|
| `run_after` 到期 | **TestClock** | `advance(500ms)` → clock-driven loop/drain | 回调副作用；DeepIdle |
| `run_interval` × N | **TestClock** | `advance(interval)` × N | N 次回调；`cancel` 后不再触发 |
| 与 UiEvent 交织 | TestClock + inject | inject + advance | Timer 不破坏事件顺序（#137） |
| `ITimer::set` 平台路径 | **FakeTimer** | `pf.timer.advance` | `set_calls` / fired ids |
| blocking wait | TestClock | DeepIdle + advance 无 register | 无 present/layout（L0） |

主循环测试：`run_window_session_loop_with_clock` 注入 `TestClock`，内部执行 `drain_due` → `main_thread_queue.drain` → reconcile…（见 [demand-driven · MainThreadQueue](demand-driven.md#mainthreadqueue)）。

**禁止**测试依赖真实 `thread::sleep` 或 wall clock；须可重复、确定性（#40）。

> **实现注记**：App 层 `AppClock` / `TestClock` 已实现，`AppTimerQueue::with_clock` 与 `run_window_session_loop_with_clock` 已用于 AppTimer deadline、RegisteredActive wait_until 与 `drain_due` 测试；`FakeTimer` 仍作为 `native/test_harness` 的平台 timer fake 独立存在。

---

## 典型流程

```text
1. FakePlatform::new()
2. TestClock::new()                    // App Timer 测试时
3. 构建 WidgetTree（ViewAdapter / 直接 mount Widget）
4. event_source.inject(UiEvent::pointer_down(...))
5. `run_window_session_loop_with_clock` 或针对 AppTimer 的 `drain_due`
6. 语义断言：handler 副作用、focus、State<T>
7. paint snapshot：FakePresenter damage / 帧缓冲对比
8. （可选）Fake 子系统调用历史断言
```

CLI / 纯逻辑单元测试可不挂载 Platform；**组件集成测试** 优先 FakePlatform 全路径。

---

## 源码模块

```text
src/tests/               镜像 src/ 布局的集成测试
  ui/                    View、Widget、theme…
  draw/                  pipeline、compositor、spatial…
  native/                （通过 FakePlatform 间接）
  data/                  SettingsService
native/test_harness/     FakePlatform 与各 Fake 子系统
  fake_timer.rs          ITimer（平台层，#139）
app/test_clock.rs        AppClock / TestClock（#139）
app/event_loop/          run_window_session_loop_with_clock（测试入口）
```
