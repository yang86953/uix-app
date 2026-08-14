// 在 feature 裁剪后拒绝不可达收尾、不可达模式和仅在其他 backend 使用的绑定。
#![deny(
    unreachable_code,
    unreachable_patterns,
    unused_imports,
    unused_variables
)]

use std::marker::PhantomData;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, ThreadId};

use crate::core::{Errc, Error, Result};

// 引入跨 feature 保持公开的 adapter 描述与 backend 身份。
use super::graphics::{GpuAdapterInfo, GraphicsBackend};
// 仅 Windows D3D11 原生枚举需要设备类别。
#[cfg(all(windows, feature = "d3d11"))]
use super::graphics::GpuDeviceType;
use super::hardware::{CpuInfo, DisplayInfo, MemoryInfo, OsInfo};
use super::imp;
// 引入平台服务值与系统通知身份、能力状态契约。
use super::services::{
    AppUserModelId, SpecialDir, SystemNotification, SystemNotificationCapability,
};

static INSTANCE_LIVE: AtomicBool = AtomicBool::new(false);

/// 线程亲和的平台能力入口。
///
/// 必须在进程主线程创建；同一时刻每个进程最多存在一个实例。该类型
/// 明确不可跨线程移动或共享。
pub struct Platform {
    owner_thread: ThreadId,
    state: Option<imp::State>,
    // 由应用显式提供的通知身份；Platform 仅拥有配置，不执行系统登记。
    notification_app_user_model_id: Option<AppUserModelId>,
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
            // 新实例默认没有通知身份，开发环境不会伪造成功。
            notification_app_user_model_id: None,
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
        // 将 backend 分派交给无状态组件，便于按 feature 直接验证契约。
        enumerate_gpu_adapters(backend)
    }

    /// 查询 OS-known user directory；不会创建目录。
    pub fn special_dir(&self, directory: SpecialDir) -> Result<PathBuf> {
        self.ensure_owner("Platform::special_dir")?;
        imp::special_dir(directory)
    }

    /// 配置部署层已经登记的 Windows AUMID。
    pub fn set_notification_app_user_model_id(
        &mut self,
        // 使用已验证的值对象，避免重复解析字符串。
        app_user_model_id: AppUserModelId,
    ) -> Result<()> {
        // 配置写入必须服从 Platform 的 owner-thread 契约。
        self.ensure_owner("Platform::set_notification_app_user_model_id")?;
        // 仅保存配置；不创建快捷方式，也不写系统注册信息。
        self.notification_app_user_model_id = Some(app_user_model_id);
        // 配置成功不代表部署登记成功，调用方应继续查询 capability。
        Ok(())
    }

    /// 查询当前环境的系统通知能力与身份就绪状态。
    pub fn system_notification_capability(&self) -> Result<SystemNotificationCapability> {
        // 能力探测必须与其他平台查询一样运行在 owner thread。
        self.ensure_owner("Platform::system_notification_capability")?;
        // 把只读身份借用交给目标平台 Provider 探测。
        imp::system_notification_capability(self.notification_app_user_model_id.as_ref())
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
        // 在发送前读取可解释的能力状态，禁止未配置或未登记时伪成功。
        match self.system_notification_capability()? {
            // 就绪时才进入目标平台 Adapter。
            SystemNotificationCapability::Available => imp::show_notification(
                // Windows 使用身份，其他平台明确忽略该参数。
                self.notification_app_user_model_id.as_ref(),
                // 通知值只借用到同步调用结束。
                &notification,
            ),
            // Windows 未配置身份时保留产品决策要求的 NotImplemented。
            SystemNotificationCapability::IdentityRequired => Err(Error::new(
                // 未提供部署身份意味着当前调用尚不可实现。
                Errc::NotImplemented,
                // 诊断给出明确配置与安装器登记指引。
                "Platform::show_notification: configure an AppUserModelId and register the same identity in an MSIX package or Start Menu shortcut",
            )),
            // 配置存在但部署登记缺失时返回可区分的状态错误。
            SystemNotificationCapability::IdentityUnregistered => Err(Error::new(
                // 缺失系统登记是环境状态错误，而不是发送成功。
                Errc::InvalidState,
                // 诊断明确要求安装器补齐同一 AUMID。
                "Platform::show_notification: configured AppUserModelId is not registered by the installed application",
            )),
            // 没有 Provider 的目标继续返回稳定的未实现错误。
            SystemNotificationCapability::Unsupported => Err(Error::new(
                // 目标平台缺少通知 Adapter。
                Errc::NotImplemented,
                // 诊断保持对调用方可操作。
                "Platform::show_notification: this target has no system-notification provider",
            )),
        }
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

// 按已编译 backend 直接返回枚举结果或类型化的不支持错误。
#[cfg(any(
    feature = "d3d11",
    feature = "vulkan",
    feature = "d3d12",
    feature = "metal",
    feature = "opengles"
))]
fn enumerate_gpu_adapters(backend: GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> {
    // 直接返回 feature 对应结果，避免无成功分支时保留幽灵值。
    match backend {
        // Windows D3D11 继续使用 DXGI，并把临时 Vec 转为 owned slice。
        #[cfg(all(windows, feature = "d3d11"))]
        GraphicsBackend::Direct3D11 => Ok(enumerate_dxgi_adapters()?.into_boxed_slice()),
        // 其他已启用 backend 保持类型化的未实现契约。
        #[cfg(any(
            all(feature = "d3d11", not(windows)),
            feature = "vulkan",
            feature = "d3d12",
            feature = "metal",
            feature = "opengles"
        ))]
        _ => Err(not_implemented_backend(backend)),
    }
}

// 没有图形 feature 时 GraphicsBackend 不可构造，空 match 是唯一穷尽控制流。
#[cfg(not(any(
    feature = "d3d11",
    feature = "vulkan",
    feature = "d3d12",
    feature = "metal",
    feature = "opengles"
)))]
fn enumerate_gpu_adapters(backend: GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> {
    // 不制造 backend 值、typed failure 或幽灵成功；调用在安全 Rust 中不可发生。
    match backend {}
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
        let software =
            desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 || desc.VendorId == 0x1414;
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

// 只有兼容失败分支存在时才需要构造该诊断错误。
#[cfg(any(
    all(feature = "d3d11", not(windows)),
    feature = "vulkan",
    feature = "d3d12",
    feature = "metal",
    feature = "opengles"
))]
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

// 仅在纯 OpenGL ES 构建中验证兼容失败分支，避免触发真实平台资源。
#[cfg(all(test, feature = "opengles", not(feature = "d3d11")))]
mod tests {
    // 引入本模块私有分派组件和公开错误码。
    use super::{Errc, GraphicsBackend, enumerate_gpu_adapters};

    // 纯 OpenGL ES 构建必须返回稳定的类型化未实现错误。
    #[test]
    fn opengles_only_adapter_enumeration_is_typed_not_implemented() {
        // 调用无状态分派组件，不创建窗口、设备或 Platform 单例。
        let result = enumerate_gpu_adapters(GraphicsBackend::OpenGlEs);
        // 同时约束失败类型和禁止伪造空成功结果。
        assert!(matches!(result, Err(error) if error.code() == Errc::NotImplemented));
    }
}
