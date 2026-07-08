//! 组件库页面 — page_feedback。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};

pub fn page_feedback(tk: &DesignTokens) -> ViewNode {
    PageBuilder::new(tk)
        .gap()
        .section("提示条 Alert — 4 种类型")
        .push(Alert::new("成功: 操作已完成").type_(StatusLevel::Success))
        .push(Alert::new("信息: 这是一个提示").type_(StatusLevel::Info))
        .push(Alert::new("警告: 请注意").type_(StatusLevel::Warning))
        .push(Alert::new("错误: 操作失败").type_(StatusLevel::Error))
        .section("加载中 Spin — 3 种尺寸")
        .push(
            demo_row(40.0)
                .child(Spin::new().small())
                .child(Spin::new())
                .child(Spin::new().large()),
        )
        .section("进度条 Progress")
        .push(demo_row(20.0).child(ProgressBar::new().progress(45.0)))
        .push(
            demo_row(20.0).child(
                ProgressBar::new()
                    .progress(78.0)
                    .stroke_color(tk.color_success),
            ),
        )
        .section("骨架屏 Skeleton")
        .push(
            Space::new()
                .size(SpaceSize::Small)
                .width(INNER_W)
                .height(80.0)
                .direction(FlexDirection::Column)
                .child(
                    Skeleton::new()
                        .shape(SkeletonShape::Rect)
                        .size(INNER_W, 16.0),
                )
                .child(
                    Skeleton::new()
                        .shape(SkeletonShape::Rect)
                        .size(INNER_W * 0.7, 16.0),
                )
                .child(
                    Skeleton::new()
                        .shape(SkeletonShape::Rect)
                        .size(INNER_W * 0.9, 16.0),
                ),
        )
        .section("空状态 Empty")
        .push(Empty::new().description("暂无数据"))
        .section("模态框 Modal")
        .push(demo_row(36.0).child(Modal::new("弹窗标题").closable(true)))
        .section("抽屉 Drawer")
        .push(demo_row(36.0).child(Drawer::new("抽屉标题").closable(true)))
        .section("气泡确认 Popconfirm")
        .push(demo_row(36.0).child(Popconfirm::new().title("确定删除此项？")))
        .section("文字提示 Tooltip")
        .push(demo_row(36.0).child(Tooltip::new("鼠标悬停查看提示").placement(TooltipPlacement::Top)))
        .section("气泡卡片 Popover")
        .push(demo_row(36.0).child(Popover::new("这是气泡内容.").title("气泡标题")))
        .build()

}
