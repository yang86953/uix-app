// ============================================================================
// native/backends/windows/consts.rs — Windows 平台常量
// ============================================================================

const CS_VREDRAW: u32 = 0x0001;
const CS_HREDRAW: u32 = 0x0002;
const CS_DBLCLKS: u32 = 0x0008;
const CS_OWNDC: u32 = 0x0020;
pub(crate) const DEFAULT_CLASS_STYLE: u32 = CS_DBLCLKS | CS_HREDRAW | CS_VREDRAW | CS_OWNDC;
pub(crate) const GWLP_USERDATA: i32 = -21;
pub(crate) const WM_NULL: u32 = 0x0000;
pub(crate) const WM_QUIT: u32 = 0x0012;
pub(crate) const WM_APP: u32 = 0x8000;
pub(crate) const WM_UIX_FRAME_OPPORTUNITY: u32 = WM_APP + 0x0051;
// 将自绘标题栏的非客户区刷新延迟到当前窗口状态事务结束后。
pub(crate) const WM_UIX_REFRESH_EXTENDED_FRAME: u32 = WM_APP + 0x0052;

pub(crate) const WM_NCCREATE: u32 = 0x0081;
pub(crate) const WM_SETICON: u32 = 0x0080;
pub(crate) const WM_NCLBUTTONDOWN: u32 = 0x00A1;
pub(crate) const WM_NCRBUTTONUP: u32 = 0x00A5;
pub(crate) const WM_DESTROY: u32 = 0x0002;
pub(crate) const WM_CLOSE: u32 = 0x0010;
pub(crate) const WM_ACTIVATE: u32 = 0x0006;
pub(crate) const WM_SIZE: u32 = 0x0005;
pub(crate) const WM_DPICHANGED: u32 = 0x02E0;
pub(crate) const WM_THEMECHANGED: u32 = 0x031A;
pub(crate) const WM_SHOWWINDOW: u32 = 0x0018;
pub(crate) const WM_SETTINGCHANGE: u32 = 0x001A;
pub(crate) const WM_ERASEBKGND: u32 = 0x0014;
pub(crate) const WM_MOVE: u32 = 0x0003;
pub(crate) const WM_GETMINMAXINFO: u32 = 0x0024;
pub(crate) const WM_SETFOCUS: u32 = 0x0007;
pub(crate) const WM_KILLFOCUS: u32 = 0x0008;
pub(crate) const WM_KEYDOWN: u32 = 0x0100;
pub(crate) const WM_KEYUP: u32 = 0x0101;
pub(crate) const WM_CHAR: u32 = 0x0102;
pub(crate) const WM_SYSKEYDOWN: u32 = 0x0104;
pub(crate) const WM_SYSKEYUP: u32 = 0x0105;
pub(crate) const WM_IME_STARTCOMPOSITION: u32 = 0x010D;
pub(crate) const WM_IME_ENDCOMPOSITION: u32 = 0x010E;
pub(crate) const WM_IME_COMPOSITION: u32 = 0x010F;
pub(crate) const WM_LBUTTONDOWN: u32 = 0x0201;
pub(crate) const WM_LBUTTONUP: u32 = 0x0202;
pub(crate) const WM_LBUTTONDBLCLK: u32 = 0x0203;
pub(crate) const WM_RBUTTONDOWN: u32 = 0x0204;
pub(crate) const WM_RBUTTONUP: u32 = 0x0205;
pub(crate) const WM_RBUTTONDBLCLK: u32 = 0x0206;
pub(crate) const WM_MBUTTONDOWN: u32 = 0x0207;
pub(crate) const WM_MBUTTONUP: u32 = 0x0208;
pub(crate) const WM_MBUTTONDBLCLK: u32 = 0x0209;
pub(crate) const WM_MOUSEMOVE: u32 = 0x0200;
pub(crate) const WM_MOUSEWHEEL: u32 = 0x020A;
pub(crate) const WM_TIMER: u32 = 0x0113;
pub(crate) const WM_DROPFILES: u32 = 0x0233;
pub(crate) const WM_SETCURSOR: u32 = 0x0020;
pub(crate) const SIZE_RESTORED: usize = 0;
pub(crate) const SIZE_MINIMIZED: usize = 1;
pub(crate) const SIZE_MAXIMIZED: usize = 2;

