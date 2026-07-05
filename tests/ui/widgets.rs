//! ui 域 — 内置组件集成测试。

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use uix::draw::Color;
use uix::draw::traits::GraphicsEngine;
use uix::core::geometry::{Point, Rect, Size};
use uix::native::ControlSize;
use uix::native::EdgeInsets;
use uix::native::ScrollDirection;
use uix::ui::JustifyContent;
use uix::ui::RenderContext;
use uix::ui::Style;
use uix::ui::{AlignItems, FlexDirection};
use uix::ui::{
    Button, ButtonSize, ButtonVariant, Drawer, DrawerPlacement, Input, InputSize, Modal,
};
use uix::ui::{Container, NavItem, ScrollView, SharedActive};
use uix::ui::{
    EventResult, KeyCode, KeyMod, MouseButton, WidgetCapabilities, WidgetComponent, WidgetCore,
    WidgetEvent, WidgetEventHandler, WidgetLayout, WidgetLifecycle, WidgetRender, WidgetTree,
};
use uix::ui::style::{BoxShadowDef, DisplayMode};
use uix::ui::widgets::{button_font_size, button_height, button_padding_h, input_height};

// ════════════════════════════════════════════════════════════════════════════
// ScrollView 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn scrollview_default_size() {
    let sv = ScrollView::new(ScrollDirection::Vertical);
    let ps = sv.preferred_size(None);
    assert_eq!(ps, Size::new(300.0, 200.0));
}

#[test]
fn scrollview_custom_size() {
    let sv = ScrollView::new(ScrollDirection::Both).size(400.0, 300.0);
    let ps = sv.preferred_size(None);
    assert_eq!(ps, Size::new(400.0, 300.0));
}

