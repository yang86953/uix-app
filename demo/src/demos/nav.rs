//! 组件库页面 — page_nav。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};

pub fn page_nav(tk: &DesignTokens) -> ViewNode {
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
            demo_row(36.0)
                .child(Dropdown::new("Actions").items(vec!["编辑", "复制", "删除", "导出"])),
        )
        .section("面包屑 Breadcrumb")
        .push(
            demo_row(28.0)
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
