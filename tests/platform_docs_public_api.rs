// 声明本文件只编译平台能力使用文档，不创建 Platform 或原生资源。
#![allow(dead_code)]

// 隔离 platform-query 围栏中的平台信息快照查询。
mod platform_query {
    // 引入文档承诺的公开平台门面。
    use uix_app::platform::Platform;
    // 引入公开 GPU adapter 与后端值类型。
    use uix_app::platform::graphics::{GpuAdapterInfo, GraphicsBackend};
    // 引入公开系统硬件 owned 描述类型。
    use uix_app::platform::hardware::{CpuInfo, DisplayInfo, MemoryInfo, OsInfo};

    // 声明不泄漏任何原生句柄的平台快照。
    type PlatformSnapshot = (
        // 保存操作系统 owned 描述。
        OsInfo,
        // 保存 CPU owned 描述。
        CpuInfo,
        // 保存内存 owned 描述。
        MemoryInfo,
        // 保存显示器 owned 描述集合。
        Box<[DisplayInfo]>,
        // 保存 GPU adapter owned 描述集合。
        Box<[GpuAdapterInfo]>,
    );

    // 编译默认 Vulkan 文档中的 Platform 创建、查询与 owner-thread 析构边界。
    #[cfg(feature = "vulkan")]
    fn query_platform() -> uix_app::core::Result<PlatformSnapshot> {
        // 在调用线程创建唯一平台 owner。
        let platform = Platform::new()?;
        // 查询操作系统 typed 信息。
        let os = platform.os_info()?;
        // 查询 CPU typed 信息。
        let cpu = platform.cpu_info()?;
        // 查询内存 typed 信息。
        let memory = platform.memory_info()?;
        // 查询显示器 owned 描述集合。
        let displays = platform.displays()?;
        // 查询默认 feature 集中的三平台 Vulkan adapter 描述。
        let adapters = platform.gpu_adapters(GraphicsBackend::Vulkan)?;
        // 返回完整快照并让 Platform 随函数作用域析构。
        Ok((os, cpu, memory, displays, adapters))
    }

    // 编译全部启用后端的公开同步枚举入口，不在测试中触碰原生驱动。
    fn query_enabled_backends(platform: &Platform) -> uix_app::core::Result<()> {
        #[cfg(feature = "vulkan")]
        let _ = platform.gpu_adapters(GraphicsBackend::Vulkan)?;
        #[cfg(feature = "d3d11")]
        let _ = platform.gpu_adapters(GraphicsBackend::Direct3D11)?;
        #[cfg(feature = "d3d12")]
        let _ = platform.gpu_adapters(GraphicsBackend::Direct3D12)?;
        #[cfg(feature = "metal")]
        let _ = platform.gpu_adapters(GraphicsBackend::Metal)?;
        #[cfg(feature = "opengles")]
        let _ = platform.gpu_adapters(GraphicsBackend::OpenGlEs)?;
        Ok(())
    }
}

// 隔离 platform-service 围栏中的文件与通知服务。
mod platform_service {
    // 引入公开 owned 路径类型。
    use std::path::PathBuf;
    // 引入文档承诺的公开平台门面。
    use uix_app::platform::Platform;
    // 引入平台中立的服务值类型。
    use uix_app::platform::services::{
        // 引入 Windows 通知身份边界值。
        AppUserModelId,
        // 引入文件对话框过滤器值。
        FileDialogFilter,
        // 引入系统目录枚举。
        SpecialDir,
        // 引入系统通知值。
        SystemNotification,
        // 引入通知 capability 状态。
        SystemNotificationCapability,
    };

    // 编译同步文件对话框的 owned 结果契约。
    fn choose_images(platform: &mut Platform) -> uix_app::core::Result<Option<Box<[PathBuf]>>> {
        // 构造并验证图片文件扩展名过滤器。
        let images = FileDialogFilter::new("Images", ["png", "jpg", "jpeg"])?;
        // 在 Platform owner thread 打开多文件选择面板。
        platform.open_files("选择图片", &[images])
    }

    // 编译系统目录查询、通知身份与 capability 分支。
    fn documents_dir_and_notify(platform: &mut Platform) -> uix_app::core::Result<PathBuf> {
        // 查询不隐式创建的文档目录 owned 路径。
        let documents = platform.special_dir(SpecialDir::Documents)?;
        // 在进入平台 Adapter 前验证 AUMID 边界。
        let identity = AppUserModelId::new("UIX.NotificationDemo")?;
        // 为当前 Platform owner 显式配置通知身份。
        platform.set_notification_app_user_model_id(identity)?;
        // 在 Provider 不可用时保留已查询目录并停止发送。
        if platform.system_notification_capability()? != SystemNotificationCapability::Available {
            // 把目录结果交还调用方。
            return Ok(documents);
        }
        // 通过公开服务值同步提交系统通知。
        platform.show_notification(SystemNotification::new("UIX", "任务完成"))?;
        // 把已查询目录交还调用方。
        Ok(documents)
    }
}

// 隔离 platform-system-info 围栏中的 UI 状态摘要。
mod platform_system_info {
    // 引入文档承诺的公开 State 与错误格式化 prelude。
    use uix_app::prelude::*;
    // 引入公开平台门面。
    use uix_app::platform::Platform;

    // 编译三项系统查询到业务状态的完整穷尽映射。
    fn show_hardware(platform: &Platform, status: &State<String>) {
        // 同时查询操作系统、CPU 与内存 typed 信息。
        match (
            // 查询操作系统描述。
            platform.os_info(),
            // 查询 CPU 描述。
            platform.cpu_info(),
            // 查询内存描述。
            platform.memory_info(),
        ) {
            // 只在三项查询全部成功时构造硬件摘要。
            (Ok(os), Ok(cpu), Ok(memory)) => {
                // 把字节换算为整数 MiB。
                let total_memory_mib = memory.total_bytes() / (1024 * 1024);
                // 通过公开访问器构造可展示摘要。
                status.set(format!(
                    // 声明系统摘要的稳定显示格式。
                    "{} · {} · {} 核 · {} MiB 内存",
                    // 展示操作系统名称。
                    os.name(),
                    // 展示 CPU 架构。
                    cpu.architecture(),
                    // 展示逻辑核心数。
                    cpu.logical_cores(),
                    // 展示物理内存 MiB。
                    total_memory_mib,
                ));
            }
            // 任一查询失败时保留对应 typed 错误。
            (Err(error), _, _) | (_, Err(error), _) | (_, _, Err(error)) => {
                // 把错误短格式发布到业务状态。
                status.set(format!("无法读取系统信息：{}", error.short_what()));
            }
        }
    }
}
