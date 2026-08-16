# windowing 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `windowing` 模块，权威持有原生窗口、事件源、输入、剪贴板、文件拖放和 IME 的平台边界。依赖：core。导出：供 app window/event-loop 使用的能力契约。

> **当前实现线索**：分布在 `src/native/windowing/`（window.rs、input.rs、event/、shared/）与 `src/native/backends/`；trait 只是当前分派手段，不是架构模块。

> **图形所有权边界**：`PlatformWindow` 只提供 native surface、窗口动作与最终 presenter，不持有、借出或关闭任何 graphics recipe context。生产 context 在构造后立即交给 renderer 的类型化 recipe owner，恢复与 checked teardown 也由 renderer/recovery 生命周期唯一执行；关闭窗口不会重复关闭图形资源。

> **标题栏装饰边界**：`set_system_title_bar_visible` 保持统一能力语义。Windows 在非客户区切换系统 caption；Wayland 通过 `xdg-decoration` 在 `ServerSide` 与 `ClientSide` 之间协商，缺少该扩展时仅客户端装饰可保证成功。自定义标题栏仍由 app/ui 绘制，platform 只拥有原生装饰模式与其生命周期。

> **自动居中边界**：`PlatformWindow::center_on_screen` 保持统一的 typed capability 契约；Wayland 不提供普通客户端绝对定位能力，因此返回 `Errc::NotImplemented`，不得伪造居中成功。App 的主窗、次窗与 `Window::create` 把该错误视为预期能力缺失，不记录 WARN，并由 compositor 使用默认放置；其他执行错误继续记录 WARN。此策略不引入首次 configure 后偏移、异步定位状态或新的窗口生命周期所有者。

> **文件拖放边界**：Application 窗口创建 Adapter 在原生窗口创建后、向调用方发布 owner 前统一调用 `enable_file_drop(true)`。`Errc::NotImplemented` 表示平台稳定缺少该能力，窗口仍可创建；其他启用失败必须关闭尚未发布的窗口并保留 typed 原因链。Windows 通过 `DragAcceptFiles` / `WM_DROPFILES` 交付本地路径；Wayland 只接受 `wl_data_device` 提供的 `text/uri-list` 与 Copy 动作，通过事件循环非阻塞读取并严格解析本地 `file:` URI。平台按 `WindowId` 投递一次 `FileDrop`，ui 再按落点命中 overlay 或主树；selection 替换、drag leave、窗口禁用/关闭和 backend teardown 必须消费 offer、pipe 与 callback owner，不得产生迟到事件。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `WindowBackend` | 平台能力 | 创建/销毁窗口、窗口属性与原生动作 |
| `PlatformWindow` | 窗口对象 | 可见性、尺寸、焦点、surface 关联 |
| `EventSource` | 事件能力 | wait、wake、drain 原生事件 |
| `InputServices` | 输入能力 | keyboard、pointer、cursor、clipboard |
| `FileDrop` | 数据传递能力 | 协商文件 offer、读取本地路径并投递逐窗落点事件 |
| `TextInputSession` | IME 能力 | start/stop、caret、composition/commit |
| `WindowEvent` | 值 | 带 WindowId/generation 的平台事件 |

## 组件：EventSource

OS callback/poll 只采集数据并投递目标窗口事件；应用 callback 在 app/ui 的 owner thread 执行。wake 必须可归属目标窗口或明确的应用级工作。

## 组件：TextInputSession

原生输入 owner 由窗口、native view 和 generation 约束；未 commit composition 不成为最终文本。Windows TSF session 将 `TsfEventSink`、`TsfTextStore` 和 `WindowsTextInput` 绑定到同一 `WindowId` 与 platform pending-failure source；composition callback 只把状态转换后的 `UiEvent` 写入共享队列，唤醒失败和 text-store 锁/借用失败由 owner-thread 取回 typed `Error`，不在 callback 栈执行应用逻辑。`wnd_proc` 外层捕获消息处理 panic 并返回 `DefWindowProcW`；TSF 的 windows-rs 生成 COM thunk 由 text-store trait guard 将 panic 转为 `E_FAIL` 和 owner failure，保证 unwind 不跨 ABI。`ITextStoreACP` 与 `ITfContextOwnerCompositionSink` 两组真实生成 vtable 均有 panic 回归覆盖。

## 模块不变量

logical/physical 换算在窗口/surface 边界完成；迟到 callback 丢弃，OS 分支不泄漏到 ui/data。
