//! Vulkan swapchain 逐 image 簿记与同步对象所有权。

use ash::vk;

use crate::core::{Errc, Error, Result};

use super::vk_err;

pub(crate) fn allocate_image_layouts(image_count: usize) -> Result<Vec<vk::ImageLayout>> {
    let mut layouts = Vec::new();
    layouts.try_reserve_exact(image_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: swapchain layout allocation for {image_count} images failed: {error}"
            ),
        )
    })?;
    layouts.resize(image_count, vk::ImageLayout::UNDEFINED);
    Ok(layouts)
}

pub(super) fn create_render_finished_semaphores(
    device: &ash::Device,
    image_count: usize,
) -> Result<Vec<vk::Semaphore>> {
    let create_info = vk::SemaphoreCreateInfo::default();
    let mut semaphores = Vec::new();
    semaphores.try_reserve_exact(image_count).map_err(|error| {
        Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "VulkanContext: present semaphore allocation for {image_count} images failed: {error}"
            ),
        )
    })?;
    for image_index in 0..image_count {
        // SAFETY: device 存活；create_info 不含调用后保留的指针。
        match unsafe { device.create_semaphore(&create_info, None) } {
            Ok(semaphore) => semaphores.push(semaphore),
            Err(error) => {
                destroy_semaphores(device, &mut semaphores);
                return Err(vk_err(
                    &format!("vkCreateSemaphore render_finished[{image_index}]"),
                    error,
                ));
            }
        }
    }
    Ok(semaphores)
}

pub(super) fn destroy_semaphores(device: &ash::Device, semaphores: &mut Vec<vk::Semaphore>) {
    for semaphore in semaphores.drain(..) {
        // SAFETY: semaphore 由同一 device 创建，且调用方已完成相应 teardown 等待。
        unsafe { device.destroy_semaphore(semaphore, None) };
    }
}
