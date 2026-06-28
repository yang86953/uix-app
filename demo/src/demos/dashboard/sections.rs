//! 组件展示页面（通用 ~ 反馈）
//! 分类：通用、布局、导航、输入、数据展示、反馈

use uix::platform::EdgeInsets;
use uix::graphics::Color;
use uix::ui::layout::{AlignItems, FlexDirection, JustifyContent};
use uix::ui::theme::DesignTokens;
use uix::ui::widget::WidgetNode;
use uix::ui::{
    Alert, AlertType, Anchor, AnchorItem, AutoComplete, BarChart, BarData,
    Breadcrumb, BreadcrumbItem, Button, ButtonSize, Calendar, Card,
    Cascader, CascaderOption, Checkbox, Collapse, CollapsePanel,
    Container, DatePicker, DateValue, Descriptions, DescriptionsItem,
    Divider, Dropdown, Empty,
    Input, InputNumber, InputSize, IntoWidgetNode, Label, LineChart, LineData,
    List, Menu, MenuItem, MenuMode, Mentions, Pagination, PieChart, PieData,
    ProgressBar, Radio, Rate, Result, ResultType,
    ScrollDirection, ScrollView, Segmented, Select, Skeleton, SkeletonShape,
    Slider, Space, SpaceSize, Spin, Splitter, Step, Steps, StepStatus,
    Switch, TabPosition, Tabs, Tag, TagColor, Timeline, TimelineItem,
    TimePicker, TimeValue, Tree, TreeNode, GridTrack,
    Typography, Modal, Drawer, Popover, Popconfirm, Tooltip, TooltipPlacement,
    Badge, Avatar, Image, QRCode, Watermark, Carousel,
};
use uix::tree;
use uix::ui::Icon;

use super::{INNER_W, PageBuilder, row, section_title};

// ═══════════════════════════════════════════════════════════════════════════
// Page 0: 通用 (General) — Button, Typography, Tag, Icon
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_general(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        // ── Typography ──
        .section("排版 — 标题层级")
        .push(tree! { Container::new().size(INNER_W, 160.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Typography::heading("Heading 1 — 一级标题", 1).into_node(),
            Typography::heading("Heading 2 — 二级标题", 2).into_node(),
            Typography::heading("Heading 3 — 三级标题", 3).into_node(),
            Typography::paragraph("正文段落：UIX 是一个 Rust 原生 UI 框架，支持响应式布局、主题系统和丰富组件。").into_node(),
        ]})
        .section("文字颜色")
        .push(
            row(28.0)
                .child(Label::new("Primary").color(tk.color_primary).font_size(14.0))
                .child(Label::new("Secondary").color(tk.color_text_secondary).font_size(14.0))
                .child(Label::new("Tertiary").color(tk.color_text_tertiary).font_size(14.0))
                .child(Label::new("Quaternary").color(tk.color_text_quaternary).font_size(14.0)),
        )
        // ── Button ──
        .section("按钮 — 5 种变体")
        .push(
            row(40.0)
                .child(Button::new("主要").primary())
                .child(Button::new("默认"))
                .child(Button::new("虚线").dashed())
                .child(Button::new("文字").text())
                .child(Button::new("链接").link()),
        )
        .section("按钮 — 3 种尺寸")
        .push(
            row(32.0)
                .child(Button::new("Small").size(ButtonSize::Small))
                .child(Button::new("Middle").size(ButtonSize::Medium))
                .child(Button::new("Large").size(ButtonSize::Large)),
        )
        .section("按钮组")
        .push(
            row(40.0)
                .child(Button::new("保存").primary())
                .child(Button::new("取消"))
                .child(Button::new("删除").primary()),
        )
        // ── Tag ──
        .section("标签 Tag")
        .push(
            row(30.0)
                .child(Tag::new("Default"))
                .child(Tag::new("成功").color(TagColor::Success))
                .child(Tag::new("警告").color(TagColor::Warning))
                .child(Tag::new("错误").color(TagColor::Error))
                .child(Tag::new("进行中").color(TagColor::Info)),
        )
        // ── Icon ──
        .section("图标 — Lucide 16px")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(100.0)
                .direction(FlexDirection::Row).wrap(true).align(AlignItems::Start)
                .child(Icon::new("search").size(16.0))
                .child(Label::new(" search").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("home").size(16.0))
                .child(Label::new(" home").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("settings").size(16.0))
                .child(Label::new(" settings").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("user").size(16.0))
                .child(Label::new(" user").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("bell").size(16.0))
                .child(Label::new(" bell").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("star").size(16.0))
                .child(Label::new(" star").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("heart").size(16.0))
                .child(Label::new(" heart").color(tk.color_text_secondary).font_size(11.0))
                .child(Icon::new("edit").size(16.0))
                .child(Label::new(" edit").color(tk.color_text_secondary).font_size(11.0)),
        )
        .build()
}

