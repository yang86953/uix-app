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

## 组件：FlexLayout / GridLayout / ScrollView / VirtualScroll

- Flex 支持主轴方向、wrap、grow/shrink、justify、align、gap 和 per-child `align_self`；`overflow_content` 只保留自然主轴尺寸，不取消分布、换行、交叉轴对齐或反向语义。单行溢出、标准固有主轴与超宽换行在获得真实 frame 后都把有限负剩余空间交给统一 justify：Center/End 分别使用半量/全量负偏移，Start 与 Space* 不产生负分布；零尺寸 bootstrap 仍从自然起点开始。wrapped 路径按当前行是否已有项目决定分行，而不是以累计占用是否大于零代替项目存在性；零主轴尺寸子项仍参与 gap 与换行边界。自然行盒形成后，默认 Stretch 再把实际交叉轴正剩余空间等分到各行并同步行起点；只有一行时与 non-wrap 填充语义一致，`align_self` 只覆盖项内对齐。单行和 wrapped 的 Stretch 最终尺寸统一经子项交叉轴 min/max 钳制；行盒可继续消费剩余空间，达到上限的子项保持行起点。非 Stretch 行组在实际交叉轴溢出时保留有限负剩余空间，Center 使用半量负偏移，End 使用全量负偏移使行组末端贴住容器末端；交叉轴 bootstrap 没有可分布的实际范围，两者都固定零偏移并由自然行组撑开输出。单行 `overflow_content` 的反向主轴在零实际主轴 bootstrap 时必须按自然内容长度镜像，不得依赖调用方另设 `intrinsic_main`；获得非零实际 frame 后与 justify 共用实际容器主轴。零交叉轴由自然外尺寸 bootstrap。空子集的固定尺寸输出保留完整 frame；固有主轴或溢出模式按零个自然子项把主轴收敛为零，交叉轴继续使用父级分配值，四种方向只决定折叠宽度或高度。
- wrapped 求解最终只有一行且已获得真实交叉轴时，唯一行盒直接覆盖容器交叉尺寸，禁止自然行高在较小容器内绕过 Stretch 收缩，也让逐项 `align_self` 相对真实行盒定位；自然交叉外尺寸继续单独保留在 `total_size` 账本。`overflow_content` 没有实际分行时，wrapped 与流式溢出路径必须在四种方向、全部公开 justify/align、非对称 margin 和两轴 bootstrap 下返回相同子项矩形与自然尺寸账本。实际多行仍按自然行高、固定 gap 与容器级行组分布处理。主轴 justify 纯计算已拆入 `flex/justify.rs`，共享入口与公开 API 不变，核心求解文件保持在 900 行门禁内。
- Grid 使用显式 column/row track、cell/span 与独立 row/column gap；空 track 或空 child 返回有限空输出，per-child `align_self` 覆盖容器级交叉轴对齐。
- Grid 先确定 `Px` 与基于子项有限测量外尺寸的 `Auto`，再让正权重 `Fr` 按比例分配剩余空间；纯 `Auto` 轨道不为填满容器而膨胀。
- Grid 水平内容对齐与单元格内子项对齐使用独立通道：`Grid::justify` / `GridLayout::with_content_justify` 在轨道解析后移动或分散整组列轨，`GridLayout::with_justify` 继续只控制子项在单元格内的位置。Center/End 分配前置剩余空间，SpaceBetween/SpaceAround/SpaceEvenly 只增加轨道间或两端分布空间，Stretch 只均分扩展 `Auto` 列；没有正剩余空间或可扩展 `Auto` 时保持 Start 几何。
- 跨多轨道子项在 span 不含正权重 `Fr` 时，先扣除 span 内部 gap、`Px` 与已知 `Auto` 尺寸，再把未覆盖的外尺寸缺口均分给所覆盖的 `Auto` 轨道；较短 span 先结算，同跨度重叠子项基于同一轨道快照登记每轨最大计划增量，子项声明顺序不得改变轨道尺寸。
- 子项同时跨越 `Auto` 与部分正权重 `Fr`、且 span 外仍有竞争 `Fr` 时，在父级轨道容量内按全局 Fr 权重把 span 外可让出的份额转入 span 内 `Auto`；约束按 span、起点与自然外尺寸确定性排序，并以最多 64 轮单调松弛处理相互影响。没有 `Auto` 时，列/行轴都保持声明的 Fr 权重并把子项收敛到单元格；span 覆盖全部 Fr 或父级剩余空间耗尽时同样服从父级容量，不伪造额外轨道尺寸。
- Grid 放置层以单轴 4096 条轨道和总计 65,536 个稠密单元格同时限制辅助分配；超限 cell/span 收敛到窗口边界，无可用矩形的自动子项留在零 frame。
- 自动放置使用可复用的二维占用数前缀和，候选矩形查询为常数时间；每个子项至多在当前矩阵和一次有界扩行后各搜索一次，不保留无限增长循环。
- RichText 的估算测量缓存以有效宽度约束为键；宽度变化会重算折行固有高度、清除旧行坐标与代码复制命中区域，并让绘制阶段在同一宽度下以真实字体度量重建缓存。测量与绘制可使用不同度量精度，但不得跨宽度复用旧高度或旧命中几何。
- Notification 的条目宽度先从规范化窗口 frame 扣除双侧 `HORIZONTAL_INSET`，再受 384px 设计上限约束；`notification_rects` 是绘制、动画后命中、Overlay bounds 与 dirty bounds 的共同几何来源，窄窗口不得为保留固定条目宽度而牺牲单侧留白。
- Table 的选择列先绘制，随后以 `COLUMN_PAINT_ORDER` 统一表头、分组表头和表体的列区层级：Middle → Left → Right；两层表头必须以列区为最外层，并在每个列区内依次完成有标题分组与叶表头阶段，跨两层无标题单列不得因处于全局叶阶段而覆盖更高层固定区。有标题分组片段在同一列区内保持声明顺序。跨行合并锚点在全部物理行之后补绘时，必须使用扣除更高列区覆盖后的最终可见裁剪；普通按层绘制仍使用完整列区裁剪。逻辑列合并不能用“锚点 x + 声明宽度之和”推导物理矩形：`span_bounds` 必须联合全部覆盖列的真实位置，末尾重绘再用 `merged_span_repaint_clip_for` 按列区提取实际覆盖片段并保留固定区层级。普通文本在统一逻辑矩形中绘制并由各片段裁剪，覆盖列命中继续由 `cell_anchor` 回落到唯一锚点。合并单元格覆绘后，物理行展开控件最后绘制。`column_at`、选择动作、展开动作与列宽调整句柄必须以视觉层级决定命中：固定列侵入 32px 选择区时，只有 `column_at` 返回空才允许复选框动作；真实数据行的尾部 32px 展开交互区优先于选择列和普通单元格，表头与展开内容区除外；调整句柄按 Right → Left → Middle 分层且仅在同层按距离择优。
- `Container` 与 `Space` 共享子 frame 内容外尺寸计算：Space 必须传递子项 margin，缓存取可见 frame 末端并补入正右/下 margin，不用父级受限的求解器总尺寸冒充真实内容范围。
- `ScrollView` 在纵向/双向纵列与横向单行中统一消费子项 margin，滚动条首轮判断使用自然外尺寸，非滚动轴填充先扣两侧 margin；双轴 `content_bounds` 补入对侧经典沟槽，使 `max_scroll` 仍按外视口相减却等价于真实内容视口。
- `VirtualScroll` 只接受有限正行高与 viewport 参与范围计算，offset 先夹到内容边界；每次最多物化 4096 行，可见行优先于 overscan，总高度、滚动状态与最终行 frame 均保持有限。
- 容器组件组合 `FlexLayout`/`GridLayout`，不复制第二套算法。

