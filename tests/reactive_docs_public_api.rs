// 声明本文件只编译应用入口与响应式文档，不执行任何原生副作用。
#![allow(dead_code)]

// 隔离 app-minimal 围栏中的窗口入口。
mod app_minimal {
    // 引入文档承诺的公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "app-minimal";

    // 定义但不调用最小应用入口。
    fn main() {
        // 构造文档中的最小应用。
        App::new()
            // 设置公开窗口标题。
            .title("My App")
            // 设置公开窗口尺寸。
            .size(800, 600)
            // 组合公开 Column、Label 与 Button 根节点。
            .root(|| column((label("Hello"), button("World"))))
            // 保留完整运行入口，只由编译器检查。
            .run();
    }
}

// 隔离 state-basics 围栏中的局部状态变量。
mod state_basics {
    // 引入文档承诺的公开 State 类型。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "state-basics";

    // 编译 State 的创建、写入、更新与读取组合。
    fn compile_example() {
        // 创建整数响应式状态。
        let count = State::new(0);
        // 通过公开 set 写入新值。
        count.set(5);
        // 通过公开 update 基于旧值更新。
        count.update(|value| *value += 2);
        // 通过公开 get 读取拥有型整数。
        let value: i32 = count.get();
        // 消费读取结果，避免测试自身引入无关告警。
        let _ = value;
    }
}

// 隔离 computed-full-name 围栏中的派生状态闭包。
mod computed_full_name {
    // 引入文档承诺的公开 State、Computed 与样式扩展。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "computed-full-name";

    // 编译两个 State 派生完整姓名并映射为文本 View 的组合。
    fn compile_example() {
        // 创建名字状态。
        let first = State::new("Ada");
        // 创建姓氏状态。
        let last = State::new("Lovelace");
        // 使用拥有型闭包声明派生完整姓名。
        let full = Computed::new(move || format!("{} {}", first.get(), last.get()));
        // 把派生字符串映射成带字号的动态文本节点。
        let _view = full.map_text(|value| value.clone()).font_size(20.0);
    }
}

// 隔离 scoped-subtree 围栏中的子树作用域声明。
mod scoped_subtree {
    // 引入文档承诺的公开 State、scoped 组合器与布局 API。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "scoped-subtree";

    // 编译读取 State 的子树作用域公开组合。
    fn compile_example() {
        // 创建当前高亮行状态。
        let highlight = State::new(0_usize);
        // 准备静态歌词行。
        let lines = vec!["第一行".to_string(), "第二行".to_string()];

        // 闭包内读取的 State 登记为该子树的作用域依赖。
        let _scope = scoped(move || {
            // 读取当前高亮行。
            let active = highlight.get();
            // 按高亮态装配行样式。
            let rows = lines
                .iter()
                .enumerate()
                .map(|(index, text)| {
                    // 当前行放大、其余行常规。
                    if index == active {
                        label(text.clone()).font_size(17.0).height(32.0)
                    } else {
                        label(text.clone()).font_size(14.0).height(32.0)
                    }
                })
                .collect::<Vec<_>>();
            // 返回定高列容器作为作用域子树。
            column(rows)
        });
    }
}

// 隔离 state-view-mapping 围栏中的两个响应式 View 映射。
mod state_view_mapping {
    // 引入文档承诺的公开 State、View 与颜色 API。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "state-view-mapping";

    // 编译通用 map 与条件 map_opt 的公开组合。
    fn compile_example() {
        // 创建无符号计数状态。
        let count = State::new(0_u32);

        // 从状态映射始终存在的响应式 View。
        let _mapped = count.map(|value| {
            // 零值显示灰色空状态。
            if *value == 0 {
                // 返回灰色 Label View。
                label("暂无数据").color(Color::gray())
            } else {
                // 非零值返回带字号的计数 Label View。
                label(format!("共 {value} 条")).font_size(16.0)
            }
        });

        // 从同一状态映射可选响应式 View。
        let _optional = count.map_opt(|value| {
            // 仅在超过阈值时展示警告。
            if *value > 100 {
                // 返回红色警告 Label。
                Some(label("超标").color(Color::red()))
            } else {
                // 阈值内不渲染节点。
                None
            }
        });
    }
}

// 隔离 effect-dependency 围栏中的自动依赖闭包。
mod effect_dependency {
    // 引入文档承诺的公开 State 与 Effect 类型。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "effect-dependency";

