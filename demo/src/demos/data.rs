//! 组件库页面 — page_data。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};

pub fn page_data(tk: &DesignTokens) -> ViewNode {
    PageBuilder::new(tk)
        .gap()
        .section("列表 List")
        .push(
            tree! { Container::new().size(INNER_W, 200.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                List::new()
                    .header("用户列表")
                    .items(vec!["Alice - 设计师", "Bob - 开发者", "Carol - 管理者"])
                    .footer("共 3 人")
                    .into_node(),
            ]},
        )
        .section("树形控件 Tree")
        .push(
            tree! { Container::new().size(INNER_W, 110.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                Tree::new(vec![
                    TreeNode::new("根节点", "1")
                        .add(TreeNode::new("子节点 A", "1-1"))
                        .add(TreeNode::new("子节点 B", "1-2")
                            .add(TreeNode::new("叶子节点", "1-2-1"))),
                ]).into_node(),
            ]},
        )
        .section("描述列表 Descriptions")
        .push(
            tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                Descriptions::new().title("用户信息")
                    .add(DescriptionsItem::new("姓名", "张三"))
                    .add(DescriptionsItem::new("邮箱", "zhang@ex.com"))
                    .add(DescriptionsItem::new("角色", "管理员"))
                    .column(3).into_node(),
            ]},
        )
        .section("时间线 Timeline")
        .push(
            tree! { Container::new().size(INNER_W, 130.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                Timeline::new()
                    .add(TimelineItem::new("创建项目").description("2024-01-15"))
                    .add(TimelineItem::new("完成设计").description("2024-02-20"))
                    .add(TimelineItem::new("部署上线").description("2024-03-10"))
                    .into_node(),
            ]},
        )
        .section("日历 Calendar")
        .push(
            tree! { Container::new().size(INNER_W, 240.0).dir(FlexDirection::Column)
                .pad(EdgeInsets::uniform(4.0)) => [
                Calendar::new().cell_size(30.0).into_node(),
            ]},
        )
        .section("轮播 Carousel")
        .push(Carousel::new().show_dots(true).show_arrows(true))
        .section("徽标 Badge")
        .push(
            demo_row(40.0)
                .child(Badge::new().count(1).color(tk.color_error))
                .child(Badge::new().count(99).max(99).color(tk.color_primary))
                .child(Badge::new().count(5).color(tk.color_success)),
        )
        .section("头像 Avatar")
        .push(
            demo_row(40.0)
                .child(Avatar::new("U"))
                .child(Avatar::new("A").bg(tk.color_primary))
                .child(Avatar::new("B").bg(tk.color_success)),
        )
        .section("图片 Image")
        .push(
            demo_row(80.0)
                .child(
                    Image::new(80.0, 60.0)
                        .src("assets/images/demo.png")
                        .alt("示例图片"),
                )
                .child(Image::new(80.0, 60.0).alt("占位图")),
        )
        .section("二维码 QRCode")
        .push(demo_row(80.0).child(QRCode::new("https://uix.dev")))
        .section("水印 Watermark")
        .push(demo_row(40.0).child(Watermark::new("UIX")))
        .section("结果页 Result")
        .push(
            tree! { Container::new().size(INNER_W, 120.0).dir(FlexDirection::Row).gap(16.0)
                .pad(EdgeInsets::uniform(4.0)) => [
                Result::new(ResultType::Success).title("提交成功").into_node(),
                Result::new(ResultType::Error).title("提交失败").into_node(),
            ]},
        )
        .build()

}
