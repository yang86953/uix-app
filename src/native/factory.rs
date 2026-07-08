//! 平台工厂函数 — #[cfg] 只在此处与 backends/ 内。

use crate::core::error::{Errc, Error};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext};
use crate::native::traits::system::ISystemInfo;
use std::ffi::c_void;

/// 创建当前平台对应的 Platform 实例。
#[cfg(windows)]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(
        crate::native::backends::windows::platform::WindowsPlatform::new(),
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    let platform = crate::native::backends::linux::platform::LinuxPlatform::new()?;
    Ok(Box::new(platform))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Err(Error::new(
        crate::core::error::Errc::PlatformError,
        "Unsupported platform: only Windows and Linux are supported".to_string(),
    ))
}

/// 创建 GPU 图形上下文，使用平台默认候选链。
pub fn create_gpu_context(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    create_gpu_context_with_backend(native_surface, width, height, GraphicsBackend::Auto)
}

/// 创建 GPU 图形上下文，可指定具体 API；`Auto` 走平台默认候选链。
pub fn create_gpu_context_with_backend(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
    requested: GraphicsBackend,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let candidates = gpu_probe_candidates(requested);
    probe_gpu_context(requested, candidates, |candidate| {
        create_gpu_context_candidate(candidate, native_surface, width, height)
    })
}

pub(crate) fn gpu_probe_candidates(requested: GraphicsBackend) -> Vec<GraphicsBackend> {
    match requested {
        GraphicsBackend::Auto => platform_default_gpu_backends(),
        backend => vec![backend],
    }
}

fn probe_gpu_context<F>(
    requested: GraphicsBackend,
    candidates: Vec<GraphicsBackend>,
    mut try_backend: F,
) -> Result<Box<dyn IGraphicsContext>, Error>
where
    F: FnMut(GraphicsBackend) -> Result<Box<dyn IGraphicsContext>, Error>,
{
    if candidates.is_empty() {
        return Err(Error::new(
            Errc::PlatformError,
            format!("Graphics factory: no GPU backend candidates for {requested}"),
        ));
    }

    let mut failures = Vec::new();
    for candidate in candidates {
        crate::core::log::info_fn(format!("Graphics factory: probing {candidate}"));
        match try_backend(candidate) {
            Ok(context) => {
                crate::core::log::info_fn(format!(
                    "Graphics factory: selected {}",
                    context.graphics_backend()
                ));
                return Ok(context);
            }
            Err(err) => {
                let message = err.short_what();
                crate::core::log::warn_fn(format!(
                    "Graphics factory: {candidate} unavailable: {message}"
                ));
                failures.push(format!("{candidate}: {message}"));
            }
        }
    }

    Err(Error::new(
        Errc::PlatformError,
        format!(
            "Graphics factory: all GPU backends failed for {requested}; {}",
            failures.join("; ")
        ),
    ))
}

fn create_gpu_context_candidate(
    backend: GraphicsBackend,
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    match backend {
        GraphicsBackend::Auto => Err(Error::new(
            Errc::InvalidArgument,
            "Graphics factory: Auto is not a concrete probe candidate",
        )),
        GraphicsBackend::OpenGlEs => create_opengles_context(native_surface, width, height),
        GraphicsBackend::D3d11 => create_d3d11_context(native_surface, width, height),
        GraphicsBackend::Vulkan => create_vulkan_context(native_surface, width, height),
        GraphicsBackend::D3d12 | GraphicsBackend::Metal => Err(planned_backend_error(backend)),
    }
}

fn planned_backend_error(backend: GraphicsBackend) -> Error {
    Error::new(
        Errc::PlatformError,
        format!("GraphicsBackend {backend} is planned but not implemented"),
    )
}

