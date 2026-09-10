# windowing 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `windowing` 模块，权威持有原生窗口、事件源、输入、剪贴板、文件拖放和 IME 的平台边界。依赖：core。导出：供 app window/event-loop 使用的能力契约。

> **当前实现线索**：中立窗口与事件协议位于 `src/platform/windowing/`，输入迁移前协议与共享实现暂位于 `src/native/windowing/`，OS 实现位于 `src/native/backends/`；trait 只是当前分派手段，不是架构模块。

> **图形所有权边界**：`PlatformWindow` 只提供 native surface、窗口动作与最终 presenter，不持有、借出或关闭任何 graphics recipe context。生产 context 在构造后立即交给 renderer 的类型化 recipe owner，恢复与 checked teardown 也由 renderer/recovery 生命周期唯一执行；关闭窗口不会重复关闭图形资源。

> **标题栏装饰边界**：`set_system_title_bar_visible` 保持统一能力语义。Windows 在非客户区切换系统 caption；Wayland 通过 `xdg-decoration` 在 `ServerSide` 与 `ClientSide` 之间协商，缺少该扩展时仅客户端装饰可保证成功。自定义标题栏仍由 app/ui 绘制，platform 只拥有原生装饰模式与其生命周期。

> **自动居中边界**：`PlatformWindow::center_on_screen` 保持统一的 typed capability 契约；Wayland 不提供普通客户端绝对定位能力，因此返回 `Errc::NotImplemented`，不得伪造居中成功。App 的主窗、次窗与 `Window::create` 把该错误视为预期能力缺失，不记录 WARN，并由 compositor 使用默认放置；其他执行错误继续记录 WARN。此策略不引入首次 configure 后偏移、异步定位状态或新的窗口生命周期所有者。

> **可选能力边界**：`src/platform/windowing/capability.rs` 是窗口可选操作与精确查询的唯一值权威；`PlatformWindow::capabilities` 返回当前实例的只读集合。共享窗口核心必须在参数验证、共享状态写入、presenter 调用和 concrete adapter 委托之前完成统一门禁；缺失能力稳定返回 `Errc::NotImplemented` 与该中立操作身份。后端必须显式实现全部 `WindowOps` 方法并声明真实能力，协议 global 缺失等实例差异由能力集合动态表达，不允许默认 trait 方法暗示支持。

> **文件拖放边界**：Application 窗口创建 Adapter 在原生窗口创建后、向调用方发布 owner 前先查询 `EnableFileDrop`，仅在支持时调用 `enable_file_drop(true)`。能力稳定缺失时窗口仍可创建；其他启用失败必须关闭尚未发布的窗口并保留 typed 原因链。Windows 通过 `DragAcceptFiles` / `WM_DROPFILES` 交付本地路径；Wayland 只接受 `wl_data_device` 提供的 `text/uri-list` 与 Copy 动作，通过事件循环非阻塞读取并严格解析本地 `file:` URI。平台按 `WindowId` 投递一次 `FileDrop`，ui 再按落点命中 overlay 或主树；selection 替换、drag leave、窗口禁用/关闭和 backend teardown 必须消费 offer、pipe 与 callback owner，不得产生迟到事件。

> **surface role 边界**：`surface_role.rs` 唯一持有平台中立的 `WindowSurfaceRole` 与 `DesktopLayerConfig`。app 只传递角色值；Wayland Adapter 把普通角色映射为 `xdg_toplevel`，把桌面角色映射为 `zwlr_layer_shell_v1`，并独占协议对象、configure/closed callback 与销毁顺序。Windows、macOS 和缺少 layer-shell 的 Wayland 实例按 `Errc::NotImplemented` 拒绝桌面角色，不能静默退化成普通窗口。公开值不包含 output、surface、PID、FD 或原生句柄。

## 当前实现状态

