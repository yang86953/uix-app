# virtualization 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的大数据视图物化、范围计算和复用策略。与[组件运行时框架](widget_runtime.md)、[layout](layout.md)及[view](view.md)的协作由 ui System 编排，Module 间不直接持有实例。导出：虚拟列表状态、物化协议和 renderer 生命周期。
>
> **当前实现线索**：固定与可变行高 `VirtualScroll` / `VirtualListScroll` 位于 `src/ui/virtualization/virtual_scroll/mod.rs`，可变行高的稀疏 measurement cache 位于 `src/ui/virtualization/measurement_cache.rs`；动态行 renderer 由 `src/ui/coordination/render_handler.rs` 的 owner side table 持有，并通过 `src/ui/coordination/adapter/mod.rs` 的 keyed reconcile 接入组件树；表格和树仍可复用共享滚动状态并保留各自绘制路径。

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

viewport、scroll offset、项目度量和 overscan 共同得到物化范围。固定模式要求行高与 viewport 是有限正值；可变模式以 `.item_height(...)` 作为未知项目的有限正估算高度，再用已测量项目的前缀坐标修正范围；非法度量仍返回 `(0, 0)`。offset 先夹到内容范围。单次窗口最多物化 4096 项：先保留从首个可见行开始的可见内容，再用剩余预算分配两侧 overscan，因此病理 viewport 或 `usize::MAX` overscan 不会扩张为整棵列表。

范围与已挂载数量都不变且没有新版声明时不调用 renderer 或 reconcile，只更新滚动 transform、damage 与暴露区域。范围改变或声明 renderer 更新时，renderer 声明当前有界窗口，keyed reconcile 原位复用重叠行组件、移除离开项并只为进入项分配新组件；renderer 回调本身可能重新声明窗口内的 `ViewNode`，不承诺只对进入索引调用。

## 组件：ItemKey / item renderer

renderer 是按 owner `WidgetId` 管理的 side-table 回调，不进入组件快照。普通 `.render(...)` 在用户行工厂执行前生成类型化绝对索引身份，框架据此统一设置节点 key 与动态组件私有状态命名空间，行工厂不得再设置根 key；它只适用于顺序不可变的数据。可排序、插入或删除的数据必须使用 `.render_keyed(key_fn, renderer)`，先计算全局唯一且稳定的业务 key，再以同一规范身份执行状态捕获和 keyed reconcile。索引身份与业务身份使用互斥内部 tag，模式切换不会误接管旧状态。

当前物化窗口内出现重复业务 key 必须在发布前失败，不能按索引或遍历顺序静默消歧。数据版本、renderer signature 与 owner/tree generation 一起约束缓存和回调；异步数据结果必须携带请求/数据 revision，晚到旧页不得覆盖新版范围。

## 组件：measurement cache

生产实现支持固定行高和显式 `.variable_height()` 模式。可变模式把已物化子项的 `measured_size.h` 写入稀疏缓存，未测量项目继续使用 `.item_height(...)` 估算；总高度、最大偏移、滚动比例、可见范围和最终行 frame 共用同一组缓存前缀坐标。非法或非有限测量不会覆盖旧值，字体、宽度、主题几何或数据版本变化时可切换 `measurement_version` 或显式失效缓存，并清除旧测量后重新锚定滚动位置。

## 模块不变量

- offset 始终按内容和 viewport clamp；空数据、非正或非有限度量返回有限空结果或 typed error，未知可变项目回退到估算高度。
- 每次物化数量必须有硬上限；可见内容优先于 overscan，资源预算耗尽时不得扩张整棵列表。
- 行身份优先采用业务稳定 key；只有顺序不可变时才能采用绝对索引后备 key。
- hit-test、绘制、语义 bounds 和行 frame 使用同一 content-to-viewport 变换。
- 虚拟化不维持固定帧；无滚动、数据或测量变化时不产生工作，测量变化才触发下一轮窗口复核。
- renderer 只在目标窗口 UI 轮次调用并受每轮物化预算约束；owner 移除、模式切换或窗口关闭同步释放 side-table 回调和测量缓存。