## 增量与缓存

- 只有尺寸约束、布局属性、子结构、文本度量或可见性变化才标 Layout dirty。
- Paint-only 主题色、hover、opacity 动画不触发布局；width/height 等几何动画触发布局。
- 声明树原位协调分别报告 Paint 与 Layout 影响；`Label`、`Container`、`Grid` 按快照字段分类，未审计组件及 `ProviderContext` 变化继续保守请求 Layout，不能以优化名义吞掉未知几何变化。
- 直接子节点增加、移除或重排后，树通过组件核心通知同步依赖子结构的派生状态；空子树不能依赖 `layout_children([])` 清理，因为 Phase 1 会跳过没有直接子节点的节点。
- `LayoutFrameScratch` / `LayoutTraversalScratch` 只跨帧复用存储，进入布局即清空本轮向量与集合；遍历缓存以 `tree_version` 为键，当前核心不持有跨帧 `LayoutOutput` 语义快照。
- 虚拟滚动在范围与挂载数量稳定且没有新版声明时不调用 renderer 或 reconcile，滚动走 composite/paint 路径；范围或 renderer 声明变化时，以业务 key 或绝对索引后备 key 协调当前有界窗口，重叠行保留原组件身份。

### 失效分类矩阵

`ViewAdapter` 对声明更新统一先比较公开 `SnapshotFields`，再把结果拆为 Paint 与 Layout 两条通道。当前分类分为三层：

