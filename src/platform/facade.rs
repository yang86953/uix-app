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
use super::hardware::{CpuInfo, DisplayInfo, MemoryInfo, OsInfo};
// 能力 Provider 只实现 host 功能域的 OS 差异。
use super::capabilities::providers;
// runtime 只负责 Platform 实例的线程与生命周期约束。
use super::runtime;
// 引入平台服务值与系统通知身份、能力状态契约。
use super::services::{
    AppUserModelId, FileDialogFilter, SpecialDir, SystemNotification, SystemNotificationCapability,
};

static INSTANCE_LIVE: AtomicBool = AtomicBool::new(false);

/// 线程亲和的平台能力入口。
///
/// 必须在进程主线程创建；同一时刻每个进程最多存在一个实例。该类型
/// 明确不可跨线程移动或共享。
pub struct Platform {
    owner_thread: ThreadId,
    state: Option<runtime::State>,
    // 由应用显式提供的通知身份；Platform 仅拥有配置，不执行系统登记。
    notification_app_user_model_id: Option<AppUserModelId>,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl Platform {
    /// 在进程主线程创建唯一的存活平台实例。
    pub fn new() -> Result<Self> {
        if !runtime::is_main_thread()? {
            return Err(Error::new(
                Errc::InvalidState,
                "Platform::new must be called on the process main thread",
            ));
        }

        let mut claim = InstanceClaim::acquire()?;
        let state = runtime::State::new()?;
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
        let value = providers::os_info()?;
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
        let (vendor, model) = providers::cpu_metadata();
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
        let value = providers::memory_info()?;
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
        let values = providers::displays()?;
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
        providers::special_dir(directory)
    }

    /// 同步打开允许多选的文件选择对话框。
    pub fn open_files(
        &mut self,
        // 标题由 Platform System 统一验证后交给 OS Provider。
        title: &str,
        // 空切片表示允许选择任意文件类型。
        filters: &[FileDialogFilter],
    ) -> Result<Option<Box<[PathBuf]>>> {
        // 原生模态对话框只能在 Platform owner thread 驱动。
        self.ensure_owner("Platform::open_files")?;
        // 标题必须满足三平台公共输入契约。
        super::file_dialog::validate_title("Platform::open_files", title)?;
        // Provider 返回 owned 路径；取消保持成功空值。
        providers::open_files(title, filters)
    }

    /// 同步打开单路径保存对话框。
    pub fn save_file(
        &mut self,
        // 标题由 Platform System 统一验证后交给 OS Provider。
        title: &str,
        // 空切片表示不限制保存文件类型。
        filters: &[FileDialogFilter],
    ) -> Result<Option<PathBuf>> {
        // 原生模态对话框只能在 Platform owner thread 驱动。
        self.ensure_owner("Platform::save_file")?;
        // 标题必须满足三平台公共输入契约。
        super::file_dialog::validate_title("Platform::save_file", title)?;
        // Provider 返回 owned 路径；取消保持成功空值。
        providers::save_file(title, filters)
    }

    /// 同步打开单路径目录选择对话框。
    pub fn open_folder(&mut self, title: &str) -> Result<Option<PathBuf>> {
        // 原生模态对话框只能在 Platform owner thread 驱动。
        self.ensure_owner("Platform::open_folder")?;
        // 标题必须满足三平台公共输入契约。
        super::file_dialog::validate_title("Platform::open_folder", title)?;
        // Provider 返回 owned 路径；取消保持成功空值。
        providers::open_folder(title)
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
        providers::system_notification_capability(self.notification_app_user_model_id.as_ref())
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
            SystemNotificationCapability::Available => providers::show_notification(
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

fn enumerate_gpu_adapters(backend: GraphicsBackend) -> Result<Box<[GpuAdapterInfo]>> {
    // 公开门面只传递中立选择值；唯一组合根负责选择并注入对应 Adapter。
    super::composition_root::enumerate_gpu_adapters(backend)
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

fn normalize_text(value: Option<String>) -> Option<String> {
    value.and_then(non_empty)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}