// ═══════════════════════════════════════════════════════════════════════════
// Page 1: 布局 (Layout) — Container, Space, Divider, Grid, Splitter, Collapse
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_layout(tk: &DesignTokens) -> WidgetNode {
    let c4 = (INNER_W - 24.0) / 4.0;

    // ── 调试辅助：简单 Flex Row ──
    let r = |label: &str, color: Color| -> WidgetNode {
        tree! { Container::new().size(80.0, 28.0).bg(color).rounded(4.0) => [
            Label::new(label).color(Color::white()).font_size(11.0),
        ]}
    };

    PageBuilder::new(tk)
        .gap()
        .section("Flex Row — 水平排列（SpaceBetween）")
        .push(tree! { Container::new().w(INNER_W).h(70.0).flex_grow(1.0).dir(FlexDirection::Row)
            .gap(12.0).rounded(4.0).align(AlignItems::Center)
            .justify(JustifyContent::SpaceBetween)
            .bg(tk.color_fill)
            .border(tk.color_border, 1.5) => [
            r("A", Color::from_rgb(64,150,255)),
            r("B", Color::from_rgb(82,196,26)),
            r("C", Color::from_rgb(250,173,20)),
            r("D", Color::from_rgb(114,46,209)),
            r("E", Color::from_rgb(245,34,45)),
            r("F", Color::from_rgb(19,194,194)),
            r("G", Color::from_rgb(250,140,22)),
        ]})
        .section("Flex Column — 垂直排列")
        .push(tree! { Container::new().w(INNER_W).h(230.0).flex_grow(1.0).dir(FlexDirection::Column)
            .gap(8.0).rounded(4.0).align(AlignItems::Center)
            .bg(tk.color_fill)
            .border(tk.color_border, 1.5) => [
            r("壹", Color::from_rgb(64,150,255)),
            r("贰", Color::from_rgb(82,196,26)),
            r("叁", Color::from_rgb(250,173,20)),
            r("肆", Color::from_rgb(114,46,209)),
            r("伍", Color::from_rgb(245,34,45)),
            r("陆", Color::from_rgb(19,194,194)),
            r("柒", Color::from_rgb(250,140,22)),
        ]})
        .section("Padding 对比 — 各 pad 值下 child x 坐标")
        .push(tree! { Container::new().size(INNER_W, 70.0).dir(FlexDirection::Row)
            .gap(12.0).rounded(4.0) => [
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0) => [
                r("pad=0", Color::from_rgb(64,150,255).with_alpha(150)),
            ]},
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0)
                .pad(EdgeInsets::uniform(8.0)) => [
                r("pad=8", Color::from_rgb(255,77,79).with_alpha(150)),
            ]},
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0)
                .pad(EdgeInsets::new(16.0, 0.0, 0.0, 0.0)) => [
                r("padL=16", Color::from_rgb(250,173,20).with_alpha(150)),
            ]},
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0)
                .pad(EdgeInsets::new(0.0, 0.0, 8.0, 8.0)) => [
                r("padBR=8", Color::from_rgb(82,196,26).with_alpha(150)),
            ]},
        ]})
        .section("网格 Grid — 2 列")
        .push(tree! { uix::ui::Grid::two_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Column 1").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Column 2").color(tk.color_success).font_size(13.0)]},
        ]})
        .section("网格 Grid — 3 列")
        .push(tree! { uix::ui::Grid::three_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell A").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell B").color(tk.color_warning).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell C").color(tk.color_success).font_size(13.0)]},
        ]})
        .section("网格 Grid — 自定义 (1fr 2fr)")
        .push(tree! { uix::ui::Grid::new()
            .columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)])
            .gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("1fr").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) =>
                [Label::new("2fr").color(tk.color_info).font_size(13.0)]},
        ]})
        .section("间距 Space")
        .push({
            let mut sp = Space::new().size(SpaceSize::Small).width(INNER_W).height(0.0)
                .direction(FlexDirection::Column);
            for (sz, label, h) in [
                (SpaceSize::Small, "Small (8px)", 32.0),
                (SpaceSize::Middle, "Middle (16px)", 56.0),
            ] {
                sp = sp.child(
                    Space::new().size(sz).width(INNER_W).height(h)
                        .direction(FlexDirection::Row).align(AlignItems::Center)
                        .child(Label::new(label).color(tk.color_text_tertiary).font_size(11.0))
                        .child(Button::new("甲").size(ButtonSize::Small))
                        .child(Button::new("乙").size(ButtonSize::Small)),
                );
            }
            sp
        })
        .section("分割线 Divider")
        .push(
            row(36.0)
                .child(Label::new("左侧").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("中间").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("右侧").color(tk.color_text).font_size(14.0)),
        )
        .section("折叠面板 Collapse")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W)
                .direction(FlexDirection::Column).align(AlignItems::Stretch)
                .child(Collapse::new().panels(vec![
                    CollapsePanel::new("面板 1：常规", "面板 1 的内容。").expanded(),
                    CollapsePanel::new("面板 2：设置", "配置选项与偏好设置。"),
                    CollapsePanel::new("面板 3：高级", "面向高级用户的设置。"),
                ])),
        )
        .section("分段器 Segmented")
        .push(
            row(36.0)
                .child(Segmented::new().options(vec!["每日", "每周", "每月", "每年"]).selected(2)),
        )
        .section("分割面板 Splitter")
        .push(
            Space::new().size(SpaceSize::Custom(200.0)).height(140.0)
                .child(Splitter::new().panels(3).vertical(false)),
        )
        .section("卡片 Card — 阴影层级 0~3")
        .push(
            Space::new().size(SpaceSize::Middle).width(INNER_W).height(130.0)
                .direction(FlexDirection::Row).align(AlignItems::Stretch)
                .child(Card::new().title("Elevation 0").elevation(0).bordered(true).size(c4, 120.0)
                    .child(Label::new("有边框，无阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 1").elevation(1).size(c4, 120.0)
                    .child(Label::new("柔和阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 2").elevation(2).size(c4, 120.0)
                    .child(Label::new("中等阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 3").elevation(3).size(c4, 120.0)
                    .child(Label::new("深阴影").color(tk.color_text_tertiary).font_size(12.0))),
        )
        .build()
}

// ═══════════════════════════════════════════════════════════════════════════
// Page 2: 导航 (Navigation) — Tabs, Menu, Breadcrumb, Dropdown, etc.
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_nav(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("标签页 Tabs — 顶部")
        .push(tree! { Tabs::new().tab("用户", "u").tab("设置", "s").tab("分析", "a")
            .active(0).position(TabPosition::Top).size(INNER_W, 180.0) => [
            tree! { Container::new().size(INNER_W, 140.0) => [
                Label::new("用户面板 — 管理团队成员").color(tk.color_text).font_size(14.0),
                Label::new("邀请、移除或更改角色。").color(tk.color_text_tertiary).font_size(12.0),
            ]},
            tree! { Container::new().size(INNER_W, 140.0) => [
                Label::new("设置面板 — 应用配置").color(tk.color_text).font_size(14.0),
                Label::new("主题、通知、隐私设置。").color(tk.color_text_tertiary).font_size(12.0),
            ]},
            tree! { Container::new().size(INNER_W, 140.0) => [
                Label::new("分析面板 — 使用指标").color(tk.color_text).font_size(14.0),
                Label::new("图表、报告、导出选项。").color(tk.color_text_tertiary).font_size(12.0),
            ]},
        ]})
        .section("横向菜单 Menu")
        .push(tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Menu::new()
                .add_item(MenuItem { key: "home".into(), label: "首页".into(), icon: "".into(), disabled: false })
                .add_item(MenuItem { key: "docs".into(), label: "文档".into(), icon: "".into(), disabled: false })
                .add_item(MenuItem { key: "about".into(), label: "关于".into(), icon: "".into(), disabled: false })
                .mode(MenuMode::Horizontal).active_key("home").into_node(),
        ]})
        .section("下拉菜单 Dropdown")
        .push(
            row(36.0)
                .child(Dropdown::new("Actions").items(vec!["编辑", "复制", "删除", "导出"])),
        )
        .section("面包屑 Breadcrumb")
        .push(
            row(28.0)
                .child(Breadcrumb::new()
                    .item(BreadcrumbItem::new("首页"))
                    .item(BreadcrumbItem::new("组件"))
                    .item(BreadcrumbItem::new("面包屑").active())),
        )
        .section("锚点 Anchor")
        .push(
            Anchor::new(vec![
                AnchorItem::new("基础用法", "#basic"),
                AnchorItem::new("高级配置", "#advanced"),
                AnchorItem::new("API 文档", "#api"),
            ]),
        )
        .section("步骤条 Steps")
        .push(tree! { Container::new().size(INNER_W, 80.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Steps::new(vec![
                Step::new("注册").status(StepStatus::Finish),
                Step::new("验证").status(StepStatus::Process),
                Step::new("完成").status(StepStatus::Wait),
            ]).current(1).into_node(),
        ]})
        .section("分页 Pagination")
        .push(tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Pagination::new(85, 10).into_node(),
        ]})
        .build()
}

