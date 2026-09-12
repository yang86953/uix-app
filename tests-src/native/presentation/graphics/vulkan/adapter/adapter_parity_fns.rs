// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

// 真实 GPU 测试不创建 OS 窗口，只选择可创建生产逻辑 device 的 graphics queue。
#[cfg(uix_gpu_parity_vulkan)]
pub(super) fn select_headless_test_queue(instance: &ash::Instance) -> Result<QueueSelection> {
    // SAFETY: instance 在枚举与属性查询期间保持存活。
    let physical_devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|err| vk_err("vkEnumeratePhysicalDevices shared-device test", err))?;
    let mut failures = AdapterSelectionRejections::default();
    for physical_device in physical_devices {
        // SAFETY: physical_device 来自同一 instance 的真实枚举结果。
        let properties = unsafe { instance.get_physical_device_properties(physical_device) };
        let summary = properties_summary(&properties);
        let extension_support = match query_device_extensions(instance, physical_device) {
            Ok(extension_support) => extension_support,
            Err(error) => {
                failures.reject_with_error(&summary, "vkEnumerateDeviceExtensionProperties", error);
                continue;
            }
        };
        if !extension_support.supports_swapchain {
            failures.reject(&summary, "missing VK_KHR_swapchain");
            continue;
        }
        #[cfg(target_os = "macos")]
        if !extension_support.enabled.portability_subset {
            failures.reject(&summary, "missing VK_KHR_portability_subset");
            continue;
        }
        // SAFETY: physical_device 来自当前存活 instance。
        let queues =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        let Some(family_index) = queues
            .iter()
            .position(|queue| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        else {
            failures.reject(&summary, "no graphics queue");
            continue;
        };
        let family_index = family_index as u32;
        return Ok(QueueSelection {
            physical_device,
            family_index,
            info: adapter_info(&properties, family_index),
            extensions: extension_support.enabled,
        });
    }
    Err(failures.into_error())
}
