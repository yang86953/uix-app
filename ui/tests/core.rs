//! uix-ui 核心模块集成测试（state, style, clipboard, children）。

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;
use uix_graphics::color::Color;
use uix_platform::geometry::{EdgeInsets, Rect};
use uix_ui::api::copy_to_clipboard;
use uix_ui::api::RenderContext;
use uix_ui::api::WidgetChildren;
use uix_ui::api::WidgetTree;
use uix_ui::api::{Computed, Effect, State};
use uix_ui::api::{Style, StyleVariant};
use uix_ui::api::{WidgetCapabilities, WidgetComponent, WidgetRender};

// ════════════════════════════════════════════════════════════════════════════
// clipboard 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn copy_when_no_handler_is_noop() {
    copy_to_clipboard("test");
}

// ════════════════════════════════════════════════════════════════════════════
// state 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn state_get_set() {
    let s = State::new(42i32);
    assert_eq!(s.get(), 42);
    s.set(100);
    assert_eq!(s.get(), 100);
}

#[test]
fn state_update() {
    let s = State::new(String::from("hello"));
    s.update(|v| v.push_str(" world"));
    assert_eq!(s.get(), "hello world");
}

#[test]
fn state_generation() {
    let s = State::new(1i32);
    assert_eq!(s.generation(), 0);
    s.set(2);
    assert_eq!(s.generation(), 1);
    s.update(|v| *v += 1);
    assert_eq!(s.generation(), 2);
}

#[test]
fn state_watch() {
    let s = State::new(0i32);
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();
    s.watch(move |v| {
        assert_eq!(*v, 42);
        called_clone.store(true, Ordering::SeqCst);
    });
    s.set(42);
    assert!(called.load(Ordering::SeqCst));
}

#[test]
fn state_clone_shares_data() {
    let a = State::new(10i32);
    let b = a.clone();
    b.set(99);
    assert_eq!(a.get(), 99);
    assert_eq!(a.generation(), 1);
    assert_eq!(b.generation(), 1);
}

#[test]
fn computed_auto_tracks_deps() {
    let a = State::new(1);
    let b = State::new(2);
    let a2 = a.clone();
    let b2 = b.clone();
    let sum = Computed::new(move || a2.get() + b2.get());
    assert_eq!(sum.get(), 3);
    a.set(10);
    assert_eq!(sum.get(), 12);
    b.set(20);
    assert_eq!(sum.get(), 30);
}

#[test]
fn computed_does_not_recompute_when_deps_unchanged() {
    let a = State::new(42);
    let a2 = a.clone();
    let compute_count = Arc::new(AtomicUsize::new(0));
    let cc = compute_count.clone();
    let c = Computed::new(move || {
        cc.fetch_add(1, Ordering::SeqCst);
        a2.get()
    });
    assert_eq!(c.get(), 42);
    assert_eq!(c.get(), 42);
    assert_eq!(compute_count.load(Ordering::SeqCst), 1);
}

#[test]
fn computed_complex_deps() {
    let a = State::new(1);
    let b = State::new(2);
    let c = State::new(3);
    let a2 = a.clone();
    let b2 = b.clone();
    let c2 = c.clone();
    let sum = Computed::new(move || a2.get() + b2.get() + c2.get());
    assert_eq!(sum.get(), 6);
    a.set(10);
    assert_eq!(sum.get(), 15);
    c.set(30);
    assert_eq!(sum.get(), 42);
}

#[test]
fn effect_auto_tracks() {
    let a = State::new(0);
    let a2 = a.clone();
    let last_val = Arc::new(AtomicI32::new(0));
    let lv = last_val.clone();
    let eff = Effect::new(move || {
        lv.store(a2.get(), Ordering::SeqCst);
    });
    assert!(!eff.tick());
    a.set(42);
    assert!(eff.tick());
    assert_eq!(last_val.load(Ordering::SeqCst), 42);
    assert!(!eff.tick());
}

#[test]
fn computed_basic() {
    let c = Computed::new(|| 42i32);
    assert_eq!(c.get(), 42);
}

#[test]
fn computed_invalidate() {
    let cell = Arc::new(AtomicI32::new(0));
    let cell2 = Arc::clone(&cell);
    let c = Computed::new(move || cell2.load(Ordering::SeqCst));
    assert_eq!(c.get(), 0);
    cell.store(5, Ordering::SeqCst);
    c.invalidate();
    assert_eq!(c.get(), 5);
}