// ═══════════════════════════════════════════════════════════════════════════
// Page 3: 输入 (Input) — 所有输入类组件
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_input(tk: &DesignTokens) -> WidgetNode {
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
        .section("输入框 Input — 3 种尺寸")
        .push(
            row(28.0)
                .child(Input::new("小型...").size(InputSize::Small))
                .child(Input::new("中型...").size(InputSize::Medium))
                .child(Input::new("大型...").size(InputSize::Large)),
        )
        .section("数字输入 InputNumber")
        .push(
            row(36.0)
                .child(InputNumber::new("数量").min(0.0).max(100.0).step(1.0))
                .child(InputNumber::new("价格").min(0.0).max(999.0).step(0.5)),
        )
        .section("选择 Select")
        .push(
            row(36.0)
                .child(Select::new().options(vec!["选项 1", "选项 2", "选项 3"]).selected(2)),
        )
        .section("级联 Cascader")
        .push(
            row(36.0)
                .child(Cascader::new(city_options, "选择地区")),
        )
        .section("提及 Mentions")
        .push(
            row(36.0)
                .child(Mentions::new("输入 @ 提及").options(vec!["Alice", "Bob", "Charlie"])),
        )
        .section("自动补全 AutoComplete")
        .push(tree! { Container::new().size(INNER_W, 40.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            AutoComplete::new().placeholder("输入城市名称...")
                .options(vec!["北京", "上海", "广州", "深圳"]).into_node(),
        ]})
        .section("开关 Switch")
        .push(
            row(30.0)
                .child(Switch::new().checked(true))
                .child(Switch::new().checked(false))
                .child(Switch::new().checked(true).disabled(true)),
        )
        .section("复选框 Checkbox")
        .push(
            row(30.0)
                .child(Checkbox::new("选项 A").checked(true))
                .child(Checkbox::new("选项 B"))
                .child(Checkbox::new("选项 C").disabled(true)),
        )
        .section("单选框 Radio")
        .push(
            row(32.0)
                .child(Radio::new().options(vec!["苹果", "香蕉", "樱桃"]).selected(1)),
        )
        .section("滑块 Slider")
        .push(
            row(30.0)
                .child(Slider::new().range(0.0, 100.0).step(5.0).value(42.0)),
        )
        .section("评分 Rate")
        .push(
            row(30.0)
                .child(Rate::new().value(3))
                .child(Rate::new().count(7).value(5))
                .child(Rate::new().value(2).allow_half()),
        )
        .section("日期选择 DatePicker")
        .push(
            row(36.0)
                .child(DatePicker::new("选择日期").value(DateValue::new(2026, 6, 20))),
        )
        .section("时间选择 TimePicker")
        .push(
            row(36.0)
                .child(TimePicker::new("选择时间").value(TimeValue::new(14, 30))),
        )
        .build()
}

