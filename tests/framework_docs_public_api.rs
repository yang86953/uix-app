// 声明本文件只编译框架能力使用文档，不启动窗口或修改资源表。
#![allow(dead_code)]

// 隔离 framework-di 围栏中的组合根依赖注入。
mod framework_di {
    // 引入文档承诺的公开 DI、AppHandle 与设置服务 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "framework-di";

    // 编译组合根注册与组件公开门面解析路径。
    fn compile_example(handle: &AppHandle) {
        // 创建进程级服务容器。
        let mut container = DiContainer::new();
        // 注册由容器共享所有权的设置服务单例。
        container.singleton(SettingsService::new());

        // 通过目标窗口句柄克隆解析同一设置服务实例。
        let _settings = handle.resolve::<SettingsService>();
    }
}

// 隔离 framework-provider 围栏中的子树配置继承。
mod framework_provider {
    // 引入文档承诺的公开 Provider、控件与 View prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "framework-provider";

    // 编译 Locale 与组件配置在独立子树中的覆写。
    fn compile_example() {
        // 构造只影响中文子树的 LocaleProvider。
        let _locale_tree = embed(
            // 创建简体中文语言作用域。
            LocaleProvider::new(zh_cn())
                // 在语言作用域内构建唯一子树。
                .child(|| {
                    // 纵向组合业务文本与内置空态组件。
                    column((
                        // 创建业务提供的中文标签。
                        label("中文界面"),
                        // 创建从 Locale 解析内置文案的空态组件。
                        embed(Empty::new().icon("inbox")),
                    ))
                })
                // 把 Provider 构建为公开 ViewNode。
                .build(),
        );

        // 构造只影响控件子树的 ConfigProvider。
        let _config_tree = embed(
            // 创建继承上层配置的作用域。
            ConfigProvider::new()
                // 覆写子树默认控件尺寸。
                .component_size(ControlSize::Large)
                // 覆写子树默认禁用状态。
                .disabled(true)
                // 在配置作用域内构建唯一子树。
                .child(|| {
                    // 组合继承配置的按钮与输入框。
                    column((
                        // 创建继承大尺寸的按钮。
                        button("继承 Large"),
                        // 创建继承禁用态的输入框。
                        input().placeholder("继承禁用态"),
                    ))
                })
                // 把 Provider 构建为公开 ViewNode。
                .build(),
        );
    }
}

// 隔离 framework-overrides 围栏中的组件窄覆盖。
mod framework_overrides {
    // 引入文档承诺的公开配置、令牌与组件 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "framework-overrides";

    // 编译组件构造覆盖与类型索引令牌补丁。
    fn compile_example() {
        // 创建空的组件构造覆盖集合。
        let mut overrides = ComponentOverrides::default();
        // 为子树中的 Input 统一注入货币前缀。
        overrides.input.prefix = Some("¥".to_string());

        // 构造应用 Input 覆写的配置子树。
        let _input_tree = embed(
            // 创建配置作用域并安装构造覆盖。
            ConfigProvider::new()
                // 替换当前子树的组件构造覆写。
                .overrides(overrides)
                // 在作用域内构造输入框。
                .child(|| input().placeholder("自动注入前缀"))
                // 把 Provider 构建为公开 ViewNode。
                .build(),
        );

        // 构造只覆写 Button 令牌的配置子树。
        let _button_tree = embed(
            // 创建继承上层值的配置作用域。
            ConfigProvider::new()
                // 为 Button 类型安装主色与圆角补丁。
                .component_tokens::<Button>(TokenPatch {
                    // 覆写按钮品牌主色。
                    color_primary: Some(Color::hex("#722ed1")),
                    // 覆写按钮默认圆角。
                    border_radius: Some(10.0),
                    // 其余令牌继续继承当前主题。
                    ..TokenPatch::default()
                })
                // 在作用域内构造主按钮。
                .child(|| button("紫色按钮").primary())
                // 把 Provider 构建为公开 ViewNode。
                .build(),
        );
    }
}

// 隔离 framework-locale 围栏中的应用语言组合根。
mod framework_locale {
    // 引入文档承诺的公开 Application、Locale 与 View prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "framework-locale";

