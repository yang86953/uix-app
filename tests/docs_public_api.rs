// 声明本文件只编译公开用法文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 quick-start-hello 围栏中的 main 名称与公开导入。
mod quick_start_hello {
    // 引入文档承诺的公开 prelude。
    use uix::prelude::*;

    // 定义但不调用文档中的第一个应用入口。
    fn main() {
        // 构造文档声明的 Hello UIX 应用。
        App::new()
            // 设置公开窗口标题。
            .title("Hello UIX")
            // 设置公开窗口尺寸。
            .size(400, 300)
            // 使用公开 Label builder 构造根 View。
            .root(|| label("Hello, world!").font_size(32.0))
            // 保留完整公开运行入口，只由编译器检查而不在测试中调用。
            .run();
    }
}

// 隔离 quick-start-counter 围栏中的 main 名称与状态闭包。
mod quick_start_counter {
    // 引入文档承诺的公开 prelude。
    use uix::prelude::*;

    // 定义但不调用文档中的计数器应用入口。
    fn main() {
        // 创建文档声明的响应式计数状态。
        let count = State::new(0);

        // 构造完整计数器应用。
        App::new()
            // 设置公开窗口标题。
            .title("计数器")
            // 设置公开窗口尺寸。
            .size(360, 200)
            // 通过拥有型闭包构造响应式根 View。
            .root(move || {
                // 组合动态文本与两个状态更新按钮。
                column((
                    // 把整数状态映射为公开动态文本节点。
                    count.map_text(|n| format!("{n}")).font_size(48.0),
                    // 横向组合递减与递增按钮。
                    row((
                        // 递减按钮通过公开 State 更新入口修改状态。
                        button("-1").on_click(&count, |c| c.update(|v| *v -= 1)),
                        // 递增按钮同时验证 primary 样式与状态更新组合。
                        button("+1")
                            // 使用公开主按钮样式。
                            .primary()
                            // 点击后通过公开 State 更新入口递增。
                            .on_click(&count, |c| c.update(|v| *v += 1)),
                    ))
                    // 设置按钮行间距。
                    .gap(8.0),
                ))
                // 设置计数器纵向间距。
                .gap(16.0)
                // 设置计数器根内边距。
                .padding(24.0)
            })
            // 保留完整公开运行入口，只由编译器检查而不在测试中调用。
            .run();
    }
}

// 隔离 quick-start-typed-route 围栏中的页面类型与辅助函数。
mod quick_start_typed_route {
    // 引入文档承诺的公开 prelude。
    use uix::prelude::*;

    // 派生文档声明的克隆与相等比较能力。
    #[derive(Clone, PartialEq)]
    // 声明类型化页面集合。
    enum Page {
        // 声明首页路由。
        Home,
        // 声明设置页路由。
        Settings,
    }

    // 为页面 key 实现 Navigation 所需的公开文本契约。
    impl std::fmt::Display for Page {
        // 把页面 key 写入格式化输出。
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // 按变体返回稳定路由名称。
            formatter.write_str(match self {
                // 首页使用稳定 home key。
                Self::Home => "home",
                // 设置页使用稳定 settings key。
                Self::Settings => "settings",
            })
        }
    }

    // 声明首页的最小公开 View 构造函数。
    fn home_view() -> ViewNode {
        // 返回可直接参与页面组合的首页节点。
        label("首页")
    }

    // 声明设置页的最小公开 View 构造函数。
    fn settings_view() -> ViewNode {
        // 返回可直接参与页面组合的设置页节点。
        label("设置")
    }

    // 编译文档中的主题化类型路由应用根。
    fn app_root(theme: &Theme, page: &State<Page>) -> ViewNode {
        // 纵向组合导航与当前页面节点。
        column((
            // 把 Navigation 组件嵌入公开 ViewNode。
            embed(
                // 创建以 Page 为 key 的导航组件。
                Navigation::<Page>::new("我的应用")
                    // 登记首页导航项。
                    .item("首页", Page::Home)
                    // 登记设置页导航项。
                    .item("设置", Page::Settings)
                    // 绑定受控页面状态。
                    .active_page(page)
                    // 使用当前主题 token 构建导航组件。
                    .build(theme.tokens()),
            ),
            // 根据页面状态映射当前内容节点。
            page.map(|current| match current {
                // 首页状态返回首页 View。
                Page::Home => home_view(),
                // 设置状态返回设置 View。
                Page::Settings => settings_view(),
            }),
        ))
    }
}

// 隔离 route-display-derive 围栏中的同名页面类型。
mod route_display_derive {
    // 引入公开 Display 派生宏及基础 trait。
    use uix::prelude::*;

    // 验证公开派生宏可与文档声明的 trait 组合。
    #[derive(Clone, PartialEq, Display)]
    // 声明无需手写 Display 实现的页面集合。
    enum Page {
        // 声明首页路由。
        Home,
        // 声明设置页路由。
        Settings,
    }
}
