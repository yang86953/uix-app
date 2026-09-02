// 声明本文件只编译导航基础文档，不启动窗口或真实滚动宿主。
#![allow(dead_code)]

// 隔离 menu-bar 围栏中的横向菜单栏。
mod menu_bar {
    // 引入文档承诺的导航组件公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "menu-bar";

    // 编译带稳定 key 的横向菜单栏构建器。
    fn compile_example() {
        // 构造两个顶级菜单及带分隔线的条目集合。
        let _menu_bar = MenuBar::new()
            // 提交由菜单栏组件拥有的 keyed 顶级菜单集。
            .keyed_menus(vec![
                // 声明文件菜单及稳定 key。
                MenuBarMenu::from_text("文件", "file")
                    // 追加图标叶子条目。
                    .item(MenuBarItem::from_text("新建任务", "file-new-task").icon("plus"))
                    // 追加分隔线条目。
                    .item(MenuBarItem::divider())
                    // 追加禁用叶子条目。
                    .item(MenuBarItem::from_text("导入", "file-import").disabled(true)),
                // 声明视图菜单及稳定 key。
                MenuBarMenu::from_text("视图", "view")
                    // 追加普通叶子条目。
                    .item(MenuBarItem::from_text("导航页", "view-nav")),
            ]);
        // 读取 keyed 门禁诊断的公开入口保持可用。
        let _diagnostics = _menu_bar.diagnostics();
        // 读取打开入口索引的公开查询保持可用。
        let _open = _menu_bar.open_index();
        // 构建 View 保持公开契约可用。
        let _view: ViewNode = MenuBar::default().build();
    }
}

// 隔离 menu-navigation 围栏中的基础菜单。
mod menu_navigation {
    // 引入文档承诺的导航组件公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "menu-navigation";

    // 编译带稳定 key 的垂直菜单构建器。
    fn compile_example() {
        // 构造两个叶菜单项及活动项配置。
        let _menu = Menu::new()
            // 提交由菜单组件拥有的条目列表。
            .items(vec![
                // 声明首页菜单项。
                MenuItem {
                    // 提供稳定导航 key。
                    key: "home".into(),
                    // 提供用户可见标签。
                    label: "首页".into(),
                    // 提供图标标识。
                    icon: "home".into(),
                    // 声明叶节点没有子菜单。
                    children: vec![],
                    // 允许当前项参与交互。
                    disabled: false,
                },
                // 声明设置菜单项。
                MenuItem {
                    // 提供稳定导航 key。
                    key: "settings".into(),
                    // 提供用户可见标签。
                    label: "设置".into(),
                    // 提供图标标识。
                    icon: "settings".into(),
                    // 声明叶节点没有子菜单。
                    children: vec![],
                    // 允许当前项参与交互。
                    disabled: false,
                },
            ])
            // 声明初始活动 key。
            .active_key("home")
            // 使用垂直键盘导航模式。
            .mode(MenuMode::Vertical);
    }
}

// 隔离 nav-menu-advanced 围栏中的菜单高级配置。
mod nav_menu_advanced {
    // 引入文档承诺的菜单公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "nav-menu-advanced";

    // 编译嵌套、内联、受控 key 与图标配置。
    fn compile_example() {
        // 构造带两个子项的系统菜单项。
        let _nested = MenuItem::new("系统").children(vec![
            // 添加用户子项。
            MenuItem::new("用户"),
            // 添加角色子项。
            MenuItem::new("角色"),
        ]);

        // 构造始终展开的内联菜单。
        let _inline = Menu::new().mode(MenuMode::Inline);

        // 创建由调用方拥有的受控选中 key 集合。
        let selected_keys = vec!["home".to_string()];
        // 把选中集合交给菜单声明。
        let _selected = Menu::new().selected_keys(&selected_keys);

        // 创建由调用方拥有的受控展开 key 集合。
        let open_keys = vec!["system".to_string()];
        // 把展开集合交给菜单声明。
        let _opened = Menu::new().open_keys(&open_keys);

        // 构造带图标的首页菜单项。
        let _icon = MenuItem::new("首页").icon("home");
    }
}

// 隔离 navigation-sidebar 围栏中的主题化侧边栏。
mod navigation_sidebar {
    // 引入文档承诺的导航与主题公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "navigation-sidebar";

    // 编译带 typed key 的复合侧边栏及其主题节点。
    fn compile_example() {
        // 创建公开浅色主题。
        let theme = Theme::antd_light();
        // 声明应用侧边栏结构与配置。
        let sidebar = Navigation::new("MyApp")
            // 添加首页导航项。
            .item_with_icon("首页", "home", "home")
            // 添加设置导航项。
            .item_with_icon("设置", "settings", "settings")
            // 声明初始活动索引。
            .active_index(0)
            // 声明展开宽度。
            .width(220.0)
            // 启用紧凑图标模式。
            .compact(true)
            // 隐藏版本信息。
            .show_version(false);
        // 使用同一主题令牌构建并嵌入复合节点。
        let _sidebar_view = embed(sidebar.build(theme.tokens()));
    }
}

