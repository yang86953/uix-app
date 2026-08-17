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

## 组件：基础组件

Button、Label、Typography、Icon、Container 等基础组件只组合 component、layout、event、theme 和 painting 能力，不私有化通用机制。

## 组件：复合组件

Form、Table、Tree、Modal、Menu 等复合组件分别复用 form、virtualization、overlay 等目标模块。每个组件通过 `WidgetComponent` 暴露必要能力，运行态只由组件实例和树 side table 持有；公开教程与参数表归[使用 · 组件](../../使用/界面构建/组件.md)。

## 模块不变量

内置组件不直接访问 platform/backend 或全局单例；业务闭包只能登记到所属树 side table，组件值与快照只保存稳定签名。图标、文本布局、overlay、虚拟滚动复用 ui System 编排的对应能力，不复制平行实现。

输入、上传、RichText、二维码和图片等外部内容都按不可信数据处理，并在解析、大小、资源与敏感值边界复用对应模块契约。组件的“操作已接纳”“状态已改变”“布局已完成”和“画面已呈现”是不同事实，不得用一次回调或布尔值混为同一成功结果。
