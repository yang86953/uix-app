// 声明本文件只编译框架能力使用文档，不启动窗口或修改资源表。
#![allow(dead_code)]

// 隔离 framework-di 围栏中的组合根依赖注入。
mod framework_di {
    // 引入文档承诺的公开 DI、AppHandle 与设置服务 prelude。
    use uix_app::prelude::*;

    // 编译同一 App 的注册与启动边界解析；独立容器不会自动注入现有 handle。
    fn compile_example() {
        let _app = App::new()
            .singleton(SettingsService::new())
            .on_start(|handle| {
                let _settings = handle
                    .resolve::<SettingsService>()
                    .expect("组合根已注册 SettingsService");
            });
    }
}

// 隔离 framework-provider 围栏中的子树配置继承。
mod framework_provider {
    // 引入文档承诺的公开 Provider、控件与 View prelude。
    use uix_app::prelude::*;

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
                .widget_size(ControlSize::Large)
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
    use uix_app::prelude::*;

    // 编译组件构造覆盖与类型索引令牌补丁。
    fn compile_example() {
        // 创建空的组件构造覆盖集合。
        let mut overrides = WidgetOverrides::default();
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
                .widget_tokens::<Button>(TokenPatch {
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
    use uix_app::prelude::*;

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
    use uix_app::prelude::*;

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

// 隔离 framework-services-i18n 围栏；资源表和 Locale 是独立的公开机制。
mod framework_services_i18n {
    use uix_app::prelude::*;

    fn compile_example() {
        // 在构建目标 UI 前注册进程级业务文案；LocaleProvider 不选择这张资源表。
        register_translations(&[("common.save", "保存")]);
        let locale = zh_cn();
        let _localized = embed(
            LocaleProvider::new(locale)
                .child(|| label(t!("common.save")))
                .build(),
        );
    }
}