| 状态 | 范围 | 已核实实现 |
|---|---|---|
| **已实现** | 中立能力权威 | `src/platform/windowing/capability.rs` 唯一定义 `WindowCapability` 与 `WindowCapabilities`，闭集共 33 个能力值；`PlatformWindow::capabilities` 返回当前窗口实例的只读事实。 |
| **已实现** | 基础能力集合 | Windows 固定声明 32 项，macOS 固定声明 12 项，Wayland 固定声明 16 项；平台名本身不推导额外能力。 |
| **已实现** | Wayland 实例动态能力 | `xdg_activation` 可用时增加 `Raise` 1 项；逐窗 `xdg-decoration` 对象建立时增加 `ShowSystemTitleBar` 1 项；seat 已建立 data-device owner 时增加 `EnableFileDrop`、`DisableFileDrop` 2 项。三组条件相互独立，全部满足时 Wayland 实例为 20 项。 |
| **已实现** | 统一拒绝边界 | 私有 `WindowOps` 可选默认方法体为 0，Windows、macOS、Wayland 必须显式实现完整方法集。共享核心在可选操作的参数验证、共享状态写入、presenter 与 native Adapter 副作用之前先执行能力门禁；缺失能力稳定返回中立 `Errc::NotImplemented`。 |
| **已实现** | Wayland `SetResizable` | 单窗口状态机唯一持有用户 min/max、最新正 logical 客户区尺寸与 resizable 状态。锁定把协议 min/max 同时设为最新有效尺寸，解锁恢复用户约束；锁定期间仍更新用户权威但不覆盖固定约束，程序化 resize 与 compositor configure 会把新有效尺寸同步为新的固定 min=max。只有协议成功后才提交规划状态。 |
| **已实现** | Wayland 桌面 layer | `App::surface_role` 与 `WindowConfig::desktop_layer` 共享同一中立配置；Wayland 绑定可选 layer-shell，映射 background/bottom/top/overlay、四边锚点、独占区与键盘交互（`OnDemand` 要求协议 v4）。layer surface 基础能力固定为 6 项，data-device 可用时再增加文件拖放 2 项；不声明 toplevel 的移动、缩放、最大化或装饰能力。 |

共享窗口层继续唯一负责正尺寸以及 min/max 交叉约束校验；Wayland 状态机只编码中立结果，不复制上层约束规则。能力查询、公开操作与 typed failure 只从外部消费者使用 `uix_app::platform` 公开门面测试；共享核心、Wayland 状态机与原生 Adapter 均为私有实现，不建立项目测试。

## 后续候选

**后续**只保留能力簇候选，不代表已实现或交付承诺：Wayland 外观/装饰能力（图标、attention/闪烁、整体透明度、边框策略），以及窗口状态/层级能力（系统菜单、lower、always-on-top 等）。候选只有在对应 compositor/桌面协议能给出真实实例能力时才可进入集合；本轮不启动开发，也不改变当前 33 项中立闭集。

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

原生输入 owner 由窗口、native view 和 generation 约束；未 commit composition 不成为最终文本。Windows TSF session 将 `TsfEventSink`、`TsfTextStore` 和 `WindowsTextInput` 绑定到同一 `WindowId` 与 platform pending-failure source；composition callback 只把状态转换后的 `UiEvent` 写入共享队列，唤醒失败和 text-store 锁/借用失败由 owner-thread 取回 typed `Error`，不在 callback 栈执行应用逻辑。`wnd_proc` 外层捕获消息处理 panic 并返回 `DefWindowProcW`；TSF 的 COM thunk 必须把 panic 转为 `E_FAIL` 和 owner failure，保证 unwind 不跨 ABI。

Wayland text-input-v3 在目标 surface 的 `enter` 前只保留期望状态；收到匹配 `enter` 后才发布 `enable`、最新已知光标矩形和同批 `commit`。后续光标矩形变化在已进入的会话中提交；`leave` 失效当前聚焦事实，下一次进入重新发布状态。停止时发送禁用并销毁该协议对象，旧代际回调不得重定向到新会话。协议时序正确或已收到预编辑事件不等于输入法候选框已可见，仍需目标桌面验证。

## 所有权、安全与生命周期

- platform System 拥有每个 `PlatformWindow`、EventSource 与原生输入会话；app/window 只通过公开窗口契约驱动，不取得私有 backend 或回调对象。
- 关闭顺序先停止新事件和文件 offer，结束 IME/composition，失效 WindowId/generation 并排空在途回调，再销毁原生窗口。迟到事件只允许丢弃或返回 stale failure。
- 文件拖放得到的 URI、路径、文件名和声明类型都视为不可信输入。platform 只验证并投递本地路径，不读取业务内容；应用仍需重新检查权限、竞态、大小、格式和内容安全。
- 剪贴板、输入文本与文件路径可能含敏感数据；不得由 platform 默认写入普通日志、错误回包或跨窗口全局状态。

## 模块不变量

logical/physical 换算在窗口/surface 边界完成；迟到 callback 丢弃，OS 分支不泄漏到 ui/data。窗口动作需要结果时返回 typed Result，传输或 OS 接收成功不能伪装成应用已处理。
