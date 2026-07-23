use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, ThreadId};

use crate::core::{Errc, Error, Result};

use super::graphics::{GpuAdapterInfo, GpuDeviceType, GraphicsBackend};
use super::hardware::{CpuInfo, DisplayInfo, MemoryInfo, OsInfo};
use super::imp;
use super::services::{SpecialDir, SystemNotification};

static INSTANCE_LIVE: AtomicBool = AtomicBool::new(false);

/// 线程亲和的平台能力入口。
///
/// 必须在进程主线程创建；同一时刻每个进程最多存在一个实例。该类型
/// 明确不可跨线程移动或共享。
pub struct Platform {
    owner_thread: ThreadId,
    state: Option<imp::State>,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl Platform {
    /// 在进程主线程创建唯一的存活平台实例。
    pub fn new() -> Result<Self> {
        if !imp::is_main_thread()? {
            return Err(Error::new(
                Errc::InvalidState,
                "Platform::new must be called on the process main thread",
            ));
        }

        let mut claim = InstanceClaim::acquire()?;
        let state = imp::State::new()?;
        claim.commit();
        Ok(Self {
            owner_thread: thread::current().id(),
            state: Some(state),
            _thread_affinity: PhantomData,
        })
    }

    /// 即时采集操作系统描述。
    pub fn os_info(&self) -> Result<OsInfo> {
        self.ensure_owner("Platform::os_info")?;
        let value = imp::os_info()?;
        if value.name().trim().is_empty() {
            return Err(Error::new(
                Errc::PlatformError,
                "Platform::os_info: provider returned an empty system name",
            ));
        }
        Ok(value)
    }

    /// 即时采集 CPU 描述。
    pub fn cpu_info(&self) -> Result<CpuInfo> {
        self.ensure_owner("Platform::cpu_info")?;
        let architecture = std::env::consts::ARCH.trim();
        if architecture.is_empty() {
            return Err(Error::new(
                Errc::PlatformError,
                "Platform::cpu_info: target architecture is unavailable",
            ));
        }
        let logical_cores = thread::available_parallelism().map_err(|source| {
            Error::new(
                Errc::PlatformError,
                format!("Platform::cpu_info: available_parallelism failed: {source}"),
            )
        })?;
        let (vendor, model) = imp::cpu_metadata();
        Ok(CpuInfo::new(
            architecture.to_owned(),
            logical_cores,
            normalize_text(vendor),
            normalize_text(model),
        ))
    }

    /// 即时采集系统物理内存。
    pub fn memory_info(&self) -> Result<MemoryInfo> {
        self.ensure_owner("Platform::memory_info")?;
        let value = imp::memory_info()?;
        if value.total_bytes() == 0 || value.available_bytes() > value.total_bytes() {
            return Err(Error::new(
                Errc::PlatformError,
                "Platform::memory_info: provider returned invalid physical-memory totals",
            ));
        }
        Ok(value)
    }

    /// 即时枚举显示器。
    pub fn displays(&self) -> Result<Box<[DisplayInfo]>> {
        self.ensure_owner("Platform::displays")?;
        let values = imp::displays()?;
        for value in values.iter() {
            let bounds = value.bounds();
            if !value.scale().is_finite()
                || value.scale() <= 0.0
                || !bounds.x.is_finite()
                || !bounds.y.is_finite()
                || !bounds.w.is_finite()
                || !bounds.h.is_finite()
                || bounds.w <= 0.0
                || bounds.h <= 0.0
            {
                return Err(Error::new(
                    Errc::PlatformError,
                    "Platform::displays: provider returned invalid display geometry",
                ));
            }
        }
        Ok(values)
    }

    /// 按具体图形 API 同步枚举 GPU adapter。
    pub fn gpu_adapters(&self, backend: GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> {
        self.ensure_owner("Platform::gpu_adapters")?;
        let backends = backend_mask(backend)?;
        if !wgpu::Instance::enabled_backend_features().contains(backends) {
            return Err(not_implemented_backend(backend));
        }

        let values = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = backends;
            let instance = wgpu::Instance::new(descriptor);
            pollster::block_on(instance.enumerate_adapters(backends))
                .into_iter()
                .map(|adapter| {
                    let info = adapter.get_info();
                    let device_type = match info.device_type {
                        wgpu::DeviceType::IntegratedGpu => GpuDeviceType::Integrated,
                        wgpu::DeviceType::DiscreteGpu => GpuDeviceType::Discrete,
                        wgpu::DeviceType::VirtualGpu => GpuDeviceType::Virtual,
                        wgpu::DeviceType::Cpu => GpuDeviceType::Software,
                        wgpu::DeviceType::Other => GpuDeviceType::Unknown,
                    };
                    let driver = joined_driver(&info.driver, &info.driver_info);
                    GpuAdapterInfo::new(
                        backend,
                        device_type,
                        non_empty(info.name),
                        (info.vendor != 0).then_some(info.vendor),
                        (info.device != 0).then_some(info.device),
                        driver,
                    )
                })
                .collect::<Vec<_>>()
        }))
        .map_err(|_| {
            Error::new(
                Errc::PlatformError,
                format!(
                    "Platform::gpu_adapters: {:?} driver enumeration panicked",
                    backend
                ),
            )
        })?;
        Ok(values.into_boxed_slice())
    }

    /// 查询 OS-known user directory；不会创建目录。
    pub fn special_dir(&self, directory: SpecialDir) -> Result<PathBuf> {
        self.ensure_owner("Platform::special_dir")?;
        imp::special_dir(directory)
    }

    /// 同步发送一条系统通知。
    pub fn show_notification(&mut self, notification: SystemNotification) -> Result<()> {
        self.ensure_owner("Platform::show_notification")?;
        if notification.title().trim().is_empty() {
            return Err(Error::new(
                Errc::InvalidArgument,
                "Platform::show_notification: title must not be empty",
            ));
        }
        if notification.title().contains('\0') || notification.message().contains('\0') {
            return Err(Error::new(
                Errc::InvalidArgument,
                "Platform::show_notification: text must not contain NUL",
            ));
        }
        imp::show_notification(&notification)
    }

    fn ensure_owner(&self, operation: &str) -> Result<()> {
        if thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(Error::new(
                Errc::InvalidState,
                format!("{operation} must run on the Platform owner thread"),
            ))
        }
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        debug_assert_eq!(
            thread::current().id(),
            self.owner_thread,
            "Platform must be dropped on its owner thread"
        );
        drop(self.state.take());
        INSTANCE_LIVE.store(false, Ordering::Release);
    }
}

