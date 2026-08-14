# UIX Lang 语言首页

> **定位**：UIX Lang 是 UIX 框架的 UI 描述语言，也是**使用方编写界面的推荐方式（产品方向声明）**——写应用时用 uix-lang 描述界面结构与样式，编译时 uix-app 框架自动将 `.uix` 文档转换为 Rust 声明式代码（过程宏 / 构建期转换），运行期零解释器开销。语言规范位于[规范/](规范/总览.md)。

## 为什么需要 UIX Lang

UIX 产品原则是「Rust 优先——没有第二门运行语言」。公开 `uix!` 生成已登记 View 子集，`uix_app!` 把 `<App>` 文档组装成现有 App builder；Rust 声明式 API 继续持有应用 / 窗口生命周期和尚未登记的完整组件能力。UIX Lang 让**应用开发以 uix-lang 描述 UI 为主路径**：`.uix` 文档是应用源码的一部分，编译时由 uix-app 框架自动转换为 Rust 声明式代码，运行时不再解析界面文本。

```text
.uix 文档（应用源码）──uix-app 编译期转换（uix! / uix_app!）──> Rust 声明式代码 ──cargo 编译──> 原生可执行文件
```

每个标签、属性、样式规则都有确定的 Rust 等价物——这是本语言与 HTML/CSS 的本质区别（无隐式盒模型差异、无浏览器历史包袱，语义与 UIX 布局引擎一一对应）。除应用开发主路径外，文本形态还服务三类场景：工具链生成、Agent 控制、文档与传输（详见[规范 · 总览](规范/总览.md)）。

## 设计原则

1. **Rust 优先**：语言是 Rust API 的文本投影，不存在 Rust 表达不了的结构。
2. **资源优先**：声明是静态的；动态行为（状态、事件、动画）显式声明，不隐藏隐式轮询。
3. **原生优先**：标签与样式直接映射 UIX 布局与绘制原语，不引入中间抽象层。
4. **容错可观测**：解析错误有位置、有原因、有修复建议；运行期错误有类型、有恢复路径。

## 文档地图

| 部分 | 文档 | 内容 |
|---|---|---|
| **规范**（权威） | [总览](规范/总览.md) | 设计原则、术语表、文档约定、完整示例 |
| | [词法](规范/词法.md) | 命名规范、注释、字面量、构造链 |
| | [语法](规范/语法.md) | EBNF 文法、文档结构、元素与属性 |
| | [表达式](规范/表达式.md) | 受限表达式、数组操作、内置操作 |
| | [组件](规范/组件.md) | 组件定义、props / state、Record、事件绑定 |
| | [控制流](规范/控制流.md) | If 条件渲染、For 循环与 key |
| | [样式与主题](规范/样式与主题.md) | 样式类、内联样式、@theme |
| | [模块系统](规范/模块系统.md) | @import / @export、增量编译 |
| | [编译契约](规范/编译契约.md) | 宏契约、Rust 映射、诊断模型 |
| **参考** | [样式属性](参考/样式属性.md) | 全部样式属性的语法、取值、默认值与适用元素 |
| | [事件](参考/事件.md) | 事件名与 $event 载荷登记 |
| | [组件](参考/组件/布局组件.md) | 布局 / 通用 / 输入 / 展示 / 反馈 / 导航六类组件参考 |
| **指南** | [快速开始](指南/快速开始.md) | 依赖配置、Hello World、第一个 View |
| | [教程](指南/教程.md) | 从零构建完整应用 |
| **变更** | [版本策略](变更/版本策略.md) | 语言版本、兼容性承诺、变更登记流程 |
| | [变更日志](变更/CHANGELOG.md) | 语言面变更记录 |
| | [目标设计](变更/目标设计.md) | 已批准改进设计（组合与表达式面） |

## 状态声明

公开 `uix!` 已支持内嵌源码与相对调用 crate 的 `.uix` 文件并在编译期生成 `ViewNode`；三个公开宏的文件入口都已支持相对当前文件的嵌套 `@import`、显式 `@export`、全量或具名组件选择，并追踪全部递归依赖。公开 `uix_app!` 已支持 `<App>` 根的 `title` / `size` / `theme` / `settings`、同文档主题与现有 `App` builder 组装。

