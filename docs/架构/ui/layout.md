# layout 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的测量、布局、盒模型和容器算法。依赖：[component](component.md)、[core/geometry](../core/geometry.md)。导出：组件布局能力与有限几何结果；公开用法见[使用 · 布局](../../使用/布局.md)。
>
> **当前实现线索**：相关实现暂位于 `src/ui/layout/` 和 `src/ui/traits/layout.rs`；重构后 trait 与算法共同归本模块。

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

## 组件：FlexLayout / GridLayout / ScrollView

- Flex 支持主轴方向、wrap、grow/shrink、justify、align、gap 和 per-child `align_self`；`overflow_content` 只保留自然主轴尺寸，不取消分布、交叉轴对齐或反向语义，固有反向主轴按自然内容长度镜像，零交叉轴由自然外尺寸 bootstrap。
- Grid 使用显式 column/row track、cell/span 与独立 row/column gap；空 track 或空 child 返回有限空输出，per-child `align_self` 覆盖容器级交叉轴对齐。
- Grid 先确定 `Px` 与基于子项有限测量外尺寸的 `Auto`，再让正权重 `Fr` 按比例分配剩余空间；纯 `Auto` 轨道不为填满容器而膨胀。
- 跨多轨道子项在 span 不含正权重 `Fr` 时，先扣除 span 内部 gap、`Px` 与已知 `Auto` 尺寸，再把未覆盖的外尺寸缺口均分给所覆盖的 `Auto` 轨道。
- Grid 放置层以单轴 4096 条轨道和总计 65,536 个稠密单元格同时限制辅助分配；超限 cell/span 收敛到窗口边界，无可用矩形的自动子项留在零 frame。
- 自动放置使用可复用的二维占用数前缀和，候选矩形查询为常数时间；每个子项至多在当前矩阵和一次有界扩行后各搜索一次，不保留无限增长循环。
- `Container` 与 `Space` 共享子 frame 内容外尺寸计算：Space 必须传递子项 margin，缓存取可见 frame 末端并补入正右/下 margin，不用父级受限的求解器总尺寸冒充真实内容范围。
- `ScrollView` 在纵向/双向纵列与横向单行中统一消费子项 margin，滚动条首轮判断使用自然外尺寸，非滚动轴填充先扣两侧 margin；双轴 `content_bounds` 补入对侧经典沟槽，使 `max_scroll` 仍按外视口相减却等价于真实内容视口。
- 容器组件组合 `FlexLayout`/`GridLayout`，不复制第二套算法。

## 增量与缓存

- 只有尺寸约束、布局属性、子结构、文本度量或可见性变化才标 Layout dirty。
- Paint-only 主题色、hover、opacity 动画不触发布局；width/height 等几何动画触发布局。
- 布局 scratch 与输出快照可按窗口复用，但树版本、约束或相关属性变化时必须失效。
- 虚拟滚动只在物化范围变化时 reconcile 行子树；稳定范围的滚动走 composite/paint 路径。

## 不变量

- 结果尺寸和 frame 有限、非负；无界轴使用约束语义，不能把 `f32::MAX` 写入实际 frame。
- measure、paint、hit-test 对同一组件使用同一实际 frame。
- 布局不执行业务 I/O、任意应用 callback、present 或跨窗 wake。

## 当前审计证据

- `7de6fcd8` 在共享 BoxModel/Flex/Grid 边界归一实际 frame、总尺寸、子测量尺寸、margin、border/padding、gap 与弹性因子；有限负坐标和 margin 继续保留，负尺寸与非有限值收敛为零，`f32::MAX` 不再物化到公开布局输出。
- `tests/layout_invariants.rs` 覆盖盒模型 content rect、空 Flex/Grid、病理 Flex/Grid 输入和正常几何稳定性。修复前四个病理用例均失败，修复后五项全部通过；正常 Flex/Grid 子 frame 与总尺寸使用精确值断言，防止安全归一化改变有效输入语义。
- 共享入口没有为首批归一化复制 Grid track；纯求解器继续借用 columns，并只沿用生成 implicit rows 所需的既有行缓冲。
- `7e03fe41` 把 Flex min/max 前移到分行与弹性分配之前，并以有界、无额外分配的迭代重新分配触顶/触底后的剩余 grow、shrink 与 Stretch 空间；wrapped bootstrap、固有主轴、反向镜像、非对称交叉轴 margin 及单行/多行总尺寸共用落实后的几何账本。
- 第二批六个聚焦断言在修复前失败；修复后 `tests/layout_invariants.rs` 9/9、内部 Flex 边界 4/4、库测试 101/101，公开 API 与使用门面回归通过。
- `33721e4a` 将 Grid `Auto` 从等权弹性轨道收敛为内容轨道，在列/行轴均统计测量尺寸与 margin，并为不含正权重 `Fr` 的 span 分摊未覆盖尺寸；修复前三个列轴聚焦断言失败，修复后公开布局契约 13/13、库测试 101/101、公开 API 2/2、使用门面 5/5，无默认特性库与全特性全目标检查均为 0 错误。
- `f2ccba05` 把 Grid 放置拆入独立有界模块，统一处理轨道/单元格预算、整数极值、显式重叠、自动密集回填与窗口耗尽；两个 5000 行级安全断言在修复前失败，修复后公开布局契约 16/16、内部放置边界 6/6（含 3×3 全占用状态和 span 的 4608 组穷举对照）、库测试 107/107、公开 API 2/2、使用门面 5/5，两套特性组合检查均为 0 错误。
- `40e66b65` 恢复 Grid 子项 `align_self` 到单元格求解器的传递，并让 Flex 溢出路径复用标准主轴分布、扣除交叉轴两侧 margin、先正向放置再沿容器主轴镜像；四个聚焦断言在修复前失败，修复后公开布局契约 20/20、库测试 107/107、公开 API 2/2、使用门面 5/5，两套特性组合检查均为 0 错误。
- `e7ad9d55` 让固有反向溢出按自然主轴镜像、零交叉轴 Stretch 保留自然外尺寸，并统一 Container/Space 的 margin 传递和内容缓存；两个公开与两个组件聚焦断言在修复前失败，修复后公开布局契约 22/22、组件缓存 2/2、库测试 109/109、公开 API 2/2、使用门面 5/5，两套特性组合检查均为 0 错误。
- `b5c4f152` 让 ScrollView 的放置、滚动条判断和内容范围共同消费子项 margin，并把对侧沟槽纳入双轴滚动坐标范围，同时在组件边界复用共享有限几何归一规则；三个聚焦断言修复前失败，修复后内部 ScrollView 4/4、公开布局契约 22/22、库测试 113/113、公开 API 2/2、使用门面 5/5、使用示例 11/11，两套特性组合检查均为 0 错误。
- 现有证据仍不替代 Grid 其余 justify/span 组合、Flex overflow 与 wrap 组合、其他组件 measure/paint/hit-test 一致性、增量缓存或真窗视觉矩阵。
