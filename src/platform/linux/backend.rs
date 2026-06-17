// ============================================================================
// platform/linux/backend.rs — Multi-backend abstraction for Linux
// ============================================================================
//
// Defines the `Backend` trait for window management.
// Uses super-traits (IWindowManager + IWindowProperties + IEventLoop +
// INativeHandle) to avoid duplicating method signatures.
//
// Backend-specific subsystems (cursor, clipboard, display, keyboard)
// are owned by the backend; cross-platform subsystems (console, filesystem,
// etc.) are owned by `LinuxPlatform` and shared.
// ============================================================================

use crate::platform::*;

// ════════════════════════════════════════════════════════════════════════════
// Backend — window management interface for X11 / Wayland
//
// Super-traits delegate the bulk of the interface. Backend subertraits
// (ICursor, IKeyboard, IDisplay, IClipboard) allow direct upcasting from
// &mut dyn Backend without separate accessor methods.
// ════════════════════════════════════════════════════════════════════════════

pub(crate) trait Backend:
    IWindowManager
    + IWindowProperties
    + IEventLoop
    + INativeHandle
    + IPresenter
    + ICursor
    + IKeyboard
    + IDisplay
    + IClipboard
{
    // ── 像素呈现 ─────────────────────────────────────────────────
    /// Present a BGRA pixel buffer to the native window.
    /// `pixels` is a slice of u32 in ARGB8888 format (0xAARRGGBB),
    /// stored in BGRA byte order on little-endian platforms.
    /// `dirty_rect` 为局部更新区域，(x, y, w, h)，None 表示全帧。
    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    );
}
