use uix::prelude::*;

use super::{qa_row, qa_target, qa_target_view, qa_variant};

fn swatch(text: &str, color: ColorValue) -> ViewNode {
    column_fit([label(text).font_size(12.0)])
        .width(96.0)
        .height(48.0)
        .padding(EdgeInsets::uniform(10.0))
        .bg(color)
        .radius(6.0)
}

pub fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "button" => qa_row([
            qa_variant(
                "Primary / target",
                qa_target(button("主要操作").primary().widget()),
            ),
            qa_variant("Default", embed(button("默认").widget())),
            qa_variant("Danger", embed(button("危险").danger().widget())),
            qa_variant("Ghost", embed(button("幽灵").ghost().widget())),
            qa_variant("Disabled", embed(button("禁用").disabled(true).widget())),
        ]),
        "icon" => qa_row([
            qa_variant("16", qa_target(Icon::new("search").size(16.0))),
            qa_variant("24", embed(Icon::new("home").size(24.0))),
            qa_variant("32", embed(Icon::new("settings").size(32.0))),
            qa_variant(
                "Semantic",
                embed(Icon::new("alert-circle").size(24.0)).color(tk.color_error),
            ),
        ]),
        "label" => column_fit([
            qa_variant(
                "Primary",
                qa_target(Label::new("主要文字 Aa 0123").color(tk.color_text)),
            ),
            qa_variant(
                "Secondary",
                embed(Label::new("次要文字与中文标点，。").color(tk.color_text_secondary)),
            ),
            qa_variant(
                "Long content",
                embed(
                    Label::new("较长标签仍应完整显示：UIX Component Visual Quality Gate")
                        .color(tk.color_text_tertiary),
                ),
            ),
        ])
        .gap(14.0),
        "typography" => column_fit([
            qa_target(Typography::heading("Heading 1 / 一级标题", 1)),
            embed(Typography::heading("Heading 2 / 二级标题", 2)),
            embed(Typography::heading("Heading 3 / 三级标题", 3)),
            embed(Typography::paragraph(
                "正文需要保持舒适行高、清晰层级，并正确处理 English、中文和 0123456789。",
            )),
        ])
        .gap(10.0),
        "divider" => column_fit([
            qa_variant("Horizontal", qa_target(Divider::new())),
            qa_row([
                label("左侧"),
                embed(Divider::new().vertical()),
                label("中间"),
                embed(Divider::new().vertical()),
                label("右侧"),
            ])
            .height(42.0),
        ])
        .gap(18.0),
        "space" => column_fit([
            qa_variant(
                "Small",
                qa_target(
                    Space::new()
                        .size(SpaceSize::Small)
                        .direction(FlexDirection::Row)
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                ),
            ),
            qa_variant(
                "Middle",
                embed(
                    Space::new()
                        .size(SpaceSize::Middle)
                        .direction(FlexDirection::Row)
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                ),
            ),
            qa_variant(
                "Large / vertical",
                embed(
                    Space::new()
                        .size(SpaceSize::Large)
                        .direction(FlexDirection::Column)
                        .child(Label::new("上"))
                        .child(Label::new("下")),
                ),
            ),
        ])
        .gap(10.0),
        "float-button" => qa_row([
            qa_variant(
                "Default / target",
                qa_target(
                    FloatButton::new("+")
                        .tooltip("新建")
                        .reserve_layout_space(true),
                ),
            ),
            qa_variant(
                "Badge",
                embed(FloatButton::new("+").badge(8).reserve_layout_space(true)),
            ),
        ]),
        "float-button-back-top" => qa_variant(
            "BackTop preset / target",
            qa_target(FloatButtonBackTop::new().reserve_layout_space(true)),
        ),
        "theme-toggle" => qa_row([
            qa_variant("Current / target", qa_target(ThemeToggle::new())),
            qa_variant("Dark state", embed(ThemeToggle::new().dark(true))),
        ]),
        "container" => column_fit([
            qa_target_view(
                qa_row([
                    swatch("A", ColorValue::Palette(PaletteColor::PrimaryBg)),
                    swatch("B", ColorValue::Palette(PaletteColor::SuccessBg)),
                    swatch("C", ColorValue::Palette(PaletteColor::WarningBg)),
                ])
                .height(64.0)
                .padding(EdgeInsets::uniform(8.0))
                .bg(ColorValue::Neutral(NeutralRole::FillTertiary))
                .border(1.0, ColorValue::Neutral(NeutralRole::Border)),
            ),
            qa_variant(
                "Column",
                column_fit([label("第一行"), label("第二行"), label("第三行")])
                    .gap(6.0)
                    .padding(EdgeInsets::uniform(8.0))
                    .bg(ColorValue::Neutral(NeutralRole::FillSecondary)),
            ),
        ])
        .gap(14.0),
        "grid" => column_fit([
            qa_target_view(
                grid([
                    swatch("1", ColorValue::Palette(PaletteColor::PrimaryBg)),
                    swatch("2", ColorValue::Palette(PaletteColor::SuccessBg)),
                    swatch("3", ColorValue::Palette(PaletteColor::WarningBg)),
                ])
                .three_columns()
                .gap(8.0)
                .width(340.0)
                .height(68.0),
            ),
            label("三列在紧凑窗口中仍需保持对齐和安全间距")
                .font_size(11.0)
                .color(ColorValue::Neutral(NeutralRole::TextTertiary)),
        ])
        .gap(12.0),
        "layout" => qa_target_view(embed(tree! { Layout::new().bg(tk.color_bg_layout) => [
            tree! { Header::new(42.0).bg(tk.color_primary_bg) => [Label::new("Header")]},
            tree! { Container::new().size(560.0, 150.0).dir(FlexDirection::Row) => [
                tree! { Sider::new(130.0).bg(tk.color_fill_secondary) => [Label::new("Sider")]},
                tree! { Content::new().bg(tk.color_bg_container) => [Label::new("Content / 主内容区域")]},
            ]},
            tree! { Footer::new(36.0).bg(tk.color_fill_tertiary) => [Label::new("Footer")]},
        ]})),
        "header" => qa_target_view(embed(
            tree! { Header::new(72.0).bg(tk.color_primary_bg) => [
                Label::new("Header / 72 px").color(tk.color_primary),
            ]},
        )),
        "sider" => qa_target_view(
            column([embed(
                tree! { Sider::new(220.0).bg(tk.color_fill_secondary) => [
                    Label::new("Sider 220 px"),
                    Label::new("导航区域"),
                ]},
            )])
            .height(160.0),
        ),
        "content" => qa_target_view(
            column([embed(tree! { Content::new().bg(tk.color_bg_container) => [
                Label::new("Content 主内容"),
                Label::new("真实内容需要保持安全边距与清晰层级"),
            ]})])
            .height(160.0),
        ),
        "footer" => qa_target_view(embed(
            tree! { Footer::new(64.0).bg(tk.color_fill_tertiary) => [
                Label::new("Footer / 64 px"),
            ]},
        )),
        "splitter" => qa_row([
            qa_variant(
                "Horizontal / target",
                qa_target(Splitter::new().panels(3).vertical(false)),
            ),
            qa_variant("Vertical", embed(Splitter::new().panels(2).vertical(true))),
        ])
        .height(180.0),
        "affix" => qa_row([
            qa_variant(
                "Default",
                ViewNode::new(Affix::new(12.0), vec![button("自然位置").into()]),
            ),
            qa_variant(
                "Affixed / target",
                qa_target_view(ViewNode::new(
                    Affix::new(12.0).scroll_y(180.0),
                    vec![button("已吸顶").primary().into()],
                )),
            ),
        ]),
        "back-top" => qa_row([
            qa_variant("Hidden", embed(BackTop::new().visibility_height(100.0))),
            qa_variant(
                "Visible / target",
                qa_target(BackTop::new().visibility_height(100.0).scroll_y(180.0)),
            ),
        ]),
        "scroll-view" => qa_target(
            ScrollView::new(ScrollDirection::Vertical)
                .size(560.0, 190.0)
                .children(
                    (0..18)
                        .map(|index| {
                            Box::new(Label::new(format!("ScrollView row {index:02} — 可读内容")))
                                as Box<dyn WidgetComponent>
                        })
                        .collect(),
                ),
        ),
        "virtual-scroll" => qa_target_view(
            VirtualScroll::new()
                .item_count(1_000)
                .item_height(30.0)
                .size(560.0, 190.0)
                .render(|index| {
                    label(format!("Virtual row {index:04}"))
                        .height(30.0)
                        .padding_h(8.0)
                })
                .into(),
        ),
        _ => return None,
    };
    Some(view)
}
