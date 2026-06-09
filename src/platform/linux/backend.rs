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
// Super-traits delegate the bulk of the interface. The Backend trait only
// adds the 4 subsystem accessors that differ per backend.
// ════════════════════════════════════════════════════════════════════════════

pub(crate) trait Backend:
    IWindowManager + IWindowProperties + IEventLoop + INativeHandle
{
    // ── 后端特有的子系统 ───────────────────────────────────────────
    fn cursor(&mut self) -> &mut dyn ICursor;
    fn keyboard(&self) -> &dyn IKeyboard;
    fn display(&self) -> &dyn IDisplay;
    fn clipboard(&mut self) -> &mut dyn IClipboard;
}
