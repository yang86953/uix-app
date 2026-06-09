//! Linux GUI Demo — UIX Framework rendered via Wayland SHM + SoftwareEngine.
//!
//! Demonstrates:
//! - Window creation via `LinuxPlatform` directly
//! - Widget tree layout and rendering via `SoftwareEngine`
//! - Pixel presentation via Wayland SHM buffer (no X11, no GPU)
//! - Keyboard/mouse event dispatch to the widget tree
//!
//! Run: `cargo run --bin uix-demo`

#![cfg(all(unix, not(target_os = "macos")))]

use uix::diag::log::{info_fn, Level, Logger};
use std::sync::atomic::{AtomicBool, Ordering};
use uix::platform::{IEventLoop, IWindowManager};
use uix::graphics::{Color, DirtyRegion, GraphicsEngine, Rect, Size, SoftwareEngine};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::platform::linux::LinuxPlatform;
use uix::platform::types::KeyCode as PlatformKeyCode;
use uix::platform::types::MouseButton as PlatformMouseButton;
use uix::ui::theme::DesignTokens;
use uix::ui::render_context::RenderContext;
use uix::ui::widget::{EventResult, WidgetEvent, WidgetTree};
use uix::ui::{
    AlignItems, Button, ButtonSize, Card, Container, Divider,
    DividerOrientation, FlexDirection, Input, InputSize, IntoWidgetNode, JustifyContent,
    KeyCode as WidgetKeyCode, Label, MouseButton as WidgetMouseButton, ScrollDirection,
    ScrollView, Space, SpaceSize, WidgetNode,
};

use uix::{define_widget, tree};

const GW: i32 = 1024;
const GH: i32 = 720;

// ── Custom component: Counter ────────────────────────────────────────────

define_widget! {
    /// Counter button that increments on click.
    pub struct Counter {
        count: u32,
    }

    @new -> Self { Self { count: 0 } }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(120.0, 36.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { .. } => { self.count += 1; EventResult::Handled }
            _ => EventResult::NotHandled
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_primary_bg();
        let color = ctx.tokens().color_primary();
        ctx.fill_rect(frame, bg, None);
        ctx.draw_text(
            &format!("Count: {}", self.count),
            uix::graphics::Point::new(frame.x + 8.0, frame.y + 8.0),
            color,
            14.0,
        );
    }
}

// ── Navigation item component (function component) ───────────────────────

/// Build a nav item row.
fn nav_item(label: &str, active: bool, text_color: Color, bg_color: Color) -> WidgetNode {
    if active {
        tree! {
            Container::new().size(200.0, 36.0).dir(FlexDirection::Row).bg(bg_color) => [
                Container::new().size(3.0, 36.0).bg(text_color),
                Container::new().size(12.0, 36.0),
                Label::new(label, text_color).font_size(14.0).size(170.0, 36.0),
            ]
        }
    } else {
        tree! {
            Container::new().size(200.0, 36.0).dir(FlexDirection::Row) => [
                Container::new().size(15.0, 36.0),
                Label::new(label, text_color).font_size(14.0).size(170.0, 36.0),
            ]
        }
    }
}

/// Build a stat card.
fn stat_card(title: &str, value: &str, stat_color: Color, elevation: u8) -> Card {
    Card::new()
        .title(title).elevation(elevation).hoverable()
        .size((GW as f32 - 200.0 - 40.0) / 4.0 - 10.0, 100.0)
        .child(Label::new(value, stat_color).font_size(28.0))
}

