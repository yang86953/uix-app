//! 组件库页面 — page_input。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};

pub fn page_input(tk: &DesignTokens) -> ViewNode {
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
    PageBuilder::new(tk)
        .gap()
        .section("输入框 Input — 3 种尺寸")
        .push(
            demo_row(28.0)
                .child(Input::new("小型...").size(ControlSize::Small))
                .child(Input::new("中型...").size(ControlSize::Medium))
                .child(Input::new("大型...").size(ControlSize::Large)),
        )
        .section("数字输入 InputNumber")
        .push(
            demo_row(36.0)
                .child(InputNumber::new("数量").min(0.0).max(100.0).step(1.0))
                .child(InputNumber::new("价格").min(0.0).max(999.0).step(0.5)),
        )
        .section("选择 Select")
        .push(
            demo_row(36.0).child(
                Select::new()
                    .options(vec!["选项 1", "选项 2", "选项 3"])
                    .selected(2),
            ),
        )
        .section("级联 Cascader")
        .push(demo_row(36.0).child(Cascader::new(city_options, "选择地区")))
        .section("提及 Mentions")
        .push(
            demo_row(36.0).child(Mentions::new("输入 @ 提及").options(vec!["Alice", "Bob", "Charlie"])),
        )
        .section("自动补全 AutoComplete")
        .push(
            tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                AutoComplete::new().placeholder("输入城市名称...")
                    .options(vec!["北京", "上海", "广州", "深圳"]).into_node(),
            ]},
        )
        .section("开关 Switch")
        .push(
            demo_row(30.0)
                .child(Switch::new().checked(true))
                .child(Switch::new().checked(false))
                .child(Switch::new().checked(true).disabled(true)),
        )
        .section("复选框 Checkbox")
        .push(
            demo_row(30.0)
                .child(Checkbox::new("选项 A").checked(true))
                .child(Checkbox::new("选项 B"))
                .child(Checkbox::new("选项 C").disabled(true)),
        )
        .section("单选框 Radio")
        .push(
            demo_row(32.0).child(
                Radio::new()
                    .options(vec!["苹果", "香蕉", "樱桃"])
                    .selected(1),
            ),
        )
        .section("滑块 Slider")
        .push(demo_row(30.0).child(Slider::new().range(0.0, 100.0).step(5.0).value(42.0)))
        .section("评分 Rate")
        .push(
            demo_row(30.0)
                .child(Rate::new().value(3))
                .child(Rate::new().count(7).value(5))
                .child(Rate::new().value(2).allow_half()),
        )
        .section("日期选择 DatePicker")
        .push(demo_row(36.0).child(DatePicker::new("选择日期").value(DateValue::new(2026, 6, 20))))
        .section("时间选择 TimePicker")
        .push(demo_row(36.0).child(TimePicker::new("选择时间").value(TimeValue::new(14, 30))))
        .build()

}
