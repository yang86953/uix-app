// 声明本文件只编译导航路由文档，不启动应用或真实窗口。
#![allow(dead_code)]

// 隔离 tabs 围栏中的基础标签页。
mod tabs {
    // 引入文档承诺的标签页公开 prelude。
    use uix::prelude::*;

    // 编译带稳定 key 的非受控标签页构建器。
    fn compile_example() {
        // 构造两个标签并声明初始活动索引。
        let _tabs = Tabs::new()
            // 提交由 Tabs 组件拥有的标签列表。
            .tabs(vec![
                // 声明第一个标签。
                Tab {
                    // 提供用户可见标签。
                    label: "标签1".into(),
                    // 提供稳定标签 key。
                    key: "tab-1".into(),
                    // 声明当前标签没有图标。
                    icon: "".into(),
                },
                // 声明第二个标签。
                Tab {
                    // 提供用户可见标签。
                    label: "标签2".into(),
                    // 提供稳定标签 key。
                    key: "tab-2".into(),
                    // 声明当前标签没有图标。
                    icon: "".into(),
                },
            ])
            // 激活第一个标签。
            .active(0);
    }
}

// 隔离 nav-tabs-advanced 围栏中的标签页高级配置。
mod nav_tabs_advanced {
    // 引入文档承诺的标签页公开 prelude。
    use uix::prelude::*;

    // 编译位置、编辑、滚动与图标配置。
    fn compile_example() {
        // 构造顶部标签页。
        let _top = Tabs::new().tab_position(TabPosition::Top);
        // 构造左侧标签页。
        let _left = Tabs::new().tab_position(TabPosition::Left);

        // 构造带新增与关闭回调的可编辑标签页。
        let _editable = Tabs::editable()
            // 注册新增事实回调。
            .on_add(|_key| {})
            // 注册关闭事实回调。
            .on_close(|_key| {});

        // 构造溢出时滚动的标签页。
        let _scrollable = Tabs::new().scrollable(true);

        // 构造带星形图标的标签。
        let _icon = Tab::new("标签").icon("star");
    }
}

// 隔离 dropdown 围栏中的基础下拉菜单。
mod dropdown {
    // 引入文档承诺的下拉菜单公开 prelude。
    use uix::prelude::*;

    // 编译由字符串选项构造的下拉菜单。
    fn compile_example() {
        // 构造包含编辑、删除与导出的操作菜单。
        let _dropdown = Dropdown::new("操作").items(vec!["编辑", "删除", "导出"]);
    }
}

// 隔离 nav-dropdown-advanced 围栏中的结构化下拉菜单。
mod nav_dropdown_advanced {
    // 引入文档承诺的下拉菜单公开 prelude。
    use uix::prelude::*;

    // 编译子菜单、分隔线、禁用项、图标与触发模式。
    fn compile_example() {
        // 构造带子菜单与分隔线的结构化下拉菜单。
        let _nested = Dropdown::new("操作").items(vec![
            // 添加编辑动作。
            DropdownItem::new("编辑"),
            // 添加不可选择的分隔线。
            DropdownItem::divider(),
            // 添加包含导入和导出的子菜单。
            DropdownItem::new("更多").children(vec![
                // 添加导入子项。
                DropdownItem::new("导入"),
                // 添加导出子项。
                DropdownItem::new("导出"),
            ]),
        ]);

        // 构造禁用的删除项。
        let _disabled = DropdownItem::new("删除").disabled(true);

        // 构造带下载图标的导出项。
        let _icon = DropdownItem::new("导出").icon("download");

        // 构造悬停触发的下拉菜单。
        let _hover = Dropdown::new("操作").trigger(TriggerMode::Hover);
        // 构造上下文菜单触发的下拉菜单。
        let _context = Dropdown::new("操作").trigger(TriggerMode::ContextMenu);
    }
}

// 隔离 typed-navigation-route 围栏中的枚举页面路由。
mod typed_navigation_route {
    // 引入文档承诺的导航、状态与视图公开 prelude。
    use uix::prelude::*;

    // 声明应用拥有的类型化页面 key。
    #[derive(Clone, PartialEq)]
    // 枚举三个可导航页面。
    enum Page {
        // 首页路由。
        Home,
        // 设置路由。
        Settings,
        // 关于路由。
        About,
    }