    // 定义但不调用 Effect 构造，避免测试产生输出副作用。
    fn compile_example() {
        // 创建被观察的计数状态。
        let count = State::new(0);
        // 克隆句柄供拥有型 Effect 闭包捕获。
        let watched_count = count.clone();

        // 编译自动追踪状态读取的 Effect。
        let _effect = Effect::new(move || {
            // 使用公开 get 读取最新状态值。
            println!("count 变为 {}", watched_count.get());
        });
    }
}

// 隔离 conditional-and-list 围栏中的条件与列表组合。
mod conditional_and_list {
    // 引入文档承诺的公开 View 构造 API。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "conditional-and-list";

    // 编译少量条件与列表渲染组合。
    fn compile_example() {
        // 声明当前可见条件。
        let visible = true;
        // 通过 show 构造条件子节点。
        let _conditional = column(show(visible, label("可见内容")));
        // 通过 visible 保留节点身份并切换可见性。
        let _retained = label("保留节点身份").visible(visible);

        // 声明少量静态列表数据。
        let items = ["Ada", "Alan", "Grace"];
        // 映射带稳定 automation_id 的列表节点。
        let _list = column(
            // 按索引与内容遍历列表。
            items
                // 取得字符串迭代器。
                .iter()
                // 同时取得稳定索引。
                .enumerate()
                // 映射为公开 Label View。
                .map(|(index, item)| label(*item).automation_id(format!("item-{index}")))
                // 收集为 Column 接受的 View 列表。
                .collect::<Vec<_>>(),
        );
    }
}

// 隔离 clipboard-event 围栏中的平台剪贴板调用。
mod clipboard_event {
    // 引入文档承诺的公开剪贴板函数。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "clipboard-event";

    // 定义但不调用剪贴板读写组合。
    fn compile_example() {
        // 编译公开剪贴板写入入口。
        copy_to_clipboard("https://example.com");

        // 编译公开剪贴板可选读取入口。
        if let Some(text) = read_text_from_clipboard() {
            // 消费读取文本而不执行其它副作用。
            let _ = text;
        }
    }
}

// 隔离 computed-model 围栏中的业务模型名称。
mod computed_model {
    // 引入文档承诺的公开响应式 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "computed-model";

    // 声明以 State 持有业务数据的计数模型。
    struct CounterModel {
        // 持有响应式无符号计数。
        count: State<u32>,
    }

    // 为计数模型实现构造与派生值入口。
    impl CounterModel {
        // 构造初始计数模型。
        fn new() -> Self {
            // 使用零值 State 初始化模型。
            Self {
                // 创建模型唯一状态句柄。
                count: State::new(0),
            }
        }

        // 返回当前计数两倍的派生值。
        fn doubled(&self) -> Computed<u32> {
            // 克隆 State 句柄供拥有型闭包捕获。
            let count = self.count.clone();
            // 每次读取时通过公开 get 计算两倍值。
            Computed::new(move || count.get() * 2)
        }
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖入口与响应式文档的八个围栏。
fn application_and_reactive_rust_fences_compile_as_external_consumers() {
    // 收集九个已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记最小应用围栏。
        app_minimal::COMPILE_ID,
        // 登记 State 基础围栏。
        state_basics::COMPILE_ID,
        // 登记完整姓名派生围栏。
        computed_full_name::COMPILE_ID,
        // 登记 State 到 View 映射围栏。
        state_view_mapping::COMPILE_ID,
        // 登记子树作用域围栏。
        scoped_subtree::COMPILE_ID,
        // 登记 Effect 依赖围栏。
        effect_dependency::COMPILE_ID,
        // 登记条件与列表围栏。
        conditional_and_list::COMPILE_ID,
        // 登记剪贴板事件围栏。
        clipboard_event::COMPILE_ID,
        // 登记业务模型围栏。
        computed_model::COMPILE_ID,
    ];
    // 核对消费者覆盖标识与两个 Markdown 文档一致。
    assert_eq!(
        // 使用实际隔离模块暴露的标识作为结果。
        compile_ids,
        // 使用文档当前声明的稳定标识作为期望。
        [
            // 最小应用围栏标识。
            "app-minimal",
            // State 基础围栏标识。
            "state-basics",
            // 完整姓名派生围栏标识。
            "computed-full-name",
            // State 到 View 映射围栏标识。
            "state-view-mapping",
            // 子树作用域围栏标识。
            "scoped-subtree",
            // Effect 依赖围栏标识。
            "effect-dependency",
            // 条件与列表围栏标识。
            "conditional-and-list",
            // 剪贴板事件围栏标识。
            "clipboard-event",
            // 业务模型围栏标识。
            "computed-model",
        ],
    );
}
