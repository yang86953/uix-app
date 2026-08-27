// 声明本文件只编译 Rust 入门教程，不执行窗口、设置或渲染副作用。
#![allow(dead_code)]

// 隔离 tutorial-step1 围栏中的窗口入口。
mod tutorial_step1 {
    // 引入文档承诺的公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-step1";

    // 定义但不调用教程第一步应用入口。
    fn main() {
        // 构造最小待办应用。
        App::new()
            // 设置窗口标题。
            .title("待办")
            // 设置窗口尺寸。
            .size(400, 500)
            // 使用公开 Label builder 构造根 View。
            .root(|| label("Hello, TODO!").font_size(24.0))
            // 保留完整运行入口，只由编译器检查。
            .run();
    }
}

// 隔离 tutorial-step2 围栏中的待办状态。
mod tutorial_step2 {
    // 引入文档承诺的公开 State 类型。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-step2";

    // 定义但不调用待办状态读写组合。
    fn compile_example() {
        // 创建两个初始待办。
        let todos = State::new(vec!["学习 UIX".to_string(), "写一个应用".to_string()]);
        // 编译公开 get 与集合长度读取。
        println!("当前有 {} 条待办", todos.get().len());
        // 编译公开 update 与 Vec 追加组合。
        todos.update(|list| list.push("部署上线".to_string()));
    }
}

// 隔离 tutorial-step3 围栏中的完整交互应用入口。
mod tutorial_step3 {
    // 引入文档承诺的公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-step3";

    // 定义但不调用完整待办应用入口。
    fn main() {
        // 创建初始待办列表状态。
        let todos = State::new(vec!["学习 UIX".to_string(), "写一个应用".to_string()]);
        // 创建输入框文本状态。
        let input_text = State::new(String::new());

        // 构造交互待办应用。
        App::new()
            // 设置窗口标题。
            .title("待办")
            // 设置窗口尺寸。
            .size(400, 500)
            // 使用拥有型闭包构造响应式根 View。
            .root(move || {
                // 克隆输入状态供添加回调捕获。
                let input_for_add = input_text.clone();
                // 克隆待办状态供行回调捕获。
                let todos_for_rows = todos.clone();
                // 纵向组合输入区与动态列表。
                column((
                    // 横向组合受控输入框与添加按钮。
                    row((
                        // 构造受控待办输入框。
                        input()
                            // 声明输入提示。
                            .placeholder("输入待办事项")
                            // 绑定公开 State<String>。
                            .value(&input_text)
                            // 让输入框占用剩余宽度。
                            .flex_grow(1.0),
                        // 构造主操作添加按钮。
                        button("添加").primary().on_click(&todos, move |list| {
                            // 在事件发生时读取最新输入文本。
                            let text = input_for_add.get();
                            // 空文本不追加待办。
                            if !text.is_empty() {
                                // 通过事件传入的 State 更新列表。
                                list.update(|items| items.push(text));
                                // 添加后清空受控输入框。
                                input_for_add.set(String::new());
                            }
                        }),
                    ))
                    // 设置输入区间距。
                    .gap(8.0),
                    // 把待办状态映射为动态列表 View。
                    todos.map(move |list| {
                        // 把每个待办映射为带完成按钮的行。
                        column(
                            // 遍历当前列表快照。
                            list.iter()
                                // 同时取得删除所需索引。
                                .enumerate()
                                // 映射为公开 Row View。
                                .map(|(index, item)| {
                                    // 为当前完成按钮克隆状态句柄。
                                    let row_todos = todos_for_rows.clone();
                                    // 构造待办文本与完成按钮行。
                                    row((
                                        // 展示一基序号与待办文本。
                                        label(format!("{}. {}", index + 1, item))
                                            // 让文本占用剩余宽度。
                                            .flex_grow(1.0),
                                        // 构造删除当前索引的完成按钮。
                                        button("完成").on_click(&row_todos, move |state| {
                                            // 通过公开 update 删除当前项。
                                            state.update(|items| {
                                                // 移除当前索引条目。
                                                items.remove(index);
                                            });
                                        }),
                                    ))
                                    // 设置行内间距。
                                    .gap(8.0)
                                    // 设置行垂直内边距。
                                    .padding_v(4.0)
                                })
                                // 收集为 Column 接受的 View 列表。
                                .collect::<Vec<_>>(),
                        )
                    }),
                ))
                // 设置根布局间距。
                .gap(16.0)
                // 设置根布局内边距。
                .padding(16.0)
            })
            // 保留完整运行入口，只由编译器检查。
            .run();
    }
}

