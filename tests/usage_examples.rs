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
        // 默认测试集合只启用 D3D11，因此显式选择同一公开 backend。
        .graphics_backend(GraphicsBackend::Direct3D11)
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
            // 可变列表使用业务标识作为行 key，而不是依赖位置后备 key。
            let key = todo.id.to_string();
            let done = done_flags.get()[index].clone();
            row((
                embed(Checkbox::new("").checked(&done)),
                label(&todo.text),
            ))
            // 将稳定业务标识绑定到行根节点。
            .key(key)
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

// ---- 全量核对修复锁定的示例形态（embed 直接收组件，不 .build()） ----

#[test]
fn embed_accepts_component_chains_without_build() {
    use uix::prelude::*;
    let _ = embed(Card::new().title("用户").child(Label::new("详细信息")).actions(vec!["编辑"]));
    let _ = embed(Tree::new(vec![TreeNode::new("root", "root")]));
    let _ = embed(QRCode::new("https://example.com").size(128.0).error_level(2));
    let _ = embed(ResultView::new(ResultType::Success).title("操作成功").subtitle("已保存"));
    let _ = embed(Descriptions::new().items(vec![DescriptionsItem::new("姓名", "Ada")]).column(2));
    let _ = embed(Timeline::new().items(vec![TimelineItem::new("创建").description("2026-07-01")]));
    let _ = embed(Select::new().options(["中文", "English"]));
    let _ = embed(Badge::new().count(120).max(99));
    let _ = embed(Popconfirm::new().title("确定删除？").confirm_text("确认").cancel_text("取消"));
    let _ = embed(Alert::success("保存成功").action("清理", || {}).banner(true));
    let _ = embed(Spin::new().delay(std::time::Duration::from_millis(200)).size(SpinSize::Large));
    let _ = embed(FloatButtonGroup::new().buttons(vec![FloatButton::new("edit")]));
    let _ = embed(Popover::new("内容").title("标题").arrow(true).bg(Color::WHITE));
    let _ = embed(Tooltip::new("提示").placement(TooltipPlacement::Top).arrow(true));
    let _ = embed(InputNumber::new().min(0.0).max(100.0).step(5.0));
    let _ = embed(RangeSlider::new(0.0..=100.0).step(5.0));
    let _ = embed(Radio::new().options(["管理员", "普通用户"]).default_selected(0));
    let _ = embed(Switch::new().checked(&State::new(false)));
    let _ = embed(DatePicker::new().value(&State::new(Date::new(2026, 7, 31))));
    let _ = embed(TimePicker::new().value(&State::new(Time::new(14, 30))));
    let _ = embed(ColorPicker::new().value(&State::new(Color::hex("#1677ff"))));
    let _ = embed(FocusTrap::new());
    let _ = embed(Splitter::new().panels(2).min_size(0, 120.0).vertical(false));
    let _ = embed(BackTop::new().visibility_height(400.0));
    let _ = embed(Affix::new(12.0).scroll_y(180.0));
    let _ = embed(Gauge::new().value(68.0).min(0.0).max(100.0)
        .range_colors(vec![GaugeRange::new(0.0, 30.0, Color::hex("#ff4d4f"))])
        .gauge_type(GaugeType::Dashboard));
    let _ = embed(ScatterChart::new().data(vec![ScatterData::new("A", 2.5, 6.3)]).x_axis("宽度"));
    let _ = embed(Menu::new().items(vec![MenuItem::new("首页")]).active_key("home").mode(MenuMode::Inline));
    let _nav_group = NavGroup::new().item("首页", "home").active_index(0).build();
    let _ = embed(Pagination::new(200, 20).current(2).simple(true).show_jumper(true));
    let _ = embed(Steps::new(vec![Step::new("填写信息")]).current(1).vertical());
    let _ = embed(Breadcrumb::new().items(vec![BreadcrumbItem::new("首页").active()]).separator(">"));
    let _ = embed(Anchor::new(vec![AnchorItem::new("基本信息", "section-1")]));
    let _ = embed(Tabs::new().tab_position(TabPosition::Top).scrollable(true));
    let _ = embed(Dropdown::new("操作").items(vec!["编辑", "删除"]).trigger(TriggerMode::Hover));
}

#[test]
fn tree_macro_requires_nested_tree_for_item_children() {
    use uix::prelude::*;
    let form = embed(tree! {
        Form::new().label_width(86.0).gap(8.0).layout(FormLayout::Vertical) => [
            tree! { FormItem::new("用户名").name("user").required(true).help("必填") => [
                Input::new("请输入用户名").into_node(),
            ]},
            tree! { FormItem::new("密码").name("password").required(true).help("至少 8 位") => [
                Input::password().into_node(),
            ]},
        ]
    });
    let _ = form;
}

#[test]
fn provider_child_closures_return_views_not_build_results() {
    use uix::prelude::*;
    let _ = embed(ConfigProvider::new()
        .component_size(ControlSize::Large)
        .disabled(true)
        .child(|| column((
            button("继承 Large"),
            input().placeholder("继承禁用态"),
        )))
        .build());
    let _ = embed(ConfigProvider::new()
        .overrides(ComponentOverrides::default())
        .child(|| input().placeholder("自动注入前缀"))
        .build());
    let _ = embed(ConfigProvider::new()
        .component_tokens::<Button>(TokenPatch {
            color_primary: Some(Color::hex("#722ed1")),
            border_radius: Some(10.0),
            ..TokenPatch::default()
        })
        .child(|| button("紫色按钮").primary())
        .build());
}
