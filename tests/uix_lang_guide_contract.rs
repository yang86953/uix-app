// 导入指南示例需要的公开应用与宏契约。
use uix::prelude::*;

// 验证快速开始中的计数器文档可以通过当前 uix_app! 入口构造应用。
#[test]
fn quick_start_counter_builds_current_app_contract() {
    // 编译完整 App、组件私有状态、动态文本与事件更新，不进入窗口事件循环。
    let _app: App = uix::uix_app!(
        r#"
// 声明拥有私有计数状态的顶层可复用组件。
<Component name="Counter" state="count: 0">
  // 纵向组织动态文本与操作区。
  <Container direction="column" gap="16px" padding="24px">
    // 延迟读取计数状态并局部刷新文本。
    <Text fontSize="heading1">{count}</Text>
    // 横向组织递减与递增操作。
    <Container direction="row" gap="8px">
      // 通过已登记事件更新组件私有状态。
      <Button @click="setState(count: count - 1)">-1</Button>
      // 通过已登记事件更新组件私有状态。
      <Button type="primary" @click="setState(count: count + 1)">+1</Button>
    // 结束操作区。
    </Container>
  // 结束计数组件布局。
  </Container>
// 结束组件声明。
</Component>

// 声明当前公开应用根配置。
<App title="计数器" size="360x200">
  // 在应用根物化计数组件实例。
  <Counter />
// 结束应用根。
</App>
"#
    );
}

// 验证教程中的完整待办应用可以通过当前 uix_app! 入口构造应用。
#[test]
fn tutorial_todo_builds_current_app_contract() {
    // 编译主题、样式、类型化集合状态、输入绑定、不可变数组更新与循环渲染。
    let _app: App = uix::uix_app!(
        r#"
// 声明应用初始使用的完整主题。
@theme light {
  // 提供主题主色。
  primaryColor: #2196F3;
  // 提供主题背景色。
  backgroundColor: #F5F7FA;
// 结束主题声明。
}

// 声明应用根内容样式。
appBg {
  // 设置页面背景。
  backgroundColor: #F5F7FA;
  // 设置页面圆角。
  borderRadius: 12px;
  // 设置页面内边距。
  padding: 20px;
  // 设置页面子项间距。
  gap: 12px;
// 结束应用根样式。
}

// 声明单条待办样式。
todoCard {
  // 设置卡片背景。
  backgroundColor: white;
  // 设置卡片圆角。
  borderRadius: 8px;
  // 设置卡片内边距。
  padding: 12px 8px;
  // 设置卡片子项间距。
  gap: 8px;
// 结束待办卡片样式。
}

// 声明拥有类型化待办集合和输入文本的组件。
<Component name="TodoApp" state="todos: Vec<String> = [], inputText: String = ''">
  // 使用已登记样式构造纵向页面。
  <Container class="appBg" direction="column">
    // 横向组织输入框和添加按钮。
    <Container direction="row" gap="8px">
      // 双向绑定组件私有输入状态。
      <Input value={inputText} placeholder="输入待办事项" flexGrow={1} />
      // 原子更新集合与输入文本。
      <Button type="primary" @click="setState(todos: todos.push(inputText), inputText: '')">添加</Button>
    // 结束输入操作区。
    </Container>
    // 按稳定声明顺序渲染全部待办。
    <For {todo} {i} in {todos}>
      // 构造单条待办卡片。
      <Container class="todoCard" direction="row" gap="8px">
        // 展示序号与待办文本。
        <Text flexGrow={1}>{i}: {todo}</Text>
        // 使用当前索引不可变移除待办。
        <Button @click="setState(todos: todos.removeAt(i))">完成</Button>
      // 结束待办卡片。
      </Container>
    // 结束循环渲染。
    </For>
  // 结束待办页面布局。
  </Container>
// 结束待办组件声明。
</Component>

// 声明当前公开应用根和初始主题。
<App title="待办" size="400x500" theme="light">
  // 在应用根物化待办组件实例。
  <TodoApp />
// 结束应用根。
</App>
"#
    );
}