// 隔离 tutorial-step4 围栏中的样式链。
mod tutorial_step4 {
    // 引入文档承诺的公开 View 与颜色 API。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-step4";

    // 编译教程中的根容器、待办卡片和输入框样式组合。
    fn compile_example() {
        // 构造浅色背景的圆角根容器。
        let _root = column((label("待办应用"),))
            // 设置十六进制背景色。
            .bg(Color::hex("#F5F7FA"))
            // 设置根圆角。
            .radius(12.0)
            // 设置根内边距。
            .padding(20.0)
            // 设置根子项间距。
            .gap(12.0);

        // 构造白色待办卡片行。
        let _card = row((
            // 让待办文本占用剩余宽度。
            label("1. 示例待办").flex_grow(1.0),
            // 构造主样式完成按钮。
            button("完成").primary().radius(4.0),
        ))
        // 设置卡片背景色。
        .bg(Color::WHITE)
        // 设置卡片圆角。
        .radius(8.0)
        // 设置水平内边距。
        .padding_h(12.0)
        // 设置垂直内边距。
        .padding_v(8.0)
        // 设置行内间距。
        .gap(8.0);

        // 构造带提示与圆角的输入框。
        let _input = input().placeholder("输入待办事项").radius(6.0);
    }
}

// 隔离 tutorial-step5 围栏中的动画应用入口。
mod tutorial_step5 {
    // 引入文档承诺的公开动画与应用 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-step5";

    // 定义但不调用动画待办应用入口。
    fn main() {
        // 创建从零过渡到一的淡入动画。
        let fade = Animated::new(0.0_f32).to(1.0, 0.3, Easing::ease_out);
        // 克隆动画句柄供根闭包捕获。
        let view_fade = fade.clone();

        // 构造动画待办应用。
        App::new()
            // 设置窗口标题。
            .title("动画待办")
            // 设置窗口尺寸。
            .size(300, 200)
            // 使用拥有型闭包构造动画根 View。
            .root(move || {
                // 组合标题与动画内容。
                column((
                    // 构造待办列表标题。
                    label("待办列表").font_size(18.0),
                    // 构造由动画值驱动透明度的内容列。
                    column((label("学习 UIX 动画").opacity(view_fade.value()),)).gap(8.0),
                ))
                // 设置根布局间距。
                .gap(12.0)
                // 设置根布局内边距。
                .padding(24.0)
            })
            // 保留完整运行入口，只由编译器检查。
            .run();
    }
}

// 隔离 tutorial-step6 围栏中的设置服务应用入口。
mod tutorial_step6 {
    // 引入文档承诺的公开设置服务路径。
    use uix::data::SettingsService;
    // 引入文档承诺的公开应用 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-step6";

    // 定义但不调用持久化待办应用入口。
    fn main() {
        // 创建空待办状态。
        let todos = State::new(Vec::<String>::new());
        // 克隆状态供启动回调加载设置。
        let todos_for_load = todos.clone();
        // 用状态槽承接容器里的设置服务。
        let services = State::new(None::<SettingsService>);
        // 克隆服务槽供启动回调写入。
        let services_for_load = services.clone();

        // 构造带设置文件和启动回调的应用。
        App::new()
            // 设置窗口标题。
            .title("待办")
            // 设置窗口尺寸。
            .size(400, 500)
            // 指定公开设置文件名。
            .settings("todos.json")
            // 在应用启动时从容器解析同一设置服务。
            .on_start(move |handle| {
                // 只在服务存在时读取保存值。
                if let Some(settings) = handle.resolve::<SettingsService>() {
                    // 把共享克隆交给界面闭包后续落盘使用。
                    services_for_load.set(Some(settings.clone()));
                    // 只在键存在时恢复待办列表。
                    if let Some(saved) = settings.get("todo_list") {
                        // 按非空行恢复拥有型字符串列表。
                        let loaded = saved
                            // 遍历保存文本的每一行。
                            .lines()
                            // 忽略空行。
                            .filter(|line| !line.is_empty())
                            // 转为拥有型字符串。
                            .map(str::to_string)
                            // 收集为公开状态接受的列表。
                            .collect::<Vec<String>>();
                        // 更新启动回调持有的待办状态。
                        todos_for_load.set(loaded);
                    }
                }
            })
            // 构造教程当前阶段的最小内容根。
            .root(move || column(()).padding(16.0))
            // 保留完整运行入口，只由编译器检查。
            .run();
    }
}

// 仅在文档声明的 settings-serde capability 下编译虚拟化持久化示例。
#[cfg(feature = "settings-serde")]
// 隔离 tutorial-virtualized-todo 围栏中的业务类型与函数。
mod tutorial_virtualized_todo {
    // 引入文档承诺的公开虚拟滚动、设置与响应式 API。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "tutorial-virtualized-todo";

