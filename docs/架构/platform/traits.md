# 原生平台层

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统中 **traits — 平台 trait**模块和 **backends — 平台后端**模块的内部设计。所属系统：`platform`。依赖：[系统列表](../系统列表.md)、`core` 系统。导出：平台能力公开用法 → [使用 · 平台能力](../../使用/平台能力.md)。

## 模块定位

platform 系统（`src/native/`）是 OS、窗口和图形平台的实现层，不对外暴露为公开命名空间。公开消费面是 `uix::platform`。`#[cfg(OS)]` 条件编译仅限此系统。

## 组件清单

### 模块：平台 trait（`traits/`）

| 组件 | 类型 | 职责 |
|------|------|------|
| `Platform` | struct | 公开门面：系统硬件查询 + 平台服务两大同步能力 |
| `InputTraits` | trait | 键盘（KeyCode/KeyMod）、鼠标（MouseButton）、滚轮、光标类型 |
| `GraphicsTraits` | trait | 图形后端能力声明、表面创建 |
| `SystemTraits` | trait | 系统通知、状态级别（StatusLevel）、ControlSize |

### 模块：平台后端（`backends/`）

| 组件 | 类型 | 职责 |
|------|------|------|
| `Backend` | trait | 平台后端统一接口 |
| `WindowsBackend` | struct | Windows 实现（Vulkan → D3D12 → OpenGL） |
| `LinuxBackend` | struct | Linux 实现（Vulkan → OpenGL） |
| `MacOsBackend` | struct | macOS 实现（Vulkan → Metal） |

### 其他模块

| 组件 | 类型 | 模块 | 职责 |
|------|------|------|------|
| `Presenter` | struct | 呈现器 | 窗口呈现桥接，连接 graphics 层与平台表面 |
| `Factory` | struct | 工厂 | 图形后端工厂，按 recipe 引导初始化 |
| `AgentTransport` | struct | Agent 传输 | Agent 协议的本机进程间传输层 |
| `TestHarness` | struct | 测试夹具 | 平台层测试基础设施 |

## 组件：Platform

**接口**：`uix::platform::Platform` 回答两类问题：

| 类别 | 查询能力 |
|------|----------|
| 硬件信息 | OS 名称/版本、CPU 核心数/架构、物理内存总量、显示器尺寸/DPI、GPU adapter 枚举（按指定图形 API） |
| 平台服务 | OS-known directory（文档、桌面、缓存等）、系统通知发送 |

**不属于公开面**：应用生命周期、渲染策略、CPU fallback、device-lost 恢复、帧调度和 backend 扩展 SPI。

## 隔离规则

- `#[cfg(OS)]` 仅限 platform 系统，上层禁 `use native::backends::*`
- OS 能力经 `native::traits` 暴露，上层只依赖 trait 不依赖具体实现
- GPU adapter 枚举由 `Platform` 公开，图形后端选择由 graphics 系统控制
