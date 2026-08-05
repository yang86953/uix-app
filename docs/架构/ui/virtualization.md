# virtualization 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的大数据视图物化、范围计算和复用策略。依赖：[component](component.md)、[layout](layout.md)、[view](view.md)。导出：虚拟列表状态、物化协议和 renderer 生命周期。
>
> **当前实现线索**：固定行高 `VirtualScroll` / `VirtualListScroll` 位于 `src/ui/virtualization/virtual_scroll.rs`，动态行 renderer 由 `src/ui/render_handler.rs` 的 owner side table 持有，并通过 `src/ui/adapter.rs` 的 keyed reconcile 接入组件树；表格和树仍可复用共享滚动状态并保留各自绘制路径。

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

viewport、scroll offset、项目度量和 overscan 共同得到物化范围。固定行高与 viewport 必须是有限正值，否则返回 `(0, 0)`；offset 先夹到内容范围。单次窗口最多物化 4096 项：先保留从首个可见行开始的可见内容，再用剩余预算分配两侧 overscan，因此病理 viewport 或 `usize::MAX` overscan 不会扩张为整棵列表。

范围与已挂载数量都不变且没有新版声明时不调用 renderer 或 reconcile，只更新滚动 transform、damage 与暴露区域。范围改变或声明 renderer 更新时，renderer 声明当前有界窗口，keyed reconcile 原位复用重叠行组件、移除离开项并只为进入项分配新组件；renderer 回调本身可能重新声明窗口内的 `ViewNode`，不承诺只对进入索引调用。

## 组件：ItemKey / item renderer

renderer 是按 owner `ComponentId` 管理的 side-table 回调，不进入组件快照。renderer 返回的业务 key 会原样保留；未声明 key 时，框架补入 `virtual-scroll-item:{absolute_index}` 作为确定性后备身份。数据会排序、插入或删除时必须返回稳定业务 key；绝对索引后备 key 仅适用于顺序不可变的数据，不能冒充业务身份。

## 组件：measurement cache

当前生产实现只支持固定行高；非法或非有限行高返回有限空范围，总高度、最大偏移、滚动比例与最终行 frame 均在有限虚拟坐标预算内饱和。虚拟化协议为后续可变高度保留测量缓存边界；字体、宽度、主题几何或数据版本变化时，相关测量必须失效并重新锚定滚动位置。

## 模块不变量

- offset 始终按内容和 viewport clamp；空数据、非正或非有限度量返回有限空结果或 typed error。
- 每次物化数量必须有硬上限；可见内容优先于 overscan，资源预算耗尽时不得扩张整棵列表。
- 行身份优先采用业务稳定 key；只有顺序不可变时才能采用绝对索引后备 key。
- hit-test、绘制、语义 bounds 和行 frame 使用同一 content-to-viewport 变换。
- 虚拟化不维持固定帧；无滚动、数据或测量变化时不产生工作。
