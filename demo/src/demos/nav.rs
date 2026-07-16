//! 组件库页面 — page_nav（导航 + Overlay Dropdown）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_nav(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    let nav_items = Navigation::new("Demo Nav")
        .item_with_icon("首页", "home", "home")
        .item_with_icon("组件", "grid", "grid")
        .item_with_icon("设置", "settings", "settings")
        .active_index(1)
        .width(180.0)
        .height(160.0)
        .show_version(false)
        .build(tk);

    let group_items = NavGroup::new()
        .item("概览", "layout-dashboard")
        .item("详情", "file-text")
        .item("统计", "bar-chart")
        .active_index(0)
        .build();

    PageBuilder::new(tk)
        .gap()
        .block("Navigation — 侧栏构建器", embed(nav_items))
        .block("NavGroup + NavItem", {
            let mut sp = Space::new()
                .size(SpaceSize::Small)
                .width(180.0)
                .height(120.0)
                .direction(FlexDirection::Column);
            for item in group_items {
                sp = sp.child(item);
            }
            embed(sp)
        })
        .block(
            "Menu — Horizontal / Vertical",
            column_fit([
                embed(
                    Menu::new()
                        .add_item(MenuItem {
                            key: "home".into(),
                            label: "首页".into(),
                            icon: "home".into(),
                            disabled: false,
                        })
                        .add_item(MenuItem {
                            key: "docs".into(),
                            label: "文档".into(),
                            icon: "file-text".into(),
                            disabled: false,
                        })
                        .add_item(MenuItem {
                            key: "about".into(),
                            label: "关于".into(),
                            icon: "info".into(),
                            disabled: false,
                        })
                        .mode(MenuMode::Horizontal)
                        .active_key("home"),
                ),
                embed(
                    Menu::new()
                        .add_item(MenuItem {
                            key: "a".into(),
                            label: "菜单 A".into(),
                            icon: "".into(),
                            disabled: false,
                        })
                        .add_item(MenuItem {
                            key: "b".into(),
                            label: "菜单 B".into(),
                            icon: "".into(),
                            disabled: false,
                        })
                        .mode(MenuMode::Vertical)
                        .active_key("a"),
                ),
            ])
            .gap(16.0),
        )
        .block(
            "Tabs",
            embed(
                tree! { Tabs::new().tab("用户", "u").tab("设置", "s").tab("分析", "a")
                    .active(0).position(TabPosition::Top).size(INNER_W - 48.0, 140.0) => [
                    tree! { Container::new().size(INNER_W - 48.0, 100.0) => [
                        Label::new("用户面板").style(Style {
                            color: ColorValue::Neutral(NeutralRole::Text),
                            ..Style::default()
                        }).font_size(14.0),
                    ]},
                    tree! { Container::new().size(INNER_W - 48.0, 100.0) => [
                        Label::new("设置面板").style(Style {
                            color: ColorValue::Neutral(NeutralRole::Text),
                            ..Style::default()
                        }).font_size(14.0),
                    ]},
                    tree! { Container::new().size(INNER_W - 48.0, 100.0) => [
                        Label::new("分析面板").style(Style {
                            color: ColorValue::Neutral(NeutralRole::Text),
                            ..Style::default()
                        }).font_size(14.0),
                    ]},
                ]},
            ),
        )
        .block(
            "Dropdown / Breadcrumb / Anchor",
            column_fit([
                embed(labeled_row(
                    tk,
                    36.0,
                    "Dropdown",
                    Dropdown::new("Actions").items(vec!["编辑", "复制", "删除", "导出"]),
                )),
                embed(
                    demo_row(28.0).child(
                        Breadcrumb::new()
                            .item(BreadcrumbItem::new("首页"))
                            .item(BreadcrumbItem::new("组件"))
                            .item(BreadcrumbItem::new("导航").active()),
                    ),
                ),
                embed(Anchor::new(vec![
                    AnchorItem::new("基础", "#basic"),
                    AnchorItem::new("高级", "#advanced"),
                    AnchorItem::new("API", "#api"),
                ])),
            ])
            .gap(12.0),
        )
        .block(
            "Steps / Pagination",
            column_fit([
                embed(
                    Steps::new(vec![
                        Step::new("注册").status(StepStatus::Finish),
                        Step::new("验证").status(StepStatus::Process),
                        Step::new("完成").status(StepStatus::Wait),
                    ])
                    .current(1),
                ),
                embed(Pagination::new(85, 10)),
            ])
            .gap(16.0),
        )
        .build()
}