// ═══════════════════════════════════════════════════════════════════════════
// Page 4: 数据展示 (Data Display) — Card, Table, List, Tree, Calendar, etc.
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_data(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("列表 List")
        .push(tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            List::new()
                .header("用户列表")
                .items(vec!["Alice - 设计师", "Bob - 开发者", "Carol - 管理者"])
                .footer("共 3 人")
                .into_node(),
        ]})
        .section("树形控件 Tree")
        .push(tree! { Container::new().size(INNER_W, 110.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Tree::new(vec![
                TreeNode::new("根节点", "1")
                    .add(TreeNode::new("子节点 A", "1-1"))
                    .add(TreeNode::new("子节点 B", "1-2")
                        .add(TreeNode::new("叶子节点", "1-2-1"))),
            ]).into_node(),
        ]})
        .section("描述列表 Descriptions")
        .push(tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Descriptions::new().title("用户信息")
                .add(DescriptionsItem::new("姓名", "张三"))
                .add(DescriptionsItem::new("邮箱", "zhang@ex.com"))
                .add(DescriptionsItem::new("角色", "管理员"))
                .column(3).into_node(),
        ]})
        .section("时间线 Timeline")
        .push(tree! { Container::new().size(INNER_W, 130.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Timeline::new()
                .add(TimelineItem::new("创建项目").description("2024-01-15"))
                .add(TimelineItem::new("完成设计").description("2024-02-20"))
                .add(TimelineItem::new("部署上线").description("2024-03-10"))
                .into_node(),
        ]})
        .section("日历 Calendar")
        .push(tree! { Container::new().size(INNER_W, 240.0).dir(FlexDirection::Column)
            .pad(EdgeInsets::uniform(4.0)) => [
            Calendar::new().cell_size(30.0).into_node(),
        ]})
        .section("轮播 Carousel")
        .push(
            Carousel::new().autoplay(3.0).show_dots(true).show_arrows(true),
        )
        .section("徽标 Badge")
        .push(
            row(40.0)
                .child(Badge::new().count(1).color(tk.color_error))
                .child(Badge::new().count(99).max(99).color(tk.color_primary))
                .child(Badge::new().count(5).color(tk.color_success)),
        )
        .section("头像 Avatar")
        .push(
            row(40.0)
                .child(Avatar::new("U"))
                .child(Avatar::new("A").bg(tk.color_primary))
                .child(Avatar::new("B").bg(tk.color_success)),
        )
        .section("图片 Image")
        .push(
            row(80.0)
                .child(Image::new("", 80.0, 60.0).alt("示例图片"))
                .child(Image::new("", 80.0, 60.0).alt("占位图")),
        )
        .section("二维码 QRCode")
        .push(
            row(80.0)
                .child(QRCode::new("https://uix.dev")),
        )
        .section("水印 Watermark")
        .push(
            row(40.0)
                .child(Watermark::new("UIX")),
        )
        .section("结果页 Result")
        .push(tree! { Container::new().size(INNER_W, 120.0).dir(FlexDirection::Row).gap(16.0)
            .pad(EdgeInsets::uniform(4.0)) => [
            Result::new(ResultType::Success).title("提交成功").into_node(),
            Result::new(ResultType::Error).title("提交失败").into_node(),
        ]})
        .build()
}

