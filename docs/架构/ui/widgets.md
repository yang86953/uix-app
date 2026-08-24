# widgets 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的内置通用组件库及组件分类边界。组件运行时、布局、事件、主题、浮层、表单和虚拟化能力均由 ui System 通过公开/私有契约编排，widgets 不直接持有兄弟 Module 实例。导出：`uix::ui` 与 prelude 中的组件类型。
>
> **当前实现线索**：组件当前主要位于 `src/ui/widgets/`；重构允许按产品域重排文件，但所有内置组件仍遵循本模块契约。

## 组件清单

| 分类 | 代表组件 | 架构职责 |
|---|---|---|
| `general` | Button、Label、Typography、Icon、Space | 基础显示与操作 |
| `containers` | Container、Grid、Splitter、Affix | 子树布局 |
| `input` | Input、Select、Form、Slider、Picker | 受控/非受控输入与校验适配 |
| `display` | Table、Tree、List、Image、Calendar | 数据展示与虚拟化 |
| `feedback` | Modal、Drawer、Message、Notification、Tooltip | 浮层和状态反馈 |
| `navigation` | Menu、Tabs、Pagination、Steps | 导航语义 |
| `other` | Chart、RichText、ScrollView | 复合能力 |

## 全部组件的 uix-lang 迁移契约

所有公开 widget 均须逐批改为 uix-lang 声明，不再保留基础控件例外。UIX 必须拥有可声明的视觉结构与静态视觉配置，包括子树、排列、尺寸、间距、图标、展示文案和主题色角色；仅把 `kernel` 原样放进 `KernelView` 不算完成视觉迁移。Rust 可以继续实现不直接替代公开组件的基础内核，例如平台窗口动作、overlay host、虚拟化、资源加载、文本整形、图表绘制与复杂布局求解；这些内核只拥有状态、事件、算法、I/O、生命周期、平台交互，以及 UIX 当前无法表达的底层栅格化与几何执行。

每个 widget 必须拥有独立目录，Rust 模块入口与 `.uix` 源文件放在同一目录；禁止恢复共享 `src/ui/widgets/uix/` 目录。大型组件可在自己的目录内继续拆分 Rust 子模块，但不得把另一 widget 的实现混入该目录。

迁移完成必须同时满足以下条件：

1. `.uix` 文件拥有公开组件的子树结构、顺序、条件分支、插槽投影、静态样式和展示文案。
2. Rust 只保留 UIX 无法安全表达的基础内核职责；业务逻辑、持久化、异步 I/O 与平台调用仍不得进入 `.uix`。
3. Rust 基础 View 需要进入声明树时，只能通过框架内部的 `KernelView value={...}`、单展示根 `KernelHost value={...}` 或拥有型节点列表 `KernelChildren value={...}` 窄桥接；桥接禁止事件与组件专有属性，单 View 桥接只允许统一 View 样式，列表桥接不接受任何展示属性，不能成为第二套公共组件 API。
4. 迁移必须保留公开类型、事件、无障碍、状态协调、稳定 key、布局和视觉结果；只用外部消费者测试公开编译入口与可观察行为。
5. 条件隐藏的分支不得提前构造 Rust 基础 View；高频路径不得因语言迁移增加无条件分配、克隆或动态解析。
6. 单 `KernelView` 组件必须由 UIX 显式向 Rust 内核注入静态视觉配置；只传 `kernel` 或 `kernel, children` 的透传壳属于未完成债务。债务清单由本文与源码审查维护，不为目录、声明位置或私有桥接结构建立测试。
7. `.uix` 是全部静态视觉值的唯一事实源。Rust 只定义视觉结构类型，不得复制完整 `DEFAULT_*_VISUAL` 表，也不得用运行时解析或 `OnceLock` 固化第二份表；构建前直接构造、测量、布局、绘制与构建后节点必须共同读取同一文件经 `uix_items!` 生成的常量。
8. UIX 视觉记录必须使用 `<Visual>` 具名字段和有语义的分组静态项；能表达为真实子树的部分继续使用 `<Container>`、`<If>`、`<Icon>` 等标签。禁止用单行巨型 `KernelView` 位置参数或多层匿名数字构造器代替声明结构，`build_*_view` 只接收 Rust 内核、动态 children 和少量具名 Visual 静态借用。

