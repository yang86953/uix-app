//! 组件库页面 — page_data（数据展示全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_data(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let c4 = (INNER_W - 24.0) / 4.0;

    PageBuilder::new(tk)
        .gap()
        .section("Card — elevation 0~3")
        .push(
            Space::new()
                .size(SpaceSize::Middle)
                .width(INNER_W)
                .height(130.0)
                .direction(FlexDirection::Row)
                .align(AlignItems::Stretch)
                .child(
                    Card::new()
                        .title("E0")
                        .elevation(0)
                        .bordered(true)
                        .size(c4, 120.0)
                        .child(
                            Label::new("有边框")
                                .color(tk.color_text_tertiary)
                                .font_size(12.0),
                        ),
                )
                .child(
                    Card::new().title("E1").elevation(1).size(c4, 120.0).child(
                        Label::new("柔和阴影")
                            .color(tk.color_text_tertiary)
                            .font_size(12.0),
                    ),
                )
                .child(
                    Card::new().title("E2").elevation(2).size(c4, 120.0).child(
                        Label::new("中等阴影")
                            .color(tk.color_text_tertiary)
                            .font_size(12.0),
                    ),
                )
                .child(
                    Card::new().title("E3").elevation(3).size(c4, 120.0).child(
                        Label::new("深阴影")
                            .color(tk.color_text_tertiary)
                            .font_size(12.0),
                    ),
                ),
        )
        .section("List")
        .push(tree! { Container::new().size(INNER_W, 180.0) => [
            List::new()
                .header("用户列表")
                .items(vec!["Alice — 设计师", "Bob — 开发者", "Carol — 管理者"])
                .footer("共 3 人")
                .into_node(),
        ]})
        .section("Tree")
        .push(tree! { Container::new().size(INNER_W, 160.0) => [
            Tree::new({
                let mut children = Vec::new();
                for i in 0..40 {
                    children.push(TreeNode::new(&format!("节点 {i}"), &format!("n-{i}")));
                }
                vec![TreeNode::new("根", "root").children(children)]
            }).into_node(),
        ]})
        .section("Collapse")
        .push(Collapse::new().panels(vec![
            CollapsePanel::new("面板 1", "常规内容。").expanded(),
            CollapsePanel::new("面板 2", "设置内容。"),
            CollapsePanel::new("面板 3", "高级内容。"),
        ]))
        .section("Descriptions")
        .push(tree! { Container::new().size(INNER_W, 100.0) => [
            Descriptions::new().title("用户信息")
                .add(DescriptionsItem::new("姓名", "张三"))
                .add(DescriptionsItem::new("邮箱", "zhang@ex.com"))
                .add(DescriptionsItem::new("角色", "管理员"))
                .column(3).into_node(),
        ]})
        .section("Timeline / Calendar")
        .push(tree! { Container::new().size(INNER_W, 120.0) => [
            Timeline::new()
                .add(TimelineItem::new("创建").description("2024-01-15"))
                .add(TimelineItem::new("设计").description("2024-02-20"))
                .add(TimelineItem::new("上线").description("2024-03-10"))
                .into_node(),
        ]})
        .push(tree! { Container::new().size(INNER_W, 220.0) => [
            Calendar::new().cell_size(28.0).into_node(),
        ]})
        .section("Carousel")
        .push(labeled_row(
            tk,
            120.0,
            "Carousel",
            Carousel::new().show_dots(true).show_arrows(true),
        ))
        .section("Avatar / Badge / Tag")
        .push(
            demo_row(40.0)
                .child(Avatar::new("U"))
                .child(Avatar::new("A").bg(tk.color_primary))
                .child(Badge::new().count(5).color(tk.color_error))
                .child(Tag::new("Tag").color(TagColor::Info)),
        )
        .section("Image / Empty / ResultView")
        .push(
            demo_row(80.0)
                .child(
                    Image::new(80.0, 60.0)
                        .src("assets/images/demo.png")
                        .alt("demo"),
                )
                .child(Image::new(80.0, 60.0).alt("placeholder")),
        )
        .push(Empty::new().description("暂无数据"))
        .push(
            tree! { Container::new().size(INNER_W, 120.0).dir(FlexDirection::Row).gap(16.0) => [
                ResultView::new(ResultType::Success).title("成功").into_node(),
                ResultView::new(ResultType::Error).title("失败").into_node(),
                ResultView::new(ResultType::Warning).title("警告").into_node(),
            ]},
        )
        .section("Skeleton")
        .push(
            Space::new()
                .size(SpaceSize::Small)
                .width(INNER_W)
                .height(60.0)
                .direction(FlexDirection::Column)
                .child(
                    Skeleton::new()
                        .shape(SkeletonShape::Rect)
                        .size(INNER_W, 16.0),
                )
                .child(
                    Skeleton::new()
                        .shape(SkeletonShape::Rect)
                        .size(INNER_W * 0.6, 16.0),
                ),
        )
        .section("Table")
        .push(tree! { Container::new().size(INNER_W, 200.0) => [
            Table::new()
                .columns(vec![
                    TableColumn::new("姓名", 100.0),
                    TableColumn::new("角色", 120.0),
                    TableColumn::new("状态", 80.0).filterable(true),
                ])
                .rows((0..60).map(|i: usize| {
                    vec![
                        format!("User {i}"),
                        if i % 3 == 0 { "Designer".into() } else { "Engineer".into() },
                        "Active".into(),
                    ]
                }).collect())
                .sortable(true)
                .selection(true)
                .bordered(true)
                .into_node(),
        ]})
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
