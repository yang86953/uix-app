// ============================================================================
// uix-platform/src/windows/consts.rs — Windows 平台常量
// ============================================================================

pub(crate) const DEFAULT_CLASS_STYLE: u32 = 0x0008 | 0x0002 | 0x0001;
pub(crate) const GWLP_USERDATA: i32 = -21;
pub(crate) const WM_QUIT: u32 = 0x0012;

pub(crate) const WM_NCCREATE: u32 = 0x0081;
pub(crate) const WM_DESTROY: u32 = 0x0002;
pub(crate) const WM_CLOSE: u32 = 0x0010;
pub(crate) const WM_SIZE: u32 = 0x0005;
pub(crate) const WM_MOVE: u32 = 0x0003;
pub(crate) const WM_SETFOCUS: u32 = 0x0007;
pub(crate) const WM_KILLFOCUS: u32 = 0x0008;
pub(crate) const WM_KEYDOWN: u32 = 0x0100;
pub(crate) const WM_KEYUP: u32 = 0x0101;
pub(crate) const WM_CHAR: u32 = 0x0102;
pub(crate) const WM_SYSKEYDOWN: u32 = 0x0104;
pub(crate) const WM_SYSKEYUP: u32 = 0x0105;
pub(crate) const WM_LBUTTONDOWN: u32 = 0x0201;
pub(crate) const WM_LBUTTONUP: u32 = 0x0202;
pub(crate) const WM_RBUTTONDOWN: u32 = 0x0204;
pub(crate) const WM_RBUTTONUP: u32 = 0x0205;
pub(crate) const WM_MBUTTONDOWN: u32 = 0x0207;
pub(crate) const WM_MBUTTONUP: u32 = 0x0208;
pub(crate) const WM_MOUSEMOVE: u32 = 0x0200;
pub(crate) const WM_MOUSEWHEEL: u32 = 0x020A;
pub(crate) const WM_TIMER: u32 = 0x0113;
pub(crate) const WM_DROPFILES: u32 = 0x0233;
pub(crate) const WM_SETCURSOR: u32 = 0x0020;
pub(crate) const SIZE_RESTORED: usize = 0;
pub(crate) const SIZE_MINIMIZED: usize = 1;
pub(crate) const SIZE_MAXIMIZED: usize = 2;

pub(crate) const HTCLIENT: u32 = 1;

pub(crate) const SW_HIDE: i32 = 0;
pub(crate) const SW_SHOWNORMAL: i32 = 1;
pub(crate) const SW_RESTORE: i32 = 9;
pub(crate) const SW_MINIMIZE: i32 = 6;
pub(crate) const SW_MAXIMIZE: i32 = 3;

pub(crate) const PM_REMOVE: u32 = 0x0001;
pub(crate) const QS_ALLINPUT: u32 = 0x04FF;
pub(crate) const WAIT_TIMEOUT: u32 = 0x00000102;

pub(crate) const CW_USEDEFAULT: i32 = -2147483648;

pub(crate) const IDI_APPLICATION: *const u16 = 32512 as *const u16;

pub(crate) const SWP_NOMOVE: u32 = 0x0002;
pub(crate) const SWP_NOSIZE: u32 = 0x0001;
pub(crate) const SWP_NOZORDER: u32 = 0x0004;
pub(crate) const SWP_FRAMECHANGED: u32 = 0x0020;

pub(crate) const HWND_TOP: isize = 0;
pub(crate) const HWND_BOTTOM: isize = 1;
pub(crate) const HWND_TOPMOST: isize = -1;
pub(crate) const HWND_NOTOPMOST: isize = -2;

pub(crate) const GWL_STYLE: i32 = -16;
pub(crate) const GWL_EXSTYLE: i32 = -20;

pub(crate) const WS_OVERLAPPED: u32 = 0x00000000;
pub(crate) const WS_POPUP: u32 = 0x80000000;
pub(crate) const WS_CAPTION: u32 = 0x00C00000;
pub(crate) const WS_SYSMENU: u32 = 0x00080000;
pub(crate) const WS_MINIMIZEBOX: u32 = 0x00020000;
pub(crate) const WS_MAXIMIZEBOX: u32 = 0x00010000;
pub(crate) const WS_OVERLAPPEDWINDOW: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub(crate) const WS_THICKFRAME: u32 = 0x00040000;
pub(crate) const WS_EX_APPWINDOW: u32 = 0x00040000;
pub(crate) const WS_EX_LAYERED: u32 = 0x00080000;

pub(crate) const LWA_ALPHA: u32 = 0x00000002;

pub(crate) const COLOR_APPWORKSPACE: u32 = 1;

pub(crate) const SM_CXSCREEN: i32 = 0;
pub(crate) const SM_CYSCREEN: i32 = 1;

pub(crate) const IDC_ARROW: u16 = 32512;

pub(crate) const VK_SHIFT: u32 = 0x10;
pub(crate) const VK_CONTROL: u32 = 0x11;
pub(crate) const VK_MENU: u32 = 0x12;
pub(crate) const VK_LWIN: u32 = 0x5B;
pub(crate) const VK_RWIN: u32 = 0x5C;
pub(crate) const VK_LEFT: u32 = 0x25;
pub(crate) const VK_UP: u32 = 0x26;
pub(crate) const VK_RIGHT: u32 = 0x27;
pub(crate) const VK_DOWN: u32 = 0x28;
pub(crate) const VK_RETURN: u32 = 0x0D;
pub(crate) const VK_ESCAPE: u32 = 0x1B;
pub(crate) const VK_BACK: u32 = 0x08;
pub(crate) const VK_DELETE: u32 = 0x2E;
pub(crate) const VK_TAB: u32 = 0x09;
pub(crate) const VK_SPACE: u32 = 0x20;
pub(crate) const VK_INSERT: u32 = 0x2D;
pub(crate) const VK_HOME: u32 = 0x24;
pub(crate) const VK_END: u32 = 0x23;
pub(crate) const VK_PRIOR: u32 = 0x21;
pub(crate) const VK_NEXT: u32 = 0x22;

pub(crate) const WS_VISIBLE: u32 = 0x10000000;

pub(crate) const TRUE: i32 = 1;
pub(crate) const FALSE: i32 = 0;