/// Build the dashboard widget tree using `tree!` macro (React-style).
fn build_dashboard(tree: &mut WidgetTree) {
    let t = DesignTokens::antd_light();
    let cw = GW as f32 - 200.0;
    let stats_w = cw - 40.0;
    let card_h = 100.0;

    tree.build(tree! {
        // ── Root: horizontal split ──
        Container::new().size(GW as f32, GH as f32).bg(t.color_bg_layout).dir(FlexDirection::Row) => [

            // ── Sidebar ──
            tree! { Container::new().size(200.0, GH as f32).bg(t.color_bg_elevated).dir(FlexDirection::Column) => [
                Label::new("UIX Framework", t.color_primary).font_size(20.0).size(200.0, 52.0),
                Divider::new().color(t.color_border_secondary),
                // Nav items from function component
                nav_item(" Dashboard", true,  t.color_primary, t.color_primary_bg),
                nav_item(" Widgets",  false, t.color_text_secondary, Color::transparent()),
                nav_item(" Settings", false, t.color_text_secondary, Color::transparent()),
                nav_item(" About",    false, t.color_text_secondary, Color::transparent()),
                Container::new().size(200.0, GH as f32 - 200.0),
                Label::new("UIX v0.1.0 — Rust Native", t.color_text_quaternary)
                    .font_size(11.0).size(200.0, 24.0),
            ]},

            // ── Main Content ──
            tree! { Container::new().size(cw, GH as f32).bg(t.color_bg_container).dir(FlexDirection::Column) => [

                // Title bar
                tree! { Container::new().size(cw, 44.0).bg(t.color_bg_elevated).dir(FlexDirection::Row) => [
                    Label::new("  Dashboard", t.color_text).font_size(16.0).size(400.0, 44.0),
                    Container::new().size(cw - 496.0, 44.0),
                ]},

                // Body — wrapped in ScrollView so content scrolls if the window is too short
                tree! { ScrollView::new(ScrollDirection::Vertical).size(cw, GH as f32 - 44.0 - 1.0) => [
                    tree! { Container::new().size(cw, 600.0).dir(FlexDirection::Column) => [

                        Label::new("Overview", t.color_text).font_size(18.0).size(200.0, 28.0),

                        // Stat cards
                        Space::new().size(SpaceSize::Custom(10.0))
                            .width(stats_w).height(card_h).direction(FlexDirection::Row)
                            .justify(JustifyContent::Start).align(AlignItems::Stretch)
                            .child(stat_card("Total Users", "1,234", t.color_primary, 2))
                            .child(stat_card("Revenue", "$8,291", t.color_success, 1))
                            .child(stat_card("Orders", "89", t.color_warning, 1))
                            .child(stat_card("Growth", "12.5%", t.color_info, 1)),

                        // Custom Counter component
                        Label::new("Counter (click me)", t.color_text).font_size(16.0).size(200.0, 24.0),
                        Counter { count: 0 }.into_node(),

                        // Buttons
                        Label::new("Buttons", t.color_text).font_size(16.0).size(200.0, 24.0),
                        tree! { Space::new().size(SpaceSize::Custom(10.0)).width(stats_w).height(36.0).align(AlignItems::Center) => [
                            Button::new("Primary").primary().size(ButtonSize::Middle),
                            Button::new("Default").size(ButtonSize::Middle),
                            Button::new("Dashed").dashed().size(ButtonSize::Middle),
                        ]},

                        // Form
                        Divider::new().with_text("Form").orientation(DividerOrientation::Left).color(t.color_border),
                        tree! { Space::new().size(SpaceSize::Custom(10.0)).width(stats_w).height(44.0).align(AlignItems::Center) => [
                            Input::new("Type here...").size(InputSize::Middle),
                            Button::new("Submit").primary().size(ButtonSize::Middle),
                        ]},
                    ]},
                ]},
            ]},
        ]
    });
}

// ── Event Conversion ──

fn map_mouse_button(btn: PlatformMouseButton) -> WidgetMouseButton {
    match btn {
        PlatformMouseButton::None => WidgetMouseButton::None,
        PlatformMouseButton::Left => WidgetMouseButton::Left,
        PlatformMouseButton::Right => WidgetMouseButton::Right,
        PlatformMouseButton::Middle => WidgetMouseButton::Middle,
        PlatformMouseButton::X1 | PlatformMouseButton::X2 => WidgetMouseButton::None,
    }
}

