use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::graphics::{AlignItems, FlexDirection};
use uix::ui::{
    Alert, AlertType, Avatar, Badge, BarChart, BarData, Button, ButtonSize, Card,
    Checkbox, Container, Empty, Icon, Input, InputSize, IntoWidgetNode,
    Label, LineChart, LineData, PieChart, PieData, ProgressBar, Radio, Rate,
    Select, Skeleton, SkeletonShape, Slider, Spin, Switch, Table, TableColumn, Tag, TagColor,
    Tooltip, TooltipPlacement, Space, SpaceSize, Popover, Popconfirm,
};
use super::{INNER_W, PageBuilder, section_title, row, col, stat_card};

// ── Page 0: Dashboard ──

pub fn page_dashboard(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("概览 — 统计卡片")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(110.0)
                .direction(FlexDirection::Row).align(AlignItems::Stretch)
                .child(stat_card(tk, "Total Users", "12,834", tk.color_primary, 2))
                .child(stat_card(tk, "Revenue", "$8,291", tk.color_success, 1))
                .child(stat_card(tk, "Orders", "1,289", tk.color_warning, 1))
                .child(stat_card(tk, "Growth", "12.5%", tk.color_info, 1)),
        )
        .build()
}

// ── Page 1: Typography ──

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

// ── Page 2: Buttons ──

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

// ── Page 3: Inputs & Selection ──

pub fn page_inputs(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("输入框 — 3 种尺寸")
        .push(
            row(28.0)
                .child(Input::new("小型...").size(InputSize::Small))
                .child(Input::new("中型...").size(InputSize::Middle))
                .child(Input::new("大型...").size(InputSize::Large)),
        )
        .section("表单行 — 输入框 + 按钮")
        .push(
            row(40.0)
                .child(Input::new("输入邮箱...").size(InputSize::Middle))
                .child(Button::new("订阅").primary().size(ButtonSize::Middle)),
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
        .section("Radio — 单选组")
        .push(row(32.0).child(Radio::new().options(vec!["Apple", "Banana", "Cherry"]).selected(1)))
        .section("Select — 下拉选择")
        .push(row(36.0).child(Select::new().options(vec!["Option 1", "Option 2", "Option 3", "Option 4"]).selected(2)))
        .section("Slider — 滑块")
        .push(row(30.0).child(Slider::new().range(0.0, 100.0).step(5.0).value(42.0)))
        .section("Rate — 评分")
        .push(
            row(30.0)
                .child(Rate::new().value(3))
                .child(Rate::new().count(7).value(5))
                .child(Rate::new().value(2).allow_half()),
        )
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
                    LineData::new("04:00", 55.0), LineData::new("05:00", 53.0),
                    LineData::new("06:00", 51.0), LineData::new("07:00", 49.0),
                ]),
        )
        .section("ProgressBar")
        .push({
            let mut pg = col(0.0); // 高度由内容决定
            for (pct, lbl) in [(0.25, "25%"), (0.50, "50%"), (0.75, "75%"), (1.00, "100%")] {
                pg = pg
                    .child(Label::new(lbl).color(tk.color_text_tertiary).font_size(11.0))
                    .child(ProgressBar::new().progress(pct).track_color(tk.color_fill_tertiary));
            }
            pg.child(ProgressBar::new().indeterminate())
                .height(0.0) // auto-height
        })
        .section("Avatar")
        .push(
            row(44.0)
                .child(Avatar::new("A").bg(tk.color_primary_bg).text_color(tk.color_primary))
                .child(Avatar::new("B").bg(tk.color_success_bg).text_color(tk.color_success))
                .child(Avatar::new("C").bg(tk.color_warning_bg).text_color(tk.color_warning))
                .child(Avatar::new("D").bg(tk.color_error_bg).text_color(tk.color_error))
                .child(Avatar::new("U").bg(tk.color_primary_bg).text_color(tk.color_primary).size(48.0)),
        )
        .section("Icons — Lucide")
        .push(
            row(36.0)
                .child(Icon::new("search").size(18.0))
                .child(Icon::new("home").size(18.0))
                .child(Icon::new("settings").size(18.0))
                .child(Icon::new("user").size(18.0))
                .child(Icon::new("menu").size(18.0))
                .child(Icon::new("bell").size(18.0))
                .child(Icon::new("heart").size(18.0))
                .child(Icon::new("star").size(18.0))
                .child(Icon::new("github").size(18.0)),
        )
        .section("Tag — 彩色标签")
        .push(
            row(32.0)
                .child(Tag::new("Default").color(TagColor::Default))
                .child(Tag::new("Success").color(TagColor::Success))
                .child(Tag::new("Info").color(TagColor::Info))
                .child(Tag::new("Warning").color(TagColor::Warning))
                .child(Tag::new("Error").color(TagColor::Error))
                .child(Tag::new("Closable").color(TagColor::Info).closable()),
        )
        .section("Badge")
        .push(
            row(32.0)
                .child(Badge::new().count(5))
                .child(Badge::new().count(23))
                .child(Badge::new().count(100).max(99))
                .child(Badge::new().dot()),
        )
        .section("Skeleton — 骨架屏")
        .push(
            row(40.0)
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(200.0, 16.0))
                .child(Skeleton::new().shape(SkeletonShape::Circle).size(32.0, 32.0))
                .child(Skeleton::new().shape(SkeletonShape::Text).size(120.0, 24.0)),
        )
        .section("Table — 4 columns")
        .push(
            row(160.0)
                .child(Table::new()
                    .columns(vec![
                        TableColumn::new("Name", 100.0),
                        TableColumn::new("Age", 60.0),
                        TableColumn::new("City", 100.0),
                        TableColumn::new("Role", 80.0),
                    ])
                    .rows(vec![
                        vec!["Alice".into(), "28".into(), "Beijing".into(), "Dev".into()],
                        vec!["Bob".into(), "35".into(), "Shanghai".into(), "PM".into()],
                        vec!["Charlie".into(), "42".into(), "Shenzhen".into(), "QA".into()],
                        vec!["Diana".into(), "31".into(), "Guangzhou".into(), "Design".into()],
                    ])),
        )
        .build()
}