| 分类 | 组件 | 规则 |
|---|---|---|
| 精细字段 | `Label`、`Container`、`Grid` | 颜色、背景等纯视觉字段只产生 Paint；文本、字号、盒模型、Flex/Grid 轨道、可见性和响应式列等几何字段产生 Layout + Paint。 |
| 配置与运行态分离 | `Input`、`Collapse`、`Carousel`、`ImageGroup` | `Input` 的受控运行值、以及其余组件快照中明确标为运行态的字段不参与 authored config 比较；配置变化仍按保守 Layout 处理，避免漏掉未知几何。 |
| 显式保守 | 其余全部内建快照、`Unknown`、`Custom` | `builtin_widget_layout_changed` 的兜底为 `Some(true)`，任何配置差异都产生 Layout + Paint；新快照变体在获得独立审计前自动落入此类。 |

显式保守清单覆盖：`Button`、`WindowControl`、`Space`、`Divider`、`Icon`、`Typography`、`Checkbox`、`Radio`、`Switch`、`Slider`、`RangeSlider`、`Rate`、`InputNumber`、`Avatar`、`Badge`、`Card`、`Empty`、`Image`、`Tag`、`Timeline`、`Calendar`、`Skeleton`、`FloatButton`、`FloatButtonGroup`、`Layout`、`Header`、`Sider`、`Content`、`Footer`、`Splitter`、`Affix`、`BackTop`、`List`、`Select`、`AutoComplete`、`Cascader`、`ColorPicker`、`DatePicker`、`DateRangePicker`、`TimePicker`、`Mentions`、`Segmented`、`FormItem`、`Form`、`Descriptions`、`Result`、`SelectableList`、`ScrollView`、`ThemeToggle`、`Transfer`、`Upload`、`Watermark`，以及 feature-gated 的 `Alert`、`Message`、`Notification`、`ProgressBar`、`Spin`、`Tooltip`、`Popover`、`Popconfirm`、`Modal`、`Drawer`、`Breadcrumb`、`Pagination`、`Anchor`、`Menu`、`Dropdown`、`Tabs`、`Steps`、`NavItem`、`Tree`、`TreeSelect`、`Table`、`BarChart`、`LineChart`、`PieChart`、`ChartPlaceholder`、`QRCode`、`RichText`。

适配器精细分类、保守回退和 `Unknown`/`Custom` 兜底由测试共同覆盖；新增 `SnapshotFields` 变体默认不会静默丢失布局失效，补齐对应测试后才能晋升为精细分类。

## 不变量

- 结果尺寸和 frame 有限、非负；无界轴使用约束语义，不能把 `f32::MAX` 写入实际 frame。
- measure、paint、hit-test 对同一组件使用同一实际 frame。
- 布局不执行业务 I/O、任意应用 callback、present 或跨窗 wake。

## 测试

- `tests/layout_invariants.rs` 覆盖盒模型、Flex、Grid、ScrollView、VirtualScroll、RichText、Notification 与 Table 的布局不变量。
- `tests/flex_overflow_invariants.rs` 与 `tests/flex_overflow_flexibility.rs` 覆盖溢出、换行、轴转置、镜像、负 margin/gap 和弹性冻结组合。
- 组件内部测试覆盖缓存失效、子结构协调、绘制与命中一致性，以及病理数值的有界处理。
- 本模块只以自动测试结果判断完成状态，不维护独立审计、截图证据或人工视觉矩阵。