// 隔离 nav-collapse-advanced 围栏中的受控折叠状态。
mod nav_collapse_advanced {
    // 引入文档承诺的导航与状态公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "nav-collapse-advanced";

    // 编译受控折叠句柄与变化回调。
    fn compile_example() {
        // 创建由业务拥有的折叠状态。
        let sidebar_collapsed = State::new(false);
        // 把折叠状态绑定给字符串 key 导航组件。
        let _controlled = Navigation::<String>::new("MyApp").collapsed(&sidebar_collapsed);

        // 注册只消费折叠事实的窄回调。
        let _callback = Navigation::<String>::new("MyApp").on_collapse(|_collapsed| {});
    }
}

// 隔离 navigation-group 围栏中的共享选中组。
mod navigation_group {
    // 引入文档承诺的导航与视图公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "navigation-group";

    // 编译 NavGroup 到列视图的公开转换。
    fn compile_example() {
        // 构造共享活动索引的导航项列表。
        let group_items = NavGroup::new()
            // 添加首页项。
            .item("首页", "home")
            // 添加设置项。
            .item("设置", "settings")
            // 声明初始活动索引。
            .active_index(0)
            // 物化由组协调的 NavItem 列表。
            .build();
        // 把每个 NavItem 嵌入并组合成列视图。
        let _group = embed(column(
            // 显式映射成 ViewNode 集合。
            group_items.into_iter().map(embed).collect::<Vec<_>>(),
        ));
    }
}

// 隔离 pagination 围栏中的基础分页配置。
mod pagination {
    // 引入文档承诺的分页公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "pagination";

    // 编译当前页、总数与页尺寸选项。
    fn compile_example() {
        // 构造二百条数据、每页二十条的分页组件。
        let _pagination = Pagination::new(200, 20)
            // 从第二页开始。
            .current(2)
            // 显示总数摘要。
            .show_total(true)
            // 启用页尺寸切换。
            .show_size_changer(true)
            // 声明合法页尺寸集合。
            .page_size_options(vec![10, 20, 50, 100]);
    }
}

// 隔离 nav-pagination-advanced 围栏中的分页高级配置。
mod nav_pagination_advanced {
    // 引入文档承诺的分页公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "nav-pagination-advanced";

    // 编译简洁模式、跳转输入与总数模板。
    fn compile_example() {
        // 构造只显示前后页动作的分页组件。
        let _simple = Pagination::new(200, 20).simple(true);

        // 构造带页码跳转输入框的分页组件。
        let _jumper = Pagination::new(200, 20).show_jumper(true);

        // 构造由应用格式化范围摘要的分页组件。
        let _template = Pagination::new(200, 20).total_template(|total, range| {
            // 返回包含当前范围与总数的拥有型文本。
            format!("第 {}-{} 条 / 共 {total} 条", range.start, range.end)
        });
    }
}

// 隔离 steps-breadcrumb-anchor 围栏中的三类线性导航。
mod steps_breadcrumb_anchor {
    // 引入文档承诺的线性导航公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "steps-breadcrumb-anchor";

    // 编译步骤、面包屑与锚点的基础公开构建器。
    fn compile_example() {
        // 构造纵向订单步骤组件。
        let _steps = Steps::new(vec![
            // 添加带描述的填写信息步骤。
            Step::new("填写信息").description("录入收货地址"),
            // 添加带描述的确认订单步骤。
            Step::new("确认订单").description("核对商品与金额"),
            // 添加完成支付步骤。
            Step::new("完成支付"),
        ])
        // 声明当前步骤索引。
        .current(1)
        // 使用纵向布局和键盘方向。
        .vertical();

        // 构造带稳定链接和末项活动状态的面包屑。
        let _breadcrumb = Breadcrumb::new()
            // 提交完整面包屑条目列表。
            .items(vec![
                // 添加首页链接。
                BreadcrumbItem::new("首页").link("/"),
                // 添加用户管理链接。
                BreadcrumbItem::new("用户管理").link("/users"),
                // 添加当前详情页链接。
                BreadcrumbItem::new("详情").link("/users/detail").active(),
            ])
            // 声明可见分隔符。
            .separator(">");

        // 构造两个稳定 href 的锚点组件。
        let _anchor = Anchor::new(vec![
            // 添加基本信息锚点。
            AnchorItem::new("基本信息", "section-1"),
            // 添加高级设置锚点。
            AnchorItem::new("高级设置", "section-2"),
        ]);
    }
}

// 隔离 nav-steps-advanced 围栏中的步骤高级配置。
mod nav_steps_advanced {
    // 引入文档承诺的步骤公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "nav-steps-advanced";