    // 派生设置结构体编解码所需的公开 serde trait。
    #[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    // 声明类型化待办业务模型。
    struct Todo {
        // 持有稳定业务 key。
        id: u64,
        // 持有待办文本。
        text: String,
        // 持有持久化完成事实。
        done: bool,
    }

    // 构造按业务 key 协调的虚拟待办列表。
    fn virtualized_todo_view(
        // 接收业务所有者持有的待办状态。
        todos: &State<Vec<Todo>>,
        // 接收与待办业务 key 同步的完成状态句柄。
        done_flags: &State<Vec<State<bool>>>,
    ) -> ViewNode {
        // 克隆待办句柄供两个静态渲染闭包捕获。
        let render_todos = todos.clone();
        // 克隆完成状态集合句柄供行渲染闭包捕获。
        let render_done_flags = done_flags.clone();
        // 构造只物化可视区行的公开虚拟滚动节点。
        embed(
            // 创建虚拟滚动组件。
            VirtualScroll::new()
                // 使用当前待办快照声明列表长度。
                .item_count(render_todos.get().len())
                // 声明稳定行高。
                .item_height(40.0)
                // 按业务 id 提供稳定 key 和行渲染闭包。
                .render_keyed(
                    // 建立独立 key 闭包捕获作用域。
                    {
                        // 为 key 闭包保留独立 State 句柄。
                        let keyed_todos = render_todos.clone();
                        // 每次请求时读取当前业务 id。
                        move |index| keyed_todos.get()[index].id
                    },
                    // 按可视索引构造当前行 View。
                    move |index| {
                        // 先取得拥有型快照，避免借用临时 Vec。
                        let snapshot = render_todos.get();
                        // 克隆当前条目供返回 View 持有文本。
                        let todo = snapshot[index].clone();
                        // 克隆当前行的受控完成状态句柄。
                        let done = render_done_flags.get()[index].clone();
                        // 组合受控复选框与待办文本。
                        row((
                            // 把 Checkbox 组件嵌入当前行。
                            embed(Checkbox::new("").checked(&done)),
                            // 文本节点持有当前条目的拥有型字符串。
                            label(todo.text),
                        ))
                    },
                )
                // 完成虚拟滚动组件构建。
                .build(),
        )
    }

    // 编译类型化待办设置读写函数。
    fn persist_todos(settings: &SettingsService, todos: &State<Vec<Todo>>) -> Result<(), Error> {
        // 通过 feature 专属公开入口写入结构体列表。
        settings.set_struct("todos", &todos.get())?;
        // 通过必需读取入口恢复同一类型。
        let _restored: Vec<Todo> = settings.require_struct("todos")?;
        // 返回成功而不执行其它副作用。
        Ok(())
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖教程的全部可用围栏。
fn tutorial_rust_fences_compile_as_external_consumers() {
    // 先收集默认 capability 下的六个教程标识。
    let mut compile_ids = vec![
        // 登记教程第一步围栏。
        tutorial_step1::COMPILE_ID,
        // 登记教程第二步围栏。
        tutorial_step2::COMPILE_ID,
        // 登记教程第三步围栏。
        tutorial_step3::COMPILE_ID,
        // 登记教程第四步围栏。
        tutorial_step4::COMPILE_ID,
        // 登记教程第五步围栏。
        tutorial_step5::COMPILE_ID,
        // 登记教程第六步围栏。
        tutorial_step6::COMPILE_ID,
    ];
    // 在 settings-serde 下登记类型化虚拟待办围栏。
    #[cfg(feature = "settings-serde")]
    // 把 feature 专属标识加入同一覆盖集合。
    compile_ids.push(tutorial_virtualized_todo::COMPILE_ID);

    // 声明默认 capability 下的稳定期望标识。
    let mut expected_ids = vec![
        // 教程第一步围栏标识。
        "tutorial-step1",
        // 教程第二步围栏标识。
        "tutorial-step2",
        // 教程第三步围栏标识。
        "tutorial-step3",
        // 教程第四步围栏标识。
        "tutorial-step4",
        // 教程第五步围栏标识。
        "tutorial-step5",
        // 教程第六步围栏标识。
        "tutorial-step6",
    ];
    // 在 settings-serde 下声明 feature 专属期望标识。
    #[cfg(feature = "settings-serde")]
    // 把虚拟待办标识加入期望集合。
    expected_ids.push("tutorial-virtualized-todo");

    // 核对实际隔离模块与 Markdown fence 标识一致。
    assert_eq!(compile_ids, expected_ids);
}