// ════════════════════════════════════════════════════════════════════════════
// style 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn default_style_values() {
    let s = Style::default();
    assert_eq!(s.background, None);
    assert_eq!(s.background_hover, None);
    assert_eq!(s.background_active, None);
    assert_eq!(s.border_color, None);
    assert_eq!(s.border_width, 0.0);
    assert_eq!(s.border_radius, 0.0);
    assert_eq!(s.padding, EdgeInsets::zero());
    assert_eq!(s.margin, EdgeInsets::zero());
    assert_eq!(s.color, Color::black());
    assert_eq!(s.font_size, 14.0);
    assert_eq!(s.opacity, 1.0);
    assert!(s.box_shadow.is_none());
}

#[test]
fn button_default_preset() {
    let s = Style::button_default();
    assert_eq!(s.background, None);
    assert!(s.border_color.is_some());
    assert_eq!(s.border_width, 1.0);
    assert_eq!(s.border_radius, 6.0);
    assert_eq!(s.font_size, 14.0);
    assert_eq!(s.padding, EdgeInsets::new(15.0, 0.0, 15.0, 0.0));
}

#[test]
fn button_primary_preset() {
    let s = Style::button_primary();
    assert_eq!(s.background, Some(Color::from_rgba(22, 119, 255, 255)));
    assert_eq!(s.color, Color::white());
    assert_eq!(s.border_width, 1.0);
    assert_eq!(s.border_radius, 6.0);
}

#[test]
fn default_font_size() {
    let s = Style::default();
    assert_eq!(s.background, None);
    assert_eq!(s.font_size, 14.0);
    assert_eq!(s.padding, EdgeInsets::zero());
}

#[test]
fn with_bg_changes_background() {
    let s = Style::default().with_bg(Color::red());
    assert_eq!(s.background, Some(Color::red()));
}

#[test]
fn with_color_changes_color() {
    let s = Style::default().with_color(Color::blue());
    assert_eq!(s.color, Color::blue());
}

#[test]
fn with_font_size_changes_size() {
    let s = Style::default().with_font_size(18.0);
    assert_eq!(s.font_size, 18.0);
}

#[test]
fn with_padding_changes_padding() {
    let p = EdgeInsets::new(5.0, 10.0, 5.0, 10.0);
    let s = Style::default().with_padding(p);
    assert_eq!(s.padding, p);
}

#[test]
fn with_margin_changes_margin() {
    let m = EdgeInsets::uniform(8.0);
    let s = Style::default().with_margin(m);
    assert_eq!(s.margin, m);
}

#[test]
fn chained_modifications() {
    let s = Style::default()
        .with_bg(Color::from_rgba(0, 0, 0, 255))
        .with_color(Color::white())
        .with_font_size(16.0)
        .with_padding(EdgeInsets::uniform(8.0))
        .with_rounded(4.0)
        .with_shadow(uix_ui::style::BoxShadowDef::new(
            Color::from_rgba(0, 0, 0, 128),
            8.0,
            0.0,
            0.0,
        ));
    assert_eq!(s.background, Some(Color::from_rgba(0, 0, 0, 255)));
    assert_eq!(s.color, Color::white());
    assert_eq!(s.font_size, 16.0);
    assert_eq!(s.padding, EdgeInsets::uniform(8.0));
    assert_eq!(s.border_radius, 4.0);
    assert!(s.box_shadow.is_some());
}

#[test]
fn style_clone_equality() {
    let s1 = Style::button_primary();
    let s2 = s1.clone();
    assert_eq!(s1, s2);
}

#[test]
fn new_is_same_as_default() {
    assert_eq!(Style::new(), Style::default());
}

#[test]
fn effective_bg_without_states_falls_back() {
    let s = Style::default().with_bg(Color::blue());
    assert_eq!(s.effective_bg(false, false), Some(Color::blue()));
    assert_eq!(s.effective_bg(true, false), Some(Color::blue()));
    assert_eq!(s.effective_bg(false, true), Some(Color::blue()));
}

#[test]
fn effective_bg_with_states() {
    let gray = Color::from_rgb(128, 128, 128);
    let s = Style::default()
        .with_bg(gray)
        .with_bg_hover(Color::blue())
        .with_bg_active(Color::red());
    assert_eq!(s.effective_bg(false, false), Some(gray));
    assert_eq!(s.effective_bg(true, false), Some(Color::blue()));
    assert_eq!(s.effective_bg(false, true), Some(Color::red()));
}

