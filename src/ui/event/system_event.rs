//! 应用边界后的系统事件载荷与种类映射。

// 引入系统事件使用的核心坐标值。
use crate::core::Point;
// 引入系统事件使用的平台输入值。
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};
// 引入父 Event Module 唯一拥有的事件种类契约。
use super::SystemEventKind;

#[derive(Debug, Clone)]
/// 应用边界后的系统事件。
pub enum SystemEvent {
    /// 指针按下事件。
    PointerDown {
        /// 事件发生位置。
        pos: Point,
        /// 触发的鼠标按钮。
        button: MouseButton,
        /// 按下时持有的修饰键。
        mods: KeyMod,
    },
    /// 指针双击事件。
    PointerDoubleClick {
        /// 事件发生位置。
        pos: Point,
        /// 触发的鼠标按钮。
        button: MouseButton,
        /// 双击时持有的修饰键。
        mods: KeyMod,
    },
    /// 指针抬起事件。
    PointerUp {
        /// 事件发生位置。
        pos: Point,
        /// 抬起的鼠标按钮。
        button: MouseButton,
        /// 抬起时持有的修饰键。
        mods: KeyMod,
    },
    /// 指针移动事件。
    PointerMove {
        /// 事件发生位置。
        pos: Point,
        /// 移动时持有的修饰键。
        mods: KeyMod,
    },
    /// 滚轮滚动事件。
    Wheel {
        /// 事件发生位置。
        pos: Point,
        /// 相对上一事件的滚动位移。
        delta: Point,
    },
    /// 按键按下事件。
    KeyDown {
        /// 触发的按键。
        key: KeyCode,
        /// 按下时持有的修饰键。
        mods: KeyMod,
    },
    /// 按键抬起事件。
    KeyUp {
        /// 抬起的按键。
        key: KeyCode,
        /// 抬起时持有的修饰键。
        mods: KeyMod,
    },
    /// 复制请求事件。
    Copy,
    /// 剪切请求事件。
    Cut,
    /// 粘贴请求事件。
    Paste {
        /// 待粘贴的文本。
        text: String,
    },
    /// 文本输入事件。
    TextInput {
        /// 输入文本。
        text: String,
    },
    /// 输入法组合开始事件。
    ImeCompositionStart,
    /// 输入法组合更新事件。
    ImeCompositionUpdate {
        /// 当前组合文本。
        text: String,
    },
    /// 输入法组合结束事件。
    ImeCompositionEnd {
        /// 组合确认后的最终文本。
        text: String,
    },
    /// 获得焦点事件。
    FocusIn,
    /// 失去焦点事件。
    FocusOut,
    /// 指针进入事件。
    PointerEnter,
    /// 指针离开事件。
    PointerLeave,
    /// 主题切换事件。
    ThemeChanged {
        /// 是否为深色主题。
        is_dark: bool,
    },
    /// 语言区域切换事件。
    LocaleChanged {
        /// 新的语言区域标识。
        locale: String,
    },
    /// 窗口尺寸变化事件。
    Resize {
        /// 新的窗口宽度。
        width: f32,
        /// 新的窗口高度。
        height: f32,
    },
    /// 窗口最大化事件。
    WindowMaximize,
    /// 窗口最小化事件。
    WindowMinimize,
    /// 窗口还原事件。
    WindowRestore,
    /// 窗口获得焦点事件。
    WindowFocus,
    /// 窗口失去焦点事件。
    WindowBlur,
    /// 定时器到期事件。
    Timer {
        /// 定时器标识。
        id: u32,
    },
    /// 文件拖放事件。
    FileDrop {
        /// 拖入的文件路径列表。
        files: Vec<String>,
        /// 拖放落点位置。
        position: Point,
    },
    /// 拖拽开始事件。
    DragStart {
        /// 事件发生位置。
        pos: Point,
        /// 拖拽按钮。
        button: MouseButton,
        /// 拖拽时持有的修饰键。
        mods: KeyMod,
    },
    /// 拖拽移动事件。
    DragMove {
        /// 当前拖拽位置。
        pos: Point,
        /// 相对上一事件的位移。
        delta: Point,
        /// 拖拽时持有的修饰键。
        mods: KeyMod,
    },
    /// 拖拽结束事件。
    DragEnd {
        /// 结束位置。
        pos: Point,
        /// 拖拽按钮。
        button: MouseButton,
        /// 结束时持有的修饰键。
        mods: KeyMod,
    },
}

// 为系统事件提供不复制载荷的种类查询。
impl SystemEvent {
    /// 返回事件对应的种类标签，用于分类匹配。
    pub fn kind(&self) -> SystemEventKind {
        // 每个载荷变体映射到唯一的稳定种类。
        match self {
            SystemEvent::PointerDown { .. } => SystemEventKind::PointerDown,
            SystemEvent::PointerDoubleClick { .. } => SystemEventKind::PointerDoubleClick,
            SystemEvent::PointerUp { .. } => SystemEventKind::PointerUp,
            SystemEvent::PointerMove { .. } => SystemEventKind::PointerMove,
            SystemEvent::Wheel { .. } => SystemEventKind::Wheel,
            SystemEvent::KeyDown { .. } => SystemEventKind::KeyDown,
            SystemEvent::KeyUp { .. } => SystemEventKind::KeyUp,
            SystemEvent::Copy => SystemEventKind::Copy,
            SystemEvent::Cut => SystemEventKind::Cut,
            SystemEvent::Paste { .. } => SystemEventKind::Paste,
            SystemEvent::TextInput { .. } => SystemEventKind::TextInput,
            SystemEvent::ImeCompositionStart => SystemEventKind::ImeCompositionStart,
            SystemEvent::ImeCompositionUpdate { .. } => SystemEventKind::ImeCompositionUpdate,
            SystemEvent::ImeCompositionEnd { .. } => SystemEventKind::ImeCompositionEnd,
            SystemEvent::FocusIn => SystemEventKind::FocusIn,
            SystemEvent::FocusOut => SystemEventKind::FocusOut,
            SystemEvent::PointerEnter => SystemEventKind::PointerEnter,
            SystemEvent::PointerLeave => SystemEventKind::PointerLeave,
            SystemEvent::ThemeChanged { .. } => SystemEventKind::ThemeChanged,
            SystemEvent::LocaleChanged { .. } => SystemEventKind::LocaleChanged,
            SystemEvent::Resize { .. } => SystemEventKind::Resize,
            SystemEvent::WindowMaximize => SystemEventKind::WindowMaximize,
            SystemEvent::WindowMinimize => SystemEventKind::WindowMinimize,
            SystemEvent::WindowRestore => SystemEventKind::WindowRestore,
            SystemEvent::WindowFocus => SystemEventKind::WindowFocus,
            SystemEvent::WindowBlur => SystemEventKind::WindowBlur,
            SystemEvent::Timer { .. } => SystemEventKind::Timer,
            SystemEvent::FileDrop { .. } => SystemEventKind::FileDrop,
            SystemEvent::DragStart { .. } => SystemEventKind::DragStart,
            SystemEvent::DragMove { .. } => SystemEventKind::DragMove,
            SystemEvent::DragEnd { .. } => SystemEventKind::DragEnd,
        }
    }
}
