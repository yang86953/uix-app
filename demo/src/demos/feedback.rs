//! 组件库页面 — page_feedback（反馈 + 浮层全覆盖）。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_feedback(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    PageBuilder::new(tk)
        .gap()
        .section("Alert — 4 种类型")
        .push(
            Alert::new(AlertType::Success)
                .title("成功")
                .description("操作已完成")
                .closable(true),
        )
        .push(
            Alert::new(AlertType::Info)
                .title("信息")
                .description("这是一条提示信息")
                .closable(true),
        )
        .push(
            Alert::new(AlertType::Warning)
                .title("警告")
                .description("请注意检查")
                .closable(true),
        )
        .push(
            Alert::new(AlertType::Error)
                .title("错误")
                .description("操作失败，请重试")
                .closable(true),
        )
        .section("Modal")
        .push(
            button("打开 Modal").on_click_fn(move || {
                Modal::show(move |ctx| {
                    column((
                        label("确认删除？").font_size(18.0),
                        row((
                            button("取消").on_click_fn(|| ctx.close()),
                            button("确认")
                                .danger()
                                .on_click_fn(|| ctx.close()),
                        ))
                        .gap(8.0),
                    ))
                    .gap(16.0)
                    .padding(24.0)
                })
                .title("提示")
                .width(400.0);
            }),
        )
        .section("Drawer")
        .push(
            button("打开 Drawer").on_click_fn(move || {
                Drawer::show(move |ctx| {
                    column((
                        label("详情内容").font_size(16.0),
                        label("这里显示详细信息...").fg(tk.color_text_secondary),
                    ))
                    .gap(12.0)
                    .padding(24.0)
                })
                .title("详情")
                .placement(Placement::Right)
                .width(480.0);
            }),
        )
        .section("Message")
        .push(
            row((
                button("Success").on_click_fn(|| Message::success("保存成功")),
                button("Error").on_click_fn(|| Message::error("保存失败")),
                button("Warning").on_click_fn(|| Message::warning("请先填写必填项")),
                button("Info").on_click_fn(|| Message::info("正在加载...")),
            ))
            .gap(8.0),
        )
        .section("Notification")
        .push(
            button("显示通知").on_click_fn(move || {
                Notification::new()
                    .title("更新完成")
                    .description("已更新到 v2.0")
                    .duration(3.0)
                    .placement(Placement::TopRight)
                    .show();
            }),
        )
        .section("Spin — 尺寸")
        .push(
            row((
                Spin::new().size(16.0),
                Spin::new().size(32.0),
                Spin::new().size(48.0),
            ))
            .gap(24.0),
        )
        .section("ProgressBar")
        .push(ProgressBar::new().percent(45.0))
        .push(ProgressBar::new().percent(78.0))
        .push(ProgressBar::new().percent(65.0))
        .push(ProgressBar::new().indeterminate())
        .section("Skeleton")
        .push(Skeleton::new().rows(3).animated(true))
        .section("Empty")
        .push(Empty::new().description("暂无反馈数据"))
        .section("Tooltip")
        .push(
            Tooltip::new("这是提示文字").attach(button("悬停查看提示")),
        )
        .section("Popover")
        .push(
            Popover::new()
                .content(|| label("弹出内容"))
                .trigger(Trigger::Click)
                .attach(button("点击弹出")),
        )
        .section("Popconfirm")
        .push(
            Popconfirm::new("确定删除？")
                .on_confirm(|| { /* 执行删除 */ })
                .attach(button("删除").danger()),
        )
        .build()
}
