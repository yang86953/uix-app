//! 平台公开面的窗口契约 — 窗口、输入值、事件、剪贴板与事件循环。
//!
//! 本模块是 platform System 私有边界对 ui/draw/app 的公开输入契约收口：
//! ui/app 层经 `crate::platform::windowing` 消费输入值、输入服务、
//! `IClipboard` 与事件协议，不再直接引用 `crate::native` 私有实现文件。

// 平台无关事件契约与纯分发逻辑集中在 windowing 的 event 边界。
pub(crate) mod event;
// 窗口可选能力值由中立叶唯一持有，禁止后端自定义平行布尔合同。
mod capability;
// 光标、键盘与文本输入的中立协议由独立叶模块唯一持有。
pub(crate) mod input;
// UI 线程策略属于 windowing 域，根模块仅再导出稳定入口。
pub(crate) mod ui_thread;
// 中立窗口生命周期、属性与帧回调协议由独立叶模块唯一持有。
pub(crate) mod window;

use crate::core::error::Result;

// 保留既有公开唤醒句柄路径；唯一源码定义位于 event 子模块。
pub use capability::{WindowCapabilities, WindowCapability};
pub use event::EventLoopWaker;
// 聚合层与实现统一消费同一组输入合同。
pub(crate) use input::{ICursor, ITextInput};

// ════════════════════════════════════════════════════════════════════════════
// 鼠标按钮 — 跨层共享
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
/// 平台无关的指针按钮标识。
pub enum MouseButton {
    /// 未指定按钮。
    None,
    /// 主按钮，通常为左键。
    Left,
    /// 次按钮，通常为右键。
    Right,
    /// 中间按钮。
    Middle,
    /// 第一个扩展按钮。
    X1,
    /// 第二个扩展按钮。
    X2,
}

// ════════════════════════════════════════════════════════════════════════════
// 光标类型
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
/// 平台无关的指针光标形状。
pub enum CursorType {
    /// 默认箭头光标。
    Arrow,
    /// 文本输入光标。
    IBeam,
    /// 十字准星光标。
    Crosshair,
    /// 可点击手形光标。
    Hand,
    /// 水平调整尺寸光标。
    ResizeH,
    /// 垂直调整尺寸光标。
    ResizeV,
    /// 东北到西南方向调整尺寸光标。
    ResizeNE,
    /// 西北到东南方向调整尺寸光标。
    ResizeNW,
    /// 移动光标。
    Move,
    /// 等待光标。
    Wait,
    /// 禁止操作光标。
    NotAllowed,
    /// 平台适配器提供的自定义光标。
    Custom,
}

// ════════════════════════════════════════════════════════════════════════════
// 窗口调整大小边 — 自定义非客户区交互
// ════════════════════════════════════════════════════════════════════════════

/// 平台无关的窗口调整大小方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowResizeEdge {
    /// 从窗口上边调整大小。
    Top,
    /// 从窗口下边调整大小。
    Bottom,
    /// 从窗口左边调整大小。
    Left,
    /// 从窗口右边调整大小。
    Right,
    /// 从窗口左上角调整大小。
    TopLeft,
    /// 从窗口右上角调整大小。
    TopRight,
    /// 从窗口左下角调整大小。
    BottomLeft,
    /// 从窗口右下角调整大小。
    BottomRight,
}

