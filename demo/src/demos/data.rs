//! 组件库页面 — page_data（数据展示全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

#[derive(Clone)]
struct User {
    name: String,
    age: u32,
    role: String,
}

pub fn page_data(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    let users = vec![
        User {
            name: "张三".into(),
            age: 28,
            role: "管理员".into(),
        },
        User {
            name: "李四".into(),
            age: 35,
            role: "开发者".into(),
        },
        User {
            name: "王五".into(),
            age: 22,
            role: "设计师".into(),
        },
        User {
            name: "赵六".into(),
            age: 30,
            role: "测试".into(),
        },
    ];

    let tree_nodes = {
        let mut children = Vec::new();
        for i in 0..40 {
            children.push(TreeNode::new(&format!("节点 {i}"), &format!("n-{i}")));
        }
        vec![TreeNode::new("根", "root").children(children)]
    };

    PageBuilder::new(tk)
        .gap()
        .section("Card — elevation 0~3")
        .push(
            row((
                Card::new()
                    .title("Elevation 0")
                    .elevation(0)
                    .child(label("有边框").fg(tk.color_text_tertiary).font_size(12.0)),
                Card::new()
                    .title("Elevation 1")
                    .elevation(1)
                    .child(label("柔和阴影").fg(tk.color_text_tertiary).font_size(12.0)),
                Card::new()
                    .title("Elevation 2")
                    .elevation(2)
                    .child(label("中等阴影").fg(tk.color_text_tertiary).font_size(12.0)),
                Card::new()
                    .title("Elevation 3")
                    .elevation(3)
                    .child(label("深阴影").fg(tk.color_text_tertiary).font_size(12.0)),
            ))
            .gap(8.0),
        )
        .section("Table")
        .push(
            Table::new()
                .columns(vec![
                    TableColumn::new("姓名", 120.0).render(|u: &User| u.name.clone()),
                    TableColumn::new("年龄", 80.0).render(|u: &User| u.age.to_string()),
                    TableColumn::new("角色", 100.0).render(|u: &User| u.role.clone()),
                ])
                .rows(users.clone())
                .sortable(true)
                .selection(true)
                .bordered(true),
        )
        .section("Table — 固定列")
        .push(
            Table::new()
                .columns(vec![
                    TableColumn::new("姓名", 120.0)
                        .fixed(Fixed::Left)
                        .render(|u: &User| u.name.clone()),
                    TableColumn::new("年龄", 80.0).render(|u: &User| u.age.to_string()),
                    TableColumn::new("邮箱", 200.0).render(|_: &User| "-".to_string()),
                    TableColumn::new("操作", 100.0)
                        .fixed(Fixed::Right)
                        .render(|_: &User| "编辑".to_string()),
                ])
                .rows(users),
        )
        .section("List")
        .push(
            List::new()
                .items(&vec!["Alice — 设计师", "Bob — 开发者", "Carol — 管理者"])
                .render(|item| label(item).font_size(14.0))
                .gap(4.0),
        )
        .section("Tree")
        .push(
            Tree::new()
                .data(&tree_nodes)
                .default_expand_all(true)
                .render(|node| label(node.label)),
        )
        .section("Collapse")
        .push(Collapse::new().panels(vec![
            CollapsePanel::new("面板 1", "常规内容。").expanded(),
            CollapsePanel::new("面板 2", "设置内容。"),
            CollapsePanel::new("面板 3", "高级内容。"),
        ]))
        .section("Descriptions")
        .push(
            Descriptions::new()
                .item("用户名", "张三")
                .item("邮箱", "zhang@ex.com")
                .item("角色", "管理员")
                .bordered(true),
        )
        .section("Timeline")
        .push(Timeline::new().items(vec![
            TimelineItem::new("2024-01-15", "创建"),
            TimelineItem::new("2024-02-20", "设计"),
            TimelineItem::new("2024-03-10", "上线"),
        ]))
        .section("Calendar")
        .push(Calendar::new().cell_size(28.0))
        .section("Carousel")
        .push(Carousel::new().show_dots(true).show_arrows(true))
        .section("Avatar / Badge / Tag")
        .push(
            row((
                Avatar::new("U"),
                Avatar::new("A").bg(tk.color_primary),
                Badge::new().count(5).color(tk.color_error),
                Tag::new("Tag").color(TagColor::Info),
            ))
            .gap(16.0),
        )
        .section("Image / Empty / ResultView")
        .push(
            row((
                Image::new(80.0, 60.0)
                    .src("assets/images/demo.png")
                    .alt("demo"),
                Image::new(80.0, 60.0).alt("placeholder"),
            ))
            .gap(16.0),
        )
        .push(Empty::new().description("暂无数据"))
        .push(
            row((
                ResultView::new(ResultType::Success).title("成功"),
                ResultView::new(ResultType::Error).title("失败"),
                ResultView::new(ResultType::Warning).title("警告"),
            ))
            .gap(16.0),
        )
        .section("Skeleton")
        .push(Skeleton::new().rows(3).animated(true))
        .section("SelectableList")
        .push({
            let mut list = SelectableList::new();
            list.items = vec![
                SelectableItem::new("1", "第一项").icon("file"),
                SelectableItem::new("2", "第二项"),
                SelectableItem::new("3", "第三项").icon("star"),
            ];
            list.header_button_text = "全选".into();
            list.footer_text = "3 项".into();
            list
        })
        .section("RichText")
        .push(RichText::new().content(vec![
            RichTextSegment::Text {
                content: "UIX ".to_string(),
                style: RichTextStyle::default(),
            },
            RichTextSegment::Link {
                content: "文档".to_string(),
                url: "https://uix.dev".to_string(),
            },
        ]))
        .build()
}
