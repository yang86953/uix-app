# 虚拟滚动

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统中**虚拟滚动**的内部设计。所属系统：`ui`（位于 `foundation/` 和 `widgets/containers/` 中）。依赖：[布局引擎模块](布局.md)、[组件核心模块](组件系统.md)。导出：虚拟滚动用法 → [使用 · 布局](../../使用/布局.md#滚动)。

## 模块定位

虚拟滚动能力分布在两处：`VirtualScroll` 声明式 API 位于 `src/ui/foundation/virtual_scroll.rs`，而 `VirtualScroll` 容器组件位于 `src/ui/widgets/containers/`。整体处理万级数据时只渲染可见区域内的行，滑出视口的行从树中移除。

## 组件清单

| 组件 | 类型 | 职责 |
|------|------|------|
| `VirtualScroll` | struct | 虚拟滚动容器；管理物化窗口和行渲染 |

## 组件：VirtualScroll

**接口**：只渲染可见区域的行。

| 参数 | 说明 |
|------|------|
| `item_count` | 数据总量 |
| `item_height` | 每行固定高度（logical px） |
| `overscan` | 可视区外预渲染行数（默认 5） |
| `size` | 视口尺寸（logical px） |
| `render` | 行进入视口时调用的工厂闭包 `|index| -> impl View` |

## 视口物化与复用

| 阶段 | 行为 |
|------|------|
| 计算物化范围 | 根据 scroll_offset、viewport_size、item_height、overscan 计算 [start_index, end_index] |
| 创建行节点 | render 闭包为每个进入物化范围的 index 创建 View |
| 移除行节点 | 滑出物化范围的行从树中移除 |
| 行身份 | 通过稳定 key（基于数据索引）保持 reconcile 时的节点复用 |

**Composite 滚动优化**：当行高度固定且行内容不变时，滚动只提交 composite-copy 而非完整重绘。

## 约束

- `render` 闭包返回普通 `View`，不暴露 `WidgetNode` 或 `.into_node()`
- 滑出视口的行从树中移除，重新进入时重建，因此不保留实例状态
- `VirtualScroll` 内部维护 offset + item_count，不参与全局 dirty 合并
