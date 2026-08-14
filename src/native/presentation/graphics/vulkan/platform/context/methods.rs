//! VulkanContext 实现。

use super::*;

impl VulkanContext {
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        let drawable = drawable_size(native_surface, width, height);
        let extent = vk::Extent2D {
            width: drawable.width as u32,
            height: drawable.height as u32,
        };

        let runtime = VulkanRuntime::acquire()?;
        let instance = runtime.instance().clone();
        let surface_loader = ash::khr::surface::Instance::new(runtime.entry(), &instance);
        let (surface, platform_loader) =
            create_platform_surface(runtime.entry(), &instance, native_surface)?;

        let selection = match select_queue(&instance, runtime.surface_loader(), surface) {
            Ok(selection) => selection,
            Err(err) => {
                destroy_failed_surface(&surface_loader, surface);
                return Err(err);
            }
        };
        let queue_family_index = selection.family_index;
        let device_lease = match runtime.acquire_device(selection) {
            Ok(device) => device,
            Err(err) => {
                destroy_failed_surface(&surface_loader, surface);
                return Err(err);
            }
        };
        let physical_device = device_lease.physical_device();
        let adapter_info = device_lease.info().clone();
        let device = device_lease.device().clone();
        let queue = device_lease.queue();
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: device 存活；command_pool_info 为栈上完整初始化的创建描述；分配器传 None。
        let command_pool = match unsafe { device.create_command_pool(&command_pool_info, None) } {
            Ok(pool) => pool,
            Err(err) => {
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateCommandPool", err));
            }
        };
        let command_alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: device 存活；command_alloc 引用刚创建的 command_pool 且描述完整。
        let command_buffer = match unsafe { device.allocate_command_buffers(&command_alloc) } {
            Ok(mut buffers) => buffers.pop(),
            Err(err) => {
                // SAFETY: command_pool 为本函数刚创建、仍存活且此后不再使用；分配器传 None。
                unsafe {
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkAllocateCommandBuffers", err));
            }
        }
        .ok_or_else(|| {
            // SAFETY: 失败路径同样只销毁本函数刚创建且不再使用的 command_pool。
            unsafe {
                device.destroy_command_pool(command_pool, None);
            }
            destroy_failed_surface(&surface_loader, surface);
            Error::new(Errc::PlatformError, "VulkanContext: no command buffer")
        })?;

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        // SAFETY: device 存活；semaphore_info 为默认初始化的创建描述；分配器传 None。
        let image_available = match unsafe { device.create_semaphore(&semaphore_info, None) } {
            Ok(sem) => sem,
            Err(err) => {
                // SAFETY: command_pool 为本函数刚创建、仍存活且此后不再使用。
                unsafe {
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateSemaphore image_available", err));
            }
        };
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        // SAFETY: device 存活；fence_info 为栈上完整初始化的创建描述；分配器传 None。
        let frame_fence = match unsafe { device.create_fence(&fence_info, None) } {
            Ok(fence) => fence,
            Err(err) => {
                // SAFETY: image_available 与 command_pool 均为本函数刚创建、仍存活且失败后不再使用。
                unsafe {
                    device.destroy_semaphore(image_available, None);
                    device.destroy_command_pool(command_pool, None);
                }
                destroy_failed_surface(&surface_loader, surface);
                return Err(device_lease.error("vkCreateFence", err));
            }
        };

        let mut ctx = Self {
            runtime: Some(runtime),
            device_lease: Some(device_lease),
            surface_loader,
            #[cfg(all(unix, not(target_os = "macos")))]
            _wayland_surface_loader: platform_loader,
            #[cfg(windows)]
            _win32_surface_loader: platform_loader,
            #[cfg(target_os = "macos")]
            _metal_surface_loader: platform_loader,
            #[cfg(target_os = "macos")]
            metal_layer: native_surface,
            surface,
            physical_device,
            adapter_info,
            device,
            queue,
            swapchain_loader,
            swapchain: vk::SwapchainKHR::null(),
            swapchain_images: Vec::new(),
            image_layouts: Vec::new(),
            swapchain_format: vk::Format::UNDEFINED,
            extent,
            command_pool,
            command_buffer,
            upload: UploadBuffer {
                buffer: vk::Buffer::null(),
                memory: vk::DeviceMemory::null(),
                size: 0,
            },
            image_available,
            render_finished: Vec::new(),
            present_fences: PresentFenceSet::empty(),
            present_lifetime: PresentLifetime::new(),
            frame_fence,
            native_surface,
            logical_width: drawable.logical_width,
            logical_height: drawable.logical_height,
            width: drawable.width,
            height: drawable.height,
            cpu_shadow: Vec::new(),
            shutdown: false,
        };
        let device = ctx.active_device()?;
        device.observe(ctx.recreate_swapchain(extent))?;
        ctx.width = ctx.extent.width as i32;
        ctx.height = ctx.extent.height as i32;
        device.observe(ctx.recreate_upload_buffer(staging_size(ctx.width, ctx.height)))?;
        tracing::info!(
            "VulkanContext: created {}x{} swapchain; {}; device_fault_reporting={}; swapchain_maintenance1={}",
            ctx.width,
            ctx.height,
            ctx.adapter_info.diagnostic_summary(),
            device.fault_reporting_enabled(),
            device.swapchain_maintenance1_enabled()
        );
        Ok(ctx)
    }

    pub(super) fn active_device(&self) -> Result<Rc<VulkanDevice>> {
        self.device_lease.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "VulkanContext: operation requested after shutdown",
            )
        })
    }

    fn swapchain_maintenance1_enabled(&self) -> bool {
        self.device_lease
            .as_ref()
            .is_some_and(|device| device.swapchain_maintenance1_enabled())
    }

    #[cfg(any(test, all(windows, feature = "vulkan")))]
    pub(crate) fn shared_device_identity(&self) -> usize {
        self.device_lease
            .as_ref()
            .map_or(0, |device| Rc::as_ptr(device) as usize)
    }

    #[cfg(any(test, all(windows, feature = "vulkan")))]
    pub(crate) fn device_fault_reporting_enabled_for_test(&self) -> bool {
        self.device_lease
            .as_ref()
            .is_some_and(|device| device.fault_reporting_enabled())
    }

    #[cfg(any(test, all(windows, feature = "vulkan")))]
    pub(crate) fn swapchain_maintenance1_enabled_for_test(&self) -> bool {
        self.device_lease
            .as_ref()
            .is_some_and(|device| device.swapchain_maintenance1_enabled())
    }

    #[cfg(any(test, all(windows, feature = "vulkan")))]
    pub(crate) fn mark_shared_device_lost_for_test(&self) {
        if let Some(device) = self.device_lease.as_ref() {
            device.mark_lost();
        }
    }

    pub(super) fn resize_active(&mut self, width: i32, height: i32) -> Result<()> {
        let drawable = drawable_size(self.native_surface, width, height);
        if drawable.width == self.width && drawable.height == self.height {
            self.logical_width = drawable.logical_width;
            self.logical_height = drawable.logical_height;
            return Ok(());
        }
        let extent = vk::Extent2D {
            width: drawable.width as u32,
            height: drawable.height as u32,
        };
        self.recreate_swapchain(extent)?;
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = self.extent.width as i32;
        self.height = self.extent.height as i32;
        self.cpu_shadow.clear();
        Ok(())
    }

    pub(super) fn recreate_swapchain(&mut self, extent: vk::Extent2D) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            // MoltenVK surface capabilities follow CAMetalLayer.drawableSize.
            // SAFETY: metal_layer is the AppKit-owned layer passed as native_surface.
            unsafe {
                macos_surface::set_metal_layer_drawable_size(
                    self.metal_layer,
                    extent.width as i32,
                    extent.height as i32,
                );
            }
        }
        let old_swapchain = self.swapchain;
        let maintenance1 = self.swapchain_maintenance1_enabled();
        if old_swapchain != vk::SwapchainKHR::null() {
            self.wait_for_frame_fence("vkWaitForFences before swapchain recreate")?;
            if maintenance1 {
                self.present_fences.wait_and_reset_all(&self.device)?;
            } else {
                self.present_lifetime
                    .complete_submission(&self.device, &self.swapchain_loader)?;
                self.present_lifetime.reserve_retirement()?;
            }
        }

        // SAFETY: physical_device 与 surface 存活，查询为只读操作。
        let caps = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(self.physical_device, self.surface)
        }
        .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceCapabilitiesKHR", err))?;
        // SAFETY: physical_device 与 surface 存活，查询为只读操作。
        let formats = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(self.physical_device, self.surface)
        }
        .map_err(|err| vk_err("vkGetPhysicalDeviceSurfaceFormatsKHR", err))?;
        // SAFETY: physical_device 与 surface 存活，查询为只读操作。
        let present_modes = unsafe {
            self.surface_loader
                .get_physical_device_surface_present_modes(self.physical_device, self.surface)
        }
        .map_err(|err| vk_err("vkGetPhysicalDeviceSurfacePresentModesKHR", err))?;

        validate_swapchain_support(&formats, &present_modes)?;

        let surface_format = choose_surface_format(&formats);
        let present_mode = choose_present_mode(&present_modes);
        let extent = choose_extent(caps, extent);
        if !caps
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_DST)
        {
            return Err(Error::new(
                Errc::PlatformError,
                "VulkanContext: surface swapchain images do not support TRANSFER_DST",
            ));
        }
        let composite_alpha =
            choose_composite_alpha(caps.supported_composite_alpha).ok_or_else(|| {
                Error::new(
                    Errc::PlatformError,
                    "VulkanContext: surface has no supported composite alpha mode",
                )
            })?;
        let desired_images = caps.min_image_count.saturating_add(1).max(2);
        let image_count = if caps.max_image_count > 0 {
            desired_images.min(caps.max_image_count)
        } else {
            desired_images
        };
        let pre_transform = if caps
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            caps.current_transform
        };
        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(composite_alpha)
            .present_mode(present_mode)
            .old_swapchain(old_swapchain)
            .clipped(true);
        // SAFETY: create_info 为栈上完整初始化的创建描述，引用的 surface/old_swapchain 均存活；分配器传 None。
        let new_swapchain = unsafe { self.swapchain_loader.create_swapchain(&create_info, None) }
            .map_err(|err| vk_err("vkCreateSwapchainKHR", err))?;
        // SAFETY: new_swapchain 为刚创建存活的交换链，查询其 image 为只读操作。
        let new_images = match unsafe { self.swapchain_loader.get_swapchain_images(new_swapchain) }
        {
            Ok(images) => images,
            Err(err) => {
                // SAFETY: new_swapchain 为本函数刚创建、仍存活且失败后不再使用。
                unsafe {
                    self.swapchain_loader.destroy_swapchain(new_swapchain, None);
                }
                return Err(vk_err("vkGetSwapchainImagesKHR", err));
            }
        };
        if let Err(error) = validate_swapchain_images(&new_images) {
            // SAFETY: new_swapchain 仍存活且校验失败后不再使用。
            unsafe {
                self.swapchain_loader.destroy_swapchain(new_swapchain, None);
            }
            return Err(error);
        }
        let new_image_layouts = match allocate_image_layouts(new_images.len()) {
            Ok(layouts) => layouts,
            Err(error) => {
                // SAFETY: new_swapchain 仍存活且分配失败后不再使用。
                unsafe {
                    self.swapchain_loader.destroy_swapchain(new_swapchain, None);
                }
                return Err(error);
            }
        };
        let new_presented_images = if maintenance1 {
            Vec::new()
        } else {
            match allocate_presented_images(new_images.len()) {
                Ok(presented) => presented,
                Err(error) => {
                    // SAFETY: new_swapchain 仍存活且分配失败后不再使用。
                    unsafe {
                        self.swapchain_loader.destroy_swapchain(new_swapchain, None);
                    }
                    return Err(error);
                }
            }
        };
        let reuse_present_sync = maintenance1
            && old_swapchain != vk::SwapchainKHR::null()
            && self.render_finished.len() == new_images.len()
            && self.present_fences.len() == new_images.len();
        let new_render_finished = if reuse_present_sync {
            Vec::new()
        } else {
            match create_render_finished_semaphores(&self.device, new_images.len()) {
                Ok(semaphores) => semaphores,
                Err(error) => {
                    // SAFETY: new_swapchain 仍存活且创建失败后不再使用。
                    unsafe {
                        self.swapchain_loader.destroy_swapchain(new_swapchain, None);
                    }
                    return Err(error);
                }
            }
        };
        let new_present_fences = if reuse_present_sync {
            PresentFenceSet::empty()
        } else {
            match PresentFenceSet::create(&self.device, new_images.len(), maintenance1) {
                Ok(fences) => fences,
                Err(error) => {
                    let mut semaphores = new_render_finished;
                    destroy_semaphores(&self.device, &mut semaphores);
                    // SAFETY: new_swapchain 仍存活且 fence 创建失败后不再使用。
                    unsafe {
                        self.swapchain_loader.destroy_swapchain(new_swapchain, None);
                    }
                    return Err(error);
                }
            }
        };

        let (mut old_render_finished, mut old_present_fences) = if reuse_present_sync {
            (Vec::new(), PresentFenceSet::empty())
        } else {
            (
                std::mem::replace(&mut self.render_finished, new_render_finished),
                std::mem::replace(&mut self.present_fences, new_present_fences),
            )
        };
        if old_swapchain != vk::SwapchainKHR::null() {
            if maintenance1 {
                // SAFETY: 每个成功 present 的 maintenance1 fence 均已等待完成。
                unsafe {
                    self.swapchain_loader.destroy_swapchain(old_swapchain, None);
                }
                destroy_semaphores(&self.device, &mut old_render_finished);
            } else {
                self.present_lifetime
                    .retire_reserved(old_swapchain, old_render_finished);
            }
        } else {
            destroy_semaphores(&self.device, &mut old_render_finished);
        }
        old_present_fences.destroy(&self.device);
        self.present_lifetime.begin_generation(new_presented_images);
        self.swapchain = new_swapchain;
        self.swapchain_images = new_images;
        self.image_layouts = new_image_layouts;
        self.swapchain_format = surface_format.format;
        self.extent = extent;
        Ok(())
    }

    pub(super) fn present_uploaded_pixels(&mut self) -> Result<()> {
        // SAFETY: swapchain 存活；超时与信号量参数有效，fence 传 null 表示不等待。
        let (image_index, acquire_suboptimal) = match unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available,
                vk::Fence::null(),
            )
        } {
            Ok((index, suboptimal)) => (index, suboptimal),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                return self.recreate_after_surface_change(
                    "vkAcquireNextImageKHR",
                    vk::Result::ERROR_OUT_OF_DATE_KHR,
                );
            }
            Err(err) => return Err(vk_err("vkAcquireNextImageKHR", err)),
        };

        let image_slot = image_index as usize;
        let render_finished = self.render_finished.get(image_slot).copied().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                format!(
                    "VulkanContext: acquired swapchain image {image_index} without a present semaphore"
                ),
            )
        })?;
        let release_count = if self.present_fences.enabled() {
            0
        } else {
            self.present_lifetime
                .release_count_for_acquire(image_slot)?
        };
        let present_fence = self
            .present_fences
            .prepare_for_present(&self.device, image_slot)?;
        self.record_upload_commands(image_slot)?;
        // SAFETY: frame_fence 为存活且非 SIGNALED 状态的 fence（acquire 后未提交）；reset 数组引用有效。
        unsafe {
            self.device
                .reset_fences(&[self.frame_fence])
                .map_err(|err| vk_err("vkResetFences", err))?;
        }
        let wait_stages = [vk::PipelineStageFlags::TRANSFER];
        let submit = vk::SubmitInfo::default()
            .wait_semaphores(std::slice::from_ref(&self.image_available))
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(std::slice::from_ref(&self.command_buffer))
            .signal_semaphores(std::slice::from_ref(&render_finished));
        // SAFETY: submit 引用的 semaphore/command buffer/fence 均存活且状态合法；切片引用在调用期间有效。
        if let Err(err) = unsafe {
            self.device
                .queue_submit(self.queue, std::slice::from_ref(&submit), self.frame_fence)
        } {
            let fence_recovery = self.restore_signaled_frame_fence();
            return Err(failed_submit_error(err, fence_recovery));
        }
        if present_fence.is_none() {
            self.present_lifetime.on_submission_queued(release_count);
        }
        self.image_layouts[image_slot] = vk::ImageLayout::PRESENT_SRC_KHR;
        let present_match = if let Some(present_fence) = present_fence {
            let fences = [present_fence];
            let mut fence_info = vk::SwapchainPresentFenceInfoEXT::default().fences(&fences);
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(std::slice::from_ref(&render_finished))
                .swapchains(std::slice::from_ref(&self.swapchain))
                .image_indices(std::slice::from_ref(&image_index))
                .push_next(&mut fence_info);
            // SAFETY: present 引用的 swapchain/semaphore/fence_info 均存活，切片在调用期间有效。
            unsafe { self.swapchain_loader.queue_present(self.queue, &present) }
        } else {
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(std::slice::from_ref(&render_finished))
                .swapchains(std::slice::from_ref(&self.swapchain))
                .image_indices(std::slice::from_ref(&image_index));
            // SAFETY: present 引用的 swapchain/semaphore 均存活，切片在调用期间有效。
            unsafe { self.swapchain_loader.queue_present(self.queue, &present) }
        };
        match present_match {
            Ok(present_suboptimal) => {
                if present_fence.is_some() {
                    self.present_fences.mark_submitted(image_slot)?;
                } else {
                    self.present_lifetime.mark_presented(image_slot)?;
                }
                if acquire_suboptimal || present_suboptimal {
                    self.recreate_after_surface_change(
                        "vkQueuePresentKHR",
                        vk::Result::SUBOPTIMAL_KHR,
                    )
                } else {
                    Ok(())
                }
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_after_surface_change(
                "vkQueuePresentKHR",
                vk::Result::ERROR_OUT_OF_DATE_KHR,
            ),
            Err(vk::Result::SUBOPTIMAL_KHR) => {
                self.recreate_after_surface_change("vkQueuePresentKHR", vk::Result::SUBOPTIMAL_KHR)
            }
            Err(err) => Err(vk_err("vkQueuePresentKHR", err)),
        }
    }

    /// A swapchain status requires recreation, but the frame that observed it
    /// was not presented. Return a typed failure after a successful rebuild so
    /// the engine preserves dirty state and retries on the next frame.
    pub(super) fn recreate_after_surface_change(&mut self, operation: &str, status: vk::Result) -> Result<()> {
        let surface_failure = vk_err(operation, status);
        match self.recreate_swapchain(self.extent) {
            Ok(()) => Err(surface_failure),
            Err(recreate_failure) => Err(merge_surface_recreate_failure(
                surface_failure,
                recreate_failure,
            )),
        }
    }

    pub(super) fn shutdown_result(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }
        // 当前 context 的提交在返回前已由 frame fence 排空；device lost 时
        // Vulkan 仍要求显式销毁本窗口拥有的 child object。
        let device = self.active_device()?;
        // SAFETY: device 存活；device_wait_idle 无指针参数，等待本 device 全部队列完成。
        let wait_result = unsafe { self.device.device_wait_idle() };
        device.observe_wait(wait_result);
        match wait_result {
            Ok(()) if self.present_fences.enabled() => {
                self.present_fences.wait_and_reset_all(&self.device)?;
            }
            Ok(()) => {
                self.present_lifetime
                    .complete_submission(&self.device, &self.swapchain_loader)?;
            }
            Err(vk::Result::ERROR_DEVICE_LOST) => {}
            Err(_) => {}
        }
        accept_device_wait_for_shutdown(wait_result)?;
        // SAFETY: device 已 idle（或 device lost），销毁的 child 对象均存活且不再被提交引用；分配器传 None。
        unsafe {
            if self.upload.buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.upload.buffer, None);
            }
            if self.upload.memory != vk::DeviceMemory::null() {
                self.device.free_memory(self.upload.memory, None);
            }
            if self.frame_fence != vk::Fence::null() {
                self.device.destroy_fence(self.frame_fence, None);
            }
            if self.image_available != vk::Semaphore::null() {
                self.device.destroy_semaphore(self.image_available, None);
            }
            if self.command_pool != vk::CommandPool::null() {
                self.device.destroy_command_pool(self.command_pool, None);
            }
            if self.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_loader
                    .destroy_swapchain(self.swapchain, None);
            }
        }
        destroy_semaphores(&self.device, &mut self.render_finished);
        self.present_fences.destroy(&self.device);
        self.present_lifetime
            .destroy_all(&self.device, &self.swapchain_loader);
        if self.surface != vk::SurfaceKHR::null() {
            // SAFETY: 当前与 retired swapchain 均已销毁，surface 不再被 child 引用。
            unsafe { self.surface_loader.destroy_surface(self.surface, None) };
        }
        self.device_lease.take();
        self.runtime.take();
        self.shutdown = true;
        Ok(())
    }
}

