# capabilities 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `capabilities` 模块，权威持有独立平台查询与轻量系统服务。依赖：[core/error](../core/error.md)、[core/geometry](../core/geometry.md)。导出：应用可使用的 `Platform` 门面和 owned 描述值。

> **当前实现线索**：公开面位于 `src/platform/`；内部 OS 提供者位于 `src/native/`，实现路径不构成公开边界。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Platform` | struct | 主线程、单实例、线程亲和的平台能力入口 |
| `OsInfo` / `CpuInfo` / `MemoryInfo` / `DisplayInfo` | value | 可跨线程保存的即时系统与硬件描述 |
| `GraphicsBackend` / `GpuAdapterInfo` | enum/value | 指定 API 的 adapter 枚举结果 |
| `SpecialDir` | enum | OS-known 用户目录 |
| `AppUserModelId` | value | 调用方提供且通过基础格式验证的 Windows 通知身份 |
| `SystemNotificationCapability` | enum | 通知 Provider、身份配置与部署登记的可解释状态 |
| `SystemNotification` | value | 系统通知参数 |

## 组件：Platform

提供系统、硬件、显示器、GPU adapter、known directory 和通知能力；返回 owned value 或 typed Result。它不拥有窗口事件循环、surface、Renderer 或 device-lost 恢复。

系统通知身份由 `Platform::set_notification_app_user_model_id` 显式接收并拥有。`Platform` 是配置的 composition root，Windows Adapter 只读探测开始菜单快捷方式的 `System.AppUserModel.ID` 并提交 Toast；UIX 不创建快捷方式、不写注册表，也不推断或代替部署身份。`Platform::system_notification_capability` 区分 `Available`、`IdentityRequired`、`IdentityUnregistered` 与 `Unsupported`。

## 组件：硬件描述值

描述值不持有原生 handle，也不是稳定设备 ID；缺失信息使用 Option，非法几何/内存值返回 typed error。

## 模块不变量

创建、调用和析构遵守 owner thread；查询不创建窗口或图形设备，目录查询不隐式创建目录。

Windows 未配置 AUMID 时，发送返回 typed `NotImplemented` 并给出配置/安装指引；已配置但 MSIX 或安装器快捷方式未登记同一 AUMID 时，发送返回 typed `InvalidState`。只有只读登记探测成功后才调用 WinRT Toast API，系统拒绝提交时返回 typed 平台错误，不伪造成功。

同步 adapter 枚举按公开 backend 精确分派：只有 Windows D3D11 返回 DXGI owned values，其他已启用 backend 返回 typed `NotImplemented`，不保留空成功或不可达的幽灵成功路径。

当所有图形 backend feature 都关闭时，`GraphicsBackend` 没有可构造变体，facade 以穷尽空分派表达这一编译期事实；它不会伪造运行期 backend、成功值或失败值。
