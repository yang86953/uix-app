use uix::prelude::*;

use super::{qa_row, qa_target, qa_target_view, qa_variant};

fn city_options() -> Vec<CascaderOption> {
    vec![
        CascaderOption::new("北京", "beijing").children(vec![
            CascaderOption::new("海淀", "haidian"),
            CascaderOption::new("朝阳", "chaoyang"),
        ]),
        CascaderOption::new("上海", "shanghai").children(vec![
            CascaderOption::new("浦东", "pudong"),
            CascaderOption::new("徐汇", "xuhui"),
        ]),
    ]
}

fn tree_nodes() -> Vec<TreeNode> {
    vec![
        TreeNode::new("前端", "frontend")
            .add(TreeNode::new("React", "react"))
            .add(TreeNode::new("Vue", "vue")),
        TreeNode::new("后端", "backend")
            .add(TreeNode::new("Rust", "rust"))
            .add(TreeNode::new("Go", "go")),
    ]
}

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

fn build_date_picker_case() -> ViewNode {
    column_fit([
        qa_row([
            qa_variant(
                "Value / target",
                qa_target(
                    DatePicker::new()
                        .default_value(Date::new(2026, 7, 17))
                        .disabled_date(|date| date.weekday().is_weekend()),
                ),
            ),
            qa_variant(
                "Long placeholder",
                embed(DatePicker::new().placeholder("选择一个完整发布日期"))
                    .automation_id("component-qa-date-empty"),
            ),
            qa_variant(
                "Week",
                embed(
                    DatePicker::new()
                        .placeholder("选择周")
                        .mode(PickerMode::Week),
                )
                .automation_id("component-qa-date-week"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Small",
                embed(
                    DatePicker::new()
                        .size(ControlSize::Small)
                        .default_value(Date::new(2026, 7, 17)),
                )
                .automation_id("component-qa-date-small"),
            ),
            qa_variant(
                "Large",
                embed(
                    DatePicker::new()
                        .size(ControlSize::Large)
                        .default_value(Date::new(2026, 7, 17)),
                )
                .automation_id("component-qa-date-large"),
            ),
            qa_variant(
                "Large / constrained",
                grid([embed(
                    DatePicker::new()
                        .size(ControlSize::Large)
                        .placeholder("受限高度下仍需裁切文字"),
                )
                .automation_id("component-qa-date-constrained")])
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

fn build_date_range_picker_case() -> ViewNode {
    column_fit([
        qa_row([
            qa_variant(
                "Range / target",
                qa_target(
                    DateRangePicker::new()
                        .default_range(Date::new(2026, 7, 1), Date::new(2026, 7, 17))
                        .presets([
                            (
                                "发布窗口",
                                PresetDate::new(Date::new(2026, 7, 1), Date::new(2026, 7, 17)),
                            ),
                            (
                                "跨季度发布窗口（需确认全部审批、风险评估、回滚方案、负责人、监控指标与复盘计划归档）",
                                PresetDate::new(Date::new(2026, 6, 29), Date::new(2026, 7, 3)),
                            ),
                        ])
                        .disabled_date(|date| date.weekday().is_weekend()),
                ),
            ),
            qa_variant(
                "Long placeholder",
                embed(DateRangePicker::new().placeholder("选择开始日期与结束日期范围"))
                    .automation_id("component-qa-date-range-empty"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Small",
                embed(
                    DateRangePicker::new()
                        .size(ControlSize::Small)
                        .default_range(Date::new(2026, 7, 1), Date::new(2026, 7, 17)),
                )
                .automation_id("component-qa-date-range-small"),
            ),
            qa_variant(
                "Large",
                embed(
                    DateRangePicker::new()
                        .size(ControlSize::Large)
                        .default_range(Date::new(2026, 7, 1), Date::new(2026, 7, 17)),
                )
                .automation_id("component-qa-date-range-large"),
            ),
            qa_variant(
                "Large / constrained",
                grid([embed(
                    DateRangePicker::new()
                        .size(ControlSize::Large)
                        .placeholder("受限高度下的长日期范围"),
                )
                .automation_id("component-qa-date-range-constrained")])
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

fn build_time_picker_case() -> ViewNode {
    column_fit([
        qa_row([
            qa_variant(
                "Value / target",
                qa_target(TimePicker::new().default_value(Time::new(14, 30))),
            ),
            qa_variant(
                "Long placeholder",
                embed(TimePicker::new().placeholder("请选择一个精确到分钟的发布时间"))
                    .automation_id("component-qa-time-empty"),
            ),
            qa_variant(
                "Late 23:59",
                embed(TimePicker::new().default_value(Time::new(23, 59)))
                    .automation_id("component-qa-time-late"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Small",
                embed(
                    TimePicker::new()
                        .size(ControlSize::Small)
                        .default_value(Time::new(9, 58)),
                )
                .automation_id("component-qa-time-small"),
            ),
            qa_variant(
                "Large",
                embed(
                    TimePicker::new()
                        .size(ControlSize::Large)
                        .default_value(Time::new(18, 7)),
                )
                .automation_id("component-qa-time-large"),
            ),
            qa_variant(
                "Large / constrained",
                grid([embed(
                    TimePicker::new()
                        .size(ControlSize::Large)
                        .placeholder("受限高度下仍需裁切时间占位文字"),
                )
                .automation_id("component-qa-time-constrained")])
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

fn build_color_picker_case(tokens: &DesignTokens) -> ViewNode {
    column_fit([
        qa_row([
            qa_variant(
                "Primary / target",
                row([qa_target(
                    ColorPicker::new().default_value(tokens.color_primary),
                )]),
            ),
            qa_variant(
                "Light preset",
                row([
                    embed(ColorPicker::new().default_value(Color::from_rgb(0xF0, 0xF0, 0xF0)))
                        .automation_id("component-qa-color-light"),
                ]),
            ),
            qa_variant(
                "Alpha",
                row([embed(
                    ColorPicker::new().default_value(Color::from_rgba(0x16, 0x77, 0xFF, 0x60)),
                )
                .automation_id("component-qa-color-alpha")]),
            ),
        ]),
        qa_row([
            qa_variant(
                "Small",
                row([embed(
                    ColorPicker::new()
                        .size(ControlSize::Small)
                        .default_value(tokens.color_error),
                )
                .automation_id("component-qa-color-small")]),
            ),
            qa_variant(
                "Large",
                row([embed(
                    ColorPicker::new()
                        .size(ControlSize::Large)
                        .default_value(tokens.color_success),
                )
                .automation_id("component-qa-color-large")]),
            ),
            qa_variant(
                "Large / constrained",
                grid([embed(
                    ColorPicker::new()
                        .size(ControlSize::Large)
                        .default_value(Color::from_rgba(0x72, 0x2E, 0xD1, 0x80)),
                )
                .automation_id("component-qa-color-constrained")])
                .columns(vec![GridTrack::Fr(1.0)])
                .rows(vec![GridTrack::Fr(1.0)])
                .width(12.0)
                .height(6.0)
                .align_self(AlignItems::Start),
            ),
        ]),
    ])
    .gap(14.0)
}

fn build_cascader_case() -> ViewNode {
    let long_options =
        vec![
            CascaderOption::new("跨区域企业发布审批与风险评估责任中心", "enterprise").children(
                vec![CascaderOption::new(
                    "华东区域上海研发与交付协同中心",
                    "east-china",
                )],
            ),
        ];
    let scrolling_options = (0..10)
        .map(|index| CascaderOption::new(format!("区域 {index}"), format!("region-{index}")))
        .collect();

    column_fit([
        qa_row([
            qa_variant(
                "Hierarchy / target",
                qa_target(Cascader::new(city_options(), "选择地区")),
            ),
            qa_variant(
                "Long placeholder",
                embed(Cascader::new(
                    Vec::<CascaderOption>::new(),
                    "请选择完整的国家省份城市和业务区域路径",
                ))
                .automation_id("component-qa-cascader-empty"),
            ),
            qa_variant(
                "Disabled branch",
                embed(Cascader::new(
                    vec![
                        CascaderOption::new("可选", "enabled"),
                        CascaderOption::new("禁用", "disabled").disabled(true),
                    ],
                    "分支状态",
                ))
                .automation_id("component-qa-cascader-disabled"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Long option",
                embed(Cascader::new(long_options, "长选项"))
                    .automation_id("component-qa-cascader-long"),
            ),
            qa_variant(
                "10 options",
                embed(Cascader::new(scrolling_options, "滚动区域"))
                    .automation_id("component-qa-cascader-scroll"),
            ),
            qa_variant(
                "Constrained",
                grid([embed(Cascader::new(
                    city_options(),
                    "受限高度下的超长地区占位文字",
                ))
                .automation_id("component-qa-cascader-constrained")])
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

fn build_tree_select_case() -> ViewNode {
    let long_nodes = vec![
        TreeNode::new("跨区域企业研发交付与风险评估责任中心", "enterprise").children(vec![
            TreeNode::new("华东区域上海研发与交付协同中心", "east-china"),
        ]),
    ];
    let scrolling_nodes = (0..40)
        .map(|index| TreeNode::new(&format!("节点 {index}"), &format!("node-{index}")))
        .collect();

    column_fit([
        qa_row([
            qa_variant(
                "Hierarchy / target",
                qa_target(
                    TreeSelect::new()
                        .nodes(tree_nodes())
                        .placeholder("选择技术栈"),
                ),
            ),
            qa_variant(
                "Empty / no data",
                embed(TreeSelect::new().placeholder("请选择节点"))
                    .automation_id("component-qa-tree-select-empty"),
            ),
            qa_variant(
                "Disabled node",
                embed(
                    TreeSelect::new()
                        .nodes(vec![
                            TreeNode::new("禁用节点", "disabled").disabled(true),
                            TreeNode::new("可用节点", "enabled"),
                        ])
                        .placeholder("节点状态"),
                )
                .automation_id("component-qa-tree-select-disabled"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Long hierarchy",
                embed(TreeSelect::new().nodes(long_nodes).placeholder("长节点"))
                    .automation_id("component-qa-tree-select-long"),
            ),
            qa_variant(
                "40 nodes",
                embed(
                    TreeSelect::new()
                        .nodes(scrolling_nodes)
                        .placeholder("40 个节点"),
                )
                .automation_id("component-qa-tree-select-scroll"),
            ),
            qa_variant(
                "Constrained",
                grid([embed(
                    TreeSelect::new()
                        .nodes(tree_nodes())
                        .placeholder("受限高度下的超长技术节点占位文字"),
                )
                .automation_id("component-qa-tree-select-constrained")])
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

fn build_autocomplete_case() -> ViewNode {
    let scrolling_options = (0..30)
        .map(|index| format!("候选 {index}"))
        .collect::<Vec<_>>();

    column_fit([
        qa_row([
            qa_variant(
                "Suggestions / target",
                qa_target(
                    AutoComplete::new()
                        .placeholder("输入城市")
                        .options(vec!["北京", "上海", "广州", "深圳"]),
                ),
            ),
            qa_variant(
                "Empty / no data",
                embed(AutoComplete::new().placeholder("无候选数据"))
                    .automation_id("component-qa-autocomplete-empty"),
            ),
            qa_variant(
                "Long placeholder",
                embed(
                    AutoComplete::new().placeholder("请输入完整的组件名称、命名空间与业务关键词"),
                )
                .automation_id("component-qa-autocomplete-placeholder"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Long options",
                embed(AutoComplete::new().placeholder("搜索组件").options(vec![
                    "跨区域企业研发交付与风险评估责任中心",
                    "ComponentVisualQualityGateWithLongNamespace",
                ]))
                .automation_id("component-qa-autocomplete-long"),
            ),
            qa_variant(
                "30 options",
                embed(
                    AutoComplete::new()
                        .placeholder("滚动候选")
                        .options(scrolling_options),
                )
                .automation_id("component-qa-autocomplete-scroll"),
            ),
            qa_variant(
                "Constrained",
                grid([embed(
                    AutoComplete::new()
                        .placeholder("受限高度下的超长自动完成占位文字")
                        .options(vec!["北京", "上海"]),
                )
                .automation_id("component-qa-autocomplete-constrained")])
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

fn build_mentions_case() -> ViewNode {
    let scrolling_options = (0..30)
        .map(|index| format!("候选 {index}"))
        .collect::<Vec<_>>();

    column_fit([
        qa_row([
            qa_variant(
                "Suggestions / target",
                qa_target(Mentions::new("输入消息并使用 @ 提及").options(vec![
                    "Ada",
                    "Alan",
                    "Grace",
                    "贝露丹迪",
                ])),
            ),
            qa_variant(
                "Empty / no data",
                embed(Mentions::new("无候选人员")).automation_id("component-qa-mentions-empty"),
            ),
            qa_variant(
                "Long placeholder",
                embed(Mentions::new(
                    "请输入完整消息并使用 @ 选择需要协作的研发与质量负责人",
                ))
                .automation_id("component-qa-mentions-placeholder"),
            ),
        ]),
        qa_row([
            qa_variant(
                "Long options",
                embed(Mentions::new("提及协作者").options(vec![
                    "跨区域企业研发交付与风险评估责任中心负责人",
                    "ComponentVisualQualityGateOwnerWithLongNamespace",
                ]))
                .automation_id("component-qa-mentions-long"),
            ),
            qa_variant(
                "30 options",
                embed(Mentions::new("滚动选择协作者").options(scrolling_options))
                    .automation_id("component-qa-mentions-scroll"),
            ),
            qa_variant(
                "Constrained",
                grid([embed(
                    Mentions::new("受限高度下的超长提及输入占位文字").options(vec!["Ada", "Alan"]),
                )
                .automation_id("component-qa-mentions-constrained")])
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
                qa_row([qa_variant(
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
                )]),
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