// ═══════════════════════════════════════════════════════════════════════════
// Page 5: 反馈 (Feedback) — Alert, Modal, Drawer, Progress, Spin, etc.
// ═══════════════════════════════════════════════════════════════════════════

pub fn page_feedback(tk: &DesignTokens) -> WidgetNode {
    PageBuilder::new(tk)
        .gap()
        .section("提示条 Alert — 4 种类型")
        .push(Alert::new("成功: 操作已完成").type_(AlertType::Success))
        .push(Alert::new("信息: 这是一个提示").type_(AlertType::Info))
        .push(Alert::new("警告: 请注意").type_(AlertType::Warning))
        .push(Alert::new("错误: 操作失败").type_(AlertType::Error))
        .section("加载中 Spin — 3 种尺寸")
        .push(
            row(40.0)
                .child(Spin::new().small())
                .child(Spin::new())
                .child(Spin::new().large()),
        )
        .section("进度条 Progress")
        .push(row(20.0).child(ProgressBar::new().progress(45.0)))
        .push(row(20.0).child(ProgressBar::new().progress(78.0)
            .stroke_color(tk.color_success)))
        .section("骨架屏 Skeleton")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W).height(80.0)
                .direction(FlexDirection::Column)
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(INNER_W, 16.0))
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(INNER_W * 0.7, 16.0))
                .child(Skeleton::new().shape(SkeletonShape::Rect).size(INNER_W * 0.9, 16.0)),
        )
        .section("空状态 Empty")
        .push(
            Empty::new().description("暂无数据"),
        )
        .section("模态框 Modal")
        .push(
            row(36.0)
                .child(Modal::new("弹窗标题").closable(true)),
        )
        .section("抽屉 Drawer")
        .push(
            row(36.0)
                .child(Drawer::new("抽屉标题").closable(true)),
        )
        .section("气泡确认 Popconfirm")
        .push(
            row(36.0)
                .child(Popconfirm::new().title("确定删除此项？")),
        )
        .section("文字提示 Tooltip")
        .push(
            row(36.0)
                .child(Tooltip::new("鼠标悬停查看提示").placement(TooltipPlacement::Top)),
        )
        .section("气泡卡片 Popover")
        .push(
            row(36.0)
                .child(Popover::new("这是气泡内容.").title("气泡标题")),
        )
        .build()
}
