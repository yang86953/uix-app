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
4. 迁移必须保留公开类型、事件、无障碍、状态协调、稳定 key、布局和视觉结果，并用真实编译测试与最接近的行为测试验收。
5. 条件隐藏的分支不得提前构造 Rust 基础 View；高频路径不得因语言迁移增加无条件分配、克隆或动态解析。
6. 单 `KernelView` 组件必须由 UIX 显式向 Rust 内核注入静态视觉配置；只传 `kernel` 或 `kernel, children` 的透传壳属于未完成债务。`tests/test_widget_uix_colocation.py` 锁定当前债务清单，新迁移不得扩大清单，回填后必须删除对应项。

已完成视觉与逻辑分离的首批组件：

- 标准 `WindowControl` 组合：`src/ui/widgets/window_controls/` 同目录保存 Rust 交互桥与 UIX 声明；UIX 拥有三个动作的图标、尺寸、状态色、条件结构和排列，`window_chrome` Rust Module 只保留窗口动作、无障碍语义与交互状态。
- `ButtonGroup`：`src/ui/widgets/general/button_group/` 同目录保存 Rust 与 UIX；UIX 拥有容器结构、横向排列与零间距，Rust 只计算每个按钮的连体位置；`KernelChildren` 直接消费已有 `ViewNode` 列表，不引入克隆或额外容器。
- `BackTop`：`src/ui/widgets/containers/back_top/` 同目录保存 Rust 与 UIX；UIX 拥有图标、固有尺寸、环形几何与主题色角色，Rust 只保留滚动阈值、状态回写、输入、焦点与绘制内核；声明配置融合进原叶节点，不新增 Icon 子节点或包装容器。
- `ThemeToggle`：`src/ui/widgets/other/theme_toggle/` 同目录保存 Rust 与 UIX；UIX 拥有亮/暗双图标、固有尺寸、焦点几何与主题色角色，Rust 只选择当前状态、发布 change 事实并执行绘制；声明配置融合进原叶节点，不物化两个 Icon 子节点。
- `Divider`：`src/ui/widgets/general/divider/` 同目录保存 Rust 与 UIX；UIX 拥有标签尺寸、线段几何、固有尺寸与主题色角色，Rust 只保留文字数据、方向、对齐、虚线状态与绘制内核；声明配置融合进原叶节点，不生成标签或线段子节点。
- `Avatar`：`src/ui/widgets/display/avatar/` 同目录保存 Rust 与 UIX；UIX 拥有默认边长、文字缩放、方圆留白、圆角令牌和主题色角色，Rust 只保留图片缓存、失效、作者覆盖、文字适配和底层绘制；显式尺寸继续优先于声明默认值。
- `Skeleton`：`src/ui/widgets/display/skeleton/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸、段落行高与间距、末行比例、圆角、流光宽度与节奏及主题色角色，Rust 只保留作者尺寸优先级、形状状态、动画相位、几何执行和绘制。
- `Empty`：`src/ui/widgets/display/empty/` 同目录保存 Rust 与 UIX；UIX 拥有尺寸边界、字号与行高、内边距、图标尺寸与高度比例、图文间距和主题色角色，Rust 只保留本地化文案选择、资源名称映射、文本测量和绘制。
- `QRCode`：`src/ui/widgets/display/qrcode/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸、前景与背景主题色、错误文案和字号角色，Rust 只保留编码矩阵、纠错等级、四模块静区、水平区间合并与底层绘制；显式尺寸继续优先于声明默认值。
- `ResultView`：`src/ui/widgets/display/result/` 同目录保存 Rust 与 UIX；UIX 拥有固有尺寸、留白与比例、排版、操作按钮几何、七种状态图标和主题色角色，Rust 只保留本地化内容选择、自适应布局求解、文本测量、交互状态与底层绘制；完整视觉参数表由实例共享，不逐实例复制。
- `Tag`：`src/ui/widgets/display/tag/` 同目录保存 Rust 与 UIX；UIX 拥有默认字号、内边距、操作区尺寸、图标与间距、勾选与焦点描边、交互叠色、15 种预设色对和圆角主题角色，Rust 只保留关闭、勾选、语义事件、主题解析、测量与底层绘制；完整视觉表由实例共享。
- `ProgressBar`：`src/ui/widgets/feedback/progress/` 同目录保存 Rust 与 UIX；UIX 拥有默认尺寸与圆角、线形、圆形与仪表盘几何比例、标签字号、渐变方向、不确定动画参数和主题色角色，Rust 只保留 fraction 归一化、模式、动画相位、几何执行与底层绘制；圆形进度复用单弧绘制命令，不再提交 64 条折线。
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

当前没有只透传 `kernel`/`children`、仍待回填静态视觉声明的内置 widget；结构测试继续以空债务集合阻止回退。

## 组件：复合组件

Form、Table、Tree、Modal、Menu 等复合组件分别复用 form、virtualization、overlay 等目标模块。每个组件通过 `Widget` 暴露必要能力，运行态只由组件实例和树 side table 持有；公开教程与参数表归[使用 · 组件](../../使用/界面构建/组件.md)。

## 模块不变量

内置组件不直接访问 platform/backend 或全局单例；业务闭包只能登记到所属树 side table，组件值与快照只保存稳定签名。图标、文本布局、overlay、虚拟滚动复用 ui System 编排的对应能力，不复制平行实现。

输入、上传、RichText、二维码和图片等外部内容都按不可信数据处理，并在解析、大小、资源与敏感值边界复用对应模块契约。组件的“操作已接纳”“状态已改变”“布局已完成”和“画面已呈现”是不同事实，不得用一次回调或布尔值混为同一成功结果。