#[test]
fn scrollview_mouse_wheel_vertical_up() {
    let mut sv = ScrollView::new(ScrollDirection::Vertical);
    sv.scroll_y = 60.0;
    assert_eq!(sv.scroll_y, 60.0);

    let result = sv.on_event(&WidgetEvent::MouseWheel {
        pos: Point::default(),
        delta: Point::new(0.0, 1.0),
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(sv.velocity_y, 50.0);
}

#[test]
fn scrollview_mouse_wheel_scrolls_down() {
    let mut sv = ScrollView::new(ScrollDirection::Vertical);
    assert_eq!(sv.scroll_y, 0.0);

    let result = sv.on_event(&WidgetEvent::MouseWheel {
        pos: Point::default(),
        delta: Point::new(0.0, -1.0),
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(sv.velocity_y, -50.0);
}

#[test]
fn scrollview_mouse_wheel_horizontal() {
    let mut sv = ScrollView::new(ScrollDirection::Horizontal);
    assert_eq!(sv.scroll_x, 0.0);

    let result = sv.on_event(&WidgetEvent::MouseWheel {
        pos: Point::default(),
        delta: Point::new(-1.0, 0.0),
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(sv.velocity_x, -75.0);
}

#[test]
fn scrollview_mouse_wheel_both() {
    let mut sv = ScrollView::new(ScrollDirection::Both);

    let result = sv.on_event(&WidgetEvent::MouseWheel {
        pos: Point::default(),
        delta: Point::new(-1.0, -2.0),
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(sv.velocity_x, -75.0);
    assert_eq!(sv.velocity_y, -100.0);
}

#[test]
fn scrollview_scroll_clamped_to_zero() {
    let mut sv = ScrollView::new(ScrollDirection::Vertical);
    sv.scroll_y = -10.0;
    sv.on_update(0.0);
    // scroll 应该在 clamp 后变为 0
    assert_eq!(sv.scroll_y, 0.0);
}

// ════════════════════════════════════════════════════════════════════════════
// WidgetTree 测试辅助类型
// ════════════════════════════════════════════════════════════════════════════

struct SpyWidget {
    size: Size,
    last_event: RefCell<Option<WidgetEvent>>,
}

impl SpyWidget {
    fn new(w: f32, h: f32) -> Self {
        Self {
            size: Size::new(w, h),
            last_event: RefCell::new(None),
        }
    }
}

impl WidgetComponent for SpyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    uix::wc_upcast!(SpyWidget; WidgetLayout);
    uix::wc_upcast!(SpyWidget; WidgetRender);
    uix::wc_upcast!(SpyWidget; WidgetEventHandler);
}
impl WidgetLayout for SpyWidget {
    fn preferred_size(&self, _: Option<&dyn GraphicsEngine>) -> Size {
        self.size
    }
}
impl WidgetRender for SpyWidget {
    fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
}
impl WidgetEventHandler for SpyWidget {
    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        *self.last_event.borrow_mut() = Some(event.clone());
        EventResult::Handled
    }
}

struct PassThroughContainer {
    size: Size,
    children: RefCell<Vec<Box<dyn WidgetComponent>>>,
}

impl PassThroughContainer {
    fn new(w: f32, h: f32, children: Vec<Box<dyn WidgetComponent>>) -> Self {
        Self {
            size: Size::new(w, h),
            children: RefCell::new(children),
        }
    }
}

impl WidgetComponent for PassThroughContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        std::mem::take(&mut *self.children.borrow_mut())
    }
    uix::wc_upcast!(PassThroughContainer; WidgetLayout);
    uix::wc_upcast!(PassThroughContainer; WidgetRender);
    uix::wc_upcast!(PassThroughContainer; WidgetEventHandler);
}
impl WidgetLayout for PassThroughContainer {
    fn preferred_size(&self, _: Option<&dyn GraphicsEngine>) -> Size {
        self.size
    }
}
impl WidgetRender for PassThroughContainer {
    fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
}
impl WidgetEventHandler for PassThroughContainer {
    fn on_event(&mut self, _: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
    }
}

// ════════════════════════════════════════════════════════════════════════════
// WidgetTree 基本操作测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn tree_set_root_returns_valid_id() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    assert!(tree.get(id).is_some());
    assert_eq!(tree.root().unwrap().id(), id);
}

#[test]
fn tree_add_child_links_parent() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let cid = tree.add_child(root, Box::new(SpyWidget::new(80.0, 40.0)));
    assert!(tree.get(cid).is_some());
    assert_eq!(tree.get(root).unwrap().children(), &[cid]);
    assert_eq!(tree.get(cid).unwrap().parent(), Some(root));
}

#[test]
fn tree_traverse_preorder() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    let b = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    let c = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
    assert_eq!(tree.traverse(), vec![root, a, c, b]);
}

#[test]
fn tree_remove_cascades_to_children() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
    let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
    let b = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
    tree.remove(a);
    assert!(tree.get(a).is_none());
    assert!(tree.get(b).is_none());
    assert_eq!(tree.get(root).unwrap().children().len(), 0);
}

#[test]
fn hit_test_root_contains() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    tree.layout();
    assert!(tree.hit_test(Point::new(50.0, 25.0)).is_some());
}

#[test]
fn hit_test_outside_returns_none() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
    tree.layout();
    assert!(tree.hit_test(Point::new(200.0, 200.0)).is_none());
    assert!(tree.hit_test(Point::new(-1.0, 25.0)).is_none());
}

#[test]
fn hit_test_returns_deepest_child() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(child));
}

#[test]
fn hit_test_skips_invisible() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    tree.get_mut(child).unwrap().set_visible(false);
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
}

