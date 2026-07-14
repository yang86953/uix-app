//! 组件库页面 — page_nav（导航）。
//! 对照 [`使用.md`](../../docs/使用.md#导航) 导航组件。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_nav(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    // State-driven 导航状态
    let menu_selected = State::new(0usize);
    let dropdown_selected = State::new(0usize);
    let active_tab = State::new(0usize);
    let page = State::new(1u32);

    let nav_items = Navigation::new("Demo Nav")
        .item("首页", "home")
        .item("组件", "grid")
        .item("设置", "settings")
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
        // 1. Navigation — 侧栏构建器
        .block("Navigation — 侧栏构建器", nav_items)
        // 2. NavGroup + NavItem
        .block("NavGroup + NavItem", {
            let mut sp = Space::new()
                .size(SpaceSize::Small)
                .width(180.0)
                .height(120.0)
                .direction(FlexDirection::Column);
            for item in group_items {
                sp = sp.child(item);
            }
            sp
        })
        // 3. Menu — Inline / Horizontal / Vertical
        .block(
            "Menu — Inline / Horizontal / Vertical",
            column([
                // Inline 模式
                Menu::new()
                    .items(vec![
                        MenuItem::new("首页").icon("home"),
                        MenuItem::new("文档").icon("file-text"),
                        MenuItem::new("设置").icon("settings"),
                    ])
                    .selected_index(&menu_selected)
                    .mode(MenuMode::Inline),
                // Horizontal 模式
                Menu::new()
                    .items(vec![
                        MenuItem::new("首页").icon("home"),
                        MenuItem::new("文档").icon("file-text"),
                        MenuItem::new("关于").icon("info"),
                    ])
                    .selected_index(&menu_selected)
                    .mode(MenuMode::Horizontal),
                // Vertical 模式
                Menu::new()
                    .items(vec![MenuItem::new("菜单 A"), MenuItem::new("菜单 B")])
                    .selected_index(&menu_selected)
                    .mode(MenuMode::Vertical),
            ])
            .gap(16.0),
        )
        // 4. Tabs
        .block(
            "Tabs",
            Tabs::new()
                .tabs(vec![
                    Tab::new("用户", || {
                        column((label("用户面板").fg(tk.color_text).font_size(14.0),))
                            .size(INNER_W - 48.0, 100.0)
                    }),
                    Tab::new("设置", || {
                        column((label("设置面板").fg(tk.color_text).font_size(14.0),))
                            .size(INNER_W - 48.0, 100.0)
                    }),
                    Tab::new("分析", || {
                        column((label("分析面板").fg(tk.color_text).font_size(14.0),))
                            .size(INNER_W - 48.0, 100.0)
                    }),
                ])
                .active(&active_tab),
        )
        // 5-7. Dropdown / Breadcrumb / Anchor
        .block(
            "Dropdown / Breadcrumb / Anchor",
            column([
                // Dropdown
                Dropdown::new()
                    .items(vec![
                        DropdownItem::new("编辑"),
                        DropdownItem::new("复制"),
                        DropdownItem::new("删除").danger(true),
                        DropdownItem::divider(),
                        DropdownItem::new("导出"),
                    ])
                    .attach(button("操作")),
                // Breadcrumb
                Breadcrumb::new().items(vec!["首页", "组件", "导航"]),
                // Anchor
                Anchor::new().items(vec![
                    AnchorItem::new("section-1", "基本信息"),
                    AnchorItem::new("section-2", "高级设置"),
                    AnchorItem::new("section-3", "API"),
                ]),
            ])
            .gap(12.0),
        )
        // 8-9. Steps / Pagination
        .block(
            "Steps / Pagination",
            column([
                // Steps with different statuses
                Steps::new().current(1).items(vec![
                    Step::new("注册").status(StepStatus::Finish),
                    Step::new("验证").status(StepStatus::Process),
                    Step::new("完成").status(StepStatus::Wait),
                ]),
                // Pagination with State
                Pagination::new()
                    .total(200)
                    .page_size(20)
                    .current(&page)
                    .on_change(|p| page.set(p)),
            ])
            .gap(16.0),
        )
        .build()
}
