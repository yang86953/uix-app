//! 展示与反馈类测试场景：alert/drawer/message/modal/notification/popconfirm/popover/spin/tooltip。
//! 由 display_feedback.rs 以 `#[path]` 引入，build 在父模块按 id 分发。

use uix::prelude::*;

use super::super::{qa_row, qa_target, qa_target_view, qa_variant};
// 复用主文件的 frame 辅助与样例数据。
use super::{alert_frame, message_frame, notification_frame, spin_frame};

pub(super) fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    let view = match id {
        "alert" => column_fit([
            alert_frame(
                Alert::new("成功：组件视觉符合基线")
                    .description("完整释放关闭，说明也进入可访问值")
                    .type_(StatusLevel::Success)
                    .action("复核", || {})
                    .closable(),
                180.0,
                54.0,
                "component-qa-target",
            ),
            alert_frame(
                Alert::new("信息：无图标状态仍保持正文对比")
                    .type_(StatusLevel::Info)
                    .show_icon(false),
                240.0,
                36.0,
                "component-qa-alert-no-icon",
            ),
            embed(Alert::new("警告：需要人工复核").type_(StatusLevel::Warning)),
            embed(Alert::new("错误：发现阻断缺陷").type_(StatusLevel::Error)),
            alert_frame(
                Alert::new("受限：超长中英文 mixed alert message")
                    .description("说明不会覆盖关闭按钮或越过 frame")
                    .type_(StatusLevel::Warning)
                    .closable(),
                132.0,
                54.0,
                "component-qa-alert-constrained",
            ),
        ])
        .gap(8.0),
        "drawer" => qa_target_view(ViewNode::new(
            Drawer::new("抽屉标题：超长中英文 mixed title 会在关闭按钮前省略")
                .placement(DrawerPlacement::Right)
                .size(360.0, 640.0)
                .extra("辅助操作")
                .closable(true),
            vec![column_fit([qa_row([
                button("取消").automation_id("component-qa-drawer-cancel"),
                button("保存")
                    .primary()
                    .automation_id("component-qa-drawer-save"),
            ])])],
        )),
        "message" => {
            let message = Message::new().placement(Placement::Top);
            for (type_, content) in [
                (StatusLevel::Success, "视觉基线已通过"),
                (
                    StatusLevel::Info,
                    "正在记录超长中英文 mixed Message 测试状态，关闭槽前必须省略",
                ),
                (StatusLevel::Warning, "一项需要复核"),
                (StatusLevel::Error, "一项存在缺陷"),
            ] {
                message.add(MessageItem {
                    type_,
                    content: content.into(),
                    duration_ms: 0,
                    closable: true,
                });
            }
            message_frame(message, 520.0, 160.0, "component-qa-target")
        }
        "modal" => qa_target_view(ViewNode::new(
            Modal::new("Modal 超长中英文 mixed title 会在关闭按钮前安全省略")
                .size(360.0, 220.0)
                .closable(true)
                .footer_visible(false)
                .overlay(true),
            vec![qa_row([
                button("取消").automation_id("component-qa-modal-cancel"),
                button("确认")
                    .primary()
                    .automation_id("component-qa-modal-confirm"),
            ])],
        )),
        "notification" => {
            let notification = Notification::new().placement(Placement::TopRight);
            for (type_, title, description) in [
                (
                    StatusLevel::Success,
                    "测试通过",
                    "Button 已符合全部适用状态",
                ),
                (StatusLevel::Info, "测试状态", "Light / Dark / Compact"),
                (
                    StatusLevel::Warning,
                    "需要复核超长中英文 mixed Notification title",
                    "检查说明文字 description 在关闭槽前安全省略",
                ),
                (StatusLevel::Error, "测试失败", "存在视觉阻断问题"),
            ] {
                notification.add(NotificationItem {
                    type_,
                    title: title.into(),
                    description: description.into(),
                    duration_ms: 0,
                    closable: true,
                });
            }
            notification_frame(notification, 520.0, 180.0, "component-qa-target")
        }
        "popconfirm" => qa_target(
            Popconfirm::new()
                .title("确认删除这条超长中英文 mixed visual baseline result？")
                .confirm_text("确认并永久删除")
                .cancel_text("取消并保留全部内容"),
        ),
        "popover" => qa_target(
            Popover::new("逐组件测试与状态矩阵 mixed popover content must stay inside the surface")
                .title("质量详情与超长中英文 mixed title")
                .placement(PopoverPlacement::Top),
        ),
        "progress-bar" => column_fit([
            qa_variant(
                "45% square / target",
                qa_target(
                    ProgressBar::new()
                        .progress(0.45)
                        .round(false)
                        .size(520.0, 14.0),
                ),
            ),
            qa_row([
                qa_variant(
                    "Success",
                    embed(
                        ProgressBar::new()
                            .progress(1.0)
                            .stroke_color(tk.color_success)
                            .size(250.0, 10.0),
                    ),
                ),
                qa_variant(
                    "Error",
                    embed(
                        ProgressBar::new()
                            .progress(0.64)
                            .stroke_color(tk.color_error)
                            .size(250.0, 10.0),
                    ),
                ),
            ]),
            qa_row([
                qa_variant(
                    "Circle",
                    embed(ProgressBar::new().circle().progress(0.72).size(96.0, 96.0)),
                ),
                qa_variant(
                    "Indeterminate",
                    embed(ProgressBar::new().indeterminate().size(380.0, 12.0)),
                ),
            ]),
        ])
        .gap(10.0),
        "spin" => column_fit([
            qa_variant(
                "Wrapper / target",
                spin_frame(
                    Spin::new()
                        .large()
                        .tip("正在加载超长中英文 mixed loading status")
                        .wrapper_mode(),
                    320.0,
                    88.0,
                    "component-qa-target",
                ),
            ),
            qa_row([
                qa_variant("Small", embed(Spin::new().small())),
                qa_variant("Middle", embed(Spin::new())),
                qa_variant("Large", embed(Spin::new().large())),
                qa_variant("Stopped", embed(Spin::new().spinning(false))),
            ]),
        ])
        .gap(10.0),
        "tooltip" => column_fit([row([
            ViewNode::new(
                Tooltip::new("顶部提示")
                    .placement(TooltipPlacement::Top)
                    .trigger(TriggerMode::Focus),
                vec![button("Top").automation_id("component-qa-target")],
            )
            .width(100.0)
            .height(36.0),
            ViewNode::new(
                Tooltip::new("底部提示").placement(TooltipPlacement::Bottom),
                vec![button("Bottom").automation_id("component-qa-tooltip-bottom")],
            )
            .width(100.0)
            .height(36.0),
            ViewNode::new(
                Tooltip::new("左侧提示").placement(TooltipPlacement::Left),
                vec![button("Left").automation_id("component-qa-tooltip-left")],
            )
            .width(100.0)
            .height(36.0),
            ViewNode::new(
                Tooltip::new("右侧提示").placement(TooltipPlacement::Right),
                vec![button("Right").automation_id("component-qa-tooltip-right")],
            )
            .width(100.0)
            .height(36.0),
        ])
        .align(AlignItems::Center)
        .gap(16.0)
        .height(60.0)])
        .gap(18.0)
        .height(60.0),
        "focus-trap" => qa_target_view(
            column([ViewNode::new(
                FocusTrap::new(),
                vec![qa_row([
                    input()
                        .placeholder("第一个焦点")
                        .automation_id("component-qa-focus-first")
                        .build(),
                    button("第二个焦点").automation_id("component-qa-focus-second"),
                    button("第三个焦点")
                        .primary()
                        .automation_id("component-qa-focus-third"),
                ])],
            )
            .flex_grow(1.0)])
            .height(52.0),
        ),
        _ => return None,
    };
    Some(view)
}
