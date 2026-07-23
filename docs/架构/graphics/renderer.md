# 图形

[← 架构索引](../../架构.md)

> **接口**：声明 graphics 系统中 **renderer — 渲染器**、**api — 绘制 API**、**resources — 资源管理**和 **scene — 场景**模块的内部设计。所属系统：`graphics`。依赖：[系统列表](../系统列表.md)（分层规则）、`core` 系统（几何/错误）、`platform` 系统（平台图形后端）。导出：PaintContext 用法 → [使用 · 组件](../../使用/组件.md#paintcontext-速查)。

## 模块定位

graphics 系统（`src/draw/`）是绘制管线的全部实现。按稳定职责拆为八个模块：`api/`、`geometry/`、`command/`、`scene/`、`renderer/`、`backend/`、`resources/`、`debug/`。依赖方向：api / geometry → command / scene → renderer → backend。
