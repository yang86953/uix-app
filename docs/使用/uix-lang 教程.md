# uix-lang 教程 —— 从零搭建一个待办应用

[← 返回使用 索引](../使用.md)

> **接口**：声明用 uix-lang（推荐界面描述方式）从零构建完整应用的连续教程。依赖：[uix-lang 快速开始](uix-lang%20快速开始.md)、[标签语法](../uix-lang/标签语法.md)。导出：各主题的详细用法。
>
> **状态声明**：本教程为**目标设计**（`uix!` 转换器尚未实现）；当前可运行对照为 [教程（Rust 版）](教程.md)，两种方式语义一一对应。

本教程用一个完整的 TODO 应用带你走完 uix-lang 的核心：文档结构、状态、事件、表单、列表渲染、样式与主题。

> 前置：已按[uix-lang 快速开始](uix-lang%20快速开始.md)配好 `Cargo.toml`。

## 1. 最小骨架

一个 `.uix` 文档：根元素 `<App>` 声明窗口，界面树声明在根内。

```uix
<App title="待办" size="400x500">
  <Text fontSize="heading3">Hello, TODO!</Text>
</App>
```

对应 Rust 入口：

```rust
use uix::prelude::*;

fn main() {
    uix!("main.uix");
}
```

## 2. 用 state 管理数据

状态在 `<Component>` 内声明：`state="名称: 初始值"`，组件内以 `{名称}` 读取。待办列表的初始值为空数组：

```uix
<Component name="TodoApp" state="todos: [], inputText: ''">
  <Text>当前有 {todos.length} 条待办</Text>
</Component>
```

- 数组状态初始值写 `[]`（空数组，对应 Rust `Vec::new()`）。
- 读取用属性访问路径：`{todos.length}`、`{todos[0]}`。

## 3. 组合视图：输入框 + 列表 + 按钮

添加待办：`@click` 中 `setState` 用不可变数组操作 `push`；列表用 `<For>` 渲染，删除用索引 `removeAt`：

```uix
<App title="待办" size="400x500">
  <Component name="TodoApp" state="todos: [], inputText: ''">
    <Container direction="column" gap="16px" padding="16px">

      // 输入区：双向绑定 inputText
      <Container direction="row" gap="8px">
        <Input value={inputText} placeholder="输入待办事项" flexGrow={1} />
        <Button type="primary"
                @click="setState(todos: todos.push(inputText), inputText: '')">
          添加
        </Button>
      </Container>

      // 列表：For 渲染 + 索引删除
      <For {todo} {i} in {todos}>
        <Container direction="row" gap="8px">
          <Text flexGrow={1}>{i + 1}. {todo}</Text>
          <Button @click="setState(todos: todos.removeAt(i))">完成</Button>
        </Container>
      </For>

    </Container>
  </Component>

  <TodoApp />
</App>
```

- **双向绑定**：`value={inputText}` 输入框内容直接写回状态。
- **追加**：`todos.push(inputText)` 返回新数组（不可变更新，原数组不变）。
- **删除**：`<For {todo} {i} in {todos}>` 声明索引绑定，`todos.removeAt(i)` 删除当前项。

## 4. 美化界面：样式类

把外观提取为顶层样式类，元素用 `class` 引用：

```uix
// 顶层声明区
appBg {
  backgroundColor: #F5F7FA;
  borderRadius: 12px;
  padding: 20px;
  gap: 12px;
}

todoCard {
  backgroundColor: white;
  borderRadius: 8px;
  padding: 12px 8px;
  gap: 8px;
}

<Component name="TodoApp" state="todos: [], inputText: ''">
  <Container class="appBg" direction="column">
    <For {todo} {i} in {todos}>
      <Container class="todoCard" direction="row" gap="8px">
        <Text flexGrow={1}>{i + 1}. {todo}</Text>
        <Button @click="setState(todos: todos.removeAt(i))">完成</Button>
      </Container>
    </For>
  </Container>
</Component>
```

- 样式类语法与[样式属性参考](../uix-lang/UIX%20样式属性参考.md)一致：`属性: 值;`。
- 优先级：内联 `style` > 样式类 `class` > 主题默认。

## 5. 主题

```uix
@theme light {
  primaryColor: #2196F3;
  backgroundColor: #F5F7FA;
}

@theme dark {
  primaryColor: #64B5F6;
  backgroundColor: #121212;
}

<App title="待办" size="400x500" theme="light">
  <Component name="TodoApp" state="todos: [], inputText: ''">
    <Container class="appBg" style="backgroundColor: #backgroundColor" direction="column">
      <Button type="primary" style="backgroundColor: #primaryColor">添加</Button>
    </Container>
  </Component>
  <TodoApp />
</App>
```

- 样式值中 `#属性名` 引用当前主题属性（与十六进制颜色的区分见[标签语法 · 主题](../uix-lang/标签语法.md#92-引用主题属性)）。
- 运行期切换：`@click="setTheme('dark')"`。

## 6. 框架能力仍在 Rust 入口

持久化、定时器、多窗口等**框架能力不属于界面描述**，仍在 Rust 入口配置（uix-lang 只描述界面，符合「Rust 优先——没有第二门运行语言」）：

```rust
// 持久化：Settings 仍在 Rust 侧配置与读写（见 配置.md）
use uix::prelude::*;
use uix::data::SettingsService;

fn main() {
    // uix!("main.uix") 生成的 App 继续链式配置框架能力
    let app = uix!("main.uix")
        .settings("todos.json")          // 持久化文件
        .on_start(|handle| {
            // 从磁盘加载已保存的待办，写入 uix 暴露的状态
        });
    app.run();
}
```

详细用法参考[配置](配置.md)。uix-lang 对应能力（如 `settings` 属性、`@onStart`）为目标设计，落地后本教程会更新。

## 7. 完整示例

```uix
@theme light {
  primaryColor: #2196F3;
  backgroundColor: #F5F7FA;
}

appBg {
  backgroundColor: #F5F7FA;
  borderRadius: 12px;
  padding: 20px;
  gap: 12px;
}

todoCard {
  backgroundColor: white;
  borderRadius: 8px;
  padding: 12px 8px;
  gap: 8px;
}

<Component name="TodoApp" state="todos: [], inputText: ''">
  <Container class="appBg" direction="column">
    <Container direction="row" gap="8px">
      <Input value={inputText} placeholder="输入待办事项" flexGrow={1} />
      <Button type="primary"
              @click="setState(todos: todos.push(inputText), inputText: '')">
        添加
      </Button>
    </Container>

    <For {todo} {i} in {todos}>
      <Container class="todoCard" direction="row" gap="8px">
        <Text flexGrow={1}>{i + 1}. {todo}</Text>
        <Button @click="setState(todos: todos.removeAt(i))">完成</Button>
      </Container>
    </For>
  </Container>
</Component>

<App title="待办" size="400x500" theme="light">
  <TodoApp />
</App>
```

## 下一步

- 深入语言细节：[标签语法](../uix-lang/标签语法.md)、[样式属性参考](../uix-lang/UIX%20样式属性参考.md)
- 浏览内置组件：[组件速查](组件速查.md) 与 [uix-lang 内置组件](../uix-lang/README.md#文档地图)
- 框架能力（响应式、动画、配置）仍以 [Rust 版使用文档](../使用.md) 为权威
