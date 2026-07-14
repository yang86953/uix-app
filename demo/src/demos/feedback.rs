//! 组件库页面 — page_feedback（反馈 + Overlay 浮层全覆盖）。

use uix::prelude::*;

use crate::common::page::{PageBuilder, INNER_W};
use crate::common::showcase::{labeled_row, widget_caption};
use crate::demos::context::DemoCtx;

fn focus_trap_modal(tk: &DesignTokens) -> ViewNode {
    row((
        widget_caption(tk, "Modal ✓"),
        Modal::new("系统键盘焦点陷阱").closable(true).overlay(true),
    ))
    .align(AlignItems::Center)
    .height(36.0)
    .automation_id("feedback-focus-modal")
}

pub fn page_feedback(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .section("Alert — 4 种类型")
        .push(Alert::new("成功: 操作已完成").type_(StatusLevel::Success))
        .push(Alert::new("信息: 提示").type_(StatusLevel::Info))
        .push(Alert::new("警告: 请注意").type_(StatusLevel::Warning))
        .push(Alert::new("错误: 失败").type_(StatusLevel::Error))
        .section("Overlay — Modal / Drawer")
        .push_view(focus_trap_modal(tk))
        .push(labeled_row(
            tk,
            36.0,
            "Drawer ✓",
            Drawer::new("抽屉标题")
                .closable(true)
                .placement(DrawerPlacement::Right),
        ))
        .section("Message / Notification")
        .push(labeled_row(
            tk,
            36.0,
            "Message",
            Message::new().placement(MessagePlacement::Top),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "Notification",
            Notification::new().placement(NotifPlacement::TopRight),
        ))
        .section("Spin — 尺寸")
        .push(
            row((Spin::new().small(), Spin::new(), Spin::new().large()))
                .height(40.0)
                .align(AlignItems::Center)
                .gap(16.0),
        )
        .section("ProgressBar — 线形 / 圆形 / 不确定")
        .push(ProgressBar::new().progress(45.0))
        .push(
            ProgressBar::new()
                .progress(78.0)
                .stroke_color(tk.color_success),
        )
        .push(ProgressBar::new().circle().progress(0.65))
        .push(ProgressBar::new().indeterminate())
        .section("Skeleton / Empty")
        .push(
            column((
                Skeleton::new()
                    .shape(SkeletonShape::Rect)
                    .size(INNER_W, 14.0),
                Skeleton::new()
                    .shape(SkeletonShape::Rect)
                    .size(INNER_W * 0.7, 14.0),
            ))
            .width(INNER_W)
            .height(56.0)
            .gap(8.0),
        )
        .push(Empty::new().description("暂无反馈数据"))
        .section("Overlay — Tooltip / Popover / Popconfirm")
        .push(labeled_row(
            tk,
            36.0,
            "Tooltip ✓",
            Tooltip::new("悬停提示").placement(TooltipPlacement::Top),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "Popover ✓",
            Popover::new("气泡内容").title("标题"),
        ))
        .push(labeled_row(
            tk,
            36.0,
            "Popconfirm ✓",
            Popconfirm::new().title("确定删除？"),
        ))
        .build()
}
