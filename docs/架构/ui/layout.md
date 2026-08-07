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

本轮门禁由适配器精细分类、保守回退和 `Unknown`/`Custom` 兜底测试共同覆盖；以后新增 `SnapshotFields` 变体默认不会静默丢失布局失效，但必须在独立审计后才能晋升为精细分类。

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
- `5b45c0d9` 以绝对索引后备 key 和 keyed reconcile 替换 VirtualScroll 的破坏性 `set_children`，让窗口重叠行与 renderer 声明更新保留组件身份；`cca439f9` 再为非法度量、超限 offset、总高度、滚动增量和最终行 frame 建立有限值规则，并以 4096 行硬预算限制 viewport/overscan。三项身份门禁与四项病理资源门禁修复前失败，修复后内部 VirtualScroll 8/8、公开布局契约 22/22、库测试 121/121、公开 API 2/2、使用门面 5/5、使用示例 11/11，两套特性组合检查均为 0 错误，文档测试 47 通过/23 忽略。
- `32a5cbf6` 将 `ViewAdapter` 的组件 patch 结果拆为 Paint/Layout 双通道，只抓取一次新旧公开快照，并为 `Label`、`Container`、`Grid` 区分颜色、背景等纯绘制字段与文本、盒模型、Flex/Grid 轨道等几何字段；未分类组件保持保守 Layout。两个纯视觉门禁修复前均错误产生 Layout，修复后协调分类 6/6、库测试 127/127、布局与公开门面 40/40，两套特性组合检查均为 0 错误，文档测试 47 通过/23 忽略。
- `8530d29` 在建树、移除与声明重排三个直接子结构入口统一发送 `on_children_changed`，让 Container/Space/Affix 清除空内容尺寸、ScrollView 清除范围/偏移/滑块交互并回写受控零值、Carousel 清除旧数量且排除自定义箭头；同时确认 ProviderContext 继续保守 Layout、布局 scratch 每轮清空、遍历缓存受树版本约束且当前没有跨帧布局输出快照。新增六项结构状态门禁后适配器 12/12、库测试 133/133、集成测试 91 通过/1 忽略，两套特性检查均为 0 错误，文档测试 47 通过/23 忽略。
- `b7f7e5c7` 让单行溢出、标准固有主轴与溢出换行统一遵守实际 frame 镜像边界，并把 `overflow_content + wrap` 接回共享 Flex 分行器，同时冻结 grow/shrink 与 Stretch 增长。三个聚焦断言修复前分别产生负坐标或未换行，修复后布局契约 25/25、库测试 133/133、公开 API/使用门面/使用示例 18/18，两套特性组合检查均成功。
- `b1aefee` 将 Grid 跨 `Auto` 轨道贡献按 span 升序分批，同跨度子项先从统一快照计算每条轨道的最大计划增量，再整批写回，消除重叠 span 的声明顺序依赖。聚焦断言修复前正序/反序的末列起点分别为 125px/100px，修复后布局契约 26/26、库测试 133/133，两套特性组合检查均成功。
- `9d86578c` 为 Grid 的 `Auto + Fr` 混合 span 增加父约束内的份额转移：当 span 外 Fr 竞争把七十像素自然宽度裁成六十像素时，按内外 Fr 权重扩张 span 内 Auto 并同步缩减全局 Fr 余量，修复后跨轨宽度与末轨起点均为 70px。聚焦断言修复前失败，修复后布局契约 27/27、库测试 133/133，两套特性组合检查均成功。
- `d81dbe5b` 把公开 `Grid::justify` 从错误的单元格内子项接线拆为整组列轨内容对齐，并保留低层 `with_justify` 的既有逐项语义；修复前 Center 在百像素容器内把首项放到 5px，修复后六种固定轨道分布模式、Auto-only Stretch 与双通道兼容契约全部通过。公开布局契约 28/28、库测试 135/135、文档测试 47 通过/23 忽略，两套特性组合检查均成功。
- `41158b3f` 固化 Grid 不可转移的 Fr span 容量边界：`Auto + Fr` 覆盖全部 Fr 时，一百二十像素自然宽度收敛到一百像素父级；纯 Fr 的列/行 span 都保持三条等权轨道各三十像素，不把八十像素自然尺寸变成隐式轨道最小值。新增三项契约直接通过，布局契约 31/31、库测试 135/135、文档测试 47 通过/23 忽略，两套特性组合检查均成功。
- `518adc67` 恢复 Flex 负剩余主轴空间的公开分布语义：单行 `overflow_content`、标准 `intrinsic_main` 与换行溢出的超宽行不再把负差值提前钳零，统一由既有 justify 分布器计算 Center/End 偏移；bootstrap 继续固定为零偏移。三项聚焦断言修复前均退化为 Start，修复后布局契约 34/34、库测试 135/135、文档测试 47 通过/23 忽略，两套特性组合检查均成功。
- `6fe836a4` 让 wrapped 默认 Stretch 消费实际交叉轴正剩余空间：单行扣除 margin 后填满容器，多行保留固定 gap 后均分扩展行高，逐项 `align_self` 继续只覆盖项内对齐；同时把 1117 行的布局契约拆为 804 行基础目标与 360 行 Flex 高阶目标。两项聚焦断言修复前分别停在 10px 自然高与自然行起点，修复后两目标合计 36/36、库测试 135/135、文档测试 47 通过/23 忽略，两套特性组合检查均成功。
- `952912e4` 将 wrapped 分行的“当前行非空”判据从累计主轴占用改为项目索引范围：零宽首项不再让后续满宽项连同 gap 错误留在同一行。聚焦断言修复前把第二项放在首行 `x=9px`，修复后移动到第二行 `y=19px`；两目标合计 37/37、库测试 135/135、文档测试 47 通过/23 忽略，两套特性组合检查均成功。
- `bdccced7` 让单行与 wrapped 的交叉轴 Stretch 最终候选统一服从子项 min/max：过小容器不再压破最小值，宽裕容器或扩展行盒也不再拉破最大值。两个内部聚焦门禁修复前分别得到 10px 而非 20px 的单行最小高度、20px 而非 12px 的换行子项高度；修复后内部 Flex 6/6、两项新增门禁 2/2、公开布局契约 37/37、库测试 137/137，两套特性组合检查均成功。
- `9cd3ae5a` 恢复 wrapped 行组在交叉轴负剩余空间下的 End 对齐：两行与固定 gap 形成 25px 自然高度、实际容器只有 15px 时，行组整体向起点外偏移 10px，使末行底边继续贴住容器末端。聚焦契约修复前首行错误停在 `y=6px`，修复后位于 `y=-4px`；公开布局契约 38/38、库测试 137/137，两套特性组合检查均成功。
- `af35d11b` 把 wrapped 行组的交叉轴 Center/End 偏移限制在获得实际交叉轴之后：零高度 bootstrap 继续按两行与固定 gap 形成的 25px 自然高度从原点排列，不围绕零尺寸范围生成负坐标。聚焦契约修复前 Center 把首行从 `y=6px` 移到 `y=-6.5px`，修复后 Center/End 都从 `y=6px` 开始；公开布局契约 39/39、库测试 137/137，两套特性组合检查均成功。
- `382dd1df` 修正公开 `FlexLayout` 空子集早返回的主轴账本：固定尺寸空布局保持父级 frame；`intrinsic_main` 与 `overflow_content` 按方向只折叠自然主轴并保留交叉轴。聚焦契约修复前水平固有布局把空内容记为 100×20px，修复后水平、垂直与溢出入口分别得到 0×20px、100×0px 与 0×20px；公开布局契约 40/40、库测试 137/137，两套特性组合检查均成功。
- `d91d9831` 把 RichText 的有效宽度纳入估算测量缓存键，并在宽度重排时使旧绘制/命中几何失效。聚焦契约先以 400px 宽建立 21px 高缓存，再缩到 80px；修复前仍错误返回 21px，修复后重新折行并增加固有高度。公开布局契约 41/41、库测试 137/137，两套特性组合检查均成功。
- `646f33de` 修复 Notification 在窄窗口中先保留 384px 固定宽度、导致 TopRight 条目贴住左边界的问题。400px frame 的聚焦契约修复前得到 `x=0,w=384`，修复后双侧各保留 24px 并得到 `x=24,w=352`；同一契约同时确认动画后命中边界复用该矩形。组件聚焦契约 1/1、库测试 138/138，两套特性组合检查均成功。
- `b14313ca` 修复 Table 固定列重叠时绘制层级与命中优先级分叉的问题。100px 视口中左右固定列各宽 80px，`x=50px` 同时落入两区；修复前返回被遮挡的左列索引 0，修复后按共享绘制层级的逆序返回最上层右列索引 1。几何聚焦契约 1/1、库测试 139/139，两套特性组合检查均成功。
- `d51d5b0e` 修复 Table 列宽调整句柄在固定区重叠边缘按全局距离选择、未遵循绘制层级的问题。100px 视口中左固定列宽 80px、右固定区两列各宽 20px，左列与右内列的调整线都落在 `x=80px`；修复前选择被遮挡的左列索引 0，修复后先按 Right → Left → Middle 分层，再在层内比较距离，返回最上层右内列索引 1。交互几何契约 1/1、库测试 140/140，两套特性组合检查均成功。
- `cd4e9a95` 修复 Table 选择动作在右固定列覆盖 32px 选择区时仍无条件优先的问题。100px 视口中 90px 右固定列覆盖 `x=10..32px`；修复前表头 `x=20px` 返回 `ToggleAll`，修复后返回可见右列的 `SortColumn(0)`，同点表体返回 `SelectRow(0)`，未覆盖的 `x=5px` 仍返回 `ToggleAll`。交互契约 1/1、库测试 141/141，两套特性组合检查均成功。
- `52f99f94` 修复 Table 展开控件最后绘制却在选择列之后解析动作的问题。24px 视口让行末 32px 展开交互区覆盖选择列；修复前表体 `x=10px` 返回 `ToggleRow(0)`，修复后先解析真实物理行并返回最上层 `ToggleExpand(0)`，表头同点仍返回 `ToggleAll`。相邻层级交互契约 2/2、库测试 142/142，两套特性组合检查均成功。
- `625b0c31` 修复 Table 有标题分组片段按声明顺序覆盖全局固定区层级的问题。100px 视口中先声明 80px 右固定组、后声明 80px 左固定组，修复前绘制访问顺序为 Right → Left，修复后按共享层级改为 Left → Right，重叠点 `x=50px` 命中仍为最后绘制的右固定列。绘制层级契约 1/1、库测试 143/143，两套特性组合检查均成功。
- `a4dada8f` 修复 Table 跨两层无标题单列在全局叶阶段覆盖更高层固定分组标题的问题。100px 视口中先声明 80px 右固定组、后声明 80px 左固定单列，`x=50px,y=10px` 的片段顺序修复前为“右分组 → 左单列”，修复后按列区交错分组/叶阶段变为“左单列 → 右分组”，同点列命中仍为右固定列索引 0。表头层级契约 2/2、库测试 144/144，两套特性组合检查均成功。
- `d4999ab3` 修复 Table 左固定行合并锚点在全部普通行之后重绘、反向覆盖右固定列的问题。100px 视口中左右固定列各宽 80px，普通左区裁剪仍为 `x=0..80px`；合并重绘裁剪修复前也错误保留 80px，修复后只保留右固定区起点之前的 `x=0..20px`，右区继续完整覆盖 `x=20..100px`，重叠点 `x=50px` 命中右列索引 1。几何层级契约 2/2、库测试 145/145，两套特性组合检查均成功。
- `3348a7f4` 修复 Table 普通文本列跨固定区合并时按锚点方向累加宽度、漏掉反向物理列的问题。100px 视口中先声明 60px 右固定锚点、后声明 40px 左固定覆盖列，旧矩形为 `x=40,w=100`，修复后联合范围为 `x=0,w=100`，末尾重绘分别得到左片段 `x=0..40px` 与右片段 `x=40..100px`；视觉左侧 `x=10px` 命中列索引 1 后仍回落到锚点 `(0,0)`。Table 聚焦测试 12/12、库测试 147/147，两套特性组合检查均成功。
- `5a3c0c1f` 将同一跨固定区几何扩展到 Table 自定义 View 单元格：子树按 `x=0,w=100` 的完整逻辑跨度只布局一次，节点保存左区 `x=0..40px` 与右区 `x=40..100px` 两个父级片段；LayerTree 在节点自身变换前逐片重放同一子树，空片段跳过整棵子树，Picture 提升对带片段节点关闭。2D 命中、指针捕获与可见区域沿祖先链消费同一片段集合，片段间隙回落到父节点。回归契约 2/2、Table 14/14、库测试 149/149，两套特性组合检查均成功。
- `09354738` 统一 wrapped 未分行与 non-wrap 的真实交叉轴行盒：八像素高容器中，带上下各两像素 margin 的逐项 Stretch 子项从错误的 10px 自然高收敛为 4px，逐项 End 子项从错误的 `y=17px` 回到 `y=11px`。失败契约修复后布局契约 24/24、Flex 高阶契约 18/18、内部 Flex 6/6、库测试 149/149、文档测试 47 通过/23 忽略，两套特性组合检查均成功；`flex.rs` 同步由 899 行降至 881 行。
- `89bc5708` 修正单行 `overflow_content` 反向零主轴 bootstrap 对 `intrinsic_main` 的额外依赖：公开引擎直接使用自然内容总长作为镜像边界，非零实际 frame 仍使用容器主轴。失败契约修复后聚焦溢出 18/18、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `144b3224` 以 336 个组合确认未实际分行的 wrapped 溢出与流式溢出完全等价：四种方向、七种 justify、四种 align 分别覆盖实际 frame、零主轴与零交叉轴，且两个子项带不同自然尺寸和四侧非对称 margin；矩阵首轮直接通过，无需改写生产求解器。聚焦溢出 19/19、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `ce51b1fe` 以 168 个组合确认实际分行的 wrapped 溢出保持轴转置对称：Row/Column 与 RowReverse/ColumnReverse 两组方向，覆盖七种 justify、四种 align、交叉轴宽裕/溢出/零尺寸 bootstrap，以及四个不同自然尺寸、非对称 margin 和逐项 Start/End/Stretch 的子项；矩阵首轮直接通过，无需改写生产求解器。聚焦溢出 20/20、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `d5db70cf` 以 168 个组合确认实际三行 wrapped 溢出的正反向物理镜像：Row/RowReverse 与 Column/ColumnReverse 两组方向覆盖七种 justify、四种 align、交叉轴宽裕/溢出/零尺寸 bootstrap；输入包含四个不同自然尺寸、非对称 margin 和逐项 Start/End/Stretch，反向输入交换主轴起止 margin 后，每个矩形均围绕真实容器主轴精确镜像且总尺寸不变。矩阵首轮直接通过，无需改写生产求解器；新增镜像契约 1/1、聚焦溢出 20/20、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `fe2acc63` 以 336 个组合确认实际三行 wrapped 溢出冻结子项弹性：四种方向、七种 justify、四种 align 与交叉轴宽裕/溢出/零尺寸 bootstrap，比较零弹性基准和分别含普通正值、大权重、`f32::MAX`、负值、正无穷及 NaN grow/shrink 的输入；分行、全部矩形与总尺寸首轮完全相同，无需改写生产求解器。新增冻结契约 1/1、镜像契约 1/1、聚焦溢出 20/20、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `c7586072` 修正实际多行负 gap 的自然交叉轴账本：旧实现只以最终行游标减去 gap 得到总量，30px 高首行与 5px 高末行重叠 8px 时误报 27px，漏掉首行最后 3px；现改为取全部行盒有限物理末端的最大值。水平与垂直失败契约修复后 2/2，通过弹性冻结 1/1、镜像 1/1、聚焦溢出 20/20、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `98bc0670` 修正实际多行负尾侧交叉轴 margin 的可见内容账本：行盒仍按外尺寸推进，但最终交叉轴总量额外覆盖全部已放置子项矩形的有限末端。30px 高/宽首项带 -10px 下/右 margin、5px 末行/末列按 20px 行盒推进时，旧结果误报 25px，现两轴均保持 30px；失败契约 2/2，负 gap 2/2、弹性冻结 1/1、镜像 1/1、聚焦溢出 20/20、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- `5b418da0` 修正实际多行负尾侧主轴 margin 的可见内容账本：分行仍按包含 margin 的自然占用推进，同时用独立于 justify 偏移与反向物理坐标的逻辑游标累计每行最远可见末端。30px 宽/高首项带 -10px 右/下 margin 时自然占用为 20px，后续 10px 子项在 25px frame 中换行/换列；旧主轴总量误报 20px，现两轴均保持 30px。失败契约 2/2，负交叉轴 margin 2/2、负 gap 2/2、弹性冻结 1/1、镜像 1/1、聚焦溢出 20/20、布局 24/24、库测试 205/205，两套特性组合与文档测试均成功。
- Flex 实际分行 overflow/wrap/固有轴中负 margin/gap 的联合矩阵、轴转置、正反向镜像与弹性冻结矩阵，及表格其他合并组合的 measure/paint/hit-test 一致性已有独立门禁；当前剩余审计为可变行高缓存与真窗视觉矩阵。
