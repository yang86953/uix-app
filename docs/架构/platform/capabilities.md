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
| `SystemNotification` | value | 系统通知参数 |

## 组件：Platform

提供系统、硬件、显示器、GPU adapter、known directory 和通知能力；返回 owned value 或 typed Result。它不拥有窗口事件循环、surface、Renderer 或 device-lost 恢复。

## 组件：硬件描述值

描述值不持有原生 handle，也不是稳定设备 ID；缺失信息使用 Option，非法几何/内存值返回 typed error。

## 模块不变量

创建、调用和析构遵守 owner thread；查询不创建窗口或图形设备，目录查询不隐式创建目录。
