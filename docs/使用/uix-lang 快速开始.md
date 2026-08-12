# uix-lang 快速开始

[← 返回使用 索引](../使用.md)

> **接口**：声明 UIX 推荐界面描述方式（uix-lang）的第一个编译期 View——依赖配置、Hello World，以及后续目标设计示例。依赖：[uix-lang 语言规范](../uix-lang/README.md)、[标签语法](../uix-lang/标签语法.md)。导出：uix-lang 教程 → [uix-lang 教程](uix-lang%20教程.md)。
>
> **状态声明**：uix-lang 为推荐界面描述方式。公开 `uix!` 已支持内嵌源码与 `.uix` 文件并返回 `ViewNode`；“第一个 View”示例是当前可编译契约。完整 `App` 应用入口、主题和包含未登记组件的示例仍是**目标设计**；当前应用 / 窗口启动方式见[快速开始（Rust 版）](快速开始.md)。

## 添加依赖

```toml
[dependencies]
uix = { path = "../uix-app" }
```

当前 crate 设为 `publish = false`，按 `yangyanhui` 专有、仅限内部授权使用的方式交付，不发布到 crates.io，也不对外分发源码或二进制；`path` 请改成已授权内部应用仓库到本仓库或内部 `.crate` 解包目录的实际相对路径。交付物与平台 / feature 边界见[交付与许可](../产品/交付与许可.md)。

## 当前可编译的第一个 View

把界面保存为调用 crate 下的 `src/main.uix`，并使用当前已登记的标签：

仓库内的可运行对应项目见 [`demo/uix-lang-demo`](../../demo/uix-lang-demo/README.md)；该项目由
`.uix` 文件生成界面，并由 Rust 薄入口持有 `App` 与窗口生命周期。

```uix
// 使用当前已登记的纵向容器作为根 View。
<Column gap="8px">
  // 创建当前已登记的文本 View。
  <Text fontSize="heading2">Hello, world!</Text>
  // 创建当前已登记的按钮 View。
  <Button>Continue</Button>
// 结束根 View。
</Column>
```

```rust
// 导入公开 uix! 宏与 ViewNode 类型。
use uix::prelude::*;

// 声明由应用组合根调用的界面构造函数。
fn build_view() -> ViewNode {
    // 编译期读取相对 CARGO_MANIFEST_DIR 的 .uix 文件。
    uix!("src/main.uix")
// 结束界面构造函数。
}
```

要点：

- **返回契约**：`uix!` 生成 `ViewNode` 表达式，不负责启动应用或窗口。
- **文件路径**：`.uix` 相对路径以调用 crate 的 `CARGO_MANIFEST_DIR` 为基准。
- **当前根元素**：使用已登记的 View 标签；`App` 应用入口仍为目标设计。
- **文本插值**：`{expr}` 在文本节点中即插值。

## 目标设计：带交互的完整计数器应用

组件 `state`、事件与 `setState` 的编译期展开已经实现，但下例使用的 `App` 应用入口仍为目标设计，因此**整段示例当前不可直接编译**：

```uix
<App title="计数器" size="360x200">
  <Component name="Counter" state="count: 0">
    <Container direction="column" gap="16px" padding="24px">
      <Text fontSize="heading1">{count}</Text>
      <Container direction="row" gap="8px">
        <Button @click="setState(count: count - 1)">-1</Button>
        <Button type="primary" @click="setState(count: count + 1)">+1</Button>
      </Container>
    </Container>
  </Component>

  <Counter />
</App>
```

要点：

- **状态**：`state="count: 0"` 声明私有状态，`{count}` 读取，`setState(count: 表达式)` 更新（命名参数）。
- **事件**：`@click="..."` 绑定点击；处理器是[受限表达式](../uix-lang/标签语法.md#6-表达式)。
- **样式**：`gap`、`padding` 等直接作属性；复杂样式用样式类（见下）。

## 目标设计：样式类与主题组成完整应用

样式类及已登记样式属性的编译期映射已经实现；下例还包含目标设计中的 `App` 与主题生成，因此**整段示例当前不可直接编译**：

```uix
// 顶层声明区：样式类与主题
baseButton {
  backgroundColor: white;
  borderRadius: 8px;
  padding: 16px;
}

@theme dark {
  primaryColor: #64B5F6;
  backgroundColor: #121212;
}

<App title="样式示例" size="400x300" theme="dark">
  <Container direction="column" gap="12px" padding="20px">
    <Button class="baseButton" type="primary">主题按钮</Button>
    <Button @click="setTheme('light')">切换主题</Button>
  </Container>
</App>
```

- **样式类**：`类名 { 属性: 值; }` 定义在顶层，`class` 属性引用。
- **主题**：`@theme` 定义、`<App theme="...">` 指定、`setTheme('主题名')` 切换（详见[主题](../uix-lang/标签语法.md#9-主题)）。

## 与 Rust 版的关系

| 场景 | uix-lang（推荐） | Rust 声明式 API（支持） |
|---|---|---|
| 当前编译期 View | 本页“当前可编译的第一个 View” | [快速开始 · 第一个应用](快速开始.md#第一个应用) |
| 完整计数器应用 | 本页目标设计 | [快速开始 · 带交互的计数器](快速开始.md#带交互的计数器) |
| 完整 TODO 应用 | [uix-lang 教程（目标设计）](uix-lang%20教程.md) | [教程](教程.md) |

已登记能力保持语义一一对应：uix-lang 是 Rust 声明式 API 的文本投影，编译时由框架自动转换。尚未登记的应用入口、组件与主题能力继续以目标设计标注，并由 Vikunja 项目 4 跟踪。
