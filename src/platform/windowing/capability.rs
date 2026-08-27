//! 平台中立的窗口可选能力值与集合。

use crate::core::{Errc, Error};

/// 可在调用前查询的单个窗口可选能力。
///
/// 每个值对应一个公开窗口操作或一个具有 fallback 的精确查询，不携带 OS 身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WindowCapability {
    RequestClose,
    BeginMoveDrag,
    BeginResizeDrag,
    ShowSystemMenu,
    CenterOnScreen,
    Raise,
    Lower,
    SetWindowIcon,
    FlashWindow,
    ResizeNotify,
    SetMinimumSize,
    SetMaximumSize,
    SetPosition,
    SetResizable,
    Maximize,
    Minimize,
    Restore,
    ShowSystemTitleBar,
    HideSystemTitleBar,
    SetBorderless,
    SetFullscreen,
    SetAlwaysOnTop,
    SetWindowOpacity,
    StartTextInput,
    StopTextInput,
    EnableFileDrop,
    DisableFileDrop,
    RequestNativeFrame,
    NativeFramePresented,
    CancelNativeFrame,
    ExactClientLogicalExtent,
    ExactOcclusionState,
    NativeSurface,
}

impl WindowCapability {
    /// 唯一权威的能力值顺序，供集合迭代与契约测试使用。
    pub const ALL: [Self; 33] = [
        Self::RequestClose,
        Self::BeginMoveDrag,
        Self::BeginResizeDrag,
        Self::ShowSystemMenu,
        Self::CenterOnScreen,
        Self::Raise,
        Self::Lower,
        Self::SetWindowIcon,
        Self::FlashWindow,
        Self::ResizeNotify,
        Self::SetMinimumSize,
        Self::SetMaximumSize,
        Self::SetPosition,
        Self::SetResizable,
        Self::Maximize,
        Self::Minimize,
        Self::Restore,
        Self::ShowSystemTitleBar,
        Self::HideSystemTitleBar,
        Self::SetBorderless,
        Self::SetFullscreen,
        Self::SetAlwaysOnTop,
        Self::SetWindowOpacity,
        Self::StartTextInput,
        Self::StopTextInput,
        Self::EnableFileDrop,
        Self::DisableFileDrop,
        Self::RequestNativeFrame,
        Self::NativeFramePresented,
        Self::CancelNativeFrame,
        Self::ExactClientLogicalExtent,
        Self::ExactOcclusionState,
        Self::NativeSurface,
    ];

    /// 返回稳定且平台无关的公开操作身份。
    pub const fn operation(self) -> &'static str {
        match self {
            Self::RequestClose => "PlatformWindow::request_close",
            Self::BeginMoveDrag => "PlatformWindow::begin_move_drag",
            Self::BeginResizeDrag => "PlatformWindow::begin_resize_drag",
            Self::ShowSystemMenu => "PlatformWindow::show_system_menu",
            Self::CenterOnScreen => "PlatformWindow::center_on_screen",
            Self::Raise => "PlatformWindow::raise",
            Self::Lower => "PlatformWindow::lower",
            Self::SetWindowIcon => "PlatformWindow::set_window_icon",
            Self::FlashWindow => "PlatformWindow::flash_window",
            Self::ResizeNotify => "PlatformWindow::resize_notify",
            Self::SetMinimumSize => "IWindowProperties::set_minimum_size",
            Self::SetMaximumSize => "IWindowProperties::set_maximum_size",
            Self::SetPosition => "IWindowProperties::set_position",
            Self::SetResizable => "IWindowProperties::set_resizable",
            Self::Maximize => "IWindowProperties::maximize",
            Self::Minimize => "IWindowProperties::minimize",
            Self::Restore => "IWindowProperties::restore",
            Self::ShowSystemTitleBar => "IWindowProperties::set_system_title_bar_visible(true)",
            Self::HideSystemTitleBar => "IWindowProperties::set_system_title_bar_visible(false)",
            Self::SetBorderless => "IWindowProperties::set_borderless",
            Self::SetFullscreen => "IWindowProperties::set_fullscreen",
            Self::SetAlwaysOnTop => "IWindowProperties::set_always_on_top",
            Self::SetWindowOpacity => "IWindowProperties::set_window_opacity",
            Self::StartTextInput => "IWindowProperties::start_text_input",
            Self::StopTextInput => "IWindowProperties::stop_text_input",
            Self::EnableFileDrop => "IWindowProperties::enable_file_drop(true)",
            Self::DisableFileDrop => "IWindowProperties::enable_file_drop(false)",
            Self::RequestNativeFrame => "PlatformWindow::request_native_frame",
            Self::NativeFramePresented => "PlatformWindow::native_frame_presented",
            Self::CancelNativeFrame => "PlatformWindow::cancel_native_frame",
            Self::ExactClientLogicalExtent => "PlatformWindow::client_logical_extent",
            Self::ExactOcclusionState => "PlatformWindow::occlusion_state",
            Self::NativeSurface => "PlatformWindow::native_surface_ptr",
        }
    }

    pub(crate) fn unsupported_error(self) -> Error {
        Error::new(
            Errc::NotImplemented,
            format!("{} is not supported by this window", self.operation()),
        )
    }

    const fn bit(self) -> u64 {
        1_u64 << self as u8
    }
}

/// 无分配、可复制的窗口能力集合。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowCapabilities {
    bits: u64,
}

impl WindowCapabilities {
    /// 空能力集合。
    pub const EMPTY: Self = Self { bits: 0 };

    /// 包含全部已定义窗口能力的集合。
    pub const ALL: Self = Self::from_slice(&WindowCapability::ALL);

    /// 从权威枚举值构造集合，重复值不会改变集合语义。
    pub const fn from_slice(capabilities: &[WindowCapability]) -> Self {
        let mut bits = 0_u64;
        let mut index = 0;
        while index < capabilities.len() {
            bits |= capabilities[index].bit();
            index += 1;
        }
        Self { bits }
    }

    /// 判断集合是否声明指定能力。
    pub const fn supports(self, capability: WindowCapability) -> bool {
        self.bits & capability.bit() != 0
    }

    /// 返回加入指定能力后的新集合。
    pub const fn with(self, capability: WindowCapability) -> Self {
        Self {
            bits: self.bits | capability.bit(),
        }
    }

    /// 返回移除指定能力后的新集合。
    pub const fn without(self, capability: WindowCapability) -> Self {
        Self {
            bits: self.bits & !capability.bit(),
        }
    }

    /// 返回两个集合的并集。
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    /// 返回集合中的能力数量。
    pub const fn len(self) -> usize {
        self.bits.count_ones() as usize
    }

    /// 判断集合是否为空。
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// 按权威枚举顺序迭代集合值。
    pub fn iter(self) -> impl Iterator<Item = WindowCapability> {
        WindowCapability::ALL
            .into_iter()
            .filter(move |capability| self.supports(*capability))
    }
}
