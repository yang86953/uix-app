# widgets 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui 系统的内置通用组件库及组件分类边界。依赖：[component](component.md)、[layout](layout.md)、[event](event.md)、[theme](theme.md)、[overlay](overlay.md)、[form](form.md)、[virtualization](virtualization.md)。导出：`uix::ui` 与 prelude 中的组件类型。
>
> **当前实现线索**：组件当前主要位于 `src/ui/widgets/`；重构允许按产品域重排文件，但所有内置组件仍遵循本模块契约。

## 组件清单

| 子模块 | 代表组件 | 架构职责 |
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

内置组件不直接访问 platform/backend，不保存业务闭包或全局单例；图标、文本布局、overlay、虚拟滚动复用对应模块，不复制平行实现。