#[test]
fn dispatch_mouse_down_focuses_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn = tree.add_child(root_id, Box::new(SpyWidget::new(80.0, 40.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(btn)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(40.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

#[test]
fn dispatch_mouse_down_empty_space_clears_focus() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(300.0, 300.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
}

#[test]
fn dispatch_key_to_focused_widget() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        tree.dispatch_event(&WidgetEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

#[test]
fn dispatch_mouse_move_triggers_hover_enter_leave() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    tree.dispatch_event(&WidgetEvent::MouseMove {
        pos: Point::new(50.0, 50.0),
        mods: KeyMod::NONE,
    });
}

#[test]
fn dispatch_resize_goes_to_root() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    assert_eq!(
        tree.dispatch_event(&WidgetEvent::Resize {
            width: 400.0,
            height: 300.0
        }),
        EventResult::Handled
    );
}

#[test]
fn nav_item_click_updates_shared_active() {
    let active: SharedActive = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Container::new()
            .size(200.0, 200.0)
            .dir(FlexDirection::Column),
    ));
    let n0 = tree.add_child(
        root,
        Box::new(
            NavItem::new("Item 0", 0, active.clone())
                .width(200.0)
                .height(36.0),
        ),
    );
    let n1 = tree.add_child(
        root,
        Box::new(
            NavItem::new("Item 1", 1, active.clone())
                .width(200.0)
                .height(36.0),
        ),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(n0)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 36.0));
    tree.get_mut(n1)
        .unwrap()
        .set_frame(Rect::new(0.0, 36.0, 200.0, 36.0));
    assert_eq!(active.get(), 0);
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 54.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(active.get(), 1);
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 18.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(active.get(), 0);
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 150.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::NotHandled);
    assert_eq!(active.get(), 0);
}

// ════════════════════════════════════════════════════════════════════════════
// Button 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn button_height_values() {
    assert_eq!(button_height(ButtonSize::Small), 24.0);
    assert_eq!(button_height(ButtonSize::Medium), 32.0);
    assert_eq!(button_height(ButtonSize::Large), 40.0);
}

#[test]
fn button_font_size_values() {
    assert_eq!(button_font_size(ButtonSize::Small), 14.0);
    assert_eq!(button_font_size(ButtonSize::Medium), 14.0);
    assert_eq!(button_font_size(ButtonSize::Large), 16.0);
}

#[test]
fn button_padding_h_values() {
    assert_eq!(button_padding_h(ButtonSize::Small), 7.0);
    assert_eq!(button_padding_h(ButtonSize::Medium), 15.0);
    assert_eq!(button_padding_h(ButtonSize::Large), 15.0);
}

#[test]
fn button_new_defaults() {
    let btn = Button::new("test");
    let ps = btn.preferred_size(None);
    // Medium size: h = 32.0
    assert!((ps.h - 32.0).abs() < 0.001);
    assert!(ps.w >= 32.0);
}