struct InstanceClaim {
    committed: bool,
}

impl InstanceClaim {
    fn acquire() -> Result<Self> {
        INSTANCE_LIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "Platform::new: another Platform instance is already alive",
                )
            })?;
        Ok(Self { committed: false })
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for InstanceClaim {
    fn drop(&mut self) {
        if !self.committed {
            INSTANCE_LIVE.store(false, Ordering::Release);
        }
    }
}

fn backend_mask(backend: GraphicsBackend) -> Result<wgpu::Backends> {
    match backend {
        GraphicsBackend::Vulkan if cfg!(feature = "vulkan") => Ok(wgpu::Backends::VULKAN),
        GraphicsBackend::Direct3D12 if cfg!(all(windows, feature = "d3d12")) => {
            Ok(wgpu::Backends::DX12)
        }
        GraphicsBackend::Metal if cfg!(all(target_os = "macos", feature = "metal")) => {
            Ok(wgpu::Backends::METAL)
        }
        GraphicsBackend::OpenGlEs if cfg!(feature = "opengles") => Ok(wgpu::Backends::GL),
        _ => Err(not_implemented_backend(backend)),
    }
}

fn not_implemented_backend(backend: GraphicsBackend) -> Error {
    Error::new(
        Errc::NotImplemented,
        format!(
            "Platform::gpu_adapters: {:?} is not compiled for this target",
            backend
        ),
    )
}

fn joined_driver(driver: &str, driver_info: &str) -> Option<String> {
    match (
        non_empty(driver.to_owned()),
        non_empty(driver_info.to_owned()),
    ) {
        (Some(driver), Some(info)) if driver != info => Some(format!("{driver} ({info})")),
        (Some(driver), _) => Some(driver),
        (None, info) => info,
    }
}

fn normalize_text(value: Option<String>) -> Option<String> {
    value.and_then(non_empty)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
