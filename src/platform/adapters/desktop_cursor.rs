//! 将桌面光标门面接到已有原生光标实现。
use crate::core::{Point, error::Result};
#[cfg(windows)]
use crate::{native::backends::windows::cursor::WindowsCursor, platform::windowing::ICursor};
pub(crate) fn position() -> Result<Point> {
    #[cfg(windows)]
    {
        WindowsCursor::new().cursor_position()
    }
    #[cfg(not(windows))]
    {
        Err(crate::core::error::Error::new(
            crate::core::error::Errc::NotImplemented,
            "Global desktop cursor access is unavailable on this backend",
        ))
    }
}
pub(crate) fn set_position(x: i32, y: i32) -> Result<()> {
    #[cfg(windows)]
    {
        WindowsCursor::new().set_cursor_position(x, y)
    }
    #[cfg(not(windows))]
    {
        let _ = (x, y);
        Err(crate::core::error::Error::new(
            crate::core::error::Errc::NotImplemented,
            "Global desktop cursor access is unavailable on this backend",
        ))
    }
}
