//! 输入组件测试场景：日期与选择器类。
//! 由 input.rs 以 `#[path]` 引入，构建函数在父模块 build 中按 id 分发。

use uix::prelude::*;

use super::super::{qa_row, qa_target, qa_variant};

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

pub(super) fn build_date_picker_case() -> ViewNode {
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

pub(super) fn build_date_range_picker_case() -> ViewNode {
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

pub(super) fn build_time_picker_case() -> ViewNode {
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

pub(super) fn build_color_picker_case(tokens: &DesignTokens) -> ViewNode {
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

pub(super) fn build_cascader_case() -> ViewNode {
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

pub(super) fn build_tree_select_case() -> ViewNode {
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

pub(super) fn build_autocomplete_case() -> ViewNode {
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

pub(super) fn build_mentions_case() -> ViewNode {
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

