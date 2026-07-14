//! 组件库页面 — page_data（数据展示全覆盖）。

use uix::prelude::*;

use crate::common::page::{PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_data(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let c4 = (INNER_W - 24.0) / 4.0;

    PageBuilder::new(tk)
        .gap()
        .section("Card — elevation 0~3")
        .push(
            row((
                Card::new()
                    .title("E0")
                    .elevation(0)
                    .bordered(true)
                    .size(c4, 120.0)
                    .child(label("有边框").font_size(12.0).fg(tk.color_text_tertiary)),
                Card::new()
                    .title("E1")
                    .elevation(1)
                    .size(c4, 120.0)
                    .child(label("柔和阴影").font_size(12.0).fg(tk.color_text_tertiary)),
                Card::new()
                    .title("E2")
                    .elevation(2)
                    .size(c4, 120.0)
                    .child(label("中等阴影").font_size(12.0).fg(tk.color_text_tertiary)),
                Card::new()
                    .title("E3")
                    .elevation(3)
                    .size(c4, 120.0)
                    .child(label("深阴影").font_size(12.0).fg(tk.color_text_tertiary)),
            ))
            .width(INNER_W)
            .height(130.0)
            .align(AlignItems::Stretch)
            .gap(8.0),
        )
        .section("List")
        .push(
            column((
                List::new()
                    .header("用户列表")
                    .items(vec!["Alice — 设计师", "Bob — 开发者", "Carol — 管理者"])
                    .footer("共 3 人"),
            ))
            .width(INNER_W)
            .height(180.0),
        )
        .section("Tree")
        .push(
            column((
                Tree::new({
                    let mut children = Vec::new();
                    for i in 0..40 {
                        children.push(TreeNode::new(&format!("节点 {i}"), &format!("n-{i}")));
                    }
                    vec![TreeNode::new("根", "root").children(children)]
                }),
            ))
            .width(INNER_W)
            .height(160.0),
        )
        .section("Collapse")
        .push(Collapse::new().panels(vec![
            CollapsePanel::new("面板 1", "常规内容。").expanded(),
            CollapsePanel::new("面板 2", "设置内容。"),
            CollapsePanel::new("面板 3", "高级内容。"),
        ]))
        .section("Descriptions")
        .push(
            column((
                Descriptions::new()
                    .title("用户信息")
                    .add(DescriptionsItem::new("姓名", "张三"))
                    .add(DescriptionsItem::new("邮箱", "zhang@ex.com"))
                    .add(DescriptionsItem::new("角色", "管理员"))
                    .column(3),
            ))
            .width(INNER_W)
            .height(100.0),
        )
        .section("Timeline / Calendar")
        .push(
            column((
                Timeline::new()
                    .add(TimelineItem::new("创建").description("2024-01-15"))
                    .add(TimelineItem::new("设计").description("2024-02-20"))
                    .add(TimelineItem::new("上线").description("2024-03-10")),
            ))
            .width(INNER_W)
            .height(120.0),
        )
        .push(
            column((
                Calendar::new().cell_size(28.0),
            ))
            .width(INNER_W)
            .height(220.0),
        )
        .section("Carousel")
        .push(labeled_row(
            tk,
            120.0,
            "Carousel",
            Carousel::new().show_dots(true).show_arrows(true),
        ))
        .section("Avatar / Badge / Tag")
        .push(
            row((
                Avatar::new("U"),
                Avatar::new("A").bg(tk.color_primary),
                Badge::new().count(5).color(tk.color_error),
                Tag::new("Tag").color(TagColor::Info),
            ))
            .height(40.0)
            .align(AlignItems::Center)
            .gap(12.0),
        )
        .section("Image / Empty / ResultView")
        .push(
            row((
                Image::new(80.0, 60.0)
                    .src("assets/images/demo.png")
                    .alt("demo"),
                Image::new(80.0, 60.0).alt("placeholder"),
            ))
            .height(80.0)
            .align(AlignItems::Center)
            .gap(12.0),
        )
        .push(Empty::new().description("暂无数据"))
        .push(
            row((
                ResultView::new(ResultType::Success).title("成功"),
                ResultView::new(ResultType::Error).title("失败"),
                ResultView::new(ResultType::Warning).title("警告"),
            ))
            .width(INNER_W)
            .height(120.0)
            .gap(16.0),
        )
        .section("Skeleton")
        .push(
            column((
                Skeleton::new()
                    .shape(SkeletonShape::Rect)
                    .size(INNER_W, 16.0),
                Skeleton::new()
                    .shape(SkeletonShape::Rect)
                    .size(INNER_W * 0.6, 16.0),
            ))
            .width(INNER_W)
            .height(60.0)
            .gap(8.0),
        )
        .section("Table")
        .push(
            column((
                Table::new()
                    .columns(vec![
                        TableColumn::new("姓名", 100.0).sortable(true),
                        TableColumn::new("角色", 120.0),
                        TableColumn::new("状态", 80.0).filterable(true),
                    ])
                    .rows((0..60).map(|i: usize| {
                        vec![
                            format!("User {i}"),
                            if i % 3 == 0 { "Designer".into() } else { "Engineer".into() },
                            "Active".into(),
                        ]
                    }).collect()),
            ))
            .width(INNER_W)
            .height(200.0),
        )
        .section("SelectableList")
        .push(labeled_row(tk, 140.0, "SelectableList", {
            let mut list = SelectableList::new();
            list.items = vec![
                SelectableItem::new("1", "第一项").icon("file"),
                SelectableItem::new("2", "第二项"),
                SelectableItem::new("3", "第三项").icon("star"),
            ];
            list.header_button_text = "全选".into();
            list.footer_text = "3 项".into();
            list
        }))
        .section("RichText")
        .push(labeled_row(
            tk,
            40.0,
            "RichText",
            RichText::new().content(vec![
                RichTextSegment::Text {
                    content: "UIX ".to_string(),
                    style: RichTextStyle::default(),
                },
                RichTextSegment::Link {
                    content: "文档".to_string(),
                    url: "https://uix.dev".to_string(),
                },
            ]),
        ))
        .build()
}
