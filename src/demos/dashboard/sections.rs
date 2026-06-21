use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::layout::{AlignItems, FlexDirection};
use uix::ui::{
    Alert, AlertType, BarChart, BarData, Button, ButtonSize, Card,
    Cascader, CascaderOption, Checkbox, Container, DatePicker, DateValue, Empty,
    Input, InputNumber, InputSize, IntoWidgetNode,
    Label, LineChart, LineData, Mentions, PieChart, PieData, ProgressBar, Radio, Rate,
    Select, Skeleton, SkeletonShape, Slider, Spin, Switch, Tag, TagColor,
    TimePicker, TimeValue, Space, SpaceSize,
};
use super::{INNER_W, PageBuilder, section_title, row};

// ── Page 0: Typography ──

pub fn page_typography(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("字体大小")
        .push(
            row(36.0)
                .child(Label::new("12px 小号").color(tk.color_text).font_size(12.0))
                .child(Label::new("14px 默认").color(tk.color_text).font_size(14.0))
                .child(Label::new("20px 大号").color(tk.color_text).font_size(20.0)),
        )
        .section("文字颜色")
        .push(
            row(28.0)
                .child(Label::new("Primary").color(tk.color_primary).font_size(14.0))
                .child(Label::new("Secondary").color(tk.color_text_secondary).font_size(14.0))
                .child(Label::new("Tertiary").color(tk.color_text_tertiary).font_size(14.0))
                .child(Label::new("Quaternary").color(tk.color_text_quaternary).font_size(14.0)),
        )
        .section("语义色")
        .push(
            row(28.0)
                .child(Label::new("Primary").color(tk.color_primary).font_size(14.0))
                .child(Label::new("Success").color(tk.color_success).font_size(14.0))
                .child(Label::new("Warning").color(tk.color_warning).font_size(14.0))
                .child(Label::new("Error").color(tk.color_error).font_size(14.0))
                .child(Label::new("Info").color(tk.color_info).font_size(14.0)),
        )
        .build()
}

// ── Page 1: Buttons ──

pub fn page_buttons(tk: &DesignTokens) -> WidgetNode {
    let mut page = PageBuilder::new(tk).gap().section("5 种变体 × 3 种尺寸");

    for (label, h, sz) in [
        ("Small", 28.0, ButtonSize::Small),
        ("Middle", 36.0, ButtonSize::Middle),
        ("Large", 44.0, ButtonSize::Large),
    ] {
        page = page
            .push(section_title(tk, label))
            .push(
                row(h)
                    .child(Button::new("主要").primary().size(sz))
                    .child(Button::new("默认").size(sz))
                    .child(Button::new("虚线").dashed().size(sz))
                    .child(Button::new("文字").text().size(sz))
                    .child(Button::new("链接").link().size(sz)),
            );
    }

    page.section("按钮组 — 保存 + 取消")
        .push(
            row(44.0)
                .child(Button::new("保存").primary().size(ButtonSize::Middle))
                .child(Button::new("取消").size(ButtonSize::Middle)),
        )
        .build()
}

// ── Page 2: Inputs ──

pub fn page_inputs(tk: &DesignTokens) -> WidgetNode {
    let city_options = vec![
        CascaderOption::new("北京", "beijing").children(vec![
            CascaderOption::new("海淀", "haidian"), CascaderOption::new("朝阳", "chaoyang"),
        ]),
        CascaderOption::new("上海", "shanghai").children(vec![
            CascaderOption::new("浦东", "pudong"), CascaderOption::new("徐汇", "xuhui"),
        ]),
    ];

    PageBuilder::new(tk)
        .gap()
        .section("输入框 — 3 种尺寸")
        .push(
            row(28.0)
                .child(Input::new("小型...").size(InputSize::Small))
                .child(Input::new("中型...").size(InputSize::Middle))
                .child(Input::new("大型...").size(InputSize::Large)),
        )
        .section("数字输入 (InputNumber)")
        .push(
            row(36.0)
                .child(InputNumber::new("数量").min(0.0).max(100.0).step(1.0))
                .child(InputNumber::new("价格").min(0.0).max(999.0).step(0.5)),
        )
        .section("提及 (Mentions)")
        .push(
            row(36.0)
                .child(Mentions::new("输入 @ 提及").options(vec!["Alice", "Bob", "Charlie"])),
        )
        .section("级联 (Cascader)")
        .push(
            row(36.0)
                .child(Cascader::new(city_options, "选择地区")),
        )
        .section("Switch")
        .push(
            row(30.0)
                .child(Switch::new().checked(true))
                .child(Switch::new().checked(false))
                .child(Switch::new().checked(true).disabled(true)),
        )
        .section("Checkbox")
        .push(
            row(30.0)
                .child(Checkbox::new("Option A").checked(true))
                .child(Checkbox::new("Option B"))
                .child(Checkbox::new("Option C").disabled(true)),
        )
        .section("Radio")
        .push(row(32.0).child(Radio::new().options(vec!["Apple", "Banana", "Cherry"]).selected(1)))
        .section("Select")
        .push(row(36.0).child(Select::new().options(vec!["Option 1", "Option 2", "Option 3"]).selected(2)))
        .section("Slider")
        .push(row(30.0).child(Slider::new().range(0.0, 100.0).step(5.0).value(42.0)))
        .section("Rate")
        .push(
            row(30.0)
                .child(Rate::new().value(3))
                .child(Rate::new().count(7).value(5))
                .child(Rate::new().value(2).allow_half()),
        )
        .build()
}

// ── Page 3: Date & Time ──

