//! VulkanContext 构造期 native 资源的唯一所有权守卫。

// 引入引用计数 owner，确保 child 回滚完成前 Vulkan device 保持存活。
use std::rc::Rc;

// 复用同一 Vulkan context module 的私有类型与清理 helper。
use super::*;

// 在 VulkanContext 成功交付前唯一持有 surface 与 device child 资源。
pub(super) struct PendingVulkanContext {
    // 保存可销毁构造期 surface 的 loader 副本。
    surface_loader: ash::khr::surface::Instance,
    // 保存尚未交付给正式 context 的 surface。
    surface: vk::SurfaceKHR,
    // 保活创建 child 的共享 device，并提供唯一销毁入口。
    device_lease: Option<Rc<VulkanDevice>>,
    // 保存已经创建且尚未交付的 command pool。
    command_pool: vk::CommandPool,
    // 保存已经创建且尚未交付的 image-available semaphore。
    image_available: vk::Semaphore,
    // 保存已经创建且尚未交付的 frame fence。
    frame_fence: vk::Fence,
    // 区分仍需回滚与已经成功移交的构造事务。
    armed: bool,
}

// 实现构造期资源登记、回滚与成功移交。
impl PendingVulkanContext {
    // 从已经成功创建的 Vulkan surface 建立唯一构造 owner。
    pub(super) fn new(
        // 借用创建 surface 的 loader，并在 guard 内保存轻量副本。
        surface_loader: &ash::khr::surface::Instance,
        // 接管尚未交付的 surface。
        surface: vk::SurfaceKHR,
    ) -> Self {
        // 初始化仅拥有 surface 的构造事务。
        Self {
            // loader 副本不复制 native surface 所有权。
            surface_loader: surface_loader.clone(),
            // 保存唯一 surface 句柄。
            surface,
            // device 尚未取得。
            device_lease: None,
            // command pool 尚未创建。
            command_pool: vk::CommandPool::null(),
            // semaphore 尚未创建。
            image_available: vk::Semaphore::null(),
            // fence 尚未创建。
            frame_fence: vk::Fence::null(),
            // surface 已存在，因此 guard 立即进入 armed 状态。
            armed: true,
        }
    }

    // 返回仍由 guard 唯一持有的 surface，供只读能力查询使用。
    pub(super) fn surface(&self) -> vk::SurfaceKHR {
        // Vulkan handle 是复制语义，所有权仍保留在 guard。
        self.surface
    }

    // 登记创建 device child 所需的共享 device lease。
    pub(super) fn attach_device(&mut self, device_lease: Rc<VulkanDevice>) {
        // 构造事务只允许接入一次 device owner。
        debug_assert!(self.device_lease.is_none());
        // guard 持有 lease，保证其 Drop 清理 child 时 device 仍存活。
        self.device_lease = Some(device_lease);
    }

    // 登记刚创建成功的 command pool。
    pub(super) fn set_command_pool(&mut self, command_pool: vk::CommandPool) {
        // 禁止覆盖尚未移交或销毁的 command pool。
        debug_assert_eq!(self.command_pool, vk::CommandPool::null());
        // 保存唯一 native 句柄。
        self.command_pool = command_pool;
    }

    // 登记刚创建成功的 image-available semaphore。
    pub(super) fn set_image_available(&mut self, image_available: vk::Semaphore) {
        // 禁止覆盖尚未移交或销毁的 semaphore。
        debug_assert_eq!(self.image_available, vk::Semaphore::null());
        // 保存唯一 native 句柄。
        self.image_available = image_available;
    }

    // 登记刚创建成功的 frame fence。
    pub(super) fn set_frame_fence(&mut self, frame_fence: vk::Fence) {
        // 禁止覆盖尚未移交或销毁的 fence。
        debug_assert_eq!(self.frame_fence, vk::Fence::null());
        // 保存唯一 native 句柄。
        self.frame_fence = frame_fence;
    }

