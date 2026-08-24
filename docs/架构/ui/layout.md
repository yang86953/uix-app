# layout 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的测量、布局、盒模型和容器算法。基础依赖：[core/geometry](../core/geometry.md)；与[组件运行时框架](widget_runtime.md)的协作由 ui System 通过树阶段契约编排，二者不直接持有彼此 Module 实例。导出：组件布局能力与有限几何结果；公开用法见[使用 · 布局](../../使用/界面构建/布局.md)。
>
> **当前实现线索**：相关实现位于 `src/ui/layout/`（engine.rs 持有 `LayoutEngine` trait）；trait 与算法共同归本模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `Constraints` | core struct | min/max 宽高输入 |
| `BoxModel` | struct | margin、border、padding 和 content rect |
| `LayoutEngine` | trait | 容器布局算法统一接口 |
| `LayoutChild` | struct | 子身份、测量尺寸、margin、flex/grid 参数 |
| `LayoutOutput` / `LayoutOutput3D` | struct | 子 frame 和内容总尺寸 |
| `FlexLayout` | struct | Flex 参数组装与算法入口 |
| `GridLayout` | struct | track、gap、span、align 的 Grid 入口 |
| `FlexDirection`、`JustifyContent`、`AlignItems`、`GridTrack` | enum | 声明式布局参数 |

## 组件：LayoutEngine

```text
父 Constraints
  → measure：自底向上得到受约束 Size
  → layout：自顶向下分配 Rect
  → 写入节点 frame / overflow / visibility
  → frame 变化产生旧/新 damage
```

- measure 不写最终位置；layout 不能返回超出已归一约束的非有限尺寸。
- margin 推开兄弟；border/padding 从外框收敛到 content rect；阴影只影响视觉 damage，不改变 content box。
- 子节点 frame 收敛后同步父级 `child_visible`，不可见子树退出后续阶段。
- 文本交互通过 `TextIndexMap` 显式区分 UTF-8 `ByteIndex`、Unicode 标量 `CharIndex`、扩展字素簇 `GraphemeIndex` 与 `ShapingCluster`；字节/字符转换只能经过边界表，命中、光标、删除和选择对外只返回扩展字素簇边界。

## 组件：FlexLayout / GridLayout / ScrollView / VirtualScroll

### Flex

- 支持四种主轴方向、wrap、grow/shrink、justify、align、gap 和逐项 `align_self`；容器与内置组件不得复制第二套 Flex 求解器。
- `overflow_content` 只保留自然主轴尺寸，不取消换行、分布、交叉轴对齐或反向语义。获得真实 frame 后，有限的正/负剩余空间统一进入 justify；零尺寸 bootstrap 使用自然内容建立有限结果。
- 零主轴尺寸子项仍参与项目计数、gap 与换行边界。单行与实际只形成一行的 wrapped 路径，在同一输入下必须得到一致 frame 和自然尺寸账本。
- Stretch、grow/shrink 和 min/max 约束采用确定性冻结；达到上下限的项目停止消费空间，声明顺序不得改变等价项目的结果。

### Grid

- 使用显式行列轨道、cell/span 和独立 row/column gap。`Px`、`Auto`、正权重 `Fr` 依次在父级有限容量内求解；纯 `Auto` 轨道不为填满容器而无条件膨胀。
- 内容对齐与单元格内子项对齐是两条独立通道；逐项 `align_self` 只覆盖本项。没有正剩余空间时，Space/Stretch 类分布不能伪造额外尺寸。
- 跨轨道子项先扣除 gap 和已知轨道，再把缺口确定性分配给可扩展轨道。相同输入不得因子项声明顺序不同而改变轨道尺寸。
- 自动放置、span 松弛和辅助矩阵均有明确轮次/容量上限；超限返回 typed error 或有限空 frame，不能无限扩行、分配或搜索。

### Scroll 与虚拟范围

- ScrollView 的内容边界统一包含子项 margin 与已占用滚动条沟槽；`max_scroll`、绘制、命中和无障碍 bounds 使用同一 content-to-viewport 变换。
- VirtualScroll 只接受有限正估算高度和 viewport；offset 先夹到内容边界，单次物化有硬上限且可见内容优先于 overscan。详细身份与测量缓存契约由 [virtualization](virtualization.md) 持有。
- CSS `float` / `clear` 不属于 UIX 目标布局模型；贴靠、换行分组和二维分区分别用 Flex、Row/Column 与 Grid 表达，不建立平行浮动格式上下文。

