# 组件系统

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统中 **core — 组件核心**模块的内部设计。所属系统：`ui`。依赖：[系统列表](../系统列表.md)（需先了解分层规则）、`graphics` 系统（PaintContext）。导出：组件开发用法 → [使用 · 组件](../../使用/组件.md)。

## 模块定位

组件核心是 ui 系统的基础设施，负责组件树的构建、diff 更新（reconciliation）、生命周期管理以及横切关注点（状态、焦点、拖拽、样式、文本、交互）的编排。上层子系统（浮层、表单、虚拟滚动等）都构建在此核心之上。

### 核心流程

```
App::new().run()
  └─ EventLoop::tick()
       ├─ WindowDriver::process_native_events() → SystemEvent
       ├─ WindowDriver::reconcile() → WidgetTree diff
       ├─ WindowDriver::layout() → measure + layout
       └─ WindowDriver::paint(&mut PaintContext)
            └─ draw::Renderer
```