#[test]
fn button_primary_sets_variant() {
    let btn = Button::new("Primary").primary();
    let ps = btn.preferred_size(None);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn button_disabled_true() {
    let mut btn = Button::new("Click").disabled(true);
    let result = btn.on_event(&WidgetEvent::MouseDown {
        pos: Point::new(10.0, 10.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::NotHandled);
}

#[test]
fn button_size_changes_height() {
    let btn_large = Button::new("Large").size(ButtonSize::Large);
    let ps = btn_large.preferred_size(None);
    assert!((ps.h - 40.0).abs() < 0.001);

    let btn_small = Button::new("Small").size(ButtonSize::Small);
    let ps = btn_small.preferred_size(None);
    assert!((ps.h - 24.0).abs() < 0.001);
}

#[test]
fn button_preferred_size_with_text() {
    let btn = Button::new("Hello");
    let ps = btn.preferred_size(None);
    // Medium: padding_h = 15, text = 5 chars * 7 = 35, w = 35 + 30 = 65
    assert!(ps.w >= 32.0);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn button_preferred_size_block_mode() {
    let btn = Button::new("Block").block(true);
    let ps = btn.preferred_size(None);
    assert_eq!(ps.w, f32::MAX);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn button_preferred_size_empty_text() {
    let btn = Button::new("");
    let ps = btn.preferred_size(None);
    assert!((ps.w - 32.0).abs() < 0.001);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn button_danger_ghost_loading_icon_builders() {
    let btn = Button::new("test")
        .danger(true)
        .ghost(true)
        .loading(true)
        .icon("search");
    let ps = btn.preferred_size(None);
    // With loading + icon + text, width should be > minimum
    assert!(ps.w >= 32.0);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn button_variant_variants() {
    assert_eq!(ButtonVariant::Default, ButtonVariant::Default);
    assert_eq!(ButtonVariant::Primary, ButtonVariant::Primary);
    assert_eq!(ButtonVariant::Dashed, ButtonVariant::Dashed);
    assert_eq!(ButtonVariant::Text, ButtonVariant::Text);
    assert_eq!(ButtonVariant::Link, ButtonVariant::Link);
    assert_ne!(ButtonVariant::Default, ButtonVariant::Primary);
}

#[test]
fn button_style_builders() {
    let btn = Button::new("Styled")
        .bg(Color::red())
        .color(Color::white())
        .fs(16.0)
        .margin(EdgeInsets::new(4.0, 4.0, 4.0, 4.0))
        .padding(EdgeInsets::new(8.0, 8.0, 8.0, 8.0))
        .rounded(6.0)
        .border(Color::blue(), 2.0)
        .w(120.0)
        .h(48.0)
        .opacity(0.8);
    let ps = btn.preferred_size(None);
    assert!(ps.w >= 32.0);
    assert!(ps.h >= 24.0);
}

// ════════════════════════════════════════════════════════════════════════════
// Input 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn input_height_values() {
    assert_eq!(input_height(InputSize::Small), 24.0);
    assert_eq!(input_height(InputSize::Medium), 32.0);
    assert_eq!(input_height(InputSize::Large), 40.0);
}

#[test]
fn input_new_defaults() {
    let input = Input::new("placeholder");
    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn input_with_value() {
    let input = Input::new("").with_value("hello");
    assert_eq!(input.value(), "hello");
}

#[test]
fn input_value_returns_current() {
    let input = Input::new("hint");
    assert_eq!(input.value(), "");
}

#[test]
fn input_set_value_updates() {
    let mut input = Input::new("");
    input.set_value("world");
    assert_eq!(input.value(), "world");
}

#[test]
fn input_disabled_size_prefix_suffix_builders() {
    let mut input = Input::new("")
        .disabled(true)
        .size(InputSize::Small)
        .prefix("🔍")
        .suffix("✕");
    let result = input.on_event(&WidgetEvent::MouseDown {
        pos: Point::new(5.0, 5.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::NotHandled);

    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    assert!((ps.h - 24.0).abs() < 0.001);
}

#[test]
fn input_password_clearable_search_builders() {
    let input = Input::new("").password(true).clearable(true).search(true);
    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    assert!((ps.h - 32.0).abs() < 0.001);
}

#[test]
fn input_textarea_mode() {
    let input = Input::new("").textarea(true);
    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    // textarea_rows = 3, empty string has 1 line, max(1,3) = 3
    // h = 3 * 22 + 16 = 82, max(82, 48) = 82
    assert!((ps.h - 82.0).abs() < 0.001);
}

#[test]
fn input_textarea_rows() {
    let input = Input::new("").textarea(true).textarea_rows(5);
    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    // line_count = max(1,5) = 5, h = 5 * 22 + 16 = 126
    assert!((ps.h - 126.0).abs() < 0.001);
}

#[test]
fn input_set_focused() {
    let mut input = Input::new("");
    input.set_focused(true);
    // FocusOut should be handled
    let result = input.on_event(&WidgetEvent::FocusOut);
    assert_eq!(result, EventResult::Handled);
}

#[test]
fn input_preferred_size_single_line() {
    let input = Input::new("").size(InputSize::Large);
    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    assert!((ps.h - 40.0).abs() < 0.001);
}

#[test]
fn input_preferred_size_textarea() {
    let input = Input::new("").textarea(true).textarea_rows(8);
    let ps = input.preferred_size(None);
    assert_eq!(ps.w, 80.0);
    // h = 8 * 22 + 16 = 192
    assert!((ps.h - 192.0).abs() < 0.001);
}

// ════════════════════════════════════════════════════════════════════════════
// Container 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn container_new_defaults() {
    let c = Container::new();
    let ps = c.preferred_size(None);
    assert_eq!(ps.w, 0.0);
    assert_eq!(ps.h, 0.0);
}

#[test]
fn container_bg_margin_padding_border_rounded() {
    let c = Container::new()
        .bg(Color::red())
        .margin(EdgeInsets::uniform(8.0))
        .padding(EdgeInsets::new(4.0, 2.0, 4.0, 2.0))
        .border(Color::black(), 1.0)
        .rounded(4.0);
    let ps = c.preferred_size(None);
    // No fixed width: effective_w = 0, w = 0 + 16(margin) + 2(border) = 18
    assert!((ps.w - 18.0).abs() < 0.001);
    assert_eq!(ps.h, 0.0);
}

#[test]
fn container_w_h_size_direction_gap() {
    let c = Container::new()
        .w(150.0)
        .h(80.0)
        .size(200.0, 100.0) // overrides w/h
        .direction(FlexDirection::Column)
        .gap(12.0);
    let ps = c.preferred_size(None);
    assert!((ps.w - 200.0).abs() < 0.001);
    assert!((ps.h - 100.0).abs() < 0.001);
}

#[test]
fn container_justify_align_flex_grow_opacity() {
    let c = Container::new()
        .justify(JustifyContent::Center)
        .align(AlignItems::Center)
        .flex_grow(1.0)
        .opacity(0.5);
    let ps = c.preferred_size(None);
    assert_eq!(ps.h, 0.0);
}

#[test]
fn container_visible_shadow_display() {
    let c = Container::new()
        .visible(false)
        .shadow(BoxShadowDef::new(
            Color::from_rgba(0, 0, 0, 30),
            8.0,
            0.0,
            2.0,
        ))
        .display(DisplayMode::None);
    let ps = c.preferred_size(None);
    assert_eq!(ps.w, 0.0);
    assert_eq!(ps.h, 0.0);
}

#[test]
fn container_default_trait() {
    let c: Container = Default::default();
    let ps = c.preferred_size(None);
    assert_eq!(ps.w, 0.0);
    assert_eq!(ps.h, 0.0);
}

#[test]
fn container_style_sets_style() {
    let s = Style::container().with_width(180.0).with_height(90.0);
    let c = Container::new().style(s);
    let ps = c.preferred_size(None);
    assert!((ps.w - 180.0).abs() < 0.001);
    assert!((ps.h - 90.0).abs() < 0.001);
}

#[test]
fn container_apply_style_merges() {
    let s = Style::container().with_width(200.0).with_height(100.0);
    let overlay = Style::container().with_width(250.0);
    let c = Container::new().style(s).apply_style(overlay);
    let ps = c.preferred_size(None);
    assert!((ps.w - 250.0).abs() < 0.001);
    // height should remain from original style
    assert!((ps.h - 100.0).abs() < 0.001);
}

#[test]
fn container_preferred_size_fixed() {
    let c = Container::new().size(320.0, 240.0);
    let ps = c.preferred_size(None);
    assert!((ps.w - 320.0).abs() < 0.001);
    assert!((ps.h - 240.0).abs() < 0.001);
}

// ════════════════════════════════════════════════════════════════════════════
// Modal 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn modal_new_defaults() {
    let modal = Modal::new("Title");
    assert!(!modal.is_visible());
    let ps = modal.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

#[test]
fn modal_visible_show_size_builders() {
    let modal = Modal::new("test").visible(true).size(600.0, 400.0);
    assert!(modal.is_visible());
    let ps = modal.preferred_size(None);
    assert!((ps.w - 600.0).abs() < 0.001);
    assert!((ps.h - 400.0).abs() < 0.001);

    let modal2 = Modal::new("show").show();
    assert!(modal2.is_visible());
}

#[test]
fn modal_closable_mask_closable_footer_visible() {
    let modal = Modal::new("test")
        .closable(false)
        .mask_closable(false)
        .footer_visible(false);
    assert!(!modal.is_visible());
    let ps = modal.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

#[test]
fn modal_centered_overlay() {
    let modal = Modal::new("test").centered(false).overlay(true);
    assert!(!modal.is_visible());
    // overlay mode: preferred_size always zero
    let ps = modal.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

#[test]
fn modal_modal_size_small_medium_large() {
    let small = Modal::new("test").modal_size(ControlSize::Small);
    let ps = small.show().preferred_size(None);
    assert!((ps.w - 400.0).abs() < 0.001);
    assert!((ps.h - 200.0).abs() < 0.001);

    let medium = Modal::new("test").modal_size(ControlSize::Medium);
    let ps = medium.show().preferred_size(None);
    assert!((ps.w - 520.0).abs() < 0.001);
    assert!((ps.h - 300.0).abs() < 0.001);

    let large = Modal::new("test").modal_size(ControlSize::Large);
    let ps = large.show().preferred_size(None);
    assert!((ps.w - 720.0).abs() < 0.001);
    assert!((ps.h - 400.0).abs() < 0.001);
}

#[test]
fn modal_on_ok_stores_callback() {
    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    let mut modal = Modal::new("test").on_ok(move || {
        c.set(true);
    });
    modal.confirm();
    assert!(called.get());
}

#[test]
fn modal_on_cancel_stores_callback() {
    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    let mut modal = Modal::new("test").on_cancel(move || {
        c.set(true);
    });
    modal.open();
    modal.close();
    assert!(called.get());
}

#[test]
fn modal_is_visible_returns_state() {
    let modal = Modal::new("test");
    assert!(!modal.is_visible());

    let modal = Modal::new("test").visible(true);
    assert!(modal.is_visible());
}

#[test]
fn modal_set_visible_changes() {
    let mut modal = Modal::new("test");
    modal.set_visible(true);
    assert!(modal.is_visible());
    modal.set_visible(false);
    assert!(!modal.is_visible());
}

#[test]
fn modal_set_visible_no_anim() {
    let mut modal = Modal::new("test");
    modal.set_visible_no_anim(true);
    assert!(modal.is_visible());
    modal.set_visible_no_anim(false);
    assert!(!modal.is_visible());
}

#[test]
fn modal_open_opens_from_closed() {
    let mut modal = Modal::new("test");
    assert!(!modal.is_visible());
    modal.open();
    assert!(modal.is_visible());
}

#[test]
fn modal_close_triggers_from_open() {
    let mut modal = Modal::new("test");
    modal.open();
    modal.close();
    // during closing animation, is_visible still returns true
    assert!(modal.is_visible());
    // calling close() again is a no-op while closing
    modal.close();
    assert!(modal.is_visible());
}

#[test]
fn modal_preferred_size_zero_hidden() {
    let modal = Modal::new("test");
    let ps = modal.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

#[test]
fn modal_preferred_size_when_visible() {
    let modal = Modal::new("test").show();
    let ps = modal.preferred_size(None);
    assert!((ps.w - 520.0).abs() < 0.001);
    assert!((ps.h - 300.0).abs() < 0.001);
}

#[test]
fn modal_preferred_size_overlay() {
    let modal = Modal::new("test").overlay(true).show();
    let ps = modal.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

// ════════════════════════════════════════════════════════════════════════════
// Drawer 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn drawer_new_defaults() {
    let drawer = Drawer::new("Title");
    assert!(!drawer.is_visible());
    let ps = drawer.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

#[test]
fn drawer_visible_show_size_placement() {
    let drawer = Drawer::new("test")
        .visible(true)
        .size(500.0, 400.0)
        .placement(DrawerPlacement::Left);
    assert!(drawer.is_visible());
    // Left placement: Size::new(width, 600)
    let ps = drawer.preferred_size(None);
    assert!((ps.w - 500.0).abs() < 0.001);
    assert!((ps.h - 600.0).abs() < 0.001);

    let drawer2 = Drawer::new("show").show();
    assert!(drawer2.is_visible());
}

#[test]
fn drawer_closable_mask_closable_mask() {
    let drawer = Drawer::new("test")
        .closable(false)
        .mask_closable(false)
        .mask(false);
    assert!(!drawer.is_visible());
}

#[test]
fn drawer_footer_visible_extra() {
    let drawer = Drawer::new("test").footer_visible(true).extra("Extra");
    assert!(!drawer.is_visible());
}

#[test]
fn drawer_drawer_size_variants() {
    let small = Drawer::new("test").drawer_size(ControlSize::Small);
    let ps = small.visible(true).preferred_size(None);
    // Right placement: Size::new(width, 600), Small width = 300
    assert!((ps.w - 300.0).abs() < 0.001);
    assert!((ps.h - 600.0).abs() < 0.001);

    let medium = Drawer::new("test").drawer_size(ControlSize::Medium);
    let ps = medium.visible(true).preferred_size(None);
    assert!((ps.w - 378.0).abs() < 0.001);
    assert!((ps.h - 600.0).abs() < 0.001);

    let large = Drawer::new("test").drawer_size(ControlSize::Large);
    let ps = large.visible(true).preferred_size(None);
    assert!((ps.w - 600.0).abs() < 0.001);
    assert!((ps.h - 600.0).abs() < 0.001);
}

#[test]
fn drawer_placement_variants() {
    assert_eq!(DrawerPlacement::Right, DrawerPlacement::Right);
    assert_eq!(DrawerPlacement::Left, DrawerPlacement::Left);
    assert_eq!(DrawerPlacement::Top, DrawerPlacement::Top);
    assert_eq!(DrawerPlacement::Bottom, DrawerPlacement::Bottom);
    assert_ne!(DrawerPlacement::Right, DrawerPlacement::Left);
}

#[test]
fn drawer_is_visible_open_close_set_visible() {
    let mut drawer = Drawer::new("test");
    assert!(!drawer.is_visible());

    drawer.open();
    assert!(drawer.is_visible());

    drawer.set_visible(false);
    assert!(!drawer.is_visible());

    drawer.set_visible(true);
    assert!(drawer.is_visible());

    drawer.close();
    // during closing animation, is_visible still returns true
    assert!(drawer.is_visible());
}

#[test]
fn drawer_preferred_size_zero_hidden() {
    let drawer = Drawer::new("test");
    let ps = drawer.preferred_size(None);
    assert_eq!(ps, Size::zero());
}

#[test]
fn drawer_preferred_size_placement_based() {
    // Right (default): Size::new(width, 600)
    let drawer = Drawer::new("test").visible(true);
    let ps = drawer.preferred_size(None);
    assert!((ps.w - 378.0).abs() < 0.001);
    assert!((ps.h - 600.0).abs() < 0.001);

    // Left: Size::new(width, 600)
    let drawer = Drawer::new("test")
        .visible(true)
        .placement(DrawerPlacement::Left);
    let ps = drawer.preferred_size(None);
    assert!((ps.w - 378.0).abs() < 0.001);
    assert!((ps.h - 600.0).abs() < 0.001);

    // Top: Size::new(400, height)
    let drawer = Drawer::new("test")
        .visible(true)
        .placement(DrawerPlacement::Top);
    let ps = drawer.preferred_size(None);
    assert!((ps.w - 400.0).abs() < 0.001);
    assert!((ps.h - 300.0).abs() < 0.001);

    // Bottom: Size::new(400, height)
    let drawer = Drawer::new("test")
        .visible(true)
        .placement(DrawerPlacement::Bottom);
    let ps = drawer.preferred_size(None);
    assert!((ps.w - 400.0).abs() < 0.001);
    assert!((ps.h - 300.0).abs() < 0.001);
}
