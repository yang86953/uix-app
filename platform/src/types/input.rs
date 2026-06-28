// ============================================================================
// platform/types/input.rs — 输入相关类型
//
// IClipboard / ICursor / ITextInput 已迁移至 crate::api::traits。
// ============================================================================

// ════════════════════════════════════════════════════════════════════════════
// 鼠标按钮 — 跨层共享
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MouseButton {
    None,
    Left,
    Right,
    Middle,
    X1,
    X2,
}

// ════════════════════════════════════════════════════════════════════════════
// 光标类型
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CursorType {
    Arrow,
    IBeam,
    Crosshair,
    Hand,
    ResizeH,
    ResizeV,
    ResizeNE,
    ResizeNW,
    Move,
    Wait,
    NotAllowed,
    Custom,
}
