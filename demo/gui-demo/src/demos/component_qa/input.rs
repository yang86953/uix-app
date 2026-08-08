use uix::prelude::*;

use super::{qa_row, qa_target, qa_target_view, qa_variant};

// 日期与选择器类测试场景拆入独立文件，保持输入主文件处于行数上限内。
#[path = "input_pickers.rs"]
mod input_pickers;
// 复用日期与选择器类场景构建。
use input_pickers::{
    build_autocomplete_case, build_cascader_case, build_color_picker_case,
    build_date_picker_case, build_date_range_picker_case, build_mentions_case,
    build_time_picker_case, build_tree_select_case,
};

fn build_rate_case() -> ViewNode {
    column_fit([
        qa_row([
            qa_variant(
                "Clearable / target",
                qa_target(Rate::new().default_value(3).clearable()),
            ),
            qa_variant(
                "Half 2.5 / 5",
                embed(Rate::new().count(5).default_value(5).allow_half())
                    .automation_id("component-qa-rate-half"),
            ),
            qa_variant(
                "Disabled",
                embed(Rate::new().default_value(4).disabled(true))
                    .automation_id("component-qa-rate-disabled"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Custom Unicode character",
                embed(Rate::new().count(2).default_value(1).character("强烈推荐"))
                    .automation_id("component-qa-rate-custom"),
            ),
            qa_variant(
                "Empty",
                embed(Rate::new()).automation_id("component-qa-rate-empty"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Small",
                embed(Rate::new().size(ControlSize::Small).default_value(2))
                    .automation_id("component-qa-rate-small"),
            ),
            qa_variant(
                "Large",
                embed(Rate::new().size(ControlSize::Large).default_value(4))
                    .automation_id("component-qa-rate-large"),
            ),
            qa_variant(
                "Large / constrained height",
                grid(
                    [embed(Rate::new().size(ControlSize::Large).default_value(3))
                        .automation_id("component-qa-rate-constrained")],
                )
                .columns(vec![GridTrack::Fr(1.0)])
                .rows(vec![GridTrack::Fr(1.0)])
                .width(80.0)
                .height(12.0)
                .align_self(AlignItems::Start),
            ),
        ]),
    ])
    .gap(14.0)
}

pub fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "input" => {
            column_fit([
                qa_row([
                    qa_variant(
                        "CJK addon / target",
                        row([qa_target(Input::new("请输入").addon_before("账号"))]),
                    ),
                    qa_variant(
                        "Clearable",
                        row([
                            embed(Input::new("").with_value("待清除").clearable(true))
                                .automation_id("component-qa-input-clearable"),
                        ]),
                    ),
                    qa_variant(
                        "Password",
                        row([
                            embed(Input::password().with_value("秘密内容"))
                                .automation_id("component-qa-input-password"),
                        ]),
                    ),
                    qa_variant(
                        "Disabled",
                        row([embed(Input::new("不可编辑").disabled(true))]),
                    ),
                ]),
                qa_row([
                    qa_variant(
                        "Trailing empty line",
                        row([
                            embed(Input::textarea().rows(2).with_value("第一行\n"))
                                .automation_id("component-qa-input-textarea"),
                        ]),
                    ),
                    qa_variant(
                        "Small",
                        row([
                            embed(Input::new("小号输入").size(ControlSize::Small))
                                .automation_id("component-qa-input-small"),
                        ]),
                    ),
                    qa_variant(
                        "Large",
                        row([
                            embed(Input::new("大号输入").size(ControlSize::Large))
                                .automation_id("component-qa-input-large"),
                        ]),
                    ),
                ]),
            ])
            .gap(14.0)
        }
        "input-number" => column_fit([
            qa_row([
                qa_variant(
                    "Precise / target",
                    row([qa_target(
                        InputNumber::new()
                            .default_value(1.2345)
                            .placeholder("精确数值")
                            .min(-10.0)
                            .max(10.0)
                            .step(0.1),
                    )]),
                ),
                qa_variant(
                    "Decimal step",
                    row([
                        embed(InputNumber::new().default_value(0.0).step(0.1))
                            .automation_id("component-qa-input-number-decimal"),
                    ]),
                ),
                qa_variant(
                    "Disabled",
                    row([
                        embed(InputNumber::new().default_value(18.0).disabled(true))
                            .automation_id("component-qa-input-number-disabled"),
                    ]),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Small",
                    row([
                        embed(
                            InputNumber::new()
                                .default_value(42.0)
                                .size(ControlSize::Small),
                        )
                        .automation_id("component-qa-input-number-small"),
                    ]),
                ),
                qa_variant(
                    "Large / negative",
                    row([
                        embed(
                            InputNumber::new()
                                .default_value(-12.5)
                                .size(ControlSize::Large),
                        )
                        .automation_id("component-qa-input-number-large"),
                    ]),
                ),
                qa_variant(
                    "Long value / clipped",
                    row([
                        embed(InputNumber::new().default_value(1_234_567_890.123_4))
                            .automation_id("component-qa-input-number-long"),
                    ]),
                ),
            ]),
        ])
        .gap(14.0),
        "select" => ViewNode::new(
            Container::new()
                .dir(FlexDirection::Column)
                .h(240.0)
                .justify(JustifyContent::SpaceBetween),
            vec![
            qa_row([
                qa_variant(
                    "Search / target",
                    qa_target(
                        Select::searchable()
                            .options(["Alpha", "Alpine", "Beta"])
                            .default_selected(2)
                            .placeholder("搜索成员"),
                    ),
                ),
                qa_variant(
                    "Multiple / clipped tags",
                    ViewNode::new(
                        Container::new()
                            .dir(FlexDirection::Column)
                            .w(160.0)
                            .h(32.0)
                            .align(AlignItems::Stretch),
                        vec![
                        embed(Select::multiple().options([
                            "超长多选标签甲",
                            "超长多选标签乙",
                            "超长多选标签丙",
                        ]))
                        .automation_id("component-qa-select-multiple"),
                        ],
                    ),
                ),
                qa_variant(
                    "Disabled",
                    embed(
                        Select::new()
                            .options(["不可用"])
                            .default_selected(0)
                            .disabled(true),
                    )
                    .automation_id("component-qa-select-disabled"),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Empty / no data",
                    embed(Select::new().placeholder("暂无选项"))
                        .automation_id("component-qa-select-empty"),
                ),
                qa_variant(
                    "Small",
                    embed(
                        Select::new()
                            .options(["小号", "选项"])
                            .size(ControlSize::Small),
                    )
                    .automation_id("component-qa-select-small"),
                ),
                qa_variant(
                    "Large",
                    embed(
                        Select::new()
                            .options(["大号", "选项"])
                            .size(ControlSize::Large),
                    )
                    .automation_id("component-qa-select-large"),
                ),
                qa_variant(
                    "Long / clipped",
                    ViewNode::new(
                        Container::new()
                            .dir(FlexDirection::Column)
                            .w(140.0)
                            .h(32.0)
                            .align(AlignItems::Stretch),
                        vec![
                        embed(
                            Select::new()
                                .options(["这是不会覆盖右侧箭头的超长选项文字"])
                                .default_selected(0),
                        )
                        .automation_id("component-qa-select-long"),
                        ],
                    ),
                ),
            ]),
                qa_row([
                    qa_variant(
                        "Custom option View",
                        Select::new()
                            .options(["设计", "研发", "测试"])
                            .render_option(|option| {
                                row([
                                    embed(Icon::new("palette").size(10.0)),
                                    label(option.to_owned()).font_size(12.0),
                                ])
                                .gap(6.0)
                                .align(AlignItems::Center)
                            })
                            .automation_id("component-qa-select-custom"),
                    ),
                    qa_variant(
                        "Bottom edge / flip above",
                        embed(
                            Select::new()
                                .options([
                                    "选项一",
                                    "选项二",
                                    "选项三",
                                    "选项四",
                                    "选项五",
                                    "选项六",
                                    "选项七",
                                    "选项八",
                                    "选项九",
                                    "选项十",
                                ])
                                .default_selected(0),
                        )
                        .automation_id("component-qa-select-bottom"),
                    ),
                ]),
            ],
        ),
        "checkbox" => column_fit([
            qa_row([
                qa_variant(
                    "CJK target",
                    row([qa_target(Checkbox::new("同意协议"))]),
                ),
                qa_variant(
                    "Checked",
                    embed(Checkbox::new("已选择").default_checked(true))
                        .automation_id("component-qa-checkbox-checked"),
                ),
                qa_variant("Disabled", embed(Checkbox::new("禁用").disabled(true))),
            ]),
            qa_row([
                qa_variant(
                    "Small",
                    embed(Checkbox::new("小号").size(ControlSize::Small)),
                ),
                qa_variant(
                    "Large",
                    embed(Checkbox::new("大号复选").size(ControlSize::Large)),
                ),
                qa_variant(
                    "Empty label",
                    row([
                        embed(Checkbox::new("").default_checked(true))
                            .automation_id("component-qa-checkbox-empty"),
                    ]),
                ),
            ]),
        ])
        .gap(14.0),
        "radio" => column_fit([
            qa_row([
                qa_variant(
                    "CJK selected / target",
                    row([qa_target(
                        Radio::new()
                            .group_name("水果")
                            .options(["苹果", "香蕉", "樱桃"])
                            .default_selected(1),
                    )]),
                ),
                qa_variant(
                    "Disabled",
                    row([embed(
                        Radio::new()
                            .options(["甲", "乙"])
                            .default_selected(0)
                            .disabled(true),
                    )]),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Small",
                    row([
                        embed(
                            Radio::new()
                                .options(["小号", "选项", "三号"])
                                .size(ControlSize::Small),
                        )
                        .automation_id("component-qa-radio-small"),
                    ]),
                ),
                qa_variant(
                    "Large",
                    row([
                        embed(
                            Radio::new()
                                .options(["大号", "选项", "三号"])
                                .size(ControlSize::Large)
                                .default_selected(2),
                        )
                        .automation_id("component-qa-radio-large"),
                    ]),
                ),
                qa_variant(
                    "Vertical",
                    row([
                        embed(
                            Radio::new()
                                .options(["本地", "云端", "混合"])
                                .default_selected(1)
                                .vertical(),
                        )
                        .automation_id("component-qa-radio-vertical"),
                    ]),
                ),
            ]),
        ])
        .gap(14.0),
        "switch" => column_fit([
            qa_row([
                qa_variant("Off / target", row([qa_target(Switch::new())])),
                qa_variant(
                    "On",
                    row([
                        embed(Switch::new().default_checked(true))
                            .automation_id("component-qa-switch-on"),
                    ]),
                ),
                qa_variant(
                    "Disabled",
                    row([embed(
                        Switch::new().default_checked(true).disabled(true),
                    )]),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Small",
                    row([
                        embed(Switch::new().size(ControlSize::Small))
                            .automation_id("component-qa-switch-small"),
                    ]),
                ),
                qa_variant(
                    "Large",
                    row([
                        embed(Switch::new().size(ControlSize::Large).default_checked(true))
                            .automation_id("component-qa-switch-large"),
                    ]),
                ),
            ]),
        ])
        .gap(14.0),
        "slider" => column_fit([
            qa_row([
                qa_variant(
                    "Off-grid 42 / target",
                    qa_target(Slider::new(0.0..=100.0).step(5.0).default_value(42.0)),
                ),
                qa_variant(
                    "Minimum",
                    embed(Slider::new(0.0..=100.0).default_value(0.0))
                        .automation_id("component-qa-slider-minimum"),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Small",
                    embed(
                        Slider::new(0.0..=100.0)
                            .size(ControlSize::Small)
                            .default_value(35.0),
                    )
                    .automation_id("component-qa-slider-small"),
                ),
                qa_variant(
                    "Large",
                    embed(
                        Slider::new(0.0..=100.0)
                            .size(ControlSize::Large)
                            .default_value(65.0),
                    )
                    .automation_id("component-qa-slider-large"),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Decimal step",
                    embed(Slider::new(0.0..=1.0).step(0.1).default_value(0.3))
                        .automation_id("component-qa-slider-decimal"),
                ),
                qa_variant(
                    "Large / constrained height",
                    grid([embed(
                            Slider::new(0.0..=100.0)
                                .size(ControlSize::Large)
                                .default_value(50.0),
                        )
                        .automation_id("component-qa-slider-constrained")])
                    .columns(vec![GridTrack::Fr(1.0)])
                    .rows(vec![GridTrack::Fr(1.0)])
                    .width(80.0)
                    .height(12.0)
                    .align_self(AlignItems::Start),
                ),
            ]),
        ])
        .width(620.0)
        .gap(14.0),
        "range-slider" => {
            let start = State::new(25.0);
            let end = State::new(70.0);
            column_fit([
                qa_variant(
                    "25–70 / target",
                    qa_target(
                        Slider::range(0.0..=100.0)
                            .step(5.0)
                            .start(&start)
                            .end(&end),
                    ),
                ),
                qa_row([
                    qa_variant(
                        "Small",
                        embed(Slider::range(0.0..=100.0).size(ControlSize::Small))
                            .automation_id("component-qa-range-slider-small"),
                    ),
                    qa_variant(
                        "Large",
                        embed(Slider::range(0.0..=100.0).size(ControlSize::Large))
                            .automation_id("component-qa-range-slider-large"),
                    ),
                ]),
            ])
            .width(620.0)
            .gap(14.0)
        }
        "rate" => build_rate_case(),
        "date-picker" => build_date_picker_case(),
        "date-range-picker" => build_date_range_picker_case(),
        "time-picker" => build_time_picker_case(),
        "color-picker" => build_color_picker_case(tk),
        "cascader" => build_cascader_case(),
        "tree-select" => build_tree_select_case(),
        "auto-complete" => build_autocomplete_case(),
        "mentions" => build_mentions_case(),
        "segmented" => column_fit([
            qa_row([
                qa_variant(
                    "CJK width / target",
                    qa_target(
                        Segmented::new(["每日", "每周", "每月", "每年"])
                            .default_selected(2),
                    ),
                ),
                qa_variant(
                    "Disabled option",
                    embed(
                        Segmented::new(["首页", "分析", "设置"])
                            .default_selected(0)
                            .disable_option(1),
                    )
                    .automation_id("component-qa-segmented-disabled-option"),
                ),
                qa_variant(
                    "Disabled selected",
                    embed(
                        Segmented::new(["甲", "乙", "丙"])
                            .default_selected(1)
                            .disabled(true),
                    )
                    .automation_id("component-qa-segmented-disabled"),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Small",
                    embed(
                        Segmented::new(["日", "周", "月"])
                            .size(ControlSize::Small),
                    )
                    .automation_id("component-qa-segmented-small"),
                ),
                qa_variant(
                    "Large",
                    embed(
                        Segmented::new(["日", "周", "月"])
                            .size(ControlSize::Large),
                    )
                    .automation_id("component-qa-segmented-large"),
                ),
                qa_variant(
                    "Long / clipped",
                    ViewNode::new(
                        Container::new()
                            .dir(FlexDirection::Column)
                            .w(180.0)
                            .h(32.0)
                            .align(AlignItems::Stretch),
                        vec![embed(
                            Segmented::new(["这是很长的第一个分段", "第二个分段"])
                                .default_selected(0),
                        )
                        .automation_id("component-qa-segmented-long")
                        .width(180.0)
                        .height(32.0)],
                    ),
                ),
                qa_variant(
                    "Large / constrained height",
                    ViewNode::new(
                        Container::new()
                            .dir(FlexDirection::Row)
                            .w(180.0)
                            .h(20.0)
                            .align(AlignItems::Stretch),
                        vec![embed(
                            Segmented::new(["紧凑", "高度"])
                                .default_selected(0)
                                .size(ControlSize::Large),
                        )
                        .automation_id("component-qa-segmented-constrained")
                        .width(180.0)
                        .height(20.0)],
                    ),
                ),
            ]),
        ])
        .gap(14.0),
        "form" => column_fit([
            qa_variant(
                "Vertical / target",
                qa_target_view(embed(tree! {
                    Form::new().label_width(86.0).gap(8.0).layout(FormLayout::Vertical) => [
                        tree! { FormItem::new("用户名").name("user").required(true).help("必填").layout(FormLayout::Vertical) => [
                            Input::new("请输入用户名").into_node(),
                        ]},
                        tree! { FormItem::new("邮箱").name("email").status(ValidateStatus::Success).layout(FormLayout::Vertical) => [
                            Input::new("name@example.com").into_node(),
                        ]},
                    ]
                })),
            ),
            qa_variant(
                "Horizontal / error",
                embed(tree! {
                    Form::new().label_width(86.0).gap(8.0).layout(FormLayout::Horizontal) => [
                        tree! { FormItem::new("密码").name("password").status(ValidateStatus::Error).help("至少 8 位").label_width(86.0).layout(FormLayout::Horizontal) => [
                            Input::new("请输入密码").into_node(),
                        ]},
                    ]
                }),
            ),
            qa_variant(
                "Inline / warning",
                embed(tree! {
                    Form::new().gap(8.0).layout(FormLayout::Inline) => [
                        tree! { FormItem::new("地区").name("region").status(ValidateStatus::Warning).help("请确认").layout(FormLayout::Inline) => [
                            Input::new("华东").into_node(),
                        ]},
                    ]
                }),
            ),
        ])
        .gap(10.0),
        "form-item" => column_fit([
            qa_target_view(
                embed(tree! { FormItem::new("用户名").required(true).help("必填字段") => [
                    Input::new("请输入用户名").into_node(),
                ]})
                .automation_id("component-qa-form-item-required"),
            ),
            embed(tree! { FormItem::new("邮箱").status(ValidateStatus::Success).help("格式正确") => [
                Input::new("name@example.com").into_node(),
            ]})
            .automation_id("component-qa-form-item-success"),
            embed(tree! { FormItem::new("密码").status(ValidateStatus::Error).help("密码强度不足") => [
                Input::new("请输入密码").into_node(),
            ]})
            .automation_id("component-qa-form-item-error"),
            qa_row([
                qa_variant(
                    "Long / clipped",
                    ViewNode::new(
                        Container::new()
                            .dir(FlexDirection::Column)
                            .w(220.0)
                            .h(60.0)
                            .align(AlignItems::Stretch),
                        vec![embed(tree! { FormItem::new("跨区域企业研发交付责任中心").status(ValidateStatus::Warning).help("帮助文字保持在字段边界内") => [
                            Input::new("华东区域").into_node(),
                        ]})
                        .automation_id("component-qa-form-item-long")
                        .width(220.0)
                        .height(60.0)],
                    ),
                ),
                qa_variant(
                    "Narrow / clipped",
                    ViewNode::new(
                        Container::new()
                            .dir(FlexDirection::Column)
                            .w(160.0)
                            .h(60.0)
                            .align(AlignItems::Stretch),
                        vec![embed(
                            FormItem::new("受限字段")
                                .required(true)
                                .status(ValidateStatus::Error)
                                .help("不能为空"),
                        )
                        .automation_id("component-qa-form-item-narrow")
                        .width(160.0)
                        .height(60.0)],
                    ),
                ),
                qa_variant(
                    "Inline / error",
                    embed(tree! { FormItem::new("邮箱").layout(FormLayout::Inline).status(ValidateStatus::Error).help("格式不正确") => [
                        Input::new("name@").into_node(),
                    ]})
                    .automation_id("component-qa-form-item-inline"),
                ),
            ]),
        ])
        .gap(10.0),
        _ => return None,
    };
    Some(view)
}
