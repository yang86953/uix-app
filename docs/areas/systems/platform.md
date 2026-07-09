# 平台系统

← [Main](../architecture.md) · 系统 **#8** · 功能域：`native`

> OS 差异**仅**在此域隔离；`native` **抹平**全部平台差异，使 `core` / `draw` / `ui` / `app` / `data` 代码跨 OS **完全一致**。traits 对外；Fail Fast。**当前开发优先级**：Windows 端先行打磨（[#167](../../decisions.md#d167)），Linux / macOS / 移动端复用同一上层、仅换 backend。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 聚合入口 | [Platform 聚合](#platform-聚合) | AGENTS |
| 窗口与呈现 | [窗口与呈现](#窗口与呈现) | #59 #70 #162 |
| 多图形 API | [多图形 API 与 factory](#多图形-api-与-factory) | #162 #164 |
| 输入 | [输入](#输入) | #71 #75 |
| 事件 | [事件模型](#事件模型) | — |
| 系统服务 | [系统服务](#系统服务) | — |
| Traits 清单 | [Traits 清单](#traits-清单) | — |
| 工厂 | [工厂与后端](#工厂与后端) | AGENTS |
| 测试 | [测试平台](#测试平台) | #40 |
| 条件编译 | [条件编译](#条件编译) | AGENTS |
| 平台贡献 | [平台贡献指南](#平台贡献指南) | AGENTS |

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

**平台层职责**（[#167](../../decisions.md#d167)）：**唯一**允许 OS 差异的代码域。窗口、事件、呈现、输入、系统服务等全部差异封装在 `native/backends/`、`native/graphics/` 与 `factory/`；上层 **不得** 感知 Win32 / Wayland / AppKit 等实现细节，只通过 trait + `Result` 交互。

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

<a id="窗口可选能力"></a>

### 窗口可选能力

`WindowOps` 将平台可选窗口能力表达为 `Result<()>`：支持的后端返回 `Ok(())`，协议或 OS 不支持的能力返回 `Err(Errc::NotImplemented)`。`PlatformWindowCore` 保持公开 `PlatformWindow` / `IWindowProperties` 的无返回值兼容 API，但必须记录错误，并且只有底层能力返回 `Ok(())` 后才更新共享 `WindowState`，避免 Wayland 等平台把未支持操作误记为成功状态。

当前能力边界：

| 能力 | Windows | Linux Wayland | macOS |
|------|---------|---------------|-------|
| position / resizable / borderless / always_on_top / opacity / file_drop | 原生或窗口管理 API 支持则 `Ok(())` | 协议不支持或由 compositor 控制，返回 `Err(NotImplemented)` | 多数未接，返回 `Err(NotImplemented)` |
| maximize / minimize / restore / fullscreen | `Ok(())` | xdg_toplevel 支持，返回 `Ok(())` | 未接 |
| text_input | 由平台 IME/text-input 通道管理，返回 `Ok(())` | 由 Wayland text-input manager 管理，返回 `Ok(())` | `UixContentView`（NSTextInputClient）+ `ITextInput`，返回 `Ok(())` |

### 呈现（#59、#70）

| 路径 | 接口 | damage | 当前 engine |
|------|------|--------|-------------|
| CPU | `IPresenter::present(pixels, w, h, PresentDamage)` | Full / Partial rects | `SoftwareEngine` |
| GPU · GL | `IGraphicsContext::swap_buffers(PresentDamage)` | 同上 | `GpuEngine` + `GpuBackend` |
| GPU · D3D/Vulkan/Metal | `IGraphicsContext::present(PresentFrame)`（`present_pixels` 转发） | 全帧像素上传 | `PresentUploadEngine` + `CpuBackend` |

`PresentDamage` 来自 `core::damage` — 物理像素矩形列表或全屏。

**Present 分派（当前）**：`create_graphics_engine` 读 `caps().pipeline` **预设** — `NativeGpuRaster` → `GpuEngine`，`CpuUploadPresent` → `PresentUploadEngine`。正交能力模型 → [graphics-backend-pluggable · 可组合渲染轴](graphics-backend-pluggable.md#可组合渲染轴)（#168）。

GPU 路径：`PlatformWindow::graphics_context()` 返回 `Option<&mut dyn IGraphicsContext>`；`app::create_preferred_engine` 调用 `draw::bootstrap_graphics_engine`（**唯一** probe 循环）。`create_gpu_context*` 为 factory **单条目**低层入口（测试 / 直接调用；`Auto` 须走 bootstrap），**非** app 主路径。具体 API 对上层 **不可见** — 选型摘要见 [rendering · 多图形 API](rendering.md#多图形-api) · [graphics-backend-pluggable · 核心抽象](graphics-backend-pluggable.md#核心抽象) · [#162](../../decisions.md#d162)。

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

<a id="ieventloop"></a>

### IEventLoop

```rust
fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool;
fn wait_event(...);
fn wait_timeout(timeout, ...);
fn waker(&self) -> EventLoopWaker;
```

Callback 返回 `false` → 请求退出循环。Backend 实现 `OsEventSource`；blanket impl 提供 `IEventLoop`。

### EventLoopWaker

跨线程唤醒 blocking `wait_event` / `wait_until`（#117、#132、#133）：

```rust
#[derive(Clone)]
pub struct EventLoopWaker { /* Arc<dyn Fn() + Send + Sync> */ }

impl EventLoopWaker {
    pub fn new<F: Fn() + Send + Sync + 'static>(wake: F) -> Self;
    pub fn wake(&self);
}
```

| 规则 | 说明 |
|------|------|
| 注入 | `App::run_gui` 从 `platform.event_loop().waker()` 写入 `AppRuntime` 与 `AppState` |
| 触发 | `post_to_ui` / Timer 注册 / semantic queue 等成功入队后调用 `wake()` |
| 后端 | Windows / Linux Wayland / macOS / FakePlatform 提供真实 wake；`Default` 为 no-op |

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

<a id="traits-清单"></a>

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
    └── mod.rs       IEventLoop, EventLoopWaker
```

---

## 工厂与后端

| 入口 | 作用 |
|------|------|
| `create_platform()` | 当前 OS 的 Platform 实例 |
| `create_gpu_context(surface, w, h)` | **单条目** `try_create_gpu_context`；`Auto` 返回错误（须 `draw::bootstrap_graphics_engine`）；供测试 / 低层直接调用 |
| `create_gpu_context_with_backend(..., backend)` | 同上，显式指定 API；**无** probe 循环 |
| `available_memory_bytes()` | 引擎选择参考（内存不足时可倾向 SoftwareEngine） |

Backend 实现位于 `native/backends/windows/`、`native/backends/linux/`（Wayland）、`native/backends/macos/`（AppKit bootstrap）。

| 平台 | Platform | GPU 上下文 | 备注 |
|------|----------|-----------|------|
| Windows | ✅ | ✅ D3D11 + WGL → OpenGL ES | D3D12 Planned |
| Linux (Wayland) | ✅ | ✅ Vulkan + EGL → OpenGL ES | — |
| macOS | ✅ AppKit bootstrap | ✅ Metal（CpuUploadPresent，feature `metal`） | native raster backlog |

<a id="多图形-api-与-factory"></a>

### 多图形 API 与 factory

**架构原则**（#163、#164）→ [graphics-backend-pluggable · 图形 API 架构原则](graphics-backend-pluggable.md#图形-api-架构原则) · [源码目录](graphics-backend-pluggable.md#源码目录) · [核心抽象](graphics-backend-pluggable.md#核心抽象)（Auto 链、probe 流程、Present 分派）。

`factory/`（`mod.rs` + `registry*.rs`）是 **唯一** 对外 factory 分派入口（`create_platform` / `create_gpu_context*`）；`#[cfg]` 还允许 `backends/`、`graphics/**/platform/`（见 [条件编译](#条件编译)）。新增图形 API 在 `native/graphics/<api>/` 实现 + registry 表登记一行。

上层 **禁止** 区分 WGL/EGL/Vulkan：只持有 `dyn IGraphicsContext`。`IGraphicsContext::get_proc_address` 供 GL 系 backend 加载扩展；非 GL API 可返回 `None`，由对应 backend 自行链接。

**P6 图形后端** 状态与 backlog → [implementation · P6 生产级框架](../implementation.md#p6-生产级框架)。

### 未实现或后续

macOS 原生运行验证、D3D12、Metal **native raster** → [implementation · P6 生产级框架](../implementation.md#p6-生产级框架) · [后续工作](../implementation.md#后续工作)。

<a id="测试平台"></a>

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
| **`TestClock`** | `app/test_clock.rs`（已接） | `ActiveWorkRegistry::drain_due`、`wait_until` |

```rust
// FakeTimer — 平台 ITimer
pf.timer.advance(Duration::from_millis(100));  // → fired timer ids

// TestClock — App 主循环测试时钟
clock.advance(Duration::from_secs(30));
step_frame(&mut clock);  // → drain_due + post_to_ui drain
```

详见 [testing · 测试时钟分层](testing.md#测试时钟分层)。

---

## 条件编译

**唯一**允许 `#[cfg(windows/unix)]` 的位置：

- `src/native/backends/**`
- `src/native/graphics/**/platform/**`（surface 绑定等 OS 薄适配，#164）
- `src/native/factory/`（`mod.rs` + `registry*.rs`）

`core` / `draw` / `ui` / `app` / `data` **不写**平台条件编译。

---

<a id="平台贡献指南"></a>

## 平台贡献指南

面向 Win32 / Wayland / macOS backend 贡献者。**新 backend 能力先在 Windows 验证**（[#167](../../decisions.md#d167)），再推广至其他 OS。上层硬约束 → [AGENTS.md](../../../AGENTS.md#架构硬约束)。

### 目录结构

```text
native/
├── factory/                ← 唯一对外 factory 分派（mod.rs + registry*.rs）
│   ├── mod.rs              create_platform / create_gpu_context*
│   ├── registry.rs         GraphicsBackendEntry 表驱动
│   └── registry_windows.rs / registry_linux.rs / registry_macos.rs
├── traits/                 ← 上层唯一依赖面；新能力先加 trait
├── shared/                 ← OsEventSource、WindowState、PlatformWindowCore
├── graphics/               ← 【#164 已落地】IGraphicsContext 对等 API 实现（vulkan/ opengl/ d3d11/ metal/ …）
├── backends/
│   ├── windows/            ← Win32：wnd_proc、GDI present、Win32 输入
│   │   ├── platform.rs     WindowsPlatform + Platform impl
│   │   ├── wnd_proc.rs     消息泵 → UiEvent
│   │   └── …               clipboard, cursor, timer, …
│   ├── linux/
│   │   ├── platform.rs     LinuxPlatform + Platform impl
│   │   └── wayland/        连接、seat、shm、xdg_toplevel
│   └── macos/              ← AppKit bootstrap + Metal CpuUpload
│       ├── platform.rs     MacosPlatform + Platform impl
│       └── …               window_delegate, text_input_view, …
├── test_harness/           FakePlatform（#40）
└── services/               file_service、notification（公开辅助）
```

**禁止**：在 `core` / `draw` / `ui` / `app` / `data` 写 `#[cfg(windows/unix)]`。

### 接入 checklist

| # | 步骤 |
|---|------|
| 1 | 在 `traits/` 定义或扩展 trait；能力差异用 `Result<()>` + `Errc::NotImplemented` |
| 2 | 在对应 backend 子模块实现 struct + trait impl |
| 3 | 在 `*Platform` 聚合 struct 中持有子系统；`Platform` 访问器返回 `&mut dyn Trait` |
| 4 | 事件：backend 产出 `UiEvent` → `OsEventSource` 队列；实现 `IEventLoop::waker()` |
| 5 | 呈现：`IPresenter::present` 或 `IGraphicsContext::swap_buffers` 接受 `PresentDamage` |
| 6 | 若需 factory 分支：改 `factory/` registry 与 `native/graphics/<api>/` |
| 7 | FakePlatform 同步 stub + 调用历史（生产路径零分叉） |
| 8 | 测试：`FakePlatform` 或 CI 目标平台；见 [testing · FakePlatform](testing.md#fakeplatform) |

### Win32 / Wayland / macOS 差异

| 主题 | Windows | Linux Wayland | macOS |
|------|---------|---------------|-------|
| 窗口句柄 | `HWND` + `wnd_proc` | `xdg_toplevel` + registry globals | `NSWindow` + `UixContentView` |
| 事件泵 | `GetMessage` / 队列 | `wl_display` dispatch | `NSApplication` run loop |
| CPU 呈现 | GDI `BitBlt` | SHM buffer + `wl_surface` commit | CALayer `present_layer_pixels` |
| GPU | D3D11/WGL + `bootstrap_graphics_engine`；低层测试 `create_gpu_context_with_backend`；失败回退 GDI | Vulkan/EGL + bootstrap；低层测试 `create_gpu_context_with_backend`；失败回退 SHM | Metal + bootstrap（`CpuUploadPresent`，feature `metal`）；失败回退 CPU present |
| 可选窗口能力 | 多数原生 API `Ok(())` | 不支持则 `WindowOps` → `NotImplemented`（见 [窗口可选能力](#窗口可选能力)） | 多数未接，`NotImplemented` |
| IME | Win32 text input | `zwp_text_input_v3` | `NSTextInputClient` + `ITextInput` |
| Wake | 平台特定 wake 注入 `EventLoopWaker` | 同上 | 同上 |

共享逻辑放 `native/shared/`（如 `WindowState`、`PlatformWindowCore`），避免双份 drift。

### factory/ 接线

`create_platform()` 在 `factory/mod.rs` 按 OS `#[cfg]` 分派；图形 API 由 **registry 表**驱动（`registry.rs` + `registry_<os>.rs`），新增 API 只登记一行。Windows 表示例 → [`factory/registry_windows.rs`](../../../src/native/factory/registry_windows.rs)：

```rust
// registry_windows.rs — GraphicsBackendEntry 表
pub(crate) const PLATFORM_ENTRIES: &[GraphicsBackendEntry] = &[
    GraphicsBackendEntry {
        id: GraphicsBackend::D3d11,
        priority: 20,
        status: D3D11_STATUS,
        create: d3d11::create,
    },
    GraphicsBackendEntry {
        id: GraphicsBackend::OpenGlEs,
        priority: 10,
        status: OPENGL_STATUS,
        create: opengl::create,
    },
    // …
];
```

新增 OS：增加 `backends/<os>/` + `registry_<os>.rs` 表；`create_platform()` 加 `#[cfg]` 分支；不支持平台返回明确 `PlatformError`。Probe 循环 **仅** `draw::bootstrap_graphics_engine`（#163）。

### 测试建议

```text
1. FakePlatform::new()
2. fake.event_source.inject(UiEvent::...)
3. run_widget_loop / dispatch
4. 断言 presenter damage、window 状态、handler 副作用
```

Timer 测试分层 → [testing · 测试时钟分层](testing.md#测试时钟分层)（`FakeTimer` vs `TestClock`）。

---

## Fail Fast

- 平台错误 → `core::Error` + `Result`
- 生产代码 `deny(clippy::unwrap_used)`（测试除外）
- 致命错误 → `diagnostic::Collector` + crash log（见 [foundation](foundation.md)）


源码目录映射 → [implementation · 源码目录详表](../implementation.md#源码目录详表) · [平台贡献指南 · 目录结构](#平台贡献指南)。