// ── Page 5: Feedback ──

pub fn page_feedback(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("Alert — 4 types")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(120.0)
                .direction(FlexDirection::Column)
                .child(Alert::new("Success: Operation completed").type_(AlertType::Success))
                .child(Alert::new("Info: This is an information message").type_(AlertType::Info))
                .child(Alert::new("Warning: Check your input").type_(AlertType::Warning))
                .child(Alert::new("Error: Something went wrong").type_(AlertType::Error)),
        )
        .section("Alert — with description")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(90.0)
                .direction(FlexDirection::Column)
                .child(Alert::new("Update available").description("Version 2.0.0 is ready to install").type_(AlertType::Info).closable())
                .child(Alert::new("Connection lost").description("Attempting to reconnect...").type_(AlertType::Warning)),
        )
        .section("Popover / Popconfirm")
        .push(
            row(36.0)
                .child(Popover::new("This is popover content.").title("Popover Title"))
                .child(Popconfirm::new().title("Delete this item?")),
        )
        .section("Tooltip — 4 placements")
        .push(
            row(40.0)
                .child(Tooltip::new("这是提示文字").placement(TooltipPlacement::Top))
                .child(Tooltip::new("底部提示").placement(TooltipPlacement::Bottom))
                .child(Tooltip::new("左侧提示").placement(TooltipPlacement::Left))
                .child(Tooltip::new("右侧提示").placement(TooltipPlacement::Right)),
        )
        .section("Spin — 加载动画")
        .push(
            row(40.0)
                .child(Spin::new().small())
                .child(Spin::new())
                .child(Spin::new().large())
                .child(Spin::new().color(tk.color_success))
                .child(Spin::new().color(tk.color_warning))
                .child(Spin::new().color(tk.color_error)),
        )
        .section("Empty — 空状态")
        .push(
            row(120.0)
                .child(Empty::new())
                .child(Empty::new().description("No search results").icon("search"))
                .child(Empty::new().description("No messages").icon("mail")),
        )
        .section("模态对话框")
        .push(uix::tree! { Container::new().size(INNER_W, 60.0).bg(tk.color_bg_elevated)
            .rounded(tk.border_radius_lg).dir(FlexDirection::Row).pad(uix::base::EdgeInsets::uniform(12.0)) => [
            Icon::new("layout").size(24.0),
            Container::new().size(12.0, 0.0),
            uix::tree! { Container::new().size(INNER_W - 80.0, 36.0).dir(FlexDirection::Column) => [
                Label::new("Modal::new(\"Title\").show()").color(tk.color_text).font_size(14.0),
                Label::new("Use .open() / .close() at runtime.").color(tk.color_text_tertiary).font_size(12.0),
            ]},
        ]})
        .build()
}
