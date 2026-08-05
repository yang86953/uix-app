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

## 组件：FlexLayout / GridLayout

- Flex 支持主轴方向、wrap、grow/shrink、justify、align、gap 和 per-child `align_self`。
- Grid 使用显式 column/row track、cell/span 与独立 row/column gap；空 track 或空 child 返回有限空输出。
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
- 第二批六个聚焦断言在修复前失败；修复后 `tests/layout_invariants.rs` 9/9、内部 Flex 边界 4/4、库测试 101/101，公开 API 与使用门面回归通过。本证据仍不替代 Grid 与 Flex 其余组合、measure/paint/hit-test 一致性、增量缓存或真窗视觉矩阵。
