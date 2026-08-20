// Wayland surface registration Component 统一维护窗口关闭时的跨注册表一致性。

// 共享注册表通过短时互斥 guard 建立单次登记或注销事务。
use std::sync::{Arc, Mutex, MutexGuard};

// 窗口身份用于防止旧窗口删除复用协议编号的新注册。
use crate::core::WindowId;
// typed error 明确暴露损坏的注册表 owner。
use crate::core::error::{Errc, Error, Result};
// surface 路由表拥有协议编号到窗口身份的映射。
use crate::native::windowing::shared::window_target::SurfaceWindowTargets;

// 指针激活注册表拥有 surface 代次及其未消费授权。
use super::pointer_activation::WaylandPointerActivationRegistry;
// 窗口 owner 类型用于把 Drop adapter 保持在注册事务边界旁。
use super::window_ops::WaylandWindowOps;

// 按固定顺序取得 surface 两份注册事实的健康 guard。
fn lock_surface_registries<'a>(
    // 指针授权注册表始终是第一把锁。
    pointer_activations: &'a Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface 路由注册表始终是第二把锁。
    surface_windows: &'a Arc<Mutex<SurfaceWindowTargets>>,
    // 操作阶段进入稳定诊断上下文。
    operation: &'static str,
    // 返回两份 guard，调用方只能在全部成功后提交修改。
) -> Result<(
    // 第一项 guard 独占授权注册事实。
    MutexGuard<'a, WaylandPointerActivationRegistry>,
    // 第二项 guard 独占窗口路由注册事实。
    MutexGuard<'a, SurfaceWindowTargets>,
)> {
    // 先取得授权注册表，任一 owner 损坏时都不允许修改共享事实。
    let pointer_registry = pointer_activations.lock().map_err(|_| {
        // 构造稳定的授权注册表损坏诊断。
        Error::new(
            // 共享状态已无法安全访问。
            Errc::InvalidState,
            // 保留授权注册表与具体生命周期阶段。
            format!(
                // 使用固定模板，避免底层 poison 文本泄漏。
                "Wayland pointer activation registry mutex poisoned during {operation}"
            ),
        )
    })?;
    // 在执行任何修改前取得第二份注册事实的健康 guard。
    let surface_registry = surface_windows.lock().map_err(|_| {
        // 构造稳定的路由注册表损坏诊断。
        Error::new(
            // 共享状态已无法安全访问。
            Errc::InvalidState,
            // 保留窗口路由注册表与具体生命周期阶段。
            format!("Wayland surface window registry mutex poisoned during {operation}"),
        )
    })?;
    // 两把锁均健康后才把 guard 交给事务操作。
    Ok((pointer_registry, surface_registry))
}

// 原子登记一个窗口 surface 的授权与事件路由注册事实。
pub(crate) fn register_window_surface(
    // 指针授权注册表必须先锁定以保持全后端一致的锁顺序。
    pointer_activations: &Arc<Mutex<WaylandPointerActivationRegistry>>,
    // surface 路由注册表与授权注册表共同组成登记事务。
    surface_windows: &Arc<Mutex<SurfaceWindowTargets>>,
    // 协议编号标识当前待登记的 surface。
    surface_id: u32,
    // 稳定窗口身份绑定两份注册事实。
    window_id: WindowId,
    // 返回 typed failure，调用方据此停止发布协议 owner。
) -> Result<()> {
    // 取得两份健康 guard 后才允许推进 surface generation。
    let (mut pointer_registry, mut surface_registry) = lock_surface_registries(
        // 传入授权注册表共享 owner。
        pointer_activations,
        // 传入窗口路由注册表共享 owner。
        surface_windows,
        // 标记稳定的登记阶段。
        "surface registration",
    )?;
    // 两份 guard 都健康后才登记新的授权代次。
    pointer_registry.register_surface(surface_id, window_id);
    // 在同一事务内登记 surface 到 WindowId 的事件路由。
    surface_registry.register_surface(surface_id, window_id);
    // 两份注册事实均已提交。
    Ok(())
}

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
    // 取得两份健康 guard 后才允许撤销任何注册事实。
    let (mut pointer_registry, mut surface_registry) = lock_surface_registries(
        // 传入授权注册表共享 owner。
        pointer_activations,
        // 传入窗口路由注册表共享 owner。
        surface_windows,
        // 标记稳定的注销阶段。
        "surface unregistration",
    )?;
    // 两份 guard 都健康后才撤销未消费的交互授权。
    pointer_registry.unregister_surface(surface_id, window_id);
    // 在同一事务内撤销 surface 到 WindowId 的事件路由。
    surface_registry.unregister_surface(surface_id);
    // 两份注册事实均已完成幂等注销。
    Ok(())
}

// Drop adapter 复用同一事务，窗口类型仍是协议生命周期 owner。
impl Drop for WaylandWindowOps {
    // 无同步返回通道时把注销失败转交 backend owner-thread。
    fn drop(&mut self) {
        // Drop 必须观察检查式注销结果。
        if let Err(error) = self.unregister_surface() {
            // source 已关闭时不存在更高层 receiver，保持既有 fail-closed 语义。
            let _ = self.pending_failures.enqueue(error);
        }
        // 独立 teardown Adapter 即使遇到 poisoned state 也释放逐窗拖放 owners。
        super::file_drop_window::force_disable_window(&self.file_drop_state, self.window_id);
        // Drop 没有显式重试入口，必须始终释放逐窗 callback owners。
        self.shutdown_window_callbacks();
    }
}
