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

已完成同目录与公开根迁移、仍待回填视觉声明的组件：`ResultView`、`Tag`、`ProgressBar`、`Spin`、`Badge`、`Watermark`、`Card`、`Carousel`、`Descriptions`、`Image` 与 `ImageGroup`。这些条目不能作为最终完成状态；对应债务清单由结构测试锁定并逐项缩减。

## 组件：复合组件

Form、Table、Tree、Modal、Menu 等复合组件分别复用 form、virtualization、overlay 等目标模块。每个组件通过 `Widget` 暴露必要能力，运行态只由组件实例和树 side table 持有；公开教程与参数表归[使用 · 组件](../../使用/界面构建/组件.md)。

## 模块不变量

内置组件不直接访问 platform/backend 或全局单例；业务闭包只能登记到所属树 side table，组件值与快照只保存稳定签名。图标、文本布局、overlay、虚拟滚动复用 ui System 编排的对应能力，不复制平行实现。

输入、上传、RichText、二维码和图片等外部内容都按不可信数据处理，并在解析、大小、资源与敏感值边界复用对应模块契约。组件的“操作已接纳”“状态已改变”“布局已完成”和“画面已呈现”是不同事实，不得用一次回调或布尔值混为同一成功结果。
