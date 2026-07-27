# windowing 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的目标 `windowing` 模块，权威持有原生窗口、事件源、输入、剪贴板和 IME 的平台边界。依赖：core。导出：供 app window/event-loop 使用的能力契约。

> **当前实现线索**：分布在 `src/native/backends/`、`src/native/traits/{window,event,input}.rs` 与 `src/native/shared/`；trait 只是当前分派手段，不是架构模块。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `WindowBackend` | 平台能力 | 创建/销毁窗口、窗口属性与原生动作 |
| `PlatformWindow` | 窗口对象 | 可见性、尺寸、焦点、surface 关联 |
| `EventSource` | 事件能力 | wait、wake、drain 原生事件 |
| `InputServices` | 输入能力 | keyboard、pointer、cursor、clipboard |
| `TextInputSession` | IME 能力 | start/stop、caret、composition/commit |
| `WindowEvent` | 值 | 带 WindowId/generation 的平台事件 |

## 组件：EventSource

OS callback/poll 只采集数据并投递目标窗口事件；应用 callback 在 app/ui 的 owner thread 执行。wake 必须可归属目标窗口或明确的应用级工作。

## 组件：TextInputSession

原生输入 owner 由窗口、native view 和 generation 约束；未 commit composition 不成为最终文本。

## 模块不变量

logical/physical 换算在窗口/surface 边界完成；迟到 callback 丢弃，OS 分支不泄漏到 ui/data。
