# capabilities 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `capabilities` 模块，权威持有独立平台查询与轻量系统服务。依赖：[core/error](../core/error.md)、[core/geometry](../core/geometry.md)。导出：应用可使用的 `Platform` 门面和 owned 描述值。

> **当前实现线索**：公开面位于 `src/platform/`；内部 OS 提供者位于 `src/native/`，实现路径不构成公开边界。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Platform` | struct | 主线程、单实例、线程亲和的平台能力入口 |
| `OsInfo` / `CpuInfo` / `MemoryInfo` / `DisplayInfo` | value | 可跨线程保存的即时系统与硬件描述 |
| `GraphicsBackend` / `GpuAdapterInfo` | enum/value | 指定 API 的 adapter 枚举结果 |
| `SpecialDir` | enum | OS-known 用户目录 |
| `FileDialogFilter` | value | 文件对话框的命名扩展名过滤器；构造时规范化并稳定去重 |
| `AppUserModelId` | value | 调用方提供且通过基础格式验证的 Windows 通知身份 |
| `SystemNotificationCapability` | enum | 通知 Provider、身份配置与部署登记的可解释状态 |
| `SystemNotification` | value | 系统通知参数 |

## 组件：Platform

提供系统、硬件、显示器、GPU adapter、known directory、文件对话框和通知能力；返回 owned value 或 typed Result。它不拥有窗口事件循环、surface、Renderer 或 device-lost 恢复。

文件对话框通过 `Platform::open_files`、`Platform::save_file` 与 `Platform::open_folder` 同步进入目标 OS Provider。应用只传入 owned `FileDialogFilter`，不拼接 Win32 双 NUL、Linux 对话框模式或 AppKit 文件类型；确认结果复制为 owned `PathBuf`，取消统一返回 `Ok(None)`。过滤器名称、扩展名与标题在进入 Adapter 前验证，路径模式、NUL 与全通配符不会被不同平台静默解释为不同语义。Platform 私有编码器分别生成 Zenity 的 `名称 | 模式` 参数和 KDialog 的 Qt name filter 列表，Linux Adapter 只把运行时桌面对应的精确协议交给 Provider，不维护含糊的跨 Provider 字符串。外部 Provider 报告确认时必须同时产生至少一条非空且可无损表示的路径；空确认、空多选项或不可表示输出返回 typed 平台错误，不得过滤、替换或伪造成 owned 路径。Linux KDialog 的 `open_files` 显式启用多选与逐行输出，保持与其他 Provider 相同的一个或多个文件契约。

系统通知身份由 `Platform::set_notification_app_user_model_id` 显式接收并拥有。`Platform` 是 platform System 的公开门面，不是应用 composition root；它只把已验证配置交给私有 Adapter。Windows Adapter 只读探测开始菜单快捷方式的 `System.AppUserModel.ID` 并提交 Toast；UIX 不创建快捷方式、不写注册表，也不推断或代替部署身份。`Platform::system_notification_capability` 区分 `Available`、`IdentityRequired`、`IdentityUnregistered` 与 `Unsupported`。

## 组件：硬件描述值

描述值不持有原生 handle，也不是稳定设备 ID；缺失信息使用 Option，非法几何/内存值返回 typed error。

## 模块不变量

创建、调用和析构遵守 owner thread；查询不创建窗口或图形设备，目录查询不隐式创建目录。文件对话框只在 owner thread 同步运行，原生 handle 与面板对象不越过 Provider 调用边界。

`Platform` 实例拥有私有 Provider 与平台服务生命周期；owned 描述值离开实例后仍可读取，但只表示查询时快照，不证明设备、路径、权限或 Provider 继续有效。文件路径、硬件描述、剪贴板与通知内容按潜在用户数据处理，模块不默认记录完整值。

Windows 未配置 AUMID 时，发送返回 typed `NotImplemented` 并给出配置/安装指引；已配置但 MSIX 或安装器快捷方式未登记同一 AUMID 时，发送返回 typed `InvalidState`。只有只读登记探测成功后才调用 WinRT Toast API，系统拒绝提交时返回 typed 平台错误，不伪造成功。

Linux 与 macOS Adapter 通过私有 Unix Provider 探测组件只读检查 `PATH` 中的 `notify-send` 与 `osascript`：仅普通可执行文件可形成 `Available`，缺失或不可执行时形成 `Unsupported`。探测不启动外部进程，发送阶段继续独立承担权限、桌面会话与退出状态的 typed failure。

同步 adapter 枚举按公开 backend 精确分派：D3D11/D3D12 返回 DXGI owned values，Vulkan 与 macOS Metal 返回各自原生设备描述；OpenGL ES 因没有无上下文的可移植枚举协议而返回 typed `NotImplemented`，不以空集合伪装不支持。

当所有图形 backend feature 都关闭时，`GraphicsBackend` 没有可构造变体，facade 以穷尽空分派表达这一编译期事实；它不会伪造运行期 backend、成功值或失败值。