当前已登记 `Text`、`Label`、`Button`、`ButtonGroup`、`FloatButton`、`FloatButtonGroup`、`FloatButtonBackTop`、`Icon`、`Divider`、`Space`、`Typography`、`ThemeToggle`、`WindowControl`、`WindowDragRegion`、`Container`、`Row`、`Column`、`Grid`、`ScrollView`、`VirtualScroll`、`Splitter`、`Affix`、`BackTop`、`Layout`、`Sider`、`Header`、`Content`、`Footer`、`Input`、`InputNumber`、`InputGroup`、`Slider`、`RangeSlider`、`Rate`、`Checkbox`、`Switch`、`Radio`、`Segmented`、`Select`、`Cascader`、`TreeSelect`、`AutoComplete`、`Mentions`、`DatePicker`、`DateRangePicker`、`TimePicker`、`ColorPicker`、`Avatar`、`Image`、`ImageGroup`、`List`、`Skeleton`、`Empty`、`ResultView`、`Tag`、`Card`、`Descriptions`、`Timeline`、`Calendar`、`Carousel`、`Tree`、`Table`、`Menu`、`Dropdown`、`Navigation`、`Steps`、`Pagination`、`Breadcrumb`、`Anchor`、`Tabs`、`QRCode`、`Watermark`、`RichText`、`Message`、`Notification`、`Alert`、`Popconfirm`、`Modal`、`Drawer`、`Popover`、`Tooltip`、`FocusTrap`、`ProgressBar`、`Spin`、`Form`、`FormInputItem`、`FormSelectItem`、`FormCheckboxItem`、`FormRadioItem`、`FormSwitchItem`、`FormSliderItem`、`Upload`、`SelectableList`、`Collapse`、`Badge`，`Col` 作为 `Row` / `Grid` 的直接子项登记。展示、反馈、导航类未登记组件仍为规划中；转换器会对未登记或规划中能力产生带来源位置和修复建议的编译错误，不会静默降级。实现差距由 [Vikunja 项目 4](https://yang-server.tail9d5559.ts.net:3456/projects/4) 跟踪。

图表组件（BarChart / LineChart / PieChart 等 12 个）的规范文档尚未编写（规划中），对应组件能力以[组件速查](../使用/组件速查.md)与[图表使用文档](../使用/图表.md)为准。

## 与 Rust API 的映射关系

UIX Lang 是 UIX Rust API 的文本投影，**转换由 uix-app 框架在编译时自动完成**：转换器按下列规则把 `.uix` 结构展开为 Rust 声明式代码，再交由 cargo 编译；不生成任何运行期解释逻辑（完整规则见[编译契约](规范/编译契约.md)）。

| UIX Lang | Rust API | 当前状态 |
|---|---|---|
| `<Button class="primary" @click="...">` | 编译期展开样式类后生成 `button("...").map_style(...).on_click(...)` | 已实现（限已登记属性与事件） |
| `styleName { ... }` 样式类 | 编译期展开 `extends` 与同名属性覆盖，再精确更新 `Style` | 已实现（限已登记样式属性） |
| `@theme light { ... }` | App 作用域 `Theme` / `ThemeTokens` | 已实现（完整 `DesignTokens` 白名单） |
| `<App title="..." size="...">` | 现有 `App` builder + 根工厂 | 已实现（由 `uix_app!` 返回，不调用 `run`） |
| `<Component name="X">` | 编译期展开 props、state、computed、Slot、external、回调与组件体 | 已实现（Slot 内容在调用方作用域内联；computed 按声明顺序求值） |
| `<For {item} in {items}>` | `for` 循环生成 `ViewNode` 列表 | 已实现（须位于容器内） |
| `@import` / `@export` | 过程宏递归解析文件依赖图并合并显式公开组件 | 已实现（仅文件入口；嵌套无环；全量或具名导入） |

本规范只描述语言本身；Rust API 的用法以[使用文档](../使用.md)为权威。