    // 编译步骤状态、点击、圆点、图标与回调配置。
    fn compile_example() {
        // 构造完成状态步骤。
        let _finish = Step::new("完成").status(StepStatus::Finish);
        // 构造进行中状态步骤。
        let _process = Step::new("进行中").status(StepStatus::Process);
        // 构造等待状态步骤。
        let _wait = Step::new("等待").status(StepStatus::Wait);
        // 构造错误状态步骤。
        let _error = Step::new("失败").status(StepStatus::Error);

        // 创建可点击步骤的数据列表。
        let steps = vec![Step::new("A"), Step::new("B")];
        // 构造允许点击切换的步骤组件。
        let _clickable = Steps::new(steps).clickable(true);

        // 创建圆点样式步骤的数据列表。
        let steps = vec![Step::new("A"), Step::new("B")];
        // 构造圆点进度样式步骤组件。
        let _dot = Steps::new(steps).dot(true);

        // 构造带信用卡图标的支付步骤。
        let _icon = Step::new("支付").icon("credit-card");

        // 创建回调示例的数据列表。
        let steps = vec![Step::new("A"), Step::new("B")];
        // 注册只消费索引与步骤事实的回调。
        let _callback = Steps::new(steps).on_step(|_index, _step| {});
    }
}

// 隔离 nav-breadcrumb-advanced 围栏中的面包屑高级配置。
mod nav_breadcrumb_advanced {
    // 引入文档承诺的面包屑公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "nav-breadcrumb-advanced";

    // 编译条目图标与最大可见项配置。
    fn compile_example() {
        // 构造带首页图标的面包屑条目。
        let _icon = BreadcrumbItem::new("首页").icon("home");

        // 构造最多显示三项的面包屑组件。
        let _collapsed = Breadcrumb::new().max_items(3);
    }
}

// 隔离 nav-anchor-advanced 围栏中的锚点高级配置。
mod nav_anchor_advanced {
    // 引入文档承诺的锚点公开 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "nav-anchor-advanced";

    // 编译偏移、指示器、边界与自定义容器工厂。
    fn compile_example() {
        // 创建可复用的锚点条目列表。
        let items = vec![AnchorItem::new("首页", "home")];
        // 构造带目标偏移的锚点组件。
        let _offset = Anchor::new(items.clone()).target_offset(16.0);

        // 构造显示墨球指示器的锚点组件。
        let _ink = Anchor::new(items.clone()).show_ink(true);

        // 构造带滚动检测边界的锚点组件。
        let _bounds = Anchor::new(items.clone()).bounds(100.0);

        // 构造由当前 Anchor 实例拥有的最小自定义容器。
        let _container = Anchor::new(items).container(|| ());
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖导航基础文档的全部十一个围栏。
fn navigation_basic_rust_fences_compile_as_external_consumers() {
    // 收集十一个已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记菜单栏围栏。
        menu_bar::COMPILE_ID,
        // 登记菜单基础围栏。
        menu_navigation::COMPILE_ID,
        // 登记菜单高级围栏。
        nav_menu_advanced::COMPILE_ID,
        // 登记侧边栏围栏。
        navigation_sidebar::COMPILE_ID,
        // 登记折叠状态围栏。
        nav_collapse_advanced::COMPILE_ID,
        // 登记导航组围栏。
        navigation_group::COMPILE_ID,
        // 登记分页基础围栏。
        pagination::COMPILE_ID,
        // 登记分页高级围栏。
        nav_pagination_advanced::COMPILE_ID,
        // 登记步骤、面包屑与锚点围栏。
        steps_breadcrumb_anchor::COMPILE_ID,
        // 登记步骤高级围栏。
        nav_steps_advanced::COMPILE_ID,
        // 登记面包屑高级围栏。
        nav_breadcrumb_advanced::COMPILE_ID,
        // 登记锚点高级围栏。
        nav_anchor_advanced::COMPILE_ID,
    ];
    // 运行阶段核对消费者覆盖标识与 Markdown 围栏一致。
    assert_eq!(
        // 使用实际模块暴露的标识作为结果。
        compile_ids,
        // 使用导航文档当前声明的稳定标识作为期望。
        [
            // 菜单栏围栏标识。
            "menu-bar",
            // 菜单基础围栏标识。
            "menu-navigation",
            // 菜单高级围栏标识。
            "nav-menu-advanced",
            // 侧边栏围栏标识。
            "navigation-sidebar",
            // 折叠状态围栏标识。
            "nav-collapse-advanced",
            // 导航组围栏标识。
            "navigation-group",
            // 分页基础围栏标识。
            "pagination",
            // 分页高级围栏标识。
            "nav-pagination-advanced",
            // 步骤、面包屑与锚点围栏标识。
            "steps-breadcrumb-anchor",
            // 步骤高级围栏标识。
            "nav-steps-advanced",
            // 面包屑高级围栏标识。
            "nav-breadcrumb-advanced",
            // 锚点高级围栏标识。
            "nav-anchor-advanced",
        ],
    );
}