### 文本与复合几何

- 文本测量缓存以有效宽度、字体、Locale、样式和资源 generation 为键；任一输入变化都使旧折行、命中和光标几何失效。
- 字体回退和换行不得拆开扩展字素簇或 shaping cluster；CRLF、NBSP、组合序列和 emoji ZWJ 遵循统一 Unicode 边界。双向文本按逻辑 cluster 分行、按视觉行重排，glyph 始终保留逻辑源区间。
- Table、RichText、Notification 等复合组件拥有自己的领域几何，但必须向 layout/paint/hit-test/damage/semantics 提供同一份已解析矩形和裁剪结果；视觉层级与命中层级不得各自推导。

## 增量与缓存

- 只有尺寸约束、布局属性、子结构、文本度量或可见性变化才标 Layout dirty。
- Paint-only 主题色、hover、opacity 动画不触发布局；width/height 等几何动画触发布局。
- 声明树原位协调分别报告 Paint 与 Layout 影响；已知字段按语义精细分类，未知组件、未知快照或未声明影响的 Provider 变化保守请求 Layout，不能以优化名义吞掉几何变化。
- 直接子节点增加、移除或重排后，树通过组件核心通知同步依赖子结构的派生状态；空子树不能依赖 `layout_children([])` 清理，因为 Phase 1 会跳过没有直接子节点的节点。
- `LayoutFrameScratch` / `LayoutTraversalScratch` 只跨帧复用容量，进入布局即清空本轮内容；遍历缓存以 `tree_version` 和相关输入 revision 为键，不持有跨帧可写 `LayoutOutput` 真相。
- 虚拟滚动在范围与挂载数量稳定且没有新版声明时不调用 renderer 或 reconcile，滚动走 composite/paint 路径；范围或 renderer 声明变化时，以业务 key 或绝对索引后备 key 协调当前有界窗口，重叠行保留原组件身份。

### 失效分类矩阵

`ViewAdapter` 对声明更新统一先比较公开 `SnapshotFields`，再把结果拆为 Paint 与 Layout 两条通道。分类分为三层：

| 分类 | 组件 | 规则 |
|---|---|---|
| 精细字段 | 已声明稳定分类的内置组件 | 颜色、背景等纯视觉字段只产生 Paint；文本、字号、盒模型、轨道、可见性等几何字段产生 Layout + Paint。 |
| 配置与运行态分离 | 明确区分 authored config 与受控运行值的组件 | 运行值不参与配置比较；尚未声明影响范围的配置变化仍按保守 Layout 处理。 |
| 显式保守 | 其余内置快照、`Unknown`、`Custom` | 任一未知配置差异产生 Layout + Paint；新增快照变体默认进入此类，获得专项证据后才可收窄。 |

组件归入哪一类必须由组件快照契约登记，不在 layout 正文维护易过期的全量名称清单。适配器必须验证精细分类、保守回退和 `Unknown`/`Custom` 兜底；缺少证据时保持保守分类。

## 不变量

- 结果尺寸和 frame 有限、非负；无界轴使用约束语义，不能把 `f32::MAX` 写入实际 frame。
- measure、paint、hit-test 对同一组件使用同一实际 frame。
- 布局不执行业务 I/O、任意应用 callback、present 或跨窗 wake。
- 布局 scratch、缓存和输出由所属窗口/树独占；稳定节点 ID 不延长组件或旧 tree generation 的生命周期。

## 公开 API 测试边界

- 只从外部消费者测试盒模型、Flex、Grid、ScrollView、VirtualScroll、RichText、Notification 与 Table 的公开入口、公开结果和可见失败语义。
- 缓存失效、子结构协调、内部几何、病理数值处理、具体算法分支和真实字体/窗口执行路径均为私有实现，不建立项目测试。
- 当前公开 API 测试结果与缺口由 Gitea 持有；本文只规定内部设计不变量，不把它们转换为测试目标。