    // 为语义事件与快照实现稳定文本 key。
    impl std::fmt::Display for Page {
        // 把枚举路由格式化为稳定短 key。
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // 写入与当前枚举变体匹配的路由文本。
            formatter.write_str(match self {
                // 首页映射为 home。
                Self::Home => "home",
                // 设置映射为 settings。
                Self::Settings => "settings",
                // 关于映射为 about。
                Self::About => "about",
            })
        }
    }

    // 编译同一 Page State 驱动导航选中与页面内容的组合。
    fn routed_view(theme: &Theme, current_page: &State<Page>) -> ViewNode {
        // 横向组合复合导航与响应式页面节点。
        row((
            // 嵌入使用同一 typed State 的复合导航。
            embed(
                // 构造类型化导航组件。
                Navigation::new("MyApp")
                    // 添加首页 key。
                    .item("首页", Page::Home)
                    // 添加设置 key。
                    .item("设置", Page::Settings)
                    // 添加关于 key。
                    .item("关于", Page::About)
                    // 双向绑定应用页面真值。
                    .active_page(current_page)
                    // 隐藏版本信息。
                    .show_version(false)
                    // 使用应用主题令牌构建复合节点。
                    .build(theme.tokens()),
            ),
            // 由同一页面 State 映射当前页面内容。
            current_page.map(|page| match page {
                // 首页展示首页文本。
                Page::Home => label("首页"),
                // 设置页展示设置文本。
                Page::Settings => label("设置"),
                // 关于页展示关于文本。
                Page::About => label("关于"),
            }),
        ))
    }
}

// 隔离 typed-tabs-route 围栏中的枚举标签路由。
mod typed_tabs_route {
    // 引入文档承诺的标签页、状态与视图公开 prelude。
    use uix::prelude::*;

    // 声明应用拥有的类型化标签 key。
    #[derive(Clone, PartialEq)]
    // 枚举列表与详情标签。
    enum TabPage {
        // 列表标签。
        List,
        // 详情标签。
        Detail,
    }

    // 为语义事件与快照实现稳定文本 key。
    impl std::fmt::Display for TabPage {
        // 把枚举标签格式化为稳定短 key。
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // 写入与当前枚举变体匹配的标签文本。
            formatter.write_str(match self {
                // 列表映射为 list。
                Self::List => "list",
                // 详情映射为 detail。
                Self::Detail => "detail",
            })
        }
    }

    // 编译同一 TabPage State 驱动 Tabs 与内容的组合。
    fn tabbed_view(active_tab: &State<TabPage>) -> ViewNode {
        // 纵向组合受控 Tabs 与响应式内容。
        column((
            // 嵌入类型化受控 Tabs。
            embed(Tabs::controlled(
                // 声明标签文本与枚举 key 的一一映射。
                [("列表", TabPage::List), ("详情", TabPage::Detail)],
                // 双向绑定应用标签真值。
                active_tab,
            )),
            // 由同一标签 State 映射当前内容。
            active_tab.map(|key| match key {
                // 列表标签展示列表内容。
                TabPage::List => label("列表内容"),
                // 详情标签展示详情内容。
                TabPage::Detail => label("详情内容"),
            }),
        ))
    }
}

// 隔离 navigation-typed-route 围栏中的完整 App 构建器。
mod navigation_typed_route {
    // 引入文档承诺的应用、导航与派生宏公开 prelude。
    use uix::prelude::*;

    // 声明应用唯一的类型化路由真值。
    #[derive(Clone, PartialEq, Display)]
    // 枚举完整应用页面。
    enum Route {
        // 首页路由。
        Home,
        // 工作台路由。
        Workspace,
        // 设置路由。
        Settings,
    }

    // 把当前路由映射为页面 ViewNode。
    fn page_for(route: &Route) -> ViewNode {
        // 按类型化路由选择用户可见页面。
        match route {
            // 构建首页内容。
            Route::Home => label("首页").into(),
            // 构建工作台内容。
            Route::Workspace => label("工作台").into(),
            // 构建设置页内容。
            Route::Settings => label("设置").into(),
        }
    }

    // 编译 App 根闭包中的类型化导航，不调用 run。
    fn compile_example() {
        // 创建应用主题。
        let theme = Theme::antd_light();
        // 创建由应用拥有的路由 State。
        let route = State::new(Route::Home);

        // 构建应用生命周期与根视图声明。
        let _app = App::new().root(move || {
            // 横向组合导航与当前页面。
            row((
                // 嵌入双向绑定同一 Route State 的导航。
                embed(
                    // 构造类型化应用导航。
                    Navigation::new("UIX")
                        // 添加首页路由。
                        .item("首页", Route::Home)
                        // 添加工作台路由。
                        .item("工作台", Route::Workspace)
                        // 添加设置路由。
                        .item("设置", Route::Settings)
                        // 双向绑定应用路由真值。
                        .active_page(&route)
                        // 使用同一应用主题构建导航节点。
                        .build(theme.tokens()),
                ),
                // 由同一路由 State 映射当前页面。
                route.map(page_for),
            ))
        });
    }
}