pub fn page_date_picker(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("日期选择 (DatePicker)")
        .push(row(36.0).child(DatePicker::new("选择日期").value(DateValue::new(2026, 6, 20))))
        .section("时间选择 (TimePicker)")
        .push(row(36.0).child(TimePicker::new("选择时间").value(TimeValue::new(14, 30))))
        .build()
}

// ── Page 4: Data Display ──

pub fn page_data_display(tk: &DesignTokens) -> WidgetNode {
    let c4 = (INNER_W - 24.0) / 4.0;

    PageBuilder::new(tk)
        .gap()
        .section("卡片 — 阴影层级 0 ~ 3")
        .push(
            Space::new().size(SpaceSize::Middle).width(INNER_W).height(130.0)
                .direction(FlexDirection::Row).align(AlignItems::Stretch)
                .child(Card::new().title("Elevation 0").elevation(0).bordered(true).size(c4, 120.0)
                    .child(Label::new("有边框，无阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 1").elevation(1).size(c4, 120.0)
                    .child(Label::new("柔和阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 2").elevation(2).size(c4, 120.0)
                    .child(Label::new("中等阴影 + 强调色").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 3").elevation(3).size(c4, 120.0)
                    .child(Label::new("深阴影 + 强调色").color(tk.color_text_tertiary).font_size(12.0))),
        )
        .section("卡片 — 悬停与空状态")
        .push(
            row(100.0).align(AlignItems::Stretch)
                .child(Card::new().title("Hoverable Card").elevation(1).hoverable().size((INNER_W - 10.0) / 2.0, 90.0)
                    .child(Label::new("悬停变亮").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Empty List").elevation(1).size((INNER_W - 10.0) / 2.0, 90.0)
                    .child(Label::new("暂无数据").color(tk.color_text_quaternary).font_size(14.0))),
        )
        .section("柱状图 — 月活跃用户")
        .push(
            BarChart::new()
                .width(INNER_W).height(180.0).show_value(true)
                .data(vec![
                    BarData::new("Jan", 420.0, tk.color_primary),
                    BarData::new("Feb", 380.0, tk.color_primary),
                    BarData::new("Mar", 530.0, tk.color_success),
                    BarData::new("Apr", 490.0, tk.color_primary),
                    BarData::new("May", 620.0, tk.color_success),
                    BarData::new("Jun", 580.0, tk.color_warning),
                    BarData::new("Jul", 710.0, tk.color_success),
                    BarData::new("Aug", 680.0, tk.color_primary),
                ]),
        )
        .section("饼图 — 浏览器市场份额")
        .push(uix::tree! { Container::new().size(INNER_W, 220.0).dir(FlexDirection::Row) => [
            PieChart::new().size(180.0).data(vec![
                PieData::new("Chrome", 65.0, tk.color_primary),
                PieData::new("Firefox", 15.0, tk.color_success),
                PieData::new("Safari", 10.0, tk.color_warning),
                PieData::new("Edge", 8.0, tk.color_error),
                PieData::new("Other", 2.0, tk.color_fill_tertiary),
            ]).into_node(),
            uix::tree! { Container::new().size(20.0, 0.0) },
            PieChart::new().size(180.0).donut(0.45).data(vec![
                PieData::new("Chrome", 65.0, tk.color_primary),
                PieData::new("Firefox", 15.0, tk.color_success),
                PieData::new("Safari", 10.0, tk.color_warning),
                PieData::new("Edge", 8.0, tk.color_error),
                PieData::new("Other", 2.0, tk.color_fill_tertiary),
            ]).into_node(),
        ]})
        .section("折线图 — CPU 温度")
        .push(
            LineChart::new()
                .width(INNER_W).height(160.0).line_color(tk.color_error)
                .show_dots(true).show_grid(true).line_width(2.0)
                .data(vec![
                    LineData::new("00:00", 42.0), LineData::new("01:00", 44.0),
                    LineData::new("02:00", 41.0), LineData::new("03:00", 48.0),
                    LineData::new("04:00", 46.0), LineData::new("05:00", 43.0),
                    LineData::new("06:00", 52.0), LineData::new("07:00", 58.0),
                    LineData::new("08:00", 63.0), LineData::new("09:00", 67.0),
                ]),
        )
        .section("标签 — Tag")
        .push(
            row(28.0)
                .child(Tag::new("Default"))
                .child(Tag::new("成功").color(TagColor::Success))
                .child(Tag::new("警告").color(TagColor::Warning))
                .child(Tag::new("错误").color(TagColor::Error))
                .child(Tag::new("进行中").color(TagColor::Info)),
        )
        .build()
}

// ── Page 5: Feedback ──

pub fn page_feedback(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("Alert — 提示条")
        .push(Alert::new("成功: 操作已完成").type_(AlertType::Success))
        .push(Alert::new("信息: 这是一个提示").type_(AlertType::Info))
        .push(Alert::new("警告: 请注意").type_(AlertType::Warning))
        .push(Alert::new("错误: 操作失败").type_(AlertType::Error))
        .section("Spin — 加载中")
        .push(row(36.0).child(Spin::new().small())
            .child(Spin::new())
            .child(Spin::new().large()))
        .section("ProgressBar — 进度条")
        .push(row(20.0).child(ProgressBar::new().progress(45.0)))
        .push(row(20.0).child(ProgressBar::new().progress(78.0)
            .stroke_color(tk.color_success)))
        .section("Skeleton — 骨架屏")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(80.0)
                .direction(FlexDirection::Column)
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(INNER_W, 16.0))
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(INNER_W * 0.7, 16.0))
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(INNER_W * 0.9, 16.0)),
        )
        .section("空状态 (Empty)")
        .push(
            Empty::new().description("暂无数据").description("请稍后再试或添加新内容"),
        )
        .build()
}
