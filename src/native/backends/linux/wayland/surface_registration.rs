// Wayland surface registration Component 统一维护窗口关闭时的跨注册表一致性。

// 共享注册表通过短时互斥 guard 建立单次注销事务。
use std::sync::{Arc, Mutex};

// 窗口身份用于防止旧窗口删除复用协议编号的新注册。
use crate::core::WindowId;
// typed error 明确暴露损坏的注册表 owner。
use crate::core::error::{Errc, Error, Result};
// surface 路由表拥有协议编号到窗口身份的映射。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// 指针激活注册表拥有 surface 代次及其未消费授权。
use super::pointer_activation::WaylandPointerActivationRegistry;

// 原子注销一个窗口 surface 的授权与事件路由注册事实。
pub(crate) fn unregister_window_surface(
    // 指针授权注册表必须先锁定以保持全后端一致的锁顺序。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface 路由注册表与授权注册表共同组成注销事务。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // 协议编号标识当前待注销的 surface。
    surface_id: u32,
    // 稳定窗口身份阻止旧 owner 删除后来复用编号的授权。
    window_id: WindowId,
    // 返回 typed failure，调用方据此保留可重试身份。
) -> Result<()> {
    // 先取得授权注册表，任一 owner 损坏时都不允许修改共享事实。
    let mut pointer_registry = pointer_activations.lock().map_err(|_| {
        // 构造稳定的授权注册表损坏诊断。
        Error::new(
            // 共享状态已无法安全访问。
            Errc::InvalidState,
            // 保留 surface 注销与授权注册表阶段。
            "Wayland pointer activation registry mutex poisoned during surface unregistration",
        )
    })?;
    // 在执行任何注销前取得第二份注册事实的健康 guard。
    let mut surface_registry = surface_windows.lock().map_err(|_| {
        // 构造稳定的路由注册表损坏诊断。
        Error::new(
            // 共享状态已无法安全访问。
            Errc::InvalidState,
            // 保留 surface 注销与窗口路由注册表阶段。
            "Wayland surface window registry mutex poisoned during surface unregistration",
        )
    })?;
    // 两份 guard 都健康后才撤销未消费的交互授权。
    pointer_registry.unregister_surface(surface_id, window_id);
    // 在同一事务内撤销 surface 到 WindowId 的事件路由。
    surface_registry.unregister_surface(surface_id);
    // 两份注册事实均已完成幂等注销。
    Ok(())
}