已完成视觉与逻辑分离的首批组件：

- 标准 `WindowControl` 组合：`src/ui/widgets/window_controls/` 同目录保存 Rust 交互桥与 UIX 声明；UIX 拥有三个动作的图标、尺寸、状态色、条件结构和排列，`window_chrome` Rust Module 只保留窗口动作、无障碍语义与交互状态。
- `ButtonGroup`：`src/ui/widgets/general/button_group/` 同目录保存 Rust 与 UIX；UIX 拥有容器结构、横向排列与零间距，Rust 只计算每个按钮的连体位置；`KernelChildren` 直接消费已有 `ViewNode` 列表，不引入克隆或额外容器。
- `BackTop`：`src/ui/widgets/containers/back_top/` 同目录保存 Rust 与 UIX；UIX 拥有图标、固有尺寸、环形几何与主题色角色，Rust 只保留滚动阈值、状态回写、输入、焦点与绘制内核；声明配置融合进原叶节点，不新增 Icon 子节点或包装容器。
- `ThemeToggle`：`src/ui/widgets/other/theme_toggle/` 同目录保存 Rust 与 UIX；UIX 拥有亮/暗双图标、固有尺寸、焦点几何与主题色角色，Rust 只选择当前状态、发布 change 事实并执行绘制；声明配置融合进原叶节点，不物化两个 Icon 子节点。
- `Divider`：`src/ui/widgets/general/divider/` 同目录保存 Rust 与 UIX；UIX 拥有标签尺寸、线段几何、固有尺寸与主题色角色，Rust 只保留文字数据、方向、对齐、虚线状态与绘制内核；声明配置融合进原叶节点，不生成标签或线段子节点。
- `Icon`：`src/ui/widgets/general/icon/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸、Lucide 字形与字体缺失后备的缩放比例及正文主题色角色，Rust 只保留名称、字体句柄、映射、测量与底层绘制；独立实例在构建期解析一次字形，精确容量名称存储使 64 位实例仍保持 32 字节，字体缺失后备使用栈缓冲，全局名称查找只搜索一个有序分片。
- `Avatar`：`src/ui/widgets/display/avatar/` 同目录保存 Rust 与 UIX；UIX 拥有默认边长、文字缩放、方圆留白、圆角令牌和主题色角色，Rust 只保留图片缓存、失效、作者覆盖、文字适配和底层绘制；显式尺寸继续优先于声明默认值。
- `Skeleton`：`src/ui/widgets/display/skeleton/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸、段落行高与间距、末行比例、圆角、流光宽度与节奏及主题色角色，Rust 只保留作者尺寸优先级、形状状态、动画相位、几何执行和绘制。
- `Empty`：`src/ui/widgets/display/empty/` 同目录保存 Rust 与 UIX；UIX 拥有尺寸边界、字号与行高、内边距、图标尺寸与高度比例、图文间距和主题色角色，Rust 只保留本地化文案选择、资源名称映射、文本测量和绘制。
- `QRCode`：`src/ui/widgets/display/qrcode/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸、前景与背景主题色、错误文案和字号角色，Rust 只保留编码矩阵、纠错等级、四模块静区、水平区间合并与底层绘制；显式尺寸继续优先于声明默认值。
- `ResultView`：`src/ui/widgets/display/result/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、留白与比例、排版、操作按钮几何、七种状态图标和主题色角色，Rust 只保留本地化内容选择、自适应布局求解、文本测量、交互状态与底层绘制；完整视觉参数表由实例共享，不逐实例复制。
- `Tag`：`src/ui/widgets/display/tag/` 同目录保存 Rust 与 UIX；UIX 拥有默认字号、内边距、操作区尺寸、图标与间距、勾选与焦点描边、交互叠色、15 种预设色对和圆角主题角色，Rust 只保留关闭、勾选、语义事件、主题解析、测量与底层绘制；完整视觉表由实例共享。
- `ProgressBar`：`src/ui/widgets/feedback/progress/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸与圆角、线形、圆形与仪表盘几何比例、标签字号、渐变方向、不确定动画参数和主题色角色，Rust 只保留 fraction 归一化、模式、动画相位、几何执行与底层绘制；圆形进度复用单弧绘制命令，不再提交 64 条折线。
- `Alert`：`src/ui/widgets/feedback/alert/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/图标/横幅、关闭区/操作区/图标/正文/强调条几何、状态图标、交互反馈、排版、圆角及主题角色，Rust 只保留内容、状态语义、输入、关闭生命周期与绘制执行；同帧主题只解析一次，完整可见的单行文本直接借用输入，不再无条件分配 `String`。
- `Drawer`：`src/ui/widgets/feedback/drawer/` 同目录保存 Rust 与 UIX；UIX 拥有三档面板尺寸、方向与遮罩默认值、触发器/标题区/关闭区/内容槽/footer 几何、静态文案与图标、排版、进退场时序、浮层层级及主题角色，Rust 只保留受控状态、输入、命中、动画生命周期、面板布局求解与底层绘制；关闭态和打开态分别只解析可见分支所需主题值，标题与附加文案使用精确容量存储。
- `Modal`：`src/ui/widgets/feedback/modal/` 同目录保存 Rust 与 UIX；UIX 拥有三档对话框尺寸与行为默认值、触发器/标题区/关闭区/内容槽/footer/窄表面收敛几何、静态文案与图标、排版、进退场时序、浮层层级及主题角色，Rust 只保留受控状态、输入、命中、动画生命周期、对话框布局求解与底层绘制；关闭态和打开态分别只解析可见分支所需主题值，footer 每帧只读取一次语言环境，标题使用精确容量存储。
- `Message`：`src/ui/widgets/feedback/message/` 同目录保存 Rust 与 UIX；UIX 拥有队列放置、单项/强调线/图标/动作/关闭区几何、四类状态图标与时长、排版、进出时序、浮层层级及主题角色，Rust 只保留队列、计时、输入、动画状态、命中与绘制执行；同帧全部可见项共享一次主题解析，普通单行动作文本直接借用输入，实例便捷入队不再克隆共享句柄。
- `Notification`：`src/ui/widgets/feedback/notification/` 同目录保存 Rust 与 UIX；UIX 拥有默认时长和放置、队列/单项/强调线/图标/标题/描述/动作/关闭区几何、四类状态图标、排版、进出时序、浮层层级及主题角色，Rust 只保留队列租约、计时、类型化关闭原因、输入、动画状态、命中与绘制执行；命令式句柄和声明式通知共享同一 UIX 默认时长，同帧全部可见项共享一次主题解析。
- `Tooltip`：`src/ui/widgets/feedback/tooltip/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/方向/箭头、气泡几何、排版、进入退出时序、浮层层级及主题角色，Rust 只保留内容、触发、输入、计时器、动画状态、表面约束与绘制执行；完整视觉表由实例共享，文字自然尺寸只在构建或声明刷新时计算，render/dirty/overlay 复用缓存，未迁移的 Slider 继续通过兼容入口消费同一默认气泡参数。
- `Popover`：`src/ui/widgets/feedback/popover/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、默认方向/箭头、定位与内容几何、触发器/气泡装饰、排版、进入退出时序、浮层层级及主题角色，Rust 只保留内容、受控状态、唯一触发子树、输入、动画状态、表面约束与绘制执行；同帧主题只解析一次，完整容纳的单行文本直接借用输入，不再无条件分配 `String`。
- `Popconfirm`：`src/ui/widgets/feedback/popconfirm/` 同目录保存 Rust 与 UIX；UIX 拥有气泡与兼容触发器尺寸、默认方向/箭头/图标/按钮文案、定位/标题/警告图标/按钮/箭头几何、排版、进退场时序、浮层层级及主题角色，Rust 只保留唯一 trigger 子树、输入与焦点、确认取消防重入、业务回调、语义事件、动画状态、表面约束与绘制执行；隐藏与打开分支共享一次按需主题解析，完整视觉表由实例共享。
- `AutoComplete`：`src/ui/widgets/input/autocomplete/` 同目录保存 Rust 与 UIX；UIX 拥有输入框/光标/候选行/弹层尺寸与几何、排版、描边、圆角、进退场时序、浮层层级及主题角色，Rust 只保留文本编辑、受控状态、候选过滤、键盘与指针输入、虚拟滚动、语义事件、动画状态、表面约束与绘制执行；原分类根单文件已迁入组件目录，输入框与弹层同帧共享一次主题解析。
- `Cascader`：`src/ui/widgets/input/cascader/` 同目录保存 Rust 与 UIX；UIX 拥有触发器/光标/多列弹层/候选行/加载指示器几何、排版、图标、描边、圆角、进退场与加载时序、浮层层级及主题角色，Rust 只保留树路径、受控状态、跨层搜索、键盘与指针输入、滚动、语义事件、动画状态、表面约束与绘制执行；触发器、普通列和搜索结果同帧共享一次主题解析。
- `ColorPicker`：`src/ui/widgets/input/color_picker/` 同目录保存 Rust 与 UIX；UIX 拥有 24 个默认预设色、触发色块/透明棋盘格/颜色网格几何、选中图标、描边、圆角、进退场时序、浮层层级及主题角色，Rust 只保留颜色值、受控回写、键盘与指针输入、焦点、高亮、动画状态、表面约束与绘制执行；原分类根单文件已迁入组件目录，触发色块与面板同帧共享一次主题解析。
- `DatePicker`：`src/ui/widgets/input/date_picker/` 同目录保存 Rust 与 UIX；UIX 拥有日期触发框、月历面板、图标、字号、间距、描边、圆角、自然尺寸、浮层层级及主题角色，Rust 只保留公历运算、受控回写、选择模式、禁用判断、键盘与指针输入、表面约束、命中和绘制执行；原分类根单文件已迁入组件目录，触发框与共享月历绘制器同帧共享一次主题解析。
- `DateRangePicker`：`src/ui/widgets/input/date_range_picker/` 同目录保存 Rust 与 UIX；UIX 拥有范围触发框、月历面板、预设页脚、图标、字号、间距、描边、圆角、自然尺寸、浮层层级及主题角色，Rust 只保留日期范围排序、预设语义、受控回写、禁用判断、两段选择、键盘与指针输入、表面约束、命中和绘制执行；原分类根单文件已迁入组件目录，触发框、月历与预设页脚同帧共享一次主题解析，共享月历不再保留 Rust 默认视觉表。
- `Input`：`src/ui/widgets/input/input/` 同目录保存 Rust 与 UIX；UIX 拥有三档控件高度、单行与多行文本区、附件和操作槽、光标、选区、输入法下划线、状态消息、图标、排版、描边、圆角及主题角色，Rust 只保留文本编辑、IME、Unicode 字素簇、受控回写、选区、滚动、键盘与指针输入、语义事件和绘制执行；单行、多行与状态消息同帧共享一次主题解析。
- `Mentions`：`src/ui/widgets/input/mentions/` 同目录保存 Rust 与 UIX；UIX 拥有输入框、光标、候选行、弹层自然尺寸、留白、排版、描边、圆角、浮层层级及主题角色，Rust 只保留完整文本编辑、受控回写、活动查询、候选过滤与替换、键盘与指针输入、虚拟滚动、表面约束、命中和绘制执行；原分类根单文件已迁入组件目录，输入框与候选弹层同帧共享一次主题解析。
- `Select`：`src/ui/widgets/input/select/` 同目录保存 Rust 与 UIX；UIX 拥有三档控件尺寸、标签、候选行、空态、加载器、弹层高度、留白、图标、排版、描边、圆角、阴影、浮层层级、加载节奏及主题角色，Rust 只保留稳定值绑定、搜索过滤、单选与多选状态、键盘与指针输入、虚拟滚动、表面约束、自定义选项子树、命中、过渡推进和绘制执行；控件与弹层同帧共享一次主题解析。
- `TimePicker`：`src/ui/widgets/input/time_picker/` 同目录保存 Rust 与 UIX；UIX 拥有三档触发器高度、时钟图标、面板自然尺寸与间隙、双列行高、留白、排版、描边、分隔线、圆角、浮层层级及主题角色，Rust 只保留 `Time` 值语义、受控回写、时分范围、滚轮步进、键盘与指针输入、滚动状态、表面约束、命中和绘制执行；原分类根单文件已迁入组件目录，触发器与面板同帧共享一次主题解析。
- `Transfer`：`src/ui/widgets/input/transfer/` 同目录保存 Rust 与 UIX；UIX 拥有双栏自然尺寸、搜索框、标题区、条目行、复选图标、操作按钮、文字光标、留白、排版、描边、圆角及主题角色，Rust 只保留稳定条目身份、搜索过滤、左右选择与移动、键盘与指针输入、自定义条目子树、回调、快照和语义事件；原分类根单文件已迁入组件目录，左右栏和按钮同帧共享一次主题解析。
- `TreeSelect`：`src/ui/widgets/input/tree_select/` 同目录保存 Rust 与 UIX；UIX 拥有触发器、树行、层级缩进、弹层自然尺寸、留白、图标、排版、描边、圆角、浮层层级及主题角色，Rust 只保留节点扁平化、稳定键绑定、选择与高亮、键盘与指针输入、虚拟滚动、表面约束、过渡推进、命中和绘制执行；原分类根单文件已迁入组件目录，触发器与弹层同帧共享一次主题解析。
- `Upload`：`src/ui/widgets/input/upload/` 同目录保存 Rust 与 UIX；UIX 拥有上传区、文件行、缩略图、状态图标、进度条、留白、排版、描边、圆角及主题角色，Rust 只保留受控文件队列、路径与大小校验、拖放、稳定身份、进度和结果写回、回调、命中与绘制执行；原分类根三个 Rust 文件已迁入组件目录，上传区和全部文件行同帧共享一次主题解析。
- `Anchor`：`src/ui/widgets/navigation/anchor/` 同目录保存 Rust 与 UIX；UIX 拥有导航行、激活墨线、标签容量、动态内容容器自然尺寸、默认激活边界、排版、分割线、焦点框、圆角及主题角色，Rust 只保留稳定 href、滚动位置缓存、动态子树捕获、键盘与指针输入、选择、语义事件、命中、布局和绘制执行；原分类根单文件已迁入组件目录，全部导航行同帧共享一次主题解析。
- `Breadcrumb`：`src/ui/widgets/navigation/breadcrumb/` 同目录保存 Rust 与 UIX；UIX 拥有主行、条目、图标、分隔符、折叠触发器、溢出菜单、文字容量、排版、描边、圆角、阴影及主题角色，Rust 只保留稳定链接、活动项归一、折叠可见项算法、键盘与指针输入、选择、语义事件、命中、布局和绘制执行；原分类根单文件已迁入组件目录，主行与菜单同帧共享一次主题解析。
- `Spin`：`src/ui/widgets/feedback/spin/` 同目录保存 Rust 与 UIX；UIX 拥有三档直径、轨道与圆点比例、八点序列、透明度节奏、提示排版、包裹遮罩、转速和主题色角色，Rust 只保留延迟、相位、子树叠加、圆点递推和底层绘制；生产路径继续每帧只计算一次相位三角函数且不分配临时集合。
- `Badge`：`src/ui/widgets/display/badge/` 同目录保存 Rust 与 UIX；UIX 拥有数字胶囊、状态点与丝带的尺寸、排版和斜切比例，以及五种预设色、五种状态色和对比文字主题角色，Rust 只保留计数、单子树生命周期、物理单位偏移、布局收敛与底层绘制；计数文本继续使用栈缓冲，测量和绘制不分配 `String`。
- `Watermark`：`src/ui/widgets/display/watermark/` 同目录保存 Rust 与 UIX；UIX 拥有默认透明度、旋转角、平铺间距、偏移、外扩数量、行高比例及正文主题色/字号角色，Rust 只保留作者覆写、主题解析、平铺和旋转绘制；完整文本行共享整形与字体回退，避免逐字命令、测量和临时字符串。
- `Card`：`src/ui/widgets/display/card/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/内边距/边框与阴影级别、标题和动作区排版、表面/强调线/焦点圈几何、三级阴影缩放表及主题色/圆角/阴影角色，Rust 只保留子树生命周期、作者覆写、交互、Flex 布局、主题解析和底层绘制。
- `Carousel`：`src/ui/widgets/display/carousel/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/控制开关/切换效果、箭头图标与命中几何、页码点槽位/尺寸/激活比例、焦点圈、淡入淡出时长与关键点及背景/控制主题角色，Rust 只保留幻灯片和自定义箭头子树、计时器、索引归一、选择、语义事件、动画状态与底层绘制；运行态仅共享 UIX motion 静态引用，不复制视觉表。
- `Descriptions`：`src/ui/widgets/display/descriptions/` 同目录保存 Rust 与 UIX；UIX 拥有默认宽度/列网格/标签宽度、标题与单元格留白、标题/条目排版、三档基础行高、边框/圆角及主题色角色，Rust 只保留类型化数据、span 装箱、列数收缩、换行估算、主题解析与底层绘制；行高结果按宽度/列数缓存并复用 `Vec` 容量。
- `Image`：`src/ui/widgets/display/image/` 同目录保存 Rust 与 UIX；UIX 拥有默认圆角/预览/适配/延迟开关、占位和焦点视觉、预览指示器、模态布局、静态文案/图标及主题角色，Rust 只保留资源缓存、加载状态、动态错误子树、事件、主题解析与底层绘制；每帧只解析一次主题值并由实例共享完整视觉表。
- `ImageGroup`：`src/ui/widgets/display/image_group/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、内嵌画廊/模态预览/缩略图布局、导航与关闭控制、静态文案/图标及主题角色，Rust 只保留资源、索引、命中、键盘交互、几何执行与底层绘制；同帧画廊和全部缩略图共享一次主题解析，预览计数复用缓存字符串。
- `Timeline`：`src/ui/widgets/display/timeline/` 同目录保存 Rust 与 UIX；UIX 拥有固有宽度、行高边界、节点/连接线几何、排版缩放及主题色/字号角色，Rust 只保留数据、反向索引、本地化、可见行裁剪、文本测量与底层绘制；完整容纳的普通单行文本经共享 `Cow` 快路径直接借用输入，不逐帧分配临时 `String`。
- `List`：`src/ui/widgets/display/list/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/边框、三档行高、水平留白、兼容文字排版、分隔线和主题角色，Rust 只保留 Empty 策略、数据、三个真实插槽的稳定身份/测量/正常流布局及底层绘制；完整容纳的 header/footer/item/load-more 文本复用共享借用型省略入口。
- `Collapse`：`src/ui/widgets/display/collapse/` 同目录保存 Rust 与 UIX；UIX 拥有宽度边界、标题/内容排版、图标槽、留白、焦点/边框几何及主题角色，Rust 只保留稳定 key、受控/非受控展开、动态内容子树、输入、动画与布局；普通单行标题宽度估算直接借用输入，面板切换只保留一份旧状态缓冲并原位启动变化动画。
- `SelectableList`：`src/ui/widgets/display/selectable_list/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、头尾/行布局、图标、状态透明度、排版、焦点几何及主题角色，Rust 只保留稳定 id、受控/非受控选择、键盘/指针输入、可见行虚拟化、语义事件与底层绘制；完整容纳的单行文本直接借用输入，同帧全部可见行共享一次主题解析。
- `Calendar`：`src/ui/widgets/display/calendar/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、标题/星期栏/日期格布局、导航图标、排版、焦点/事件标记几何及主题角色，Rust 只保留受控/非受控日期、动态日期格、月份导航、输入、语义事件与底层绘制；同帧主题只解析一次，事件按月份一次线性分桶，日期数字复用静态文本。
- `Tree`：`src/ui/widgets/display/tree/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、搜索栏、行槽位/缩进、图标、焦点及主题角色，Rust 只保留稳定 key、展开/勾选、筛选、拖拽、键盘/指针输入、虚拟化与树算法；单行文本直接借用，键盘移动不再构造索引数组，搜索筛选改为单次递归裁剪。
- `Table`：`src/ui/widgets/display/table/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、表头/单元格/分页/加载几何、图标、排版及主题角色，Rust 只保留数据、跨度、固定列、排序、选择、分页状态、输入与虚拟化；同帧主题只解析一次，分页标签复用字符串缓冲，加载圆点每帧只计算一次相位三角函数。
- `BarChart`：`src/ui/widgets/display/chart/bar_chart/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/间距、标题/图例/坐标轴/标签/提示框几何、排版及主题角色，Rust 只保留数据、分组/堆叠、缩放、平移、刷选、命中与绘制执行；系列遍历不创建临时集合，图例不拼接临时字符串，刻度与数值使用栈缓冲。
- `LineChart`：`src/ui/widgets/display/chart/line_chart/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/开关、标题/图例/坐标轴/分类标签/提示框几何、数据点装饰、平滑采样密度、排版及主题角色，Rust 只保留数据、范围、缩放、平移、刷选、命中与绘制执行；系列遍历不创建临时集合，原始点与平滑点复用缓冲，图例不拼接临时字符串，刻度使用栈缓冲。
- `PieChart`：`src/ui/widgets/display/chart/pie_chart/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸/标签/图例、标题/图例/绘图区几何、扇区标签、内环、交互叠层、提示框、排版及主题角色，Rust 只保留有效数据摘要、极坐标几何、玫瑰映射、命中、交互与绘制执行；基础绘制借用原数据，不创建切片集合或图例字符串，数值文本使用栈缓冲，标签角度只计算一次正余弦。
- `ChartPlaceholder`：`src/ui/widgets/display/chart/advanced/` 同目录保存共享 Rust 内核与 UIX；UIX 拥有面积、散点、气泡、雷达、热力、漏斗、瀑布、组合、树图和仪表盘的默认配置、画布、标题、图例、坐标轴、提示、状态装饰、排版、描边及主题角色，Rust 只保留数据载荷、作者覆写、尺度与分块算法、动画、缩放平移、刷选、命中和绘制执行；全部图表分支同帧共享一次主题解析，既有九个类型入口继续复用同一公开内核。
- `RichText`：`src/ui/widgets/display/rich_text/` 同目录保存 Rust 与 UIX；UIX 拥有默认字号、行高/字宽估算、代码/链接/选区装饰、复制按钮、主题分隔线、图片占位及主题角色，Rust 只保留 Markdown、Unicode 断行/双向布局、资源、选择、命中与绘制执行；缓存布局按借用复用，绘制 run 复用 UTF-8 缓冲，字形不再逐个保存链接 URL。

当前没有只透传 `kernel`/`children`、仍待回填静态视觉声明的已迁移 widget；Rust 完整默认视觉表与 UIX 巨型位置参数壳两类债务也已清零。公开视觉 widget 清单当前为 84 个，其中 77 个已完成、7 个仍在显式债务集合；该清单是架构迁移记录，不构成测试覆盖。已迁移的简单叶组件、分组视觉组件以及由 Tooltip 与 Slider 共享的 `TooltipBubble` 均完成 `<Visual>` 单源闭环；共享视觉资源允许通过 `uix_items!` 只声明模块级 `Visual`，不需要伪造可实例化视图根。

## 组件：复合组件

Form、Table、Tree、Modal、Menu 等复合组件分别复用 form、virtualization、overlay 等目标模块。每个组件通过 `Widget` 暴露必要能力，运行态只由组件实例和树 side table 持有；公开教程与参数表归[使用 · 组件](../../使用/界面构建/组件.md)。

## 模块不变量

内置组件不直接访问 platform/backend 或全局单例；业务闭包只能登记到所属树 side table，组件值与快照只保存稳定签名。图标、文本布局、overlay、虚拟滚动复用 ui System 编排的对应能力，不复制平行实现。

输入、上传、RichText、二维码和图片等外部内容都按不可信数据处理，并在解析、大小、资源与敏感值边界复用对应模块契约。组件的“操作已接纳”“状态已改变”“布局已完成”和“画面已呈现”是不同事实，不得用一次回调或布尔值混为同一成功结果。