#[test]
fn with_border_sets_both() {
    let s = Style::default().with_border(Color::red(), 2.0);
    assert_eq!(s.border_color, Some(Color::red()));
    assert_eq!(s.border_width, 2.0);
}

#[test]
fn with_shadow_sets_box_shadow() {
    use uix_ui::style::BoxShadowDef;
    let shadow = BoxShadowDef::new(Color::from_rgba(0, 0, 0, 100), 10.0, 0.0, 0.0);
    let s = Style::default().with_shadow(shadow);
    assert_eq!(s.box_shadow, Some(shadow));
}

#[test]
fn style_variant_default_resolves_normal() {
    let v = StyleVariant::default();
    let r = v.resolve(false, false, false);
    assert_eq!(r.background, None);
}

#[test]
fn style_variant_resolves_hover() {
    let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let hover_s = Style::default().with_bg(Color::blue());
    let v = StyleVariant::new(normal.clone()).hover(hover_s);
    assert_eq!(v.resolve(false, false, false).background, normal.background);
    assert_eq!(
        v.resolve(true, false, false).background,
        Some(Color::blue())
    );
}

#[test]
fn style_variant_resolves_active() {
    let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let active_s = Style::default().with_bg(Color::red());
    let v = StyleVariant::new(normal).active(active_s);
    assert_eq!(v.resolve(false, true, false).background, Some(Color::red()));
}

#[test]
fn style_variant_resolves_disabled() {
    let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let disabled_s = Style::default().with_bg(Color::from_rgb(64, 64, 64));
    let v = StyleVariant::new(normal).disabled(disabled_s);
    assert_eq!(
        v.resolve(false, false, true).background,
        Some(Color::from_rgb(64, 64, 64))
    );
}

#[test]
fn style_variant_fallback() {
    let normal = Style::default().with_bg(Color::from_rgb(128, 128, 128));
    let v = StyleVariant::new(normal.clone());
    assert_eq!(v.resolve(true, false, false).background, normal.background);
    assert_eq!(v.resolve(false, true, false).background, normal.background);
    assert_eq!(v.resolve(false, false, true).background, normal.background);
}

// ════════════════════════════════════════════════════════════════════════════
// children 测试
// ════════════════════════════════════════════════════════════════════════════

// ── 用于 children 测试的本地 dummy widget ──

struct DummyWidget;
impl WidgetComponent for DummyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::RENDER)
    }
    uix_ui::wc_upcast!(DummyWidget; WidgetRender);
}
impl WidgetRender for DummyWidget {
    fn render(&self, _: Rect, _: &mut RenderContext, _: &WidgetTree) {}
}

#[test]
fn children_new_is_empty() {
    let c = WidgetChildren::new();
    assert!(!c.is_set());
    assert_eq!(c.len(), 0);
    assert!(c.take().is_empty());
}

#[test]
fn children_add_and_take() {
    let c = WidgetChildren::new();
    c.add(DummyWidget);
    assert!(c.is_set());
    assert_eq!(c.len(), 1);
    let taken = c.take();
    assert_eq!(taken.len(), 1);
    assert!(!c.is_set());
}

#[test]
fn children_add_boxed() {
    let c = WidgetChildren::new();
    c.add_boxed(Box::new(DummyWidget));
    assert!(c.is_set());
    assert_eq!(c.len(), 1);
}

#[test]
fn children_set_all() {
    let c = WidgetChildren::new();
    c.set_all(vec![Box::new(DummyWidget), Box::new(DummyWidget)]);
    assert!(c.is_set());
    assert_eq!(c.len(), 2);
}

#[test]
fn children_double_take_returns_empty() {
    let c = WidgetChildren::new();
    c.add(DummyWidget);
    let first = c.take();
    assert_eq!(first.len(), 1);
    let second = c.take();
    assert!(second.is_empty());
}

#[test]
fn children_is_set_after_add() {
    let c = WidgetChildren::new();
    assert!(!c.is_set());
    c.add(DummyWidget);
    assert!(c.is_set());
}

#[test]
fn children_multiple_adds() {
    let c = WidgetChildren::new();
    c.add(DummyWidget);
    c.add(DummyWidget);
    c.add(DummyWidget);
    assert_eq!(c.len(), 3);
    let taken = c.take();
    assert_eq!(taken.len(), 3);
}
