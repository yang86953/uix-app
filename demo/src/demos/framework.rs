//! 框架能力 — LocaleProvider、ConfigProvider、构造覆盖与 typed token。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::common::showcase::info_note;
use crate::demos::context::{DemoCtx, FrameworkLocale};

pub const LOCALE_STATUS_ID: &str = "framework-locale-status";
pub const LOCALE_SAMPLE_ID: &str = "framework-locale-sample";
pub const LOCALE_ZH_ID: &str = "framework-locale-zh";
pub const LOCALE_EN_ID: &str = "framework-locale-en";
pub const FRAMEWORK_SCROLL_ID: &str = "framework-scroll";
pub const PROVIDER_LARGE_BUTTON_ID: &str = "framework-provider-large-button";
pub const PROVIDER_SMALL_BUTTON_ID: &str = "framework-provider-small-button";
pub const TOKEN_BUTTON_ID: &str = "framework-token-button";
pub const EMPTY_SAMPLE_ID: &str = "framework-empty-sample";

fn locale_block(ctx: &DemoCtx<'_>) -> ViewNode {
    let control = ctx.framework_control();
    let locale_state = control.locale_state();
    let locale_for_status = locale_state.clone();
    let locale = control.locale();

    let locale_sample = LocaleProvider::new(locale)
        .child(|| {
            let locale = use_locale();
            column_fit([
                label(format!(
                    "Locale.empty_description：{}",
                    locale.empty_description
                ))
                .automation_id(LOCALE_SAMPLE_ID)
                .font_size(13.0),
                embed(Empty::new().icon("inbox")),
            ])
            .gap(8.0)
        })
        .build();

    column_fit([
        row([
            button("简体中文")
                .on_click(&locale_state, |state| state.set(FrameworkLocale::ZhCn))
                .automation_id(LOCALE_ZH_ID)
                .build(),
            button("English")
                .on_click(&locale_state, |state| state.set(FrameworkLocale::EnUs))
                .automation_id(LOCALE_EN_ID)
                .build(),
            dynamic_label(move || {
                let name = match locale_for_status.get() {
                    FrameworkLocale::ZhCn => "简体中文",
                    FrameworkLocale::EnUs => "English",
                };
                format!("当前语言：{name}")
            })
            .automation_id(LOCALE_STATUS_ID)
            .color(ColorValue::Palette(PaletteColor::Primary)),
        ])
        .align(AlignItems::Center)
        .gap(10.0),
        locale_sample,
        info_note(
            ctx.tk,
            "LocaleProvider 只覆写当前子树；点击语言按钮后，状态与组件文案一起 reconcile。",
        ),
    ])
    .gap(12.0)
}

fn config_provider_block(ctx: &DemoCtx<'_>) -> ViewNode {
    let configured = ConfigProvider::new()
        .component_size(ControlSize::Large)
        .child(|| {
            column_fit([
                row([
                    button("继承 Large")
                        .automation_id(PROVIDER_LARGE_BUTTON_ID)
                        .build(),
                    button("显式 Small")
                        .size(ControlSize::Small)
                        .automation_id(PROVIDER_SMALL_BUTTON_ID)
                        .build(),
                ])
                .align(AlignItems::Center)
                .gap(12.0),
                input().placeholder("继承 Large 的 Input").build(),
                ConfigProvider::new()
                    .disabled(true)
                    .child(|| button("最近 Provider：disabled"))
                    .build(),
            ])
            .gap(12.0)
        })
        .build();

    column_fit([
        configured,
        info_note(
            ctx.tk,
            "优先级：组件显式 setter > 最近 ConfigProvider > App::config > 框架默认。",
        ),
    ])
    .gap(12.0)
}

fn constructor_overrides_block(ctx: &DemoCtx<'_>) -> ViewNode {
    let mut overrides = ComponentOverrides::default();
    overrides.input.prefix = Some("¥".to_string());
    overrides.input.suffix = Some("CNY".to_string());

    column_fit([
        ConfigProvider::new()
            .overrides(overrides)
            .child(|| input().placeholder("组件构造时自动注入前后缀"))
            .build(),
        info_note(
            ctx.tk,
            "ComponentOverrides 在组件构造时生效；业务页面无需逐个传递共享前后缀。",
        ),
    ])
    .gap(12.0)
}

fn tokens_and_empty_block(ctx: &DemoCtx<'_>) -> ViewNode {
    let configured = ConfigProvider::new()
        .component_tokens::<Button>(TokenPatch {
            color_primary: Some(Color::rgba(114, 46, 209, 255)),
            border_radius: Some(10.0),
            ..TokenPatch::default()
        })
        .render_empty(|context| {
            label(format!("{} 自定义空态", context.component_name()))
                .automation_id(EMPTY_SAMPLE_ID)
                .color(ColorValue::Palette(PaletteColor::Primary))
        })
        .child(|| {
            column_fit([
                button("typed TokenPatch")
                    .primary()
                    .automation_id(TOKEN_BUTTON_ID)
                    .build(),
                List::new().build(),
            ])
            .gap(12.0)
        })
        .build();

    column_fit([
        configured,
        info_note(
            ctx.tk,
            "typed token 只影响 Button；统一空态 View factory 会收到 EmptyContext::component_name()。",
        ),
    ])
    .gap(12.0)
}

pub fn page_framework(ctx: &DemoCtx<'_>) -> ViewNode {
    PageBuilder::new(ctx.tk)
        .block("LocaleProvider — 子树国际化", locale_block(ctx))
        .block(
            "ConfigProvider — 继承与显式覆盖",
            config_provider_block(ctx),
        )
        .block(
            "ComponentOverrides — 构造覆盖",
            constructor_overrides_block(ctx),
        )
        .block("组件令牌与统一空态 View", tokens_and_empty_block(ctx))
        .build()
        .automation_id(FRAMEWORK_SCROLL_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demos::context::FrameworkControl;
    use uix::ui::test_harness::ViewAdapter;

    #[test]
    fn framework_page_reflects_provider_state_and_custom_empty_view() {
        let tokens = DesignTokens::antd_light();
        let ticks = State::new(0u32);
        let control = FrameworkControl::default();
        let initial_ctx = DemoCtx::new(&tokens, &ticks, None).with_framework_control(&control);
        let initial = ViewAdapter::build(page_framework(&initial_ctx));
        let initial_labels: Vec<String> = initial
            .find_all_by_type::<Label>()
            .into_iter()
            .map(|(_, label)| label.text().to_string())
            .collect();
        assert!(initial_labels
            .iter()
            .any(|text| text == "Locale.empty_description：暂无数据"));
        assert!(initial_labels.iter().any(|text| text == "List 自定义空态"));

        control.locale_state().set(FrameworkLocale::EnUs);
        let english_ctx = DemoCtx::new(&tokens, &ticks, None).with_framework_control(&control);
        let english = ViewAdapter::build(page_framework(&english_ctx));
        assert!(english
            .find_all_by_type::<Label>()
            .into_iter()
            .map(|(_, label)| label.text().to_string())
            .any(|text| text == "Locale.empty_description：No data"));
    }
}
