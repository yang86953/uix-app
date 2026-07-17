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

pub fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "input" => {
            let value = State::new("已填写内容".to_string());
            qa_row([
                qa_variant("Default / target", qa_target(Input::new("请输入内容"))),
                qa_variant("Value", embed(Input::new("").value(&value))),
                qa_variant("Disabled", embed(Input::new("不可编辑").disabled(true))),
            ])
        }
        "input-number" => qa_row([
            qa_variant(
                "Default / target",
                qa_target(InputNumber::new().placeholder("数量").min(0.0).max(100.0)),
            ),
            qa_variant("Value", embed(InputNumber::new().default_value(42.0))),
            qa_variant(
                "Disabled",
                embed(InputNumber::new().default_value(18.0).disabled(true)),
            ),
        ]),
        "select" => qa_row([
            qa_variant(
                "Selected / target",
                qa_target(
                    Select::new()
                        .options(["设计", "开发", "测试"])
                        .default_selected(1),
                ),
            ),
            qa_variant(
                "Multiple",
                embed(Select::multiple().options(["A", "B", "C"])),
            ),
            qa_variant(
                "Disabled",
                embed(
                    Select::new()
                        .options(["不可用"])
                        .default_selected(0)
                        .disabled(true),
                ),
            ),
        ]),
        "checkbox" => qa_row([
            qa_variant("Target", qa_target(Checkbox::new("未选择"))),
            qa_variant("Checked", embed(Checkbox::new("已选择").default_checked(true))),
            qa_variant("Disabled", embed(Checkbox::new("禁用").disabled(true))),
        ]),
        "radio" => column_fit([
            qa_variant(
                "Selected / target",
                qa_target(
                    Radio::new()
                        .options(["苹果", "香蕉", "樱桃"])
                        .default_selected(1),
                ),
            ),
            qa_variant(
                "Disabled",
                embed(
                    Radio::new()
                        .options(["甲", "乙"])
                        .default_selected(0)
                        .disabled(true),
                ),
            ),
        ])
        .gap(14.0),
        "switch" => qa_row([
            qa_variant("Off / target", qa_target(Switch::new())),
            qa_variant("On", embed(Switch::new().default_checked(true))),
            qa_variant(
                "Disabled",
                embed(Switch::new().default_checked(true).disabled(true)),
            ),
        ]),
        "slider" => column_fit([
            qa_variant(
                "42 / target",
                qa_target(Slider::new(0.0..=100.0).step(5.0).default_value(42.0)),
            ),
            qa_variant(
                "Minimum",
                embed(Slider::new(0.0..=100.0).default_value(0.0)),
            ),
        ])
        .width(620.0)
        .gap(18.0),
        "rate" => qa_row([
            qa_variant("Selected / target", qa_target(Rate::new().default_value(3))),
            qa_variant("Empty", embed(Rate::new())),
            qa_variant(
                "Half",
                embed(Rate::new().count(7).default_value(5).allow_half()),
            ),
            qa_variant(
                "Disabled",
                embed(Rate::new().default_value(4).disabled(true)),
            ),
        ]),
        "date-picker" => qa_row([
            qa_variant(
                "Value / target",
                qa_target(
                    DatePicker::new()
                        .default_value(Date::new(2026, 7, 17))
                        .disabled_date(|date| date.weekday().is_weekend()),
                ),
            ),
            qa_variant("Empty", embed(DatePicker::new().placeholder("选择日期"))),
            qa_variant(
                "Week",
                embed(DatePicker::new().placeholder("选择周").mode(PickerMode::Week)),
            ),
        ]),
        "date-range-picker" => column_fit([
            qa_variant(
                "Range / target",
                qa_target(
                    DateRangePicker::new()
                        .default_range(Date::new(2026, 7, 1), Date::new(2026, 7, 17))
                        .presets([
                            ("今天", PresetDate::today()),
                            ("最近 7 天", PresetDate::last_days(7)),
                        ])
                        .disabled_date(|date| date.weekday().is_weekend()),
                ),
            ),
            qa_variant(
                "Empty",
                embed(DateRangePicker::new().placeholder("开始日期 — 结束日期")),
            ),
        ])
        .gap(14.0),
        "time-picker" => qa_row([
            qa_variant(
                "Value / target",
                qa_target(TimePicker::new().default_value(Time::new(14, 30))),
            ),
            qa_variant("Empty", embed(TimePicker::new().placeholder("选择时间"))),
        ]),
        "color-picker" => qa_row([
            qa_variant("Primary / target", qa_target(ColorPicker::new().default_value(tk.color_primary))),
            qa_variant("Error", embed(ColorPicker::new().default_value(tk.color_error))),
            qa_variant("Success", embed(ColorPicker::new().default_value(tk.color_success))),
        ]),
        "cascader" => qa_row([
            qa_variant(
                "Hierarchy / target",
                qa_target(Cascader::new(city_options(), "选择地区")),
            ),
            qa_variant(
                "Disabled branch",
                embed(Cascader::new(
                    vec![
                        CascaderOption::new("可选", "enabled"),
                        CascaderOption::new("禁用", "disabled").disabled(true),
                    ],
                    "分支状态",
                )),
            ),
        ]),
        "tree-select" => qa_row([
            qa_variant(
                "Hierarchy / target",
                qa_target(TreeSelect::new().nodes(tree_nodes()).placeholder("选择技术栈")),
            ),
            qa_variant(
                "Large data",
                embed(
                    TreeSelect::new()
                        .nodes(
                            (0..40)
                                .map(|index| {
                                    TreeNode::new(
                                        &format!("节点 {index}"),
                                        &format!("node-{index}"),
                                    )
                                })
                                .collect(),
                        )
                        .placeholder("40 个节点"),
                ),
            ),
        ]),
        "auto-complete" => qa_row([
            qa_variant(
                "Suggestions / target",
                qa_target(
                    AutoComplete::new()
                        .placeholder("输入城市")
                        .options(vec!["北京", "上海", "广州", "深圳"]),
                ),
            ),
            qa_variant(
                "Long options",
                embed(
                    AutoComplete::new()
                        .placeholder("搜索组件")
                        .options(vec!["ComponentVisualQualityGate", "ComponentSnapshot"]),
                ),
            ),
        ]),
        "mentions" => qa_row([
            qa_variant(
                "Suggestions / target",
                qa_target(
                    Mentions::new("输入 @ 提及")
                        .options(vec!["Alice", "Bob", "Charlie", "UIX-QA"]),
                ),
            ),
            qa_variant(
                "Long placeholder",
                embed(Mentions::new("输入消息并使用 @ 选择协作者")),
            ),
        ]),
        "segmented" => qa_row([
            qa_variant(
                "Selected / target",
                qa_target(Segmented::new(["每日", "每周", "每月", "每年"]).default_selected(2)),
            ),
            qa_variant(
                "Disabled",
                embed(Segmented::new(["甲", "乙", "丙"]).default_selected(1).disabled(true)),
            ),
        ]),
        "form" => qa_target_view(embed(tree! {
            Form::new().label_width(86.0).gap(8.0).layout(FormLayout::Vertical) => [
                tree! { FormItem::new("用户名").name("user").required(true).help("必填") => [
                    Input::new("请输入用户名").into_node(),
                ]},
                tree! { FormItem::new("邮箱").name("email").status(ValidateStatus::Success) => [
                    Input::new("name@example.com").into_node(),
                ]},
                tree! { FormItem::new("密码").name("password").status(ValidateStatus::Error).help("至少 8 位") => [
                    Input::new("请输入密码").into_node(),
                ]},
            ]
        })),
        "form-item" => column_fit([
            qa_target_view(embed(tree! { FormItem::new("用户名").required(true).help("必填字段") => [
                Input::new("请输入用户名").into_node(),
            ]})),
            embed(tree! { FormItem::new("邮箱").status(ValidateStatus::Success).help("格式正确") => [
                Input::new("name@example.com").into_node(),
            ]}),
            embed(tree! { FormItem::new("密码").status(ValidateStatus::Error).help("密码强度不足") => [
                Input::new("请输入密码").into_node(),
            ]}),
        ])
        .gap(10.0),
        _ => return None,
    };
    Some(view)
}
