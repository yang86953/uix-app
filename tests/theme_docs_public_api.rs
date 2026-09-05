// 声明本文件只编译样式与主题使用文档，不启动窗口或原生资源。
#![allow(dead_code)]

// 隔离 style-semantic 围栏中的语义样式声明。
mod style_semantic {
    // 引入文档承诺的公开样式 prelude。
    use uix::prelude::*;

    // 编译文本、按钮和标签的语义样式入口。
    fn compile_example() {
        // 使用中性色板的主要文字角色。
        let _label = label("主要文字").color(ColorValue::Neutral(NeutralRole::Text));
        // 使用公开主按钮语义。
        let _primary = button("保存").primary();
        // 使用公开危险按钮语义。
        let _danger = button("危险").danger();
        // 使用公开成功标签语义。
        let _success = Tag::new("成功").color(TagColor::Success);
    }
}

// 隔离 style-chain 围栏中的细粒度样式链。
mod style_chain {
    // 引入文档承诺的公开样式 prelude。
    use uix::prelude::*;

    // 编译背景、边框、圆角、间距与内边距组合。
    fn compile_example() {
        // 构造包含单个文本节点的卡片列。
        let _card = column((label("卡片"),))
            // 使用容器背景语义色。
            .bg(ColorValue::Neutral(NeutralRole::BgContainer))
            // 使用次级边框语义色。
            .border(
                // 声明逻辑边框宽度。
                1.0,
                // 声明次级边框颜色角色。
                ColorValue::Neutral(NeutralRole::BorderSecondary),
            )
            // 声明卡片圆角。
            .radius(8.0)
            // 声明带显式 spread 的卡片阴影。
            .box_shadow(Some(
                // 通过兼容构造入口创建阴影并追加 spread。
                BoxShadowDef::new(Color::from_rgba(0, 0, 0, 40), 8.0, 0.0, 2.0)
                    // 将阴影基准盒向外扩张一个逻辑像素。
                    .with_spread(1.0),
            ))
            // 声明卡片内边距。
            .padding(16.0)
            // 声明子节点间距。
            .gap(8.0);
    }
}

// 隔离 theme-toggle 围栏中的组件级与应用级切换入口。
mod theme_toggle {
    // 引入文档承诺的公开主题与应用句柄 prelude。
    use uix::prelude::*;

    // 为围栏中的 handle 与 is_dark 上下文提供显式参数。
    fn switch_theme(handle: &AppHandle, is_dark: bool) -> Result<(), Error> {
        // 构造由外层主题事实初始化的声明式切换按钮。
        let _toggle = embed(ThemeToggle::new().dark(is_dark));
        // 通过应用级权威入口广播暗色主题。
        handle.set_theme(Theme::antd_dark())?;
        // 返回成功并保留类型化错误契约。
        Ok(())
    }
}

// 隔离 theme-custom 围栏中的组件令牌窄覆盖。
mod theme_custom {
    // 引入文档承诺的公开 Provider 与令牌 prelude。
    use uix::prelude::*;

    // 编译只作用于 ConfigProvider 子树的 Button 令牌补丁。
    fn compile_example() {
        // 构造带组件级令牌覆盖的唯一子树。
        let _provider = embed(
            // 创建继承外层配置的 Provider。
            ConfigProvider::new()
                // 只覆盖 Button 的主色与圆角令牌。
                .widget_tokens::<Button>(TokenPatch {
                    // 覆盖 Button 主色。
                    color_primary: Some(Color::hex("#722ed1")),
                    // 覆盖 Button 圆角。
                    border_radius: Some(10.0),
                    // 其余令牌继续继承默认值。
                    ..TokenPatch::default()
                })
                // 在解析后的局部配置作用域内构建按钮。
                .child(|| button("紫色按钮").primary())
                // 构建 Provider 声明节点。
                .build(),
        );
    }
}

// 隔离 theme-custom-app 围栏中的自定义主题与令牌消费组件。
mod theme_custom_app {
    // 引入文档承诺的公开应用、主题与绘制 prelude。
    use uix::prelude::*;

    // 为围栏中的 render 片段提供最小公开 widget 宿主。
    widget! {
        // 声明无业务状态的主题表面组件。
        pub struct ThemedSurface {}
        // 声明最小公开构造入口。
        @new -> Self {
            // 构造无状态主题表面。
            Self {}
        }

        // 按围栏原样从 PaintContext 令牌作用域消费颜色。
        render => (&self, frame: Rect, ctx: &mut PaintContext) {
            // 使用解析后的主题背景色记录填充意图。
            ctx.fill_rect(frame, ctx.tokens().color_primary_bg(), None);
        }
    }

    // 为文档中的 app_root 提供最小公开 View 根。
    fn app_root() -> ViewNode {
        // 嵌入只消费主题令牌的自定义组件。
        embed(ThemedSurface::new())
    }

    // 编译从品牌基色生成令牌并注入 App 的完整入口。
    fn main() {
        // 从亮色基础基元和品牌主色生成设计令牌。
        let theme = Theme::new(DesignTokens::from_primitives(
            // 声明窄覆盖后的主题基元。
            ThemePrimitives {
                // 使用文档声明的紫色品牌种子。
                primary: Color::hex("#722ed1"),
                // 其余基元继承 Ant Design 亮色默认值。
                ..ThemePrimitives::antd_light()
            },
            // 声明生成亮色设计令牌。
            false,
        ));

        // 构造使用自定义主题并跟随系统明暗变化的应用。
        App::new()
            // 注入应用拥有的初始主题。
            .theme(theme)
            // 声明运行中跟随系统主题。
            .follow_system_theme(true)
            // 使用最小公开主题表面作为根 View。
            .root(app_root)
            // 保留完整运行入口，只由编译器检查而不在测试中调用。
            .run();
    }
}

// 隔离 theme-custom-seed 围栏中的品牌色种子入口。
mod theme_custom_seed {
    // 引入文档承诺的公开应用与主题 prelude。
    use uix::prelude::*;

    // 为文档中的 app_root 提供最小公开 View 根。
    fn app_root() -> ViewNode {
        // 返回不持有主题状态的展示节点。
        label("品牌主题")
    }

    // 编译通过 PrimaryHue 生成品牌主题的完整应用入口。
    fn main() {
        // 构造以紫色种子自动生成色板的应用。
        App::new()
            // 从亮色基元与公开品牌色枚举生成主题。
            .theme(Theme::custom(
                // 只替换品牌主色种子。
                ThemePrimitives::antd_light().with_brand_primary(PrimaryHue::Purple),
            ))
            // 声明运行中跟随系统主题。
            .follow_system_theme(true)
            // 使用最小公开 View 根。
            .root(app_root)
            // 保留完整运行入口，只由编译器检查而不在测试中调用。
            .run();
    }
}