    // 成功创建后一次性移交正式 VulkanContext 所需的全部 native 句柄。
    pub(super) fn into_handles(
        // 消费构造期唯一 owner，防止移交后继续回滚。
        mut self,
    ) -> (
        // 返回正式 surface。
        vk::SurfaceKHR,
        // 返回正式 command pool。
        vk::CommandPool,
        // 返回正式 image-available semaphore。
        vk::Semaphore,
        // 返回正式 frame fence。
        vk::Fence,
    ) {
        // 成功路径必须已经接入 device lease。
        debug_assert!(self.device_lease.is_some());
        // 成功路径必须已经登记 command pool。
        debug_assert_ne!(self.command_pool, vk::CommandPool::null());
        // 成功路径必须已经登记 semaphore。
        debug_assert_ne!(self.image_available, vk::Semaphore::null());
        // 成功路径必须已经登记 fence。
        debug_assert_ne!(self.frame_fence, vk::Fence::null());
        // 从 guard 取出 surface。
        let surface = std::mem::replace(&mut self.surface, vk::SurfaceKHR::null());
        // 从 guard 取出 command pool。
        let command_pool = std::mem::replace(&mut self.command_pool, vk::CommandPool::null());
        // 从 guard 取出 semaphore。
        let image_available = std::mem::replace(&mut self.image_available, vk::Semaphore::null());
        // 从 guard 取出 fence。
        let frame_fence = std::mem::replace(&mut self.frame_fence, vk::Fence::null());
        // 句柄已经完成显式移交，Drop 不得执行 native 回滚。
        self.armed = false;
        // 释放 guard 的额外 lease；调用方仍持有将移入正式 context 的 lease。
        self.device_lease = None;
        // 返回正式 context 将接管的句柄集合。
        (surface, command_pool, image_available, frame_fence)
    }

    // 按 fence、semaphore、command pool、surface 的依赖逆序执行回滚。
    fn rollback(&mut self) {
        // 已完成回滚或成功移交时保持幂等空操作。
        if !self.armed {
            // 禁止重复触碰已经释放或移交的 native 句柄。
            return;
        }
        // device child 只有在共享 device lease 存活时才允许销毁。
        if let Some(device_lease) = self.device_lease.as_ref() {
            // 取得与 child 同源且仍由 lease 保活的 Vulkan device。
            let device = device_lease.device();
            // SAFETY: 所有非空句柄均由该 device 在本构造事务中创建，尚未交付且不会再提交。
            unsafe {
                // fence 是最晚创建的 child，必须最先销毁。
                if self.frame_fence != vk::Fence::null() {
                    // 销毁唯一持有的 frame fence。
                    device.destroy_fence(self.frame_fence, None);
                    // 清空 owner 槽位，保证幂等。
                    self.frame_fence = vk::Fence::null();
                }
                // semaphore 依赖 device，且在 command pool 之前回收。
                if self.image_available != vk::Semaphore::null() {
                    // 销毁唯一持有的 image-available semaphore。
                    device.destroy_semaphore(self.image_available, None);
                    // 清空 owner 槽位，保证幂等。
                    self.image_available = vk::Semaphore::null();
                }
                // command buffer 随其 parent command pool 一并释放。
                if self.command_pool != vk::CommandPool::null() {
                    // 销毁唯一持有的 command pool。
                    device.destroy_command_pool(self.command_pool, None);
                    // 清空 owner 槽位，保证幂等。
                    self.command_pool = vk::CommandPool::null();
                }
            }
        } else {
            // 未接入 device 时不应存在任何 device child。
            debug_assert_eq!(self.frame_fence, vk::Fence::null());
            // 同步核对 semaphore owner 槽位。
            debug_assert_eq!(self.image_available, vk::Semaphore::null());
            // 同步核对 command pool owner 槽位。
            debug_assert_eq!(self.command_pool, vk::CommandPool::null());
        }
        // surface 必须在全部 device child 销毁后回收。
        if self.surface != vk::SurfaceKHR::null() {
            // 复用平台 surface helper 销毁唯一持有的 native surface。
            destroy_failed_surface(&self.surface_loader, self.surface);
            // 清空 owner 槽位，保证幂等。
            self.surface = vk::SurfaceKHR::null();
        }
        // 所有构造期资源均已完成回滚。
        self.armed = false;
    }
}

// 让任意提前返回都自动触发唯一构造 owner 的完整回滚。
impl Drop for PendingVulkanContext {
    // 执行不可失败的 Vulkan child/surface 销毁序列。
    fn drop(&mut self) {
        // 统一处理成功移交前的全部错误出口。
        self.rollback();
    }
}
