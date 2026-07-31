use std::path::PathBuf;

use uix::core::Result;
use uix::platform::graphics::{GpuAdapterInfo, GraphicsBackend};
use uix::platform::hardware::{CpuInfo, DisplayInfo, MemoryInfo, OsInfo};
use uix::platform::services::{SpecialDir, SystemNotification};
use uix::platform::Platform;

fn assert_owned_info<T: Clone + Send + Sync>() {}

#[test]
fn platform_public_surface_is_concrete_typed_and_owned() {
    assert_owned_info::<OsInfo>();
    assert_owned_info::<CpuInfo>();
    assert_owned_info::<MemoryInfo>();
    assert_owned_info::<DisplayInfo>();
    assert_owned_info::<GpuAdapterInfo>();

    let _: fn() -> Result<Platform> = Platform::new;
    let _: fn(&Platform) -> Result<OsInfo> = Platform::os_info;
    let _: fn(&Platform) -> Result<CpuInfo> = Platform::cpu_info;
    let _: fn(&Platform) -> Result<MemoryInfo> = Platform::memory_info;
    let _: fn(&Platform) -> Result<Box<[DisplayInfo]>> = Platform::displays;
    let _: fn(&Platform, GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> = Platform::gpu_adapters;
    let _: fn(&Platform, SpecialDir) -> Result<PathBuf> = Platform::special_dir;
    let _: fn(&mut Platform, SystemNotification) -> Result<()> = Platform::show_notification;
}
