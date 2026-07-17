//! 组件库页面 — page_input（输入控件全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_input(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    let city_options = vec![
        CascaderOption::new("北京", "beijing").children(vec![
            CascaderOption::new("海淀", "haidian"),
            CascaderOption::new("朝阳", "chaoyang"),
        ]),
        CascaderOption::new("上海", "shanghai").children(vec![
            CascaderOption::new("浦东", "pudong"),
            CascaderOption::new("徐汇", "xuhui"),
        ]),
    ];

    let tree_nodes = vec![
        TreeNode::new("前端", "fe").add(TreeNode::new("React", "react")),
        TreeNode::new("后端", "be").add(TreeNode::new("Rust", "rust")),
    ];

    PageBuilder::new(tk)
        .gap()
        .section("Input — 尺寸")
        .push(
            demo_row(32.0)
                .child(Input::new("Small...").size(ControlSize::Small))
                .child(Input::new("Medium...").size(ControlSize::Medium))
                .child(Input::new("Large...").size(ControlSize::Large)),
        )
        .section("InputNumber")
        .push(
            demo_row(36.0)
                .child(
                    InputNumber::new()
                        .placeholder("数量")
                        .min(0.0)
                        .max(100.0)
                        .step(1.0),
                )
                .child(
                    InputNumber::new()
                        .placeholder("价格")
                        .min(0.0)
                        .max(999.0)
                        .step(0.5),
                ),
        )
        .section("Select")
        .push(
            demo_row(36.0)
                .child(
                    Select::new()
                        .options(vec!["选项 1", "选项 2", "选项 3"])
                        .default_selected(1),
                )
                .child(Select::multiple().options(vec!["多选 A", "多选 B", "多选 C"])),
        )
        .push(labeled_row(
            tk,
            36.0,
            "Select 搜索 (100)",
            Select::searchable()
                .options((0..100).map(|i| format!("选项 {i}")).collect::<Vec<_>>())
                .default_selected(0),
        ))
        .section("TreeSelect")
        .push(labeled_row(
            tk,
            36.0,
            "TreeSelect",
            TreeSelect::new()
                .nodes(tree_nodes)
                .placeholder("选择技术栈"),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "TreeSelect (80)",
            TreeSelect::new()
                .nodes(
                    (0..80)
                        .map(|i| TreeNode::new(&format!("节点 {i}"), &format!("n-{i}")))
                        .collect(),
                )
                .placeholder("大列表树选择"),
        ))
        .section("Cascader")
        .push(labeled_row(
            tk,
            36.0,
            "Cascader",
            Cascader::new(city_options, "选择地区"),
        ))
        .section("AutoComplete / Mentions")
        .push(labeled_row(
            tk,
            36.0,
            "AutoComplete",
            AutoComplete::new()
                .placeholder("输入城市...")
                .options(vec!["北京", "上海", "广州", "深圳"]),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "Mentions",
            Mentions::new("输入 @ 提及").options(vec!["Alice", "Bob", "Charlie"]),
        ))
        .section("Checkbox / Radio / Switch")
        .push(
            demo_row(30.0)
                .child(Checkbox::new("选项 A").default_checked(true))
                .child(Checkbox::new("选项 B"))
                .child(Checkbox::new("禁用").disabled(true)),
        )
        .push(
            demo_row(32.0).child(
                Radio::new()
                    .options(vec!["苹果", "香蕉", "樱桃"])
                    .default_selected(1),
            ),
        )
        .push(
            demo_row(30.0)
                .child(Switch::new().default_checked(true))
                .child(Switch::new().default_checked(false))
                .child(Switch::new().default_checked(true).disabled(true)),
        )
        .section("Slider / Rate")
        .push(demo_row(30.0).child(Slider::new(0.0..=100.0).step(5.0).default_value(42.0)))
        .push(
            demo_row(30.0)
                .child(Rate::new().default_value(3))
                .child(Rate::new().count(7).default_value(5))
                .child(Rate::new().default_value(2).allow_half()),
        )
        .section("DatePicker / TimePicker / ColorPicker")
        .push(
            demo_row(36.0)
                .child(
                    DatePicker::new()
                        .placeholder("选择日期")
                        .default_value(Date::new(2026, 6, 20))
                        .disabled_date(|date| date.weekday().is_weekend()),
                )
                .child(
                    DatePicker::new()
                        .placeholder("选择周")
                        .mode(PickerMode::Week),
                ),
        )
        .push(
            demo_row(36.0).child(
                DateRangePicker::new()
                    .default_range(Date::new(2026, 6, 1), Date::new(2026, 6, 7))
                    .presets([
                        ("今天", PresetDate::today()),
                        ("最近 7 天", PresetDate::last_days(7)),
                    ])
                    .disabled_date(|date| date.weekday().is_weekend()),
            ),
        )
        .push(
            demo_row(36.0).child(
                TimePicker::new()
                    .placeholder("选择时间")
                    .default_value(Time::new(14, 30)),
            ),
        )
        .push(labeled_row(
            tk,
            36.0,
            "ColorPicker",
            ColorPicker::new().default_value(tk.color_primary),
        ))
        .section("Segmented")
        .push(
            demo_row(36.0)
                .child(Segmented::new(["每日", "每周", "每月", "每年"]).default_selected(2)),
        )
        .section("Form / FormItem")
        .push(tree! {
            Form::new().label_width(80.0).gap(8.0).layout(FormLayout::Vertical) => [
                tree! { FormItem::new("用户名").name("user").required(true).help("必填") => [
                    Input::new("请输入用户名").into_node(),
                ]},
                tree! { FormItem::new("邮箱").name("email").status(ValidateStatus::Success) => [
                    Input::new("name@example.com").into_node(),
                ]},
                tree! { FormItem::new("密码").name("pwd").status(ValidateStatus::Error) => [
                    Input::new("请输入密码").into_node(),
                ]},
            ]
        })
        .build()
}
