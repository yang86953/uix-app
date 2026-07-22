use uix::prelude::*;

const TITLE_BAR_HEIGHT: f32 = 40.0;
const WINDOW_CONTROL_WIDTH: f32 = 46.0;

fn title_bar_control(
    control: WindowControl,
    accessible_name: &str,
    automation_id: &str,
    icon: &str,
) -> ViewNode {
    window_control_named(control, accessible_name, embed(Icon::new(icon).size(14.0)))
        .automation_id(automation_id)
        .width(WINDOW_CONTROL_WIDTH)
        .height(TITLE_BAR_HEIGHT)
        .bg_hover(ColorValue::Neutral(NeutralRole::FillSecondary))
        .bg_focus(ColorValue::Neutral(NeutralRole::Fill))
        .bg_active(ColorValue::Neutral(NeutralRole::FillTertiary))
}

pub(super) fn demo_title_bar() -> ViewNode {
    let drag_region = window_drag_region(
        row([
            embed(Icon::new("box").size(16.0)),
            label("UIX Demo")
                .font_size(13.0)
                .color(ColorValue::Neutral(NeutralRole::Text)),
        ])
        .align(AlignItems::Center)
        .gap(8.0)
        .padding_h(12.0),
    )
    .automation_id("window-titlebar-drag")
    .flex_grow(1.0)
    .height(TITLE_BAR_HEIGHT);

    let minimize = title_bar_control(
        WindowControl::Minimize,
        "最小化窗口",
        "window-control-minimize",
        "minus",
    );
    let maximize_restore = title_bar_control(
        WindowControl::MaximizeRestore,
        "最大化或还原窗口",
        "window-control-maximize-restore",
        "maximize-2",
    );
    let close = title_bar_control(
        WindowControl::Close,
        "关闭窗口",
        "window-control-close",
        "x",
    )
    .bg_hover(Color::hex("#E81123"))
    .bg_focus(Color::hex("#F1707A"))
    .bg_active(Color::hex("#C50F1F"));

    row([drag_region, minimize, maximize_restore, close])
        .automation_id("window-titlebar")
        .height(TITLE_BAR_HEIGHT)
        .align(AlignItems::Center)
        .bg(ColorValue::Neutral(NeutralRole::BgElevated))
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
}

pub(super) fn demo_window(content: ViewNode) -> ViewNode {
    column([demo_title_bar(), content]).flex_grow(1.0)
}
