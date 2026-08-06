//! 展示与反馈类验收场景：avatar/badge/calendar/card/carousel/collapse。
//! 由 display_feedback.rs 以 `#[path]` 引入，build 在父模块按 id 分发。

use uix::prelude::*;

use super::super::{qa_row, qa_target, qa_target_view, qa_variant};
// 复用主文件的 frame 辅助与样例数据。
use super::{avatar_frame, calendar_frame, card_frame, carousel_compact_slide, carousel_frame, carousel_slide, collapse_frame};

pub(super) fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "avatar" => column_fit([
            qa_row([
                qa_variant(
                    "32 / target",
                    avatar_frame(
                        Avatar::new("UI").size(32.0),
                        32.0,
                        32.0,
                        "component-qa-target",
                    ),
                ),
                qa_variant(
                    "48 / CJK",
                    avatar_frame(
                        Avatar::new("贝")
                            .size(48.0)
                            .bg(tk.color_primary)
                            .text_color(Color::white()),
                        48.0,
                        48.0,
                        "component-qa-avatar-cjk",
                    ),
                ),
                qa_variant(
                    "64 / custom",
                    avatar_frame(
                        Avatar::new("QA")
                            .size(64.0)
                            .bg(tk.color_success)
                            .text_color(tk.color_text),
                        64.0,
                        64.0,
                        "component-qa-avatar-large",
                    ),
                ),
                qa_variant(
                    "Square",
                    avatar_frame(
                        Avatar::new("方").size(48.0).square(true),
                        48.0,
                        48.0,
                        "component-qa-avatar-square",
                    ),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Long / fitted",
                    avatar_frame(
                        Avatar::new("研发中心").size(48.0),
                        48.0,
                        48.0,
                        "component-qa-avatar-long",
                    ),
                ),
                qa_variant(
                    "Invalid / fallback",
                    avatar_frame(
                        Avatar::new("回退").size(48.0).src("missing-avatar.png"),
                        48.0,
                        48.0,
                        "component-qa-avatar-fallback",
                    ),
                ),
                qa_variant(
                    "1px / masked",
                    avatar_frame(
                        Avatar::new("图").size(48.0).src("assets/images/demo.png"),
                        48.0,
                        48.0,
                        "component-qa-avatar-image",
                    ),
                ),
                qa_variant(
                    "Square / image",
                    avatar_frame(
                        Avatar::new("图")
                            .size(48.0)
                            .square(true)
                            .src("assets/images/demo.png"),
                        48.0,
                        48.0,
                        "component-qa-avatar-square-image",
                    ),
                ),
                qa_variant(
                    "64x24 / constrained",
                    avatar_frame(
                        Avatar::new("约").size(64.0),
                        64.0,
                        24.0,
                        "component-qa-avatar-constrained",
                    ),
                ),
            ]),
        ])
        .gap(14.0),
        "badge" => column_fit([
            qa_row([
                qa_variant(
                    "CJK text over count / target",
                    row([qa_target(Badge::new().count(8).text("新消息"))]),
                ),
                qa_variant(
                    "Overflow 99+",
                    row([embed(Badge::new().count(120).max(99))
                        .automation_id("component-qa-badge-overflow")]),
                ),
                qa_variant("Show zero", embed(Badge::new().show_zero(true))),
            ]),
            qa_row([
                qa_variant(
                    "Success status",
                    embed(Badge::new().status(BadgeStatus::Success).text("已同步")),
                ),
                qa_variant("Dot text", embed(Badge::new().dot().text("新消息"))),
                qa_variant(
                    "Error status",
                    embed(Badge::new().status(BadgeStatus::Error).text("失败")),
                ),
            ]),
        ])
        .gap(14.0),
        "calendar" => column_fit([
            qa_row([
                qa_variant(
                    "Selected / target",
                    calendar_frame(
                        Calendar::new()
                            .cell_size(22.0)
                            .default_date(Date::new(2026, 7, 15)),
                        154.0,
                        172.0,
                        "component-qa-target",
                    ),
                ),
                qa_variant(
                    "en-US / compact",
                    LocaleProvider::en_us()
                        .child(|| {
                            calendar_frame(
                                Calendar::new()
                                    .cell_size(20.0)
                                    .default_date(Date::new(2026, 9, 30)),
                                140.0,
                                160.0,
                                "component-qa-calendar-english",
                            )
                        })
                        .build(),
                ),
                qa_variant(
                    "Year jump / 9999",
                    calendar_frame(
                        Calendar::new()
                            .cell_size(20.0)
                            .default_date(Date::new(9999, 12, 31))
                            .year_jump(true),
                        140.0,
                        160.0,
                        "component-qa-calendar-year-jump",
                    ),
                ),
            ]),
            qa_row([
                qa_variant(
                    "140x100 / constrained",
                    calendar_frame(
                        Calendar::new().cell_size(40.0).default_displayed(2026, 7),
                        140.0,
                        100.0,
                        "component-qa-calendar-constrained",
                    ),
                ),
                qa_variant(
                    "Compact / empty",
                    calendar_frame(
                        Calendar::new().cell_size(20.0).default_displayed(2026, 2),
                        140.0,
                        160.0,
                        "component-qa-calendar-empty",
                    ),
                ),
            ]),
        ])
        .gap(14.0),
        "card" => column_fit([
            qa_row([
                qa_variant(
                    "Bordered / target",
                    card_frame(
                        Card::new()
                            .title("质量卡片")
                            .bordered(true)
                            .hoverable()
                            .actions(vec!["Open", "Cancel"])
                            .size(210.0, 128.0)
                            .child(Label::new("正文层级与安全边距")),
                        210.0,
                        128.0,
                        "component-qa-target",
                    ),
                ),
                qa_variant(
                    "Long title / clipped body",
                    card_frame(
                        Card::new()
                            .title("A very long title must fit safely")
                            .size(180.0, 110.0)
                            .child(Label::new("Body stays safe")),
                        180.0,
                        110.0,
                        "component-qa-card-long",
                    ),
                ),
                qa_variant(
                    "Elevation / hover",
                    card_frame(
                        Card::new()
                            .title("Elevation 2")
                            .elevation(2)
                            .hoverable()
                            .size(180.0, 110.0)
                            .child(Label::new("Hover ready")),
                        180.0,
                        110.0,
                        "component-qa-card-hover",
                    ),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Actions / keyboard",
                    card_frame(
                        Card::new()
                            .title("Deployment")
                            .actions(vec!["Open detailed settings", "Cancel operation"])
                            .size(210.0, 128.0)
                            .child(Label::new("Keyboard ready")),
                        210.0,
                        128.0,
                        "component-qa-card-actions",
                    ),
                ),
                qa_variant(
                    "120x64 / constrained",
                    card_frame(
                        Card::new()
                            .title("Tight title")
                            .actions(vec!["Confirm", "Cancel"])
                            .size(120.0, 64.0),
                        120.0,
                        64.0,
                        "component-qa-card-constrained",
                    ),
                ),
                qa_variant(
                    "92x72 / large padding",
                    card_frame(
                        Card::new()
                            .title("Safe")
                            .padding(60.0)
                            .elevation(2)
                            .size(92.0, 72.0)
                            .child(Label::new("clipped")),
                        92.0,
                        72.0,
                        "component-qa-card-padding",
                    ),
                ),
            ]),
        ])
        .gap(14.0),
        "carousel" => column_fit([
            qa_variant(
                "Default / target",
                carousel_frame(
                    ViewNode::new(
                        Carousel::new()
                            .show_dots(true)
                            .show_arrows(true)
                            .size(560.0, 190.0),
                        vec![
                            carousel_slide(
                                "Slide 1 / 默认",
                                ColorValue::Palette(PaletteColor::PrimaryBg),
                            )
                            .automation_id("component-qa-carousel-slide-1"),
                            carousel_slide(
                                "Slide 2 / 切换",
                                ColorValue::Palette(PaletteColor::SuccessBg),
                            )
                            .automation_id("component-qa-carousel-slide-2"),
                            carousel_slide(
                                "Slide 3 / 回归",
                                ColorValue::Palette(PaletteColor::WarningBg),
                            )
                            .automation_id("component-qa-carousel-slide-3"),
                        ],
                    ),
                    560.0,
                    190.0,
                    "component-qa-target",
                ),
            ),
            qa_row([
                qa_variant(
                    "140x88 / constrained",
                    carousel_frame(
                        ViewNode::new(
                            Carousel::new().size(140.0, 88.0),
                            (1..=8)
                                .map(|index| {
                                    carousel_compact_slide(
                                        &format!("Compact {index}"),
                                        ColorValue::Palette(PaletteColor::PrimaryBg),
                                    )
                                })
                                .collect(),
                        ),
                        140.0,
                        88.0,
                        "component-qa-carousel-constrained",
                    ),
                ),
                qa_variant(
                    "No controls / three slides",
                    carousel_frame(
                        ViewNode::new(
                            Carousel::new()
                                .show_dots(false)
                                .show_arrows(false)
                                .size(180.0, 88.0),
                            vec![
                                carousel_compact_slide(
                                    "Quiet 1",
                                    ColorValue::Palette(PaletteColor::SuccessBg),
                                ),
                                carousel_compact_slide(
                                    "Quiet 2",
                                    ColorValue::Palette(PaletteColor::WarningBg),
                                ),
                                carousel_compact_slide(
                                    "Quiet 3",
                                    ColorValue::Palette(PaletteColor::PrimaryBg),
                                ),
                            ],
                        ),
                        180.0,
                        88.0,
                        "component-qa-carousel-hidden-controls",
                    ),
                ),
                qa_variant(
                    "Single slide",
                    carousel_frame(
                        ViewNode::new(
                            Carousel::new().size(120.0, 88.0),
                            vec![carousel_compact_slide(
                                "Only",
                                ColorValue::Palette(PaletteColor::WarningBg),
                            )],
                        ),
                        120.0,
                        88.0,
                        "component-qa-carousel-single",
                    ),
                ),
            ]),
        ])
        .gap(14.0),
        "collapse" => qa_row([
            qa_variant(
                "Default / target",
                qa_target(Collapse::new().panels(vec![
                    CollapsePanel::new("已展开", "展开内容需要保持完整可读。").expanded(),
                    CollapsePanel::new("已折叠", "折叠内容"),
                    CollapsePanel::new("保持折叠", "折叠内容"),
                ])),
            ),
            qa_variant(
                "140px / accordion",
                collapse_frame(
                    Collapse::new()
                        .panels(vec![
                            CollapsePanel::new(
                                "很长的手风琴标题需要省略",
                                "窄宽度下的中文正文需要自动换行，并始终留在面板边界内。",
                            )
                            .expanded(),
                            CollapsePanel::new("第二个面板", "切换后仍只展开一个面板。"),
                        ])
                        .accordion(),
                    140.0,
                    180.0,
                    "component-qa-collapse-constrained",
                ),
            ),
        ]),
        _ => return None,
    };
    Some(view)
}
