# virtualization 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的大数据视图物化、范围计算和复用策略。依赖：[component](component.md)、[layout](layout.md)、[view](view.md)。导出：虚拟列表状态、物化协议和 renderer 生命周期。
>
> **当前实现线索**：当前固定行高实现位于 `src/ui/foundation/virtual_scroll.rs`，部分表格和树组件各自持有范围逻辑；重构后应收敛到统一虚拟化模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `VirtualViewport` | struct | 保存 viewport、offset、overscan 和可见范围 |
| `VirtualListModel` | struct | 计算固定/可测量项目的范围、偏移和总尺寸 |
| `MaterializedRange` | value | 表示当前实际存在于组件树中的项目区间 |
| `ItemKey` | value/trait | 在排序、插入和回收中保持数据身份 |
| item renderer | side-table callback | 把数据索引或 key 转换为普通 `ViewNode` |
| measurement cache | internal store | 保存可变高度项目的受版本约束测量结果 |

## 组件：VirtualViewport

viewport、scroll offset、项目度量和 overscan 共同得到物化范围。范围不变时保留现有子树，只更新滚动 transform、damage 与暴露区域；范围改变时只构建进入项并移除离开项。

## 组件：ItemKey / item renderer

renderer 是按 owner `ComponentId` 管理的 side-table 回调，不进入组件快照。数据会排序或插入时必须使用稳定业务 key；仅在不可变顺序数据中才允许位置索引作为 identity。

## 组件：measurement cache

虚拟化协议允许从固定高度起步，并为可变高度保留测量缓存边界。字体、宽度、主题几何或数据版本变化时，相关测量必须失效并重新锚定滚动位置。

## 模块不变量

- offset 始终按内容和 viewport clamp；空数据与非有限度量返回有限空结果或 typed error。
- hit-test、绘制、语义 bounds 和行 frame 使用同一 content-to-viewport 变换。
- 虚拟化不维持固定帧；无滚动、数据或测量变化时不产生工作。
