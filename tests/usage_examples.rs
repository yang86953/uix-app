//! F 批次「使用落地」：使用文档锚点示例形态的抽样编译验证。
//!
//! 覆盖高风险锚点：`layout-app-shell`、`navigation-typed-route`、
//! `settings-typed-struct`、`tutorial-virtualized-todo`、
//! `component-timer-badge`、`multi-window-config`、`async-typed-three-state`、
//! `chart-state-driven`（`AnimationConfig`/`Transition` 已由 usage_facade 覆盖）。

use uix::prelude::*;

fn sample_theme() -> Theme {
    Theme::custom(ThemePrimitives::antd_light())
}

// ---- layout-app-shell ----

#[test]
fn layout_app_shell_anchor_compiles() {
    fn cards() -> Vec<Card> {
        vec![Card::new().title("A"), Card::new().title("B")]
    }
    fn app_shell() -> ViewNode {
        row((
            embed(Sider::new(200.0).collapsible(true)),
            column((
                embed(Header::new(48.0).title("工作台")),
                embed(Content::new().child(
                    Grid::responsive()
                        .cols(vec![
                            Col::new().span(24).sm(12).md(8).lg(6),
                            Col::new().span(24).sm(12).md(8).lg(6),
                            Col::new().span(24).sm(12).md(8).lg(6),
                        ])
                        .children(cards()),
                )),
            ))
            .flex_grow(1.0),
        ))
    }
    let _node = app_shell();
}

// ---- navigation-typed-route ----

#[test]
fn navigation_typed_route_anchor_compiles() {
    #[derive(Clone, PartialEq, Display)]
    enum Route {
        Home,
        Workspace,
        Settings,
    }
    let theme = sample_theme();
    let route = State::new(Route::Home);
    let nav = Navigation::new("UIX")
        .item("首页", Route::Home)
        .item("工作台", Route::Workspace)
        .item("设置", Route::Settings)
        .active_page(&route)
        .build(theme.tokens());
    let _ = nav;
}

// ---- settings-typed-struct（编译契约：不 run） ----

#[test]
fn settings_typed_struct_anchor_compiles() {
    #[derive(Clone, Default)]
    struct AppPrefs {
        theme: String,
    }
    let app = App::new()
        .settings("app.settings.json")
        .graphics_backend(GraphicsBackend::Vulkan)
        .on_start(|handle| {
            let Some(_settings) = handle.resolve::<SettingsService>() else {
                return;
            };
        });
    let _ = app;
}

// ---- tutorial-virtualized-todo ----

#[test]
fn tutorial_virtualized_todo_anchor_compiles() {
    #[derive(Clone, PartialEq)]
    struct Todo {
        id: u64,
        text: String,
        done: bool,
    }
    let todos = State::new(Vec::<Todo>::new());
    let render_todos = todos.clone();
    let done_flags = State::new(Vec::<State<bool>>::new());
    let view = VirtualScroll::new()
        .item_count(render_todos.get().len())
        .item_height(40.0)
        .render(move |index| {
            let todo = &render_todos.get()[index];
            let done = done_flags.get()[index].clone();
            row((
                embed(Checkbox::new("").checked(&done)),
                label(&todo.text),
            ))
        })
        .build();
    let _ = view;
}

// ---- component-timer-badge（component! 槽位形态） ----

component! {
    /// F 批次验证：TimerBadge 示例形态。
    pub struct FTestTimerBadge {
        pub seconds: u32,
        pub pulse: bool,
    }
    @new -> Self { Self { seconds: 0, pulse: false } }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(160.0, 48.0))
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        Rect::new(frame.x, frame.y, 48.0, frame.h)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext) {
        let color = if self.pulse { ctx.tokens().color_warning() } else { ctx.tokens().color_primary() };
        ctx.draw_text(&format!("{}s", self.seconds), Point::new(frame.x + 8.0, frame.y + 8.0), color, 16.0);
    }
}

#[test]
fn component_timer_badge_anchor_compiles() {
    let badge = FTestTimerBadge::new();
    let _node = badge.into_node();
}

// ---- multi-window-config ----

#[test]
fn multi_window_config_anchor_compiles() {
    let settings_window = WindowConfig::new("设置", 480, 360, || label("settings"))
        .custom_title_bar(true);
    let app = App::new()
        .root(|| column((label("Hello"), button("World"))))
        .on_start(|handle| {
            let _ = handle.open_window(settings_window);
        });
    let _ = app;
}

// ---- async-typed-three-state（UI 三态渲染链） ----

#[test]
fn async_typed_three_state_anchor_compiles() {
    #[derive(Clone)]
    enum AsyncState<T, E> {
        Loading,
        Ready(T),
        Failed(E),
    }
    let users: State<AsyncState<Vec<String>, Error>> = State::new(AsyncState::Loading);
    let view = users.map(|state| match state {
        AsyncState::Loading => embed(Spin::new().large()),
        AsyncState::Ready(data) => {
            let _ = data;
            embed(column(()))
        }
        AsyncState::Failed(error) => embed(
            ResultView::new(ResultType::Error)
                .title("加载失败")
                .subtitle(&error.short_what())
                .extra_text("重试"),
        ),
    });
    let _ = view;
}

// ---- chart-state-driven ----

#[test]
fn chart_state_driven_anchor_compiles() {
    let series = State::new(vec![BarData::new("Q1", 120.0, Color::BLUE)]);
    let chart = series.map(|data| {
        embed(
            BarChart::new()
                .data(data.clone())
                .responsive(true),
        )
    });
    let _ = chart;
    let empty = series.map_opt(|data| (!data.is_empty()).then(|| column(())));
    let _ = empty;
}