pub(crate) const HTCLIENT: u32 = 1;
pub(crate) const HTCAPTION: usize = 2;
pub(crate) const HTLEFT: u32 = 10;
pub(crate) const HTRIGHT: u32 = 11;
pub(crate) const HTTOP: u32 = 12;
pub(crate) const HTTOPLEFT: u32 = 13;
pub(crate) const HTTOPRIGHT: u32 = 14;
pub(crate) const HTBOTTOM: u32 = 15;
pub(crate) const HTBOTTOMLEFT: u32 = 16;
pub(crate) const HTBOTTOMRIGHT: u32 = 17;
pub(crate) const WM_NCCALCSIZE: u32 = 0x0083;
pub(crate) const WM_NCHITTEST: u32 = 0x0084;

pub(crate) const SW_HIDE: i32 = 0;
pub(crate) const SW_SHOWNORMAL: i32 = 1;
pub(crate) const SW_RESTORE: i32 = 9;
pub(crate) const SW_MINIMIZE: i32 = 6;
pub(crate) const SW_MAXIMIZE: i32 = 3;

pub(crate) const PM_REMOVE: u32 = 0x0001;
pub(crate) const QS_ALLINPUT: u32 = 0x04FF;
pub(crate) const INFINITE: u32 = 0xFFFF_FFFF;
pub(crate) const WAIT_TIMEOUT: u32 = 0x00000102;

pub(crate) const CW_USEDEFAULT: i32 = -2147483648;

pub(crate) const IDI_APPLICATION: *const u16 = 32512 as *const u16;
pub(crate) const IMAGE_ICON: u32 = 1;
pub(crate) const LR_LOADFROMFILE: u32 = 0x0010;
pub(crate) const ICON_SMALL: usize = 0;
pub(crate) const ICON_BIG: usize = 1;

pub(crate) const SWP_NOMOVE: u32 = 0x0002;
pub(crate) const SWP_NOSIZE: u32 = 0x0001;
pub(crate) const SWP_NOZORDER: u32 = 0x0004;
pub(crate) const SWP_NOACTIVATE: u32 = 0x0010;
pub(crate) const SWP_FRAMECHANGED: u32 = 0x0020;
pub(crate) const SWP_NOOWNERZORDER: u32 = 0x0200;

pub(crate) const HWND_TOP: isize = 0;
pub(crate) const HWND_BOTTOM: isize = 1;
pub(crate) const HWND_TOPMOST: isize = -1;
pub(crate) const HWND_NOTOPMOST: isize = -2;

pub(crate) const GWL_STYLE: i32 = -16;
pub(crate) const GWL_EXSTYLE: i32 = -20;

pub(crate) const SM_CXICON: i32 = 11;
pub(crate) const SM_CYICON: i32 = 12;
pub(crate) const SM_CXSMICON: i32 = 49;
pub(crate) const SM_CYSMICON: i32 = 50;

pub(crate) const WS_OVERLAPPED: u32 = 0x00000000;
pub(crate) const WS_POPUP: u32 = 0x80000000;
pub(crate) const WS_CAPTION: u32 = 0x00C00000;
pub(crate) const WS_SYSMENU: u32 = 0x00080000;
pub(crate) const WS_MINIMIZEBOX: u32 = 0x00020000;
pub(crate) const WS_MAXIMIZEBOX: u32 = 0x00010000;
pub(crate) const WS_OVERLAPPEDWINDOW: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub(crate) const WS_THICKFRAME: u32 = 0x00040000;
pub(crate) const WS_MAXIMIZE: u32 = 0x01000000;
pub(crate) const WS_EX_APPWINDOW: u32 = 0x00040000;
pub(crate) const WS_EX_LAYERED: u32 = 0x00080000;

pub(crate) const LWA_ALPHA: u32 = 0x00000002;

pub(crate) const FLASHW_TRAY: u32 = 0x00000002;
pub(crate) const FLASHW_TIMERNOFG: u32 = 0x0000000C;

pub(crate) const COLOR_APPWORKSPACE: u32 = 1;

pub(crate) const MONITOR_DEFAULTTONEAREST: u32 = 2;

pub(crate) const IDC_ARROW: u16 = 32512;

pub(crate) const GCS_COMPSTR: u32 = 0x0008;
pub(crate) const GCS_RESULTSTR: u32 = 0x0800;

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
