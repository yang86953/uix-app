use super::*;
// 显式引入平台无关按键码，避免依赖父模块的旧重导出。
use crate::platform::windowing::KeyCode;

// 保留拆分前 MacosAppEvent 的诊断与复制能力。
#[derive(Debug, Clone)]
pub(super) struct MacosAppEvent {
    // 窗口身份供平台根模块执行文本输入抑制判定。
    pub(super) window_id: Option<WindowId>,
    // 原生事件种类供本组件转换 UIX 事件。
    pub(super) kind: isize,
    // 原生坐标供指针事件转换使用。
    pub(super) location: crate::core::Point,
    // 原生鼠标按钮编号供按钮映射使用。
    pub(super) button_number: isize,
    // 水平滚动增量供滚轮事件转换使用。
    pub(super) delta_x: f64,
    // 垂直滚动增量供滚轮事件转换使用。
    pub(super) delta_y: f64,
    // 原生键码供键盘事件映射使用。
    pub(super) key_code: u16,
    // 原生修饰键位图供键盘事件映射使用。
    pub(super) modifiers: usize,
    // 原生文本载荷供文本输入事件转换使用。
    pub(super) text: String,
}
impl MacosAppEvent {
    // 将 AppKit 传输对象转换为平台中立 UIX 事件。
    pub(super) fn into_ui_events(self, suppress_keydown_text: bool) -> Vec<UiEvent> {
        let Some(window_id) = self.window_id else {
            return Vec::new();
        };
        let events = match self.kind {
            cocoa::NSEVENT_TYPE_LEFT_MOUSE_DOWN => {
                vec![UiEvent::pointer_down(self.location, MouseButton::Left)]
            }
            cocoa::NSEVENT_TYPE_LEFT_MOUSE_UP => {
                vec![UiEvent::pointer_up(self.location, MouseButton::Left)]
            }
            cocoa::NSEVENT_TYPE_RIGHT_MOUSE_DOWN => {
                vec![UiEvent::pointer_down(self.location, MouseButton::Right)]
            }
            cocoa::NSEVENT_TYPE_RIGHT_MOUSE_UP => {
                vec![UiEvent::pointer_up(self.location, MouseButton::Right)]
            }
            cocoa::NSEVENT_TYPE_OTHER_MOUSE_DOWN => {
                vec![UiEvent::pointer_down(
                    self.location,
                    macos_button_to_mouse_button(self.button_number),
                )]
            }
            cocoa::NSEVENT_TYPE_OTHER_MOUSE_UP => {
                vec![UiEvent::pointer_up(
                    self.location,
                    macos_button_to_mouse_button(self.button_number),
                )]
            }
            cocoa::NSEVENT_TYPE_MOUSE_MOVED
            | cocoa::NSEVENT_TYPE_LEFT_MOUSE_DRAGGED
            | cocoa::NSEVENT_TYPE_RIGHT_MOUSE_DRAGGED
            | cocoa::NSEVENT_TYPE_OTHER_MOUSE_DRAGGED => vec![UiEvent::pointer_move(self.location)],
            cocoa::NSEVENT_TYPE_SCROLL_WHEEL => vec![UiEvent::wheel(
                self.location,
                -(self.delta_x as f32) / 120.0,
                -(self.delta_y as f32) / 120.0,
                macos_mods_to_key_mod(self.modifiers),
            )],
            cocoa::NSEVENT_TYPE_KEY_DOWN => {
                let mut events = vec![UiEvent::key_down(
                    macos_keycode_to_keycode(self.key_code),
                    macos_mods_to_key_mod(self.modifiers),
                )];
                if !suppress_keydown_text && is_text_input_payload(&self.text) {
                    events.push(UiEvent::text_input(self.text));
                }
                events
            }
            cocoa::NSEVENT_TYPE_KEY_UP => vec![UiEvent::key_up(
                macos_keycode_to_keycode(self.key_code),
                macos_mods_to_key_mod(self.modifiers),
            )],
            _ => Vec::new(),
        };
        events
            .into_iter()
            .map(|event| event.for_window(window_id))
            .collect()
    }
}

fn is_text_input_payload(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|ch| !ch.is_control())
}

fn macos_button_to_mouse_button(button: isize) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::X1,
        4 => MouseButton::X2,
        _ => MouseButton::None,
    }
}

fn macos_mods_to_key_mod(modifiers: usize) -> KeyMod {
    let mut mods = KeyMod::NONE;
    if modifiers & cocoa::NSEVENT_MODIFIER_FLAG_SHIFT != 0 {
        mods |= KeyMod::SHIFT;
    }
    if modifiers & cocoa::NSEVENT_MODIFIER_FLAG_CONTROL != 0 {
        mods |= KeyMod::CTRL;
    }
    if modifiers & cocoa::NSEVENT_MODIFIER_FLAG_OPTION != 0 {
        mods |= KeyMod::ALT;
    }
    if modifiers & cocoa::NSEVENT_MODIFIER_FLAG_COMMAND != 0 {
        mods |= KeyMod::SUPER;
    }
    mods
}

fn macos_keycode_to_keycode(code: u16) -> KeyCode {
    match code {
        0 => KeyCode::A,
        1 => KeyCode::S,
        2 => KeyCode::D,
        3 => KeyCode::F,
        4 => KeyCode::H,
        5 => KeyCode::G,
        6 => KeyCode::Z,
        7 => KeyCode::X,
        8 => KeyCode::C,
        9 => KeyCode::V,
        11 => KeyCode::B,
        12 => KeyCode::Q,
        13 => KeyCode::W,
        14 => KeyCode::E,
        15 => KeyCode::R,
        16 => KeyCode::Y,
        17 => KeyCode::T,
        18 => KeyCode::Num1,
        19 => KeyCode::Num2,
        20 => KeyCode::Num3,
        21 => KeyCode::Num4,
        22 => KeyCode::Num6,
        23 => KeyCode::Num5,
        25 => KeyCode::Num9,
        26 => KeyCode::Num7,
        28 => KeyCode::Num8,
        29 => KeyCode::Num0,
        31 => KeyCode::O,
        32 => KeyCode::U,
        34 => KeyCode::I,
        35 => KeyCode::P,
        36 => KeyCode::Enter,
        37 => KeyCode::L,
        38 => KeyCode::J,
        40 => KeyCode::K,
        45 => KeyCode::N,
        46 => KeyCode::M,
        48 => KeyCode::Tab,
        49 => KeyCode::Space,
        51 => KeyCode::Backspace,
        53 => KeyCode::Escape,
        55 => KeyCode::Super,
        56 | 60 => KeyCode::Shift,
        58 | 61 => KeyCode::Alt,
        59 | 62 => KeyCode::Ctrl,
        114 => KeyCode::Insert,
        115 => KeyCode::Home,
        116 => KeyCode::PageUp,
        117 => KeyCode::Delete,
        119 => KeyCode::End,
        121 => KeyCode::PageDown,
        122 => KeyCode::F1,
        120 => KeyCode::F2,
        99 => KeyCode::F3,
        118 => KeyCode::F4,
        96 => KeyCode::F5,
        97 => KeyCode::F6,
        98 => KeyCode::F7,
        100 => KeyCode::F8,
        101 => KeyCode::F9,
        109 => KeyCode::F10,
        103 => KeyCode::F11,
        111 => KeyCode::F12,
        123 => KeyCode::Left,
        124 => KeyCode::Right,
        125 => KeyCode::Down,
        126 => KeyCode::Up,
        _ => KeyCode::Unknown,
    }
}