// ════════════════════════════════════════════════════════════════════════════
// 按键码 — 跨层共享（平台输入 → UI 事件）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
/// 平台键盘输入归一化后的按键码。
pub enum KeyCode {
    /// 无法识别的按键。
    Unknown = 0,
    /// 字母 A 键。
    A,
    /// 字母 B 键。
    B,
    /// 字母 C 键。
    C,
    /// 字母 D 键。
    D,
    /// 字母 E 键。
    E,
    /// 字母 F 键。
    F,
    /// 字母 G 键。
    G,
    /// 字母 H 键。
    H,
    /// 字母 I 键。
    I,
    /// 字母 J 键。
    J,
    /// 字母 K 键。
    K,
    /// 字母 L 键。
    L,
    /// 字母 M 键。
    M,
    /// 字母 N 键。
    N,
    /// 字母 O 键。
    O,
    /// 字母 P 键。
    P,
    /// 字母 Q 键。
    Q,
    /// 字母 R 键。
    R,
    /// 字母 S 键。
    S,
    /// 字母 T 键。
    T,
    /// 字母 U 键。
    U,
    /// 字母 V 键。
    V,
    /// 字母 W 键。
    W,
    /// 字母 X 键。
    X,
    /// 字母 Y 键。
    Y,
    /// 字母 Z 键。
    Z,
    /// 数字 0 键。
    Num0,
    /// 数字 1 键。
    Num1,
    /// 数字 2 键。
    Num2,
    /// 数字 3 键。
    Num3,
    /// 数字 4 键。
    Num4,
    /// 数字 5 键。
    Num5,
    /// 数字 6 键。
    Num6,
    /// 数字 7 键。
    Num7,
    /// 数字 8 键。
    Num8,
    /// 数字 9 键。
    Num9,
    /// 功能键 F1。
    F1,
    /// 功能键 F2。
    F2,
    /// 功能键 F3。
    F3,
    /// 功能键 F4。
    F4,
    /// 功能键 F5。
    F5,
    /// 功能键 F6。
    F6,
    /// 功能键 F7。
    F7,
    /// 功能键 F8。
    F8,
    /// 功能键 F9。
    F9,
    /// 功能键 F10。
    F10,
    /// 功能键 F11。
    F11,
    /// 功能键 F12。
    F12,
    /// 向上方向键。
    Up,
    /// 向下方向键。
    Down,
    /// 向左方向键。
    Left,
    /// 向右方向键。
    Right,
    /// 行首或列表起始键。
    Home,
    /// 行尾或列表结束键。
    End,
    /// 向上翻页键。
    PageUp,
    /// 向下翻页键。
    PageDown,
    /// 回车键。
    Enter,
    /// 退出键。
    Escape,
    /// 退格键。
    Backspace,
    /// 向前删除键。
    Delete,
    /// Tab 导航键。
    Tab,
    /// 空格键。
    Space,
    /// 插入键。
    Insert,
    /// Shift 修饰键。
    Shift,
    /// Control 修饰键。
    Ctrl,
    /// Alt 修饰键。
    Alt,
    /// 系统或 Command 修饰键。
    Super,
}

// ════════════════════════════════════════════════════════════════════════════
// 修饰键掩码 — 跨层共享
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// 可组合的平台键盘修饰键掩码。
pub struct KeyMod(u32);

#[allow(non_upper_case_globals)]
impl KeyMod {
    /// 不包含任何修饰键。
    pub const NONE: Self = Self(0);
    /// Shift 修饰键位。
    pub const SHIFT: Self = Self(1 << 0);
    /// Control 修饰键位。
    pub const CTRL: Self = Self(1 << 1);
    /// Alt 修饰键位。
    pub const ALT: Self = Self(1 << 2);
    /// 系统或 Command 修饰键位。
    pub const SUPER: Self = Self(1 << 3);
    /// 框架内部合成的按键事件；平台 backend 不得设置。
    pub(crate) const SYNTHETIC: Self = Self(1 << 31);

    /// 判断是否完整包含指定修饰键集合。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// 判断是否与指定修饰键集合存在任意交集。
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for KeyMod {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for KeyMod {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 控件尺寸与滚动方向
// ════════════════════════════════════════════════════════════════════════════

/// 通用控件尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSize {
    /// 小尺寸控件。
    Small,
    /// 中等尺寸控件。
    Medium,
    /// 大尺寸控件。
    Large,
}

/// 滚动方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// 仅纵向滚动。
    Vertical,
    /// 仅横向滚动。
    Horizontal,
    /// 双向滚动。
    Both,
}

impl ScrollDirection {
    /// 判断滚动容器是否允许横向滚动。
    pub fn can_scroll_x(&self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    /// 判断滚动容器是否允许纵向滚动。
    pub fn can_scroll_y(&self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 剪贴板契约
// ════════════════════════════════════════════════════════════════════════════

/// 平台剪贴板文本读写契约。
pub trait IClipboard {
    /// 读取当前剪贴板文本。
    fn text(&self) -> Result<String>;
    /// 替换当前剪贴板文本。
    fn set_text(&mut self, text: &str) -> Result<()>;
    /// 判断剪贴板是否包含可读取文本。
    fn has_text(&self) -> Result<bool>;
}
