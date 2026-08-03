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
        // 显式标注返回元素类型：d3d11 feature 未启用时 match 无产生值分支，
        // 需要类型标注才能推断 `values`（否则 E0282）。
        let values: Vec<GpuAdapterInfo> = match backend {
            #[cfg(all(windows, feature = "d3d11"))]
            GraphicsBackend::Direct3D11 => enumerate_dxgi_adapters()?,
            _ => return Err(not_implemented_backend(backend)),
        };
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

/// 通过 DXGI 枚举 D3D11 图形适配器（替代已移除的 wgpu 枚举）。
#[cfg(all(windows, feature = "d3d11"))]
fn enumerate_dxgi_adapters() -> Result<Vec<GpuAdapterInfo>, Error> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_DESC1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
        IDXGIFactory1,
    };

    // SAFETY: CreateDXGIFactory1 返回进程级 DXGI 工厂，无需传入句柄。
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|error| {
        Error::new(
            Errc::PlatformError,
            format!("Platform::gpu_adapters: CreateDXGIFactory1 failed: {error}"),
        )
    })?;
    let mut adapters = Vec::new();
    for index in 0.. {
        // SAFETY: EnumAdapters1 返回的 adapter 由 factory 管理生命周期，仅在本函数内查询。
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => {
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("Platform::gpu_adapters: EnumAdapters1 failed: {error}"),
                ));
            }
        };
        // SAFETY: desc 为输出缓冲，GetDesc1 调用期间有效。
        let desc: DXGI_ADAPTER_DESC1 = unsafe { adapter.GetDesc1() }.map_err(|error| {
            Error::new(
                Errc::PlatformError,
                format!("Platform::gpu_adapters: GetDesc1 failed: {error}"),
            )
        })?;
        // DXGI 无法直接区分集成/独显；仅识别软件适配器（WARP/基本显示）。
        let software = desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 || desc.VendorId == 0x1414;
        adapters.push(GpuAdapterInfo::new(
            GraphicsBackend::Direct3D11,
            if software {
                GpuDeviceType::Software
            } else {
                GpuDeviceType::Unknown
            },
            non_empty(String::from_utf16_lossy(&desc.Description)),
            (desc.VendorId != 0).then_some(desc.VendorId),
            (desc.DeviceId != 0).then_some(desc.DeviceId),
            None,
        ));
    }
    Ok(adapters)
}

fn not_implemented_backend(backend: GraphicsBackend) -> Error {
    Error::new(
        Errc::NotImplemented,
        format!(
            "Platform::gpu_adapters: {:?} has no native enumerator on this target",
            backend
        ),
    )
}

fn normalize_text(value: Option<String>) -> Option<String> {
    value.and_then(non_empty)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
