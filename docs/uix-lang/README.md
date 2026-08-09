# UIX Lang 语言规范

> **定位**：UIX Lang 是 UIX 框架的 UI 描述语言，也是**使用方编写界面的推荐方式（产品方向声明）**——写应用时用 uix-lang 描述界面结构与样式，编译时 uix-app 框架自动将 `.uix` 文档转换为 Rust 声明式代码（过程宏 / 构建期转换），运行期零解释器开销。本文档是语言规范的唯一当前位置。
>
> **状态声明**：本规范为**目标设计、领先实现**。界面描述支持两种方式（见[定位与原则](../产品/定位与原则.md#定位)）：uix-lang 为**推荐方式**，Rust 声明式 API 为**支持方式**（二者语义一一对应）。当前转换器（`uix!` 宏）尚未实现，落地前 Rust 声明式 API（`uix::prelude::*`，见 [使用文档](../使用.md)）是唯一可用入口；规范中的每一项能力都映射到已有的 Rust API 能力，实现差距由 [Vikunja 项目 4](https://yang-server.tail9d5559.ts.net:3456/projects/4) 跟踪，仓库内不复制带时点的状态。

## 为什么需要 UIX Lang

UIX 产品原则是「Rust 优先——没有第二门运行语言」，Rust 声明式 API 是当前唯一可用入口。UIX Lang 让**应用开发以 uix-lang 描述 UI 为主路径**：`.uix` 文档是应用源码的一部分，编译时由 uix-app 框架自动转换为 Rust 声明式代码，写界面不再直接面对组件构造代码。

```text
.uix 文档（应用源码）──uix-app 编译期转换（uix! 宏）──> Rust 声明式代码 ──cargo 编译──> 原生可执行文件
```

转换发生在编译时：`.uix` 文档经 uix-app 转换器生成 Rust 声明式代码，随 crate 一起编译，运行期没有解释器、没有运行时开销。每个标签、属性、样式规则都有确定的 Rust 等价物——这是本语言与 HTML/CSS 的本质区别（无隐式盒模型差异、无浏览器历史包袱，语义与 UIX 布局引擎一一对应）。除应用开发主路径外，文本形态还服务三类场景：

| 场景 | 说明 |
|---|---|
| 工具链生成 | 设计工具、代码生成器直接产出 `.uix` 文本，构建期转换为 Rust 组件树 |
| Agent 控制 | 与 [Agent Bridge](../使用/Agent控制.md) 配合，以文本形式描述目标界面供 Agent 生成或修改 |
| 文档与传输 | 界面结构可用文本交换、评审、存档，不依赖编译产物 |

## 设计原则

UIX Lang 继承产品四原则（见[定位与原则](../产品/定位与原则.md)）：

1. **Rust 优先**：语言是 Rust API 的文本投影，每个结构都能在编译期展开为确定的 Rust 代码，不存在 Rust 表达不了的结构。
2. **资源优先**：声明是静态的；动态行为（状态、事件、动画）显式声明，不隐藏隐式轮询。
3. **原生优先**：标签与样式直接映射 UIX 布局与绘制原语，不引入中间抽象层。
4. **容错可观测**：解析错误有位置、有原因、有修复建议；运行期错误有类型、有恢复路径。

## 文档地图

| 文档 | 内容 |
|---|---|
| [标签语法](标签语法.md) | 词法、文档结构、元素、组件、状态与事件、表达式、条件渲染、导入导出 |
| [UIX 样式属性参考](UIX%20样式属性参考.md) | 全部样式属性的语法、取值、默认值、适用元素与示例 |
| [内置组件/布局组件](内置组件/布局组件.md) | Row / Col / Container / Grid / Splitter / ScrollView / Layout 等布局组件 |
| [内置组件/通用组件](内置组件/通用组件.md) | Button / Icon / Label / Typography / Divider / Space / ThemeToggle 等 |
| [内置组件/输入组件](内置组件/输入组件.md) | Input / Select / Form / DatePicker / Upload 等输入与表单组件 |
| [内置组件/展示组件](内置组件/展示组件.md) | Avatar / Card / Table / Tree / RichText / QRCode 等展示组件 |
| [内置组件/反馈组件](内置组件/反馈组件.md) | Message / Modal / Drawer / Popover / ProgressBar / Spin 等反馈组件 |
| [内置组件/导航组件](内置组件/导航组件.md) | Menu / Tabs / Breadcrumb / Steps / Pagination 等导航组件 |

图表组件（BarChart / LineChart / PieChart 等 12 个）的规范文档尚未编写（规划中），对应组件能力以 [组件速查](../使用/组件速查.md) 与[图表使用文档](../使用/图表.md)为准。

## 与 Rust API 的映射关系

UIX Lang 是 UIX Rust API 的文本投影，**转换由 uix-app 框架在编译时自动完成**：转换器按下列规则把 `.uix` 结构展开为 Rust 声明式代码，再交由 cargo 编译；不生成任何运行期解释逻辑。

| UIX Lang | Rust API |
|---|---|
| `<Button class="primary" @click="...">` | `button("...").class("primary").on_click(...)` |
| `styleName { ... }` 样式类 | `Style` 定义 + `apply_style` |
| `@theme light { ... }` | 主题令牌（`ThemeTokens`） |
| `<Component name="X">` | `component!` 宏或函数组件 |
| `<For {item} in {items}>` | `for` 循环生成 `ViewNode` 列表 |

本规范只描述语言本身；Rust API 的用法以[使用文档](../使用.md)为权威。

## 维护约定

1. 先更新持有事实的正文（本目录），再更新本文档地图中的摘要。
2. 文档使用仓库相对 Markdown 链接，不保留 Obsidian Frontmatter、Wiki 链接或生成式目录索引。
3. 文档改动后运行链接检查：`python tests/test_check_docs_links.py -q`。
4. 语言能力的新增或变更需要同时在本文档（目标设计）与 Vikunja 项目 4（实现差距）登记。