fn map_key_code(key: PlatformKeyCode) -> WidgetKeyCode {
    match key {
        PlatformKeyCode::A => WidgetKeyCode::A,
        PlatformKeyCode::B => WidgetKeyCode::B,
        PlatformKeyCode::C => WidgetKeyCode::C,
        PlatformKeyCode::D => WidgetKeyCode::D,
        PlatformKeyCode::E => WidgetKeyCode::E,
        PlatformKeyCode::F => WidgetKeyCode::F,
        PlatformKeyCode::G => WidgetKeyCode::G,
        PlatformKeyCode::H => WidgetKeyCode::H,
        PlatformKeyCode::I => WidgetKeyCode::I,
        PlatformKeyCode::J => WidgetKeyCode::J,
        PlatformKeyCode::K => WidgetKeyCode::K,
        PlatformKeyCode::L => WidgetKeyCode::L,
        PlatformKeyCode::M => WidgetKeyCode::M,
        PlatformKeyCode::N => WidgetKeyCode::N,
        PlatformKeyCode::O => WidgetKeyCode::O,
        PlatformKeyCode::P => WidgetKeyCode::P,
        PlatformKeyCode::Q => WidgetKeyCode::Q,
        PlatformKeyCode::R => WidgetKeyCode::R,
        PlatformKeyCode::S => WidgetKeyCode::S,
        PlatformKeyCode::T => WidgetKeyCode::T,
        PlatformKeyCode::U => WidgetKeyCode::U,
        PlatformKeyCode::V => WidgetKeyCode::V,
        PlatformKeyCode::W => WidgetKeyCode::W,
        PlatformKeyCode::X => WidgetKeyCode::X,
        PlatformKeyCode::Y => WidgetKeyCode::Y,
        PlatformKeyCode::Z => WidgetKeyCode::Z,
        PlatformKeyCode::Num0 => WidgetKeyCode::Num0,
        PlatformKeyCode::Num1 => WidgetKeyCode::Num1,
        PlatformKeyCode::Num2 => WidgetKeyCode::Num2,
        PlatformKeyCode::Num3 => WidgetKeyCode::Num3,
        PlatformKeyCode::Num4 => WidgetKeyCode::Num4,
        PlatformKeyCode::Num5 => WidgetKeyCode::Num5,
        PlatformKeyCode::Num6 => WidgetKeyCode::Num6,
        PlatformKeyCode::Num7 => WidgetKeyCode::Num7,
        PlatformKeyCode::Num8 => WidgetKeyCode::Num8,
        PlatformKeyCode::Num9 => WidgetKeyCode::Num9,
        PlatformKeyCode::F1 => WidgetKeyCode::F1,
        PlatformKeyCode::F2 => WidgetKeyCode::F2,
        PlatformKeyCode::F3 => WidgetKeyCode::F3,
        PlatformKeyCode::F4 => WidgetKeyCode::F4,
        PlatformKeyCode::F5 => WidgetKeyCode::F5,
        PlatformKeyCode::F6 => WidgetKeyCode::F6,
        PlatformKeyCode::F7 => WidgetKeyCode::F7,
        PlatformKeyCode::F8 => WidgetKeyCode::F8,
        PlatformKeyCode::F9 => WidgetKeyCode::F9,
        PlatformKeyCode::F10 => WidgetKeyCode::F10,
        PlatformKeyCode::F11 => WidgetKeyCode::F11,
        PlatformKeyCode::F12 => WidgetKeyCode::F12,
        PlatformKeyCode::Up => WidgetKeyCode::Up,
        PlatformKeyCode::Down => WidgetKeyCode::Down,
        PlatformKeyCode::Left => WidgetKeyCode::Left,
        PlatformKeyCode::Right => WidgetKeyCode::Right,
        PlatformKeyCode::Home => WidgetKeyCode::Home,
        PlatformKeyCode::End => WidgetKeyCode::End,
        PlatformKeyCode::Enter => WidgetKeyCode::Enter,
        PlatformKeyCode::Escape => WidgetKeyCode::Escape,
        PlatformKeyCode::Backspace => WidgetKeyCode::Backspace,
        PlatformKeyCode::Delete => WidgetKeyCode::Delete,
        PlatformKeyCode::Tab => WidgetKeyCode::Tab,
        PlatformKeyCode::Space => WidgetKeyCode::Space,
        _ => WidgetKeyCode::Unknown,
    }
}

