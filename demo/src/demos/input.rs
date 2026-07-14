//! 组件库页面 — page_input（输入控件全覆盖）。

use uix::prelude::*;

use crate::common::page::{PageBuilder, INNER_W};
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
            row((
                Input::new("Small...").size(ControlSize::Small),
                Input::new("Medium...").size(ControlSize::Medium),
                Input::new("Large...").size(ControlSize::Large),
            ))
            .height(32.0)
            .align(AlignItems::Center)
            .gap(8.0),
        )
        .section("InputNumber")
        .push(
            row((
                InputNumber::new("数量").min(0.0).max(100.0).step(1.0),
                InputNumber::new("价格").min(0.0).max(999.0).step(0.5),
            ))
            .height(36.0)
            .align(AlignItems::Center)
            .gap(12.0),
        )
        .section("Select")
        .push(
            Select::new()
                .options(vec!["选项 1", "选项 2", "选项 3"])
                .selected(1),
        )
        .push(labeled_row(
            tk,
            36.0,
            "Select (100)",
            Select::new()
                .options((0..100).map(|i| format!("选项 {i}")).collect::<Vec<_>>())
                .selected(0),
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
            row((
                Checkbox::new("选项 A").checked(true),
                Checkbox::new("选项 B"),
                Checkbox::new("禁用").disabled(true),
            ))
            .height(30.0)
            .align(AlignItems::Center)
            .gap(16.0),
        )
        .push(
            Radio::new()
                .options(vec!["苹果", "香蕉", "樱桃"])
                .selected(1),
        )
        .push(
            row((
                Switch::new().checked(true),
                Switch::new().checked(false),
                Switch::new().checked(true).disabled(true),
            ))
            .height(30.0)
            .align(AlignItems::Center)
            .gap(12.0),
        )
        .section("Slider / Rate")
        .push(Slider::new().range(0.0, 100.0).step(5.0).value(42.0))
        .push(
            row((
                Rate::new().value(3),
                Rate::new().count(7).value(5),
                Rate::new().value(2).allow_half(),
            ))
            .height(30.0)
            .align(AlignItems::Center)
            .gap(16.0),
        )
        .section("DatePicker / TimePicker / ColorPicker")
        .push(DatePicker::new("选择日期").value(DateValue::new(2026, 6, 20)))
        .push(TimePicker::new("选择时间").value(TimeValue::new(14, 30)))
        .push(labeled_row(
            tk,
            36.0,
            "ColorPicker",
            ColorPicker::new(tk.color_primary),
        ))
        .section("Segmented")
        .push(
            Segmented::new()
                .options(vec!["每日", "每周", "每月", "每年"])
                .selected(2),
        )
        .section("Form / FormItem")
        .push(
            column((
                Form::new()
                    .label_width(80.0)
                    .gap(8.0)
                    .layout(FormLayout::Vertical),
                FormItem::new("用户名")
                    .name("user")
                    .required(true)
                    .help("必填"),
                FormItem::new("邮箱")
                    .name("email")
                    .status(ValidateStatus::Success),
                FormItem::new("密码")
                    .name("pwd")
                    .status(ValidateStatus::Error),
            ))
            .width(INNER_W)
            .height(200.0)
            .gap(8.0),
        )
        .build()
}