    // 编译应用级 Locale 注册与组件内语言消费。
    fn documented_main() {
        // 创建简体中文语言包。
        let locale = zh_cn();

        // 配置应用级语言及其根视图。
        App::new()
            // 把 Locale 注册到 Application 组合根。
            .locale(locale)
            // 声明从当前 Provider 上下文消费 Locale 的根视图。
            .root(move || {
                // 取得当前子树生效的语言包快照。
                let locale = use_locale();
                // 组合业务格式化文案与内置空态组件。
                column((
                    // 展示由业务格式化的当前空态文案。
                    label(format!("空态文案：{}", locale.empty_description)),
                    // 创建使用同一 Locale 的内置空态组件。
                    embed(Empty::new().icon("inbox")),
                ))
            })
            // 保留文档运行入口供编译器检查。
            .run();
    }
}

// 隔离 framework-singleton 围栏中的 App builder 单例注册。
mod framework_singleton {
    // 引入文档承诺的公开 Application 与设置服务 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "framework-singleton";

    // 编译组合根单例注册与根视图装配。
    fn documented_main() {
        // 配置带全局设置服务单例的应用。
        App::new()
            // 注册由 Application 生命周期拥有的设置服务。
            .singleton(SettingsService::new())
            // 安装业务根视图工厂。
            .root(main_view)
            // 保留文档运行入口供编译器检查。
            .run();
    }

    // 声明示例所需的业务根视图。
    fn main_view() -> ViewNode {
        // 返回稳定的占位业务内容。
        label("应用")
    }
}

// 隔离 framework-services-i18n 围栏中的聚合服务与资源表。
mod framework_services_i18n {
    // 引入文档承诺的公开服务注册、i18n 与 View prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "framework-services-i18n";

    // 声明应用自注册的分析服务占位实现。
    #[derive(Clone, Default)]
    // 保持分析能力由应用提供而非组件构造。
    struct Analytics;

    // 声明一次整体注册的应用服务集合。
    #[derive(Clone)]
    // 聚合设置与分析两个进程级服务。
    struct AppServices {
        // 保存共享设置服务。
        settings: SettingsService,
        // 保存应用自注册分析服务。
        analytics: Analytics,
    }

    // 为聚合服务提供组合根构造入口。
    impl AppServices {
        // 创建所有权完整的服务集合。
        fn new() -> Self {
            // 返回默认设置与分析服务。
            Self {
                // 创建共享设置服务。
                settings: SettingsService::new(),
                // 创建应用分析服务。
                analytics: Analytics,
            }
        }
    }

    // 编译聚合服务注册与资源表驱动的子树文案。
    fn documented_main() {
        // 把服务集合整体注册到 Application 组合根。
        App::new()
            // 注册聚合服务单例。
            .register(AppServices::new())
            // 安装业务根视图工厂。
            .root(main_view)
            // 保留文档运行入口供编译器检查。
            .run();

        // 增量注册业务文案资源。
        register_translations(&[("common.save", "保存")]);
        // 创建独立 Locale 作用域内的业务文案节点。
        let locale = zh_cn();
        // 构造只影响当前子树的语言 Provider。
        let _localized = embed(
            // 使用应用选择的语言包创建作用域。
            LocaleProvider::new(locale)
                // 通过稳定 key 解析业务文案。
                .child(|| label(t!("common.save")))
                // 把 Provider 构建为公开 ViewNode。
                .build(),
        );
    }

    // 声明示例所需的业务根视图。
    fn main_view() -> ViewNode {
        // 返回稳定的占位业务内容。
        label("应用")
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖框架能力文档的六个真实围栏。
fn framework_rust_fences_compile_as_external_consumers() {
    // 收集六个已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记组合根 DI 围栏。
        framework_di::COMPILE_ID,
        // 登记 Provider 继承围栏。
        framework_provider::COMPILE_ID,
        // 登记组件窄覆盖围栏。
        framework_overrides::COMPILE_ID,
        // 登记应用 Locale 围栏。
        framework_locale::COMPILE_ID,
        // 登记 App 单例围栏。
        framework_singleton::COMPILE_ID,
        // 登记聚合服务与 i18n 围栏。
        framework_services_i18n::COMPILE_ID,
    ];
    // 运行阶段核对消费者覆盖标识与 Markdown 围栏一致。
    assert_eq!(
        // 使用实际模块暴露的标识作为结果。
        compile_ids,
        // 使用框架能力文档当前声明的稳定标识作为期望。
        [
            // 组合根 DI 围栏标识。
            "framework-di",
            // Provider 继承围栏标识。
            "framework-provider",
            // 组件窄覆盖围栏标识。
            "framework-overrides",
            // 应用 Locale 围栏标识。
            "framework-locale",
            // App 单例围栏标识。
            "framework-singleton",
            // 聚合服务与 i18n 围栏标识。
            "framework-services-i18n",
        ],
    );
}
