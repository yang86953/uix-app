# uix-lang 快速开始

[← 返回使用 索引](../使用.md)

> **接口**：声明 UIX 推荐界面描述方式（uix-lang）的第一个应用——依赖配置、Hello World、计数器。依赖：[uix-lang 语言规范](../uix-lang/README.md)、[标签语法](../uix-lang/标签语法.md)。导出：uix-lang 教程 → [uix-lang 教程](uix-lang%20教程.md)。
>
> **状态声明**：uix-lang 为推荐界面描述方式，本页示例为**目标设计**（`uix!` 转换器尚未实现）；当前可运行入口是 [快速开始（Rust 版）](快速开始.md)，两种方式语义一一对应。

## 添加依赖

```toml
[dependencies]
uix = { path = "../uix-app" }
```

当前 crate 设为 `publish = false`，按 `yangyanhui` 专有、仅限内部授权使用的方式交付，不发布到 crates.io，也不对外分发源码或二进制；`path` 请改成已授权内部应用仓库到本仓库或内部 `.crate` 解包目录的实际相对路径。交付物与平台 / feature 边界见[交付与许可](../产品/交付与许可.md)。

## 第一个应用

界面就是一个 `.uix` 文档（如 `main.uix`），在 Rust 入口中通过 `uix!` 宏引入：

```uix
<App title="Hello UIX" size="400x300">
  <Text fontSize="heading2">Hello, world!</Text>
</App>
```

```rust
// Rust 入口只负责启动与配置，界面在 .uix 文档中声明
use uix::prelude::*;

uix!("main.uix");
```

要点：

- **根元素 `<App>`**：声明窗口标题、尺寸与初始主题（属性见[标签语法 · 文档结构](../uix-lang/标签语法.md#21-文档与根元素)）。
- **界面即声明**：没有组件构造代码，`<Text>` 直接对应文本组件。
- **文本插值**：`{expr}` 在文本节点中即插值。

## 带交互的计数器

计数器需要状态：`<Component>` 内用 `state` 声明，事件用 `@click` 绑定，更新用 `setState`：

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

## 使用样式类与主题

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
| 第一个应用 | 本页上例 | [快速开始 · 第一个应用](快速开始.md#第一个应用) |
| 计数器 | 本页上例 | [快速开始 · 带交互的计数器](快速开始.md#带交互的计数器) |
| 完整应用 | [uix-lang 教程](uix-lang%20教程.md) | [教程](教程.md) |

两种方式语义一一对应：uix-lang 是 Rust 声明式 API 的文本投影，编译时由框架自动转换。
