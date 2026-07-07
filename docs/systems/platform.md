# 平台系统

← [Main](../Main.md) · 系统 **#8** · 功能域：`native`

> OS 差异隔离；traits 对外；Fail Fast。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 聚合入口 | [Platform 聚合](#platform-聚合) | AGENTS |
| 窗口与呈现 | [窗口与呈现](#窗口与呈现) | #59 #70 |
| 输入 | [输入](#输入) | #71 #75 |
| 事件 | [事件模型](#事件模型) | — |
| 系统服务 | [系统服务](#系统服务) | — |
| Traits 清单 | [Traits 清单](#traits-清单) | — |
| 工厂 | [工厂与后端](#工厂与后端) | AGENTS |
| 测试 | [测试平台](#测试平台) | #40 |
| 条件编译 | [条件编译](#条件编译) | AGENTS |

**关联**：[application](application.md) · [event](event.md) · [rendering](rendering.md) · [foundation](foundation.md) · [demand-driven](demand-driven.md)

---

## Platform 聚合

单一入口 trait（`native/traits/platform.rs`）：

```rust
trait Platform {
    fn window_manager(&mut self) -> &mut dyn IWindowManager;
    fn event_loop(&mut self) -> &mut dyn IEventLoop;
    fn event_bus(&mut self) -> &mut EventBus;
    fn clipboard(&mut self) -> &mut dyn IClipboard;
    fn cursor(&mut self) -> &mut dyn ICursor;
    fn display(&self) -> &dyn IDisplay;
    fn file_dialog(&mut self) -> &mut dyn IFileDialog;
    fn keyboard(&self) -> &dyn IKeyboard;
    fn text_input(&mut self) -> &mut dyn ITextInput;
    fn timer(&mut self) -> &mut dyn ITimer;
    fn notification(&mut self) -> &mut dyn INotification;
    fn console(&mut self) -> &mut dyn IConsole;
    fn file_system(&self) -> &dyn IFileSystem;
    fn system_info(&self) -> &dyn ISystemInfo;
}
```

**硬约束**：`core` / `draw` / `ui` / `app` 只依赖 `native::traits` 与公开 services；**禁止** `use native::backends::*`。`data` 仅依赖 `core`（见 [data](data.md)）。

能力差异用 trait + `Result` 表达，不用上层 `#[cfg]`。

---

## 窗口与呈现

### IWindowManager / PlatformWindow

| API | 作用 |
|-----|------|
| `create_window()` | 创建原生窗口 |
| `show/hide/close/center/raise` | 生命周期 |
| `presenter()` | CPU/GPU 呈现 |
| `graphics_context()` | GPU 初始化（可选） |

`Window`（app 层）是轻量包装；多窗 native 能力具备，app 编排见 [application · 多窗](application.md#appstate--多窗--settings)。

### 窗口可选能力

`WindowOps` 将平台可选窗口能力表达为 `Result<()>`：支持的后端返回 `Ok(())`，协议或 OS 不支持的能力返回 `Err(Errc::NotImplemented)`。`PlatformWindowCore` 保持公开 `PlatformWindow` / `IWindowProperties` 的无返回值兼容 API，但必须记录错误，并且只有底层能力返回 `Ok(())` 后才更新共享 `WindowState`，避免 Wayland 等平台把未支持操作误记为成功状态。

当前能力边界：

| 能力 | Windows | Linux Wayland |
|------|---------|---------------|
| position / resizable / borderless / always_on_top / opacity / file_drop | 原生或窗口管理 API 支持则 `Ok(())` | 协议不支持或由 compositor 控制，返回 `Err(NotImplemented)` |
| maximize / minimize / restore / fullscreen | `Ok(())` | xdg_toplevel 支持，返回 `Ok(())` |
| text_input | 由平台 IME/text-input 通道管理，返回 `Ok(())` | 由 Wayland text-input manager 管理，返回 `Ok(())` |

### 呈现（#59、#70）

| 路径 | 接口 | damage |
|------|------|--------|
| CPU | `IPresenter::present(pixels, w, h, PresentDamage)` | Full / Partial rects |
| GPU | `IGraphicsContext::swap_buffers(PresentDamage)` | 同上 |

`PresentDamage` 来自 `core::damage` — 物理像素矩形列表或全屏。

### IDisplay

`dpi_scale()`、`is_dark_mode()`、`count()`、`info(index)` — HiDPI 与 OS 主题查询（#74）。

---

## 输入

共享枚举（`traits/input.rs`，跨层复用）：

`MouseButton`、`KeyCode`、`KeyMod`、`CursorType`、`ControlSize`、`ScrollDirection`。

| Trait | 关键 API |
|-------|----------|
| `IClipboard` | `text()`, `set_text()` |
| `ICursor` | `set_cursor`, `cursor_position`, `capture_mouse` |
| `ITextInput` | `start()`, `stop()` — IME 会话 |
| `IKeyboard` | `is_down(key)`, `double_click_ms()` |

---

## 事件模型

### UiEvent

`UiEvent { type_: UiEventType, payload: UiEventPayload }`

**UiEventType**（24 种）：窗口生命周期、指针、滚轮、键盘、剪贴板、IME、Timer、FileDrop、ThemeChanged、LocaleChanged…

工厂方法：`UiEvent::pointer_down`、`wheel`、`key_down`、`text_input`、`theme_changed`、`file_drop` 等。

### IEventLoop

```rust
fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
fn wait_event(...);
fn wait_timeout(timeout, ...);
fn waker(&self) -> Option<Arc<dyn IEventLoopWaker>>;
```

Callback 返回 `false` → 请求退出循环。Backend 实现 `OsEventSource`；blanket impl 提供 `IEventLoop`。

**DeepIdle**（#106）：App 主循环须 blocking `wait_event`。**RegisteredActive** 须 `wait_until(next_deadline)`（#117），**不**用固定 `wait_timeout` 探活。`wait_timeout` 仅作 App opt-in 或测试辅助。详见 [demand-driven · ActiveWorkRegistry](demand-driven.md#activeworkregistry) · [application · 事件轮询策略](application.md#事件轮询策略)。

### EventBus

发布/订阅 `UiEvent`；handler 返回 `false` 也可请求退出。

| API | 说明 |
|-----|------|
| `subscribe(type, handler)` / `subscribe_all(handler)` | 订阅特定类型 / 通配符订阅所有 |
| `subscribe_with_priority(type, prio, handler)` / `subscribe_all_with_priority(prio, handler)` | 带优先级订阅 |
| `subscribe_once(type, handler)` / `subscribe_all_once(handler)` / `subscribe_once_with_priority(...)` | 一次性（触发后自动移除） |
| `unsubscribe(id)` | 取消订阅 |
| `publish(event)` | 按优先级降序执行；自动清理已触发一次性订阅 |
| `clear()` / `subscriber_count()` | 清空 / 查询 |

优先级常量：`PRIORITY_HIGHEST` (i32::MAX) / `PRIORITY_DEFAULT` (0) / `PRIORITY_LOWEST` (i32::MIN)。

app 主循环在 dispatch 后 `event_bus.publish(&ev)` 供外围监听。

### app 边界

```text
UiEvent → map_ui_event (application.rs) → SystemEvent
```

逻辑像素（#70）；platform 不感知 WidgetTree。

---

## 系统服务

非 UI 热路径；失败返回 `Result`，不 panic。

| 服务 | Trait | 用途 |
|------|-------|------|
| 文件对话框 | `IFileDialog` | open / save / folder |
| 文件系统 | `IFileSystem` | SpecialDir、read_file |
| 通知 | `INotification` | 系统 toast |
| 定时器 | `ITimer` | set interval / clear |
| 系统信息 | `ISystemInfo` | OS、内存、字体探测 |
| 控制台 | `IConsole` | CLI 输出着色 |

公开路径：`native::services::file_service`、`notification`。

---

## Traits 清单

```text
native/traits/
├── platform.rs      Platform
├── window.rs        IWindowManager, PlatformWindow, IWindowProperties
├── present.rs       IPresenter, IGraphicsContext
├── display.rs       IDisplay
├── input.rs         IClipboard, ICursor, ITextInput, IKeyboard
├── system.rs        IFileDialog, IFileSystem, INotification, ITimer, ISystemInfo, IConsole
└── event/
    ├── types.rs     UiEvent, UiEventType, UiEventPayload
    ├── bus.rs       EventBus
    └── mod.rs       IEventLoop
```

---

## 工厂与后端

| 入口 | 作用 |
|------|------|
| `create_platform()` | 当前 OS 的 Platform 实例 |
| `create_gpu_context(window)` | GPU 上下文 |
| `available_memory_bytes()` | 引擎选择参考 |

Backend 实现位于 `native/backends/windows/`、`native/backends/linux/`（Wayland）。

| 平台 | 状态 |
|------|------|
| Windows | ✅ |
| Linux (Wayland) | ✅ |
| macOS | 未实现 |

---

## 测试平台

**详见 [testing.md](testing.md)** — 跨域测试策略与端到端流程。

`native/test_harness/FakePlatform` — **真实内存实现**，非 stub。

### 设计

- 每个 Fake（clipboard、window、presenter…）记录 **调用历史**
- 实现完整 `Platform` trait → 生产代码路径零分叉
- 公开字段供测试直接断言

### FakeEventSource

```rust
inject(event)          // FIFO 队列
inject_all(events)
pending_count / processed_count
should_exit            // 控制 dispatch 返回值
```

### 典型测试流（#40）

```text
FakePlatform::new()
event_source.inject(UiEvent::pointer_down(...))
run_widget_loop / dispatch
assert handler 副作用 + presenter damage rects
```

Fake 不通过 `Platform` 暴露 presenter；测试通过 `FakeWindow` 获取 `FakePresenter` 历史。

### FakeTimer 与 TestClock（#139）

测试时间分 **两层**，不可混用：

| 类型 | 路径 | 驱动对象 |
|------|------|----------|
| **`FakeTimer`** | `native/test_harness/fake_timer.rs`（**已有**） | `Platform::timer()` / `ITimer` |
| **`TestClock`** | `app/event_loop/test_clock.rs`（设计） | `ActiveWorkRegistry::drain_due`、`wait_until` |

```rust
// FakeTimer — 平台 ITimer
pf.timer.advance(Duration::from_millis(100));  // → fired timer ids

// TestClock — App 主循环（设计）
clock.advance(Duration::from_secs(30));
step_frame(&mut clock);  // → drain_due + post_to_ui drain
```

详见 [testing · 测试时钟分层](testing.md#测试时钟分层)。

---

## 条件编译

**唯一**允许 `#[cfg(windows/unix)]` 的位置：

- `src/native/backends/**`
- `src/native/factory.rs`

`core` / `draw` / `ui` / `app` / `data` **不写**平台条件编译。

---

## Fail Fast

- 平台错误 → `core::Error` + `Result`
- 生产代码 `deny(clippy::unwrap_used)`（测试除外）
- 致命错误 → `diagnostic::Collector` + crash log（见 [foundation](foundation.md)）

---

## 源码模块

```text
native/
├── traits/           Platform, IWindowManager, UiEvent, …（上层唯一依赖面）
├── factory.rs        create_platform, create_gpu_context
├── backends/
│   ├── windows/      Win32 窗口、GDI/GPU present、输入
│   └── linux/        Wayland seat、shm、EGL
├── shared/           跨 backend 共用（window state、event_loop）
├── services/         file_service, notification（公开辅助）
├── presenter.rs      PresentDamage 适配
└── test_harness/     FakePlatform, FakeEventSource（#40）
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。