fn ui_event_to_widget_event(ev: &UiEvent) -> Option<WidgetEvent> {
    match ev.type_ {
        UiEventType::MouseDown => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(WidgetEvent::MouseDown { pos: d.pos, button: map_mouse_button(d.btn) })
            } else { None }
        }
        UiEventType::MouseUp => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(WidgetEvent::MouseUp { pos: d.pos, button: map_mouse_button(d.btn) })
            } else { None }
        }
        UiEventType::MouseMove => {
            if let UiEventPayload::MouseMove(ref d) = ev.payload {
                Some(WidgetEvent::MouseMove { pos: d.pos })
            } else { None }
        }
        UiEventType::MouseWheel => {
            if let UiEventPayload::MouseWheel(ref d) = ev.payload {
                Some(WidgetEvent::MouseWheel {
                    delta: uix::base::Point::new(d.delta_x, d.delta_y),
                })
            } else { None }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyDown { key: map_key_code(d.key) })
            } else { None }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyUp { key: map_key_code(d.key) })
            } else { None }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(WidgetEvent::Resize { width: d.width as f32, height: d.height as f32 })
            } else { None }
        }
        _ => None,
    }
}

/// Run the Wayland GUI demo using the platform directly.
pub fn run_gui_demo() {
    Logger::instance().set_level(Level::Info);
    info_fn("UIX Linux Demo starting...");

    let mut engine = SoftwareEngine::new();
    let mut platform = LinuxPlatform::new();

    // Create native Wayland window
    if !platform.create_window("UIX on Wayland", GW, GH) {
        eprintln!("Failed to create window");
        return;
    }

    if let Err(e) = engine.initialize(GW, GH) {
        eprintln!("Engine init failed: {}", e.short_what());
        return;
    }

    let tokens = DesignTokens::antd_light();

    // Build widget tree
    let mut tree = WidgetTree::new();
    build_dashboard(&mut tree);

    // ── Event Loop — first render happens inside the loop after processing
    // pending configure/resize events, ensuring the widget tree has correct
    // dimensions before rendering. ──
    let pending_events = std::cell::RefCell::new(Vec::<UiEvent>::new());
    let running = AtomicBool::new(true);
    let mut first_frame = true;

    // Track whether the FIRST render has happened — first render must
    // always use DirtyRegion::full() to fully initialize the framebuffer.
    let mut rendered_first_frame = false;

    while running.load(Ordering::Relaxed) {
        if first_frame {
            // First iteration: drain pending configure/resize events from
            // window creation without blocking, so the tree is configured.
            platform.poll_event(&|ev: &UiEvent| {
                pending_events.borrow_mut().push(ev.clone());
                true
            });
            first_frame = false;
        } else {
            // Subsequent iterations: wait for Wayland events (blocks when idle)
            let alive = platform.wait_event(&|ev: &UiEvent| {
                match ev.type_ {
                    UiEventType::WindowClose => {
                        running.store(false, Ordering::Relaxed);
                        return false;
                    }
                    UiEventType::KeyDown => {
                        if let UiEventPayload::Key(ref d) = ev.payload {
                            if d.key == PlatformKeyCode::Escape {
                                running.store(false, Ordering::Relaxed);
                                return false;
                            }
                        }
                    }
                    _ => {}
                }
                pending_events.borrow_mut().push(ev.clone());
                true
            });

            if !alive {
                break;
            }

            // Drain any remaining events
            platform.poll_event(&|ev: &UiEvent| {
                pending_events.borrow_mut().push(ev.clone());
                true
            });
        }

        // Dispatch events to widget tree
        for ev in pending_events.borrow_mut().drain(..) {
            if let Some(we) = ui_event_to_widget_event(&ev) {
                tree.dispatch_event(&we);
            }
        }

        // Determine render region:
        // - First frame always uses full clear to initialize framebuffer
        // - Subsequent frames use incremental dirty region when available
        let region = if !rendered_first_frame || tree.dirty_region().full_frame {
            DirtyRegion::full()
        } else {
            tree.dirty_region().clone()
        };
        engine.begin_frame(&region);
        tree.layout();
        let mut rctx = RenderContext::new(&mut engine, uix::graphics::FontHandle::default(), &tokens);
        tree.render_tree(&mut rctx);
        engine.end_frame(&region);
        tree.reset_dirty();
        rendered_first_frame = true;

        // Present to Wayland surface
        platform.present_pixels(engine.pixels(), GW, GH);
    }

    engine.shutdown();
    platform.destroy_window();
    info_fn("UIX Wayland Demo exited.");
}