#[cfg(windows)]
fn platform_default_gpu_backends() -> Vec<GraphicsBackend> {
    vec![
        GraphicsBackend::D3d12,
        GraphicsBackend::D3d11,
        GraphicsBackend::OpenGlEs,
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_default_gpu_backends() -> Vec<GraphicsBackend> {
    vec![GraphicsBackend::Vulkan, GraphicsBackend::OpenGlEs]
}

#[cfg(target_os = "macos")]
fn platform_default_gpu_backends() -> Vec<GraphicsBackend> {
    vec![GraphicsBackend::Metal]
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
fn platform_default_gpu_backends() -> Vec<GraphicsBackend> {
    Vec::new()
}

#[cfg(windows)]
fn create_opengles_context(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let wgl =
        crate::native::backends::windows::gpu::WglContext::new(native_surface, width, height)?;
    Ok(Box::new(wgl))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn create_opengles_context(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let egl = crate::native::backends::linux::gpu::EglContext::new(native_surface, width, height)?;
    Ok(Box::new(egl))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn create_opengles_context(
    _native_surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend opengles is not supported on this platform",
    ))
}

#[cfg(windows)]
fn create_d3d11_context(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let d3d11 =
        crate::native::backends::windows::gpu::D3d11Context::new(native_surface, width, height)?;
    Ok(Box::new(d3d11))
}

#[cfg(not(windows))]
fn create_d3d11_context(
    _native_surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend d3d11 is only supported on Windows",
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn create_vulkan_context(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let vulkan =
        crate::native::backends::linux::gpu::VulkanContext::new(native_surface, width, height)?;
    Ok(Box::new(vulkan))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn create_vulkan_context(
    _native_surface: *mut c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GraphicsBackend vulkan is only supported on Linux Wayland",
    ))
}

/// 探测系统可用空闲内存（字节）。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn available_memory_bytes() -> u64 {
    crate::native::backends::linux::system_info::LinuxSystemInfo::new()
        .memory_info()
        .available_bytes
}

#[cfg(windows)]
pub fn available_memory_bytes() -> u64 {
    crate::native::backends::windows::system_info::WindowsSystemInfo::new()
        .memory_info()
        .available_bytes
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::test_harness::FakeGraphicsContext;

    #[test]
    fn explicit_backend_probes_only_requested_backend() {
        assert_eq!(
            gpu_probe_candidates(GraphicsBackend::Vulkan),
            vec![GraphicsBackend::Vulkan]
        );
    }

    #[test]
    fn auto_backend_uses_platform_default_order() {
        let candidates = gpu_probe_candidates(GraphicsBackend::Auto);

        #[cfg(windows)]
        assert_eq!(
            candidates,
            vec![
                GraphicsBackend::D3d12,
                GraphicsBackend::D3d11,
                GraphicsBackend::OpenGlEs
            ]
        );

        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(
            candidates,
            vec![GraphicsBackend::Vulkan, GraphicsBackend::OpenGlEs]
        );

        #[cfg(target_os = "macos")]
        assert_eq!(candidates, vec![GraphicsBackend::Metal]);
    }

    #[test]
    fn probe_stops_after_first_successful_candidate() {
        let mut attempts = Vec::new();

        let context = probe_gpu_context(
            GraphicsBackend::Auto,
            vec![
                GraphicsBackend::D3d12,
                GraphicsBackend::D3d11,
                GraphicsBackend::OpenGlEs,
            ],
            |candidate| {
                attempts.push(candidate);
                if candidate == GraphicsBackend::OpenGlEs {
                    Ok(Box::new(FakeGraphicsContext::new()) as Box<dyn IGraphicsContext>)
                } else {
                    Err(planned_backend_error(candidate))
                }
            },
        )
        .expect("OpenGL ES fake context should be selected");

        assert_eq!(context.graphics_backend(), GraphicsBackend::OpenGlEs);
        assert_eq!(
            attempts,
            vec![
                GraphicsBackend::D3d12,
                GraphicsBackend::D3d11,
                GraphicsBackend::OpenGlEs
            ]
        );
    }

    #[test]
    fn probe_error_includes_failed_candidates() {
        let result = probe_gpu_context(
            GraphicsBackend::Auto,
            vec![GraphicsBackend::D3d12, GraphicsBackend::D3d11],
            |candidate| Err(planned_backend_error(candidate)),
        );
        let err = match result {
            Ok(_) => panic!("probe should fail when every candidate fails"),
            Err(err) => err,
        };
        let message = err.message();

        assert!(message.contains("d3d12"));
        assert!(message.contains("d3d11"));
        assert!(message.contains("all GPU backends failed"));
    }

    #[test]
    fn vulkan_candidate_is_real_linux_backend_or_platform_specific_error() {
        let err = match create_vulkan_context(std::ptr::null_mut(), 1, 1) {
            Ok(_) => panic!("null surface should not create a Vulkan context"),
            Err(err) => err,
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        assert!(err.message().contains("WaylandSurfaceHandle"));

        #[cfg(not(all(unix, not(target_os = "macos"))))]
        assert!(err.message().contains("only supported on Linux Wayland"));
    }
}
