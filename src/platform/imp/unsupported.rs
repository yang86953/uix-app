use std::path::PathBuf;

use crate::core::{Errc, Error, Result};
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
use crate::platform::services::{SpecialDir, SystemNotification};

pub(crate) struct State;

impl State {
    pub(crate) fn new() -> Result<Self> {
        Err(unsupported("Platform::new"))
    }
}

pub(crate) fn is_main_thread() -> Result<bool> {
    Err(unsupported("Platform::new"))
}

pub(crate) fn os_info() -> Result<OsInfo> {
    Err(unsupported("Platform::os_info"))
}

pub(crate) fn cpu_metadata() -> (Option<String>, Option<String>) {
    (None, None)
}

pub(crate) fn memory_info() -> Result<MemoryInfo> {
    Err(unsupported("Platform::memory_info"))
}

pub(crate) fn displays() -> Result<Box<[DisplayInfo]>> {
    Err(unsupported("Platform::displays"))
}

pub(crate) fn special_dir(_directory: SpecialDir) -> Result<PathBuf> {
    Err(unsupported("Platform::special_dir"))
}

pub(crate) fn show_notification(_notification: &SystemNotification) -> Result<()> {
    Err(unsupported("Platform::show_notification"))
}

fn unsupported(operation: &str) -> Error {
    Error::new(
        Errc::NotImplemented,
        format!("{operation}: this target has no UIX platform provider"),
    )
}
