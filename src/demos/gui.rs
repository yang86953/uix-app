//! GUI Dashboard Demo — Ant Design 5 style interface using all UI components.

use std::cell::{Cell, RefCell};
use std::time::Instant;
use uix::graphics::{Color, FontHandle, GraphicsEngine, SoftwareEngine};
use uix::platform::event::{UiEvent, UiEventPayload, UiEventType};
use uix::platform::log::info_fn;
use uix::platform::types::{KeyCode as PlatformKeyCode, MouseButton as PlatformMouseButton};
use uix::platform::win32::Win32Platform;
use uix::platform::Platform;
use uix::ui::theme::DesignTokens;
use uix::ui::{
    AlignItems, Button, ButtonSize, Card, Container, Divider, DividerOrientation, FlexDirection,
    Input, InputSize, JustifyContent, KeyCode as WidgetKeyCode, Label,
    MouseButton as WidgetMouseButton, RenderContext, Space, SpaceSize, WidgetEvent, WidgetTree,
};

const GW: i32 = 1100;
const GH: i32 = 740;

/// Build the complete widget tree for the dashboard.
fn build_dashboard(tree: &mut WidgetTree) {
    let t = DesignTokens::antd_light();

    let root = Container::new()
        .size(GW as f32, GH as f32)
        .bg(t.color_bg_layout)
        .dir(FlexDirection::Row);
    let root_id = tree.set_root(Box::new(root));

    // ── Sidebar ──
    let sidebar = Container::new()
        .size(220.0, GH as f32)
        .bg(t.color_bg_elevated)
        .dir(FlexDirection::Column);
    let sidebar_id = tree.add_child(root_id, Box::new(sidebar));

    let brand = Label::new("UIX  Framework", t.color_primary)
        .font_size(20.0)
        .size(220.0, 56.0);
    tree.add_child(sidebar_id, Box::new(brand));

    let sd = Divider::new().color(t.color_border_secondary);
    tree.add_child(sidebar_id, Box::new(sd));

    let nav_items: [(&str, bool); 5] = [
        (" Dashboard", true),
        (" Widgets", false),
        (" Charts", false),
        (" Settings", false),
        (" About", false),
    ];
    for (label, active) in &nav_items {
        let color = if *active {
            t.color_primary
        } else {
            t.color_text_secondary
        };
        let bg_nav = if *active {
            t.color_primary_bg
        } else {
            Color::transparent()
        };
        let item_bg = Container::new()
            .size(220.0, 40.0)
            .dir(FlexDirection::Row)
            .bg(bg_nav);
        let item_id = tree.add_child(sidebar_id, Box::new(item_bg));

        if *active {
            let indicator = Container::new().size(3.0, 40.0).bg(t.color_primary);
            tree.add_child(item_id, Box::new(indicator));
            let sp = Container::new().size(12.0, 40.0);
            tree.add_child(item_id, Box::new(sp));
        } else {
            let sp = Container::new().size(15.0, 40.0);
            tree.add_child(item_id, Box::new(sp));
        }

        let nav_lbl = Label::new(label, color).font_size(14.0).size(180.0, 40.0);
        tree.add_child(item_id, Box::new(nav_lbl));
    }

    let sp = Container::new().size(220.0, GH as f32 - 240.0);
    tree.add_child(sidebar_id, Box::new(sp));

    let ver = Label::new("UIX v0.1.0 — Rust Native", t.color_text_quaternary)
        .font_size(11.0)
        .size(220.0, 28.0);
    tree.add_child(sidebar_id, Box::new(ver));

    // ── Main Content ──
    let content_w = GW as f32 - 220.0;
    let content = Container::new()
        .size(content_w, GH as f32)
        .bg(t.color_bg_container)
        .dir(FlexDirection::Column);
    let content_id = tree.add_child(root_id, Box::new(content));

    let titlebar = Container::new()
        .size(content_w, 48.0)
        .bg(t.color_bg_elevated)
        .dir(FlexDirection::Row);
    let titlebar_id = tree.add_child(content_id, Box::new(titlebar));

    let title_label = Label::new("  Dashboard", t.color_text)
        .font_size(16.0)
        .size(400.0, 48.0);
    tree.add_child(titlebar_id, Box::new(title_label));

    let tb_spacer = Container::new().size(content_w - 496.0, 48.0);
    tree.add_child(titlebar_id, Box::new(tb_spacer));

    let close_btn = Container::new().size(48.0, 48.0).bg(t.color_error);
    let close_id = tree.add_child(titlebar_id, Box::new(close_btn));
    let close_lbl = Label::new("✕", t.color_white)
        .font_size(16.0)
        .size(48.0, 48.0);
    tree.add_child(close_id, Box::new(close_lbl));

    let tb_div = Divider::new().color(t.color_border_secondary);
    tree.add_child(content_id, Box::new(tb_div));

    let body = Container::new()
        .size(content_w, GH as f32 - 48.0 - 1.0)
        .dir(FlexDirection::Column);
    let body_id = tree.add_child(content_id, Box::new(body));

    let stats_w = content_w - 48.0;

    let section_header = Label::new("Overview", t.color_text)
        .font_size(18.0)
        .size(200.0, 28.0);
    tree.add_child(body_id, Box::new(section_header));

    let section_desc = Label::new("Key metrics at a glance", t.color_text_tertiary)
        .font_size(12.0)
        .size(300.0, 18.0);
    tree.add_child(body_id, Box::new(section_desc));

    let vs1 = Container::new().size(stats_w, 8.0);
    tree.add_child(body_id, Box::new(vs1));

    let stat_data: [(&str, &str, Color, u8); 4] = [
        ("Total Users", "1,234", t.color_primary, 2),
        ("Revenue", "$8,291", t.color_success, 1),
        ("Orders", "89", t.color_warning, 1),
        ("Growth", "12.5%", t.color_info, 1),
    ];

    let card_w = (stats_w / 4.0) - 10.0;
    let card_h = 110.0;

    let stat_row = Space::new()
        .size(SpaceSize::Custom(10.0))
        .width(stats_w)
        .height(card_h)
        .justify(JustifyContent::Start)
        .align(AlignItems::Stretch);

    let mut stat_row = stat_row;
    for (title, value, stat_color, elevation) in &stat_data {
        let card = Card::new()
            .title(title)
            .elevation(*elevation)
            .hoverable()
            .size(card_w, card_h)
            .child(Label::new(*value, *stat_color).font_size(28.0));
        stat_row = stat_row.child(card);
    }
    tree.add_child(body_id, Box::new(stat_row));

    let vs2 = Container::new().size(stats_w, 24.0);
    tree.add_child(body_id, Box::new(vs2));

    let btn_section_header = Label::new("Buttons", t.color_text)
        .font_size(16.0)
        .size(200.0, 24.0);
    tree.add_child(body_id, Box::new(btn_section_header));

    let btn_section_desc = Label::new(
        "Six button variants following Ant Design 5",
        t.color_text_tertiary,
    )
    .font_size(12.0)
    .size(400.0, 18.0);
    tree.add_child(body_id, Box::new(btn_section_desc));

    let vs3 = Container::new().size(stats_w, 12.0);
    tree.add_child(body_id, Box::new(vs3));

    let btn_row = Space::new()
        .size(SpaceSize::Custom(10.0))
        .width(stats_w)
        .height(40.0)
        .align(AlignItems::Center);
    let btn_row_id = tree.add_child(body_id, Box::new(btn_row));

    let btn_data = [
        "Primary", "Default", "Dashed", "TextBtn", "LinkBtn", "Disabled",
    ];
    let btn_variants: [fn(&str) -> Button; 6] = [
        |t| Button::new(t).primary(),
        |t| Button::new(t),
        |t| Button::new(t).dashed(),
        |t| Button::new(t).text(),
        |t| Button::new(t).link(),
        |t| Button::new(t).disabled(true),
    ];
    for (i, label) in btn_data.iter().enumerate() {
        tree.add_child(
            btn_row_id,
            Box::new(btn_variants[i](label).size(ButtonSize::Middle)),
        );
    }

    let vs4 = Container::new().size(stats_w, 16.0);
    tree.add_child(body_id, Box::new(vs4));

    let div_form = Divider::new()
        .with_text("Form Controls")
        .orientation(DividerOrientation::Left)
        .color(t.color_border);
    tree.add_child(body_id, Box::new(div_form));

    let form_desc = Label::new(
        "Input fields and submission controls",
        t.color_text_tertiary,
    )
    .font_size(12.0)
    .size(400.0, 18.0);
    tree.add_child(body_id, Box::new(form_desc));

    let vs5 = Container::new().size(stats_w, 12.0);
    tree.add_child(body_id, Box::new(vs5));

    let form_row = Space::new()
        .size(SpaceSize::Custom(10.0))
        .width(stats_w)
        .height(50.0)
        .align(AlignItems::Center);
    let form_id = tree.add_child(body_id, Box::new(form_row));

    let input = Input::new("Search or enter command...").size(InputSize::Middle);
    tree.add_child(form_id, Box::new(input));

    let submit = Button::new("Submit").primary().size(ButtonSize::Middle);
    tree.add_child(form_id, Box::new(submit));

    let vs6 = Container::new().size(stats_w, 16.0);
    tree.add_child(body_id, Box::new(vs6));

    let div_end = Divider::new().color(t.color_border_secondary);
    tree.add_child(body_id, Box::new(div_end));

    let vs7 = Container::new().size(stats_w, 8.0);
    tree.add_child(body_id, Box::new(vs7));

    let info_card = Card::new()
        .title("Welcome to UIX Framework")
        .elevation(1)
        .hoverable()
        .size(stats_w, 150.0)
        .child(
            Label::new(
                "UIX is a native Rust UI framework for Windows. \
             This dashboard demonstrates the Ant Design 5 themed \
             widget set including Container, Label, Button, Input, \
             Card, Divider, and Space components.",
                t.color_text_secondary,
            )
            .font_size(13.0),
        );
    tree.add_child(body_id, Box::new(info_card));
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
                Some(WidgetEvent::MouseDown {
                    pos: d.pos,
                    button: map_mouse_button(d.btn),
                })
            } else {
                None
            }
        }
        UiEventType::MouseUp => {
            if let UiEventPayload::MouseButton(ref d) = ev.payload {
                Some(WidgetEvent::MouseUp {
                    pos: d.pos,
                    button: map_mouse_button(d.btn),
                })
            } else {
                None
            }
        }
        UiEventType::MouseMove => {
            if let UiEventPayload::MouseMove(ref d) = ev.payload {
                Some(WidgetEvent::MouseMove { pos: d.pos })
            } else {
                None
            }
        }
        UiEventType::MouseWheel => {
            if let UiEventPayload::MouseWheel(ref d) = ev.payload {
                Some(WidgetEvent::MouseWheel {
                    delta: uix::graphics::Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyDown {
                    key: map_key_code(d.key),
                })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(WidgetEvent::KeyUp {
                    key: map_key_code(d.key),
                })
            } else {
                None
            }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(WidgetEvent::Resize {
                    width: d.width as f32,
                    height: d.height as f32,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn run_gui_demo() {
    let mut tree = WidgetTree::new();
    build_dashboard(&mut tree);

    let mut engine = SoftwareEngine::new();
    let mut platform = Win32Platform::new();

    if !platform.create_window("UIX Dashboard — Ant Design 5", GW, GH) {
        eprintln!("Failed to create window");
        return;
    }
    platform.center_on_screen();
    platform.show();
    let hwnd = platform.native_window();
    if hwnd.is_null() {
        eprintln!("Native window handle is null");
        platform.destroy_window();
        return;
    }
    if let Err(e) = engine.initialize(hwnd, GW, GH) {
        eprintln!("Engine init failed: {}", e.short_what());
        platform.destroy_window();
        return;
    }

    // 加载字体用于文字渲染。
    let font_handle = match engine.load_font("C:\\Windows\\Fonts\\segoeui.ttf", 14.0) {
        Ok(f) => *f,
        Err(e) => {
            eprintln!(
                "Warning: font load failed ({}), text will be invisible",
                e.short_what()
            );
            FontHandle::default()
        }
    };

    let running = Cell::new(true);
    let pending_events: RefCell<Vec<UiEvent>> = RefCell::new(Vec::new());
    let target_frame_time = std::time::Duration::from_secs_f64(1.0 / 60.0);

    while running.get() {
        let frame_start = Instant::now();

        platform.poll_event(&|ev: &UiEvent| {
            match ev.type_ {
                UiEventType::WindowClose => {
                    running.set(false);
                    return false;
                }
                UiEventType::KeyDown => {
                    if let UiEventPayload::Key(ref d) = ev.payload {
                        if d.key == PlatformKeyCode::Escape {
                            running.set(false);
                            return false;
                        }
                    }
                }
                _ => {}
            }
            pending_events.borrow_mut().push(ev.clone());
            true
        });

        if !running.get() {
            break;
        }

        for ev in pending_events.borrow_mut().drain(..) {
            if let Some(we) = ui_event_to_widget_event(&ev) {
                tree.dispatch_event(&we);
            }
        }

        let dirty = tree.dirty_region().clone();
        engine.begin_frame(&dirty);

        tree.layout();

        let mut rctx = RenderContext::new(&mut engine, font_handle);
        tree.render_tree(&mut rctx);

        engine.end_frame(&dirty);
        tree.reset_dirty();

        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed);
        }
    }

    engine.shutdown();
    platform.destroy_window();
    info_fn("UIX Dashboard Demo exited.");
}
