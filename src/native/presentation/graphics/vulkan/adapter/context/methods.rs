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
        // surface 创建成功后立即交给构造期唯一 owner。
        let mut pending = PendingVulkanContext::new(&surface_loader, surface);

        let selection = match select_queue(&instance, runtime.surface_loader(), pending.surface()) {
            Ok(selection) => selection,
            // 提前返回时 pending Drop 统一销毁 surface。
            Err(err) => return Err(err),
        };
        let queue_family_index = selection.family_index;
        let device_lease = match runtime.acquire_device(selection) {
            Ok(device) => device,
            // device 获取失败时 pending Drop 统一销毁 surface。
            Err(err) => return Err(err),
        };
        // guard 持有额外 lease，保证失败回滚 device child 时 native device 仍存活。
        pending.attach_device(device_lease.clone());
        let physical_device = device_lease.physical_device();
        let adapter_info = device_lease.info().clone();
        let device = device_lease.device().clone();
        let swapchain_loader = ash::khr::swapchain::Device::new(&instance, &device);
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        // SAFETY: device 存活；command_pool_info 为栈上完整初始化的创建描述；分配器传 None。
        let command_pool = match unsafe { device.create_command_pool(&command_pool_info, None) } {
            Ok(pool) => pool,
            // 失败时 pending Drop 仍会销毁 surface 并释放额外 device lease。
            Err(err) => return Err(device_lease.error("vkCreateCommandPool", err)),
        };
        // command pool 创建成功后立即登记到唯一构造 owner。
        pending.set_command_pool(command_pool);
        let command_alloc = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        // SAFETY: device 存活；command_alloc 引用刚创建的 command_pool 且描述完整。
        let command_buffer = match unsafe { device.allocate_command_buffers(&command_alloc) } {
            Ok(mut buffers) => buffers.pop(),
            // 失败时 pending Drop 统一销毁 command pool 与 surface。
            Err(err) => return Err(device_lease.error("vkAllocateCommandBuffers", err)),
        }
        .ok_or_else(|| {
            // 空 command-buffer 结果只构造 typed error，native 回滚由 pending Drop 负责。
            Error::new(Errc::PlatformError, "VulkanContext: no command buffer")
        })?;

        let semaphore_info = vk::SemaphoreCreateInfo::default();
        // SAFETY: device 存活；semaphore_info 为默认初始化的创建描述；分配器传 None。
        let image_available = match unsafe { device.create_semaphore(&semaphore_info, None) } {
            Ok(sem) => sem,
            // 失败时 pending Drop 统一销毁 command pool 与 surface。
            Err(err) => return Err(device_lease.error("vkCreateSemaphore image_available", err)),
        };
        // semaphore 创建成功后立即登记到唯一构造 owner。
        pending.set_image_available(image_available);
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);
        // SAFETY: device 存活；fence_info 为栈上完整初始化的创建描述；分配器传 None。
        let frame_fence = match unsafe { device.create_fence(&fence_info, None) } {
            Ok(fence) => fence,
            // 失败时 pending Drop 统一销毁 semaphore、command pool 与 surface。
            Err(err) => return Err(device_lease.error("vkCreateFence", err)),
        };
        // fence 创建成功后立即登记到唯一构造 owner。
        pending.set_frame_fence(frame_fence);

        // 所有前置资源创建完成后一次性移交给正式 VulkanContext。
        let (surface, command_pool, image_available, frame_fence) = pending.into_handles();

        // 帧内 Uniform 快照偏移必须满足当前物理设备的 descriptor 对齐限制。
        // SAFETY: physical_device 来自同一存活 instance 的 adapter 选择结果。
        let uniform_alignment = unsafe {
            runtime
                .instance()
                .get_physical_device_properties(physical_device)
                .limits
                .min_uniform_buffer_offset_alignment
                .max(1)
        };
        let mut ctx = Self {
            runtime: Some(runtime),
            device_lease: Some(device_lease),
            surface_loader,
            #[cfg(target_os = "linux")]
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
            swapchain_loader,
            swapchain: vk::SwapchainKHR::null(),
            swapchain_images: Vec::new(),
            swapchain_image_views: Vec::new(),
            image_layouts: Vec::new(),
            swapchain_format: vk::Format::UNDEFINED,
            surface_supported_usage_flags: vk::ImageUsageFlags::empty(),
            extent,
            command_pool,
            command_buffer,
            // 资源表必须先于任何 Drawing 资源创建完成初始化。
            rhi_device: VulkanRhiDevice::new(uniform_alignment),
            surface_readback: VulkanSurfaceReadbackBuffer::new(),
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
            acquired_frame: None,
            submitted_frame: None,
            #[cfg(uix_gpu_parity_vulkan)]
            surface_fault_for_parity: None,
            #[cfg(uix_gpu_parity_vulkan)]
            replace_present_sync_for_parity: false,
            surface_lifecycle:
                crate::platform::presentation::rhi::RhiSurfaceLifecycle::uninitialized(
                    crate::platform::presentation::rhi::RhiExtent::new(extent.width, extent.height),
                ),
            native_surface,
            logical_width: drawable.logical_width,
            logical_height: drawable.logical_height,
            width: drawable.width,
            height: drawable.height,
            cpu_shadow: Vec::new(),
            shutdown: false,
        };
        let device = ctx.active_device()?;
        device.observe(ctx.recreate_swapchain(
            extent,
            crate::platform::presentation::rhi::RhiSurfaceRecreateReason::Initialize,
        ))?;
        ctx.width = ctx.extent.width as i32;
        ctx.height = ctx.extent.height as i32;
        // GpuNative 不需要 CPU 像素上传缓冲；PixelUpload 在首次上传时按需申请。
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
        let device = self.device_lease.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "VulkanContext: operation requested after shutdown",
            )
        })?;
        // 所有业务入口在触碰旧代原生对象前统一观察共享 device 的 lost 事实。
        device.ensure_healthy()?;
        Ok(device)
    }

    fn swapchain_maintenance1_enabled(&self) -> bool {
        self.device_lease
            .as_ref()
            .is_some_and(|device| device.swapchain_maintenance1_enabled())
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
        self.recreate_swapchain(
            extent,
            crate::platform::presentation::rhi::RhiSurfaceRecreateReason::Resize,
        )?;
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = self.extent.width as i32;
        self.height = self.extent.height as i32;
        self.cpu_shadow.clear();
        Ok(())
    }

    pub(super) fn recreate_swapchain(
        &mut self,
        extent: vk::Extent2D,
        reason: crate::platform::presentation::rhi::RhiSurfaceRecreateReason,
    ) -> Result<crate::platform::presentation::rhi::RhiSurfaceRecreateCommit> {
        let transaction = self.surface_lifecycle.begin_recreate(
            crate::platform::presentation::rhi::RhiExtent::new(extent.width, extent.height),
            reason,
        )?;
        let requested = transaction.requested();
        match self.recreate_swapchain_native(vk::Extent2D {
            width: requested.width,
            height: requested.height,
        }) {
            Ok(actual) => self.surface_lifecycle.commit_recreate(transaction, actual),
            Err(error) => match self.surface_lifecycle.abort_recreate(transaction) {
                Ok(()) => Err(error),
                Err(lifecycle_error) => Err(lifecycle_error.with_source(error)),
            },
        }
    }

    // Vulkan 私有实现只创建、交换和释放原生对象，不决定当前帧重试语义。
    fn recreate_swapchain_native(
        &mut self,
        extent: vk::Extent2D,
    ) -> Result<crate::platform::presentation::rhi::RhiExtent> {
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
            // reset command buffer 会释放上一帧对旧 image view/framebuffer 的引用。
            self.rhi_device
                .prepare_frame(&self.device, self.command_buffer)?;
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
        let required_usage =
            vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::COLOR_ATTACHMENT;
        if !caps.supported_usage_flags.contains(required_usage) {
            return Err(Error::new(
                Errc::PlatformError,
                "VulkanContext: surface swapchain images do not support transfer and color attachment usage",
            ));
        }
        // Surface 查询是 TRANSFER_SRC 能力的唯一事实来源；不支持时不得强制请求。
        let transfer_src_supported = caps
            .supported_usage_flags
            .contains(vk::ImageUsageFlags::TRANSFER_SRC);
        let image_usage = if transfer_src_supported {
            required_usage | vk::ImageUsageFlags::TRANSFER_SRC
        } else {
            required_usage
        };
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
            .image_usage(image_usage)
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
        let mut new_image_views =
            match create_swapchain_image_views(&self.device, &new_images, surface_format.format) {
                Ok(views) => views,
                Err(error) => {
                    // SAFETY: new_swapchain 尚未移交正式 owner。
                    unsafe { self.swapchain_loader.destroy_swapchain(new_swapchain, None) };
                    return Err(error);
                }
            };
        let new_image_layouts = match allocate_image_layouts(new_images.len()) {
            Ok(layouts) => layouts,
            Err(error) => {
                destroy_image_views(&self.device, &mut new_image_views);
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
                    destroy_image_views(&self.device, &mut new_image_views);
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
        // parity 的 present OUT_OF_DATE 在 queue present 前返回；旧代 signaled semaphore
        // 没有 presentation wait 消费，因此只能在 frame fence 完成后随旧代销毁。
        #[cfg(uix_gpu_parity_vulkan)]
        let reuse_present_sync =
            reuse_present_sync && !std::mem::take(&mut self.replace_present_sync_for_parity);
        let new_render_finished = if reuse_present_sync {
            Vec::new()
        } else {
            match create_render_finished_semaphores(&self.device, new_images.len()) {
                Ok(semaphores) => semaphores,
                Err(error) => {
                    destroy_image_views(&self.device, &mut new_image_views);
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
                    destroy_image_views(&self.device, &mut new_image_views);
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
            // framebuffer 已在上方 reset 后释放，旧 view 不再被命令引用。
            destroy_image_views(&self.device, &mut self.swapchain_image_views);
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
        self.swapchain_image_views = new_image_views;
        self.image_layouts = new_image_layouts;
        self.swapchain_format = surface_format.format;
        // 只在新 swapchain 完整成功后提交本代 Surface usage 事实。
        self.surface_supported_usage_flags = caps.supported_usage_flags;
        self.extent = extent;
        self.acquired_frame = None;
        self.submitted_frame = None;
        tracing::info!(
            "VulkanContext: Surface supported_usage_flags=0x{:08x}; swapchain_format={}({}); transfer_src={}; readback={}",
            self.surface_supported_usage_flags.as_raw(),
            rhi_surface_readback::surface_readback_format_name(self.swapchain_format),
            self.swapchain_format.as_raw(),
            transfer_src_supported,
            transfer_src_supported
                && rhi_surface_readback::supports_surface_readback_format(self.swapchain_format),
        );
        Ok(crate::platform::presentation::rhi::RhiExtent::new(
            self.extent.width,
            self.extent.height,
        ))
    }

    pub(super) fn present_uploaded_pixels(&mut self, owner: &VulkanDevice) -> Result<()> {
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
                    crate::platform::presentation::rhi::RhiSurfaceRecreateReason::AcquisitionRejected,
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
        let submit_result = owner.with_queue("vkQueueSubmit PixelUpload", |queue| unsafe {
            self.device
                .queue_submit(queue, std::slice::from_ref(&submit), self.frame_fence)
        })?;
        if let Err(err) = submit_result {
            // DEVICE_LOST 后禁止再创建 replacement fence；旧 fence 由 shutdown 直接销毁。
            let fence_recovery = if err == vk::Result::ERROR_DEVICE_LOST {
                Ok(())
            } else {
                self.restore_signaled_frame_fence()
            };
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
            owner.with_queue("vkQueuePresentKHR PixelUpload", |queue| unsafe {
                self.swapchain_loader.queue_present(queue, &present)
            })?
        } else {
            let present = vk::PresentInfoKHR::default()
                .wait_semaphores(std::slice::from_ref(&render_finished))
                .swapchains(std::slice::from_ref(&self.swapchain))
                .image_indices(std::slice::from_ref(&image_index));
            // SAFETY: present 引用的 swapchain/semaphore 均存活，切片在调用期间有效。
            owner.with_queue("vkQueuePresentKHR PixelUpload", |queue| unsafe {
                self.swapchain_loader.queue_present(queue, &present)
            })?
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
                        crate::platform::presentation::rhi::RhiSurfaceRecreateReason::PresentedNeedsRecreate,
                    )
                } else {
                    Ok(())
                }
            }
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_after_surface_change(
                "vkQueuePresentKHR",
                vk::Result::ERROR_OUT_OF_DATE_KHR,
                crate::platform::presentation::rhi::RhiSurfaceRecreateReason::PresentationRejected,
            ),
            Err(vk::Result::SUBOPTIMAL_KHR) => {
                // SUBOPTIMAL 已接受本次 present；先登记同步对象，再重建后续代际。
                if present_fence.is_some() {
                    self.present_fences.mark_submitted(image_slot)?;
                } else {
                    self.present_lifetime.mark_presented(image_slot)?;
                }
                self.recreate_after_surface_change(
                    "vkQueuePresentKHR",
                    vk::Result::SUBOPTIMAL_KHR,
                    crate::platform::presentation::rhi::RhiSurfaceRecreateReason::PresentedNeedsRecreate,
                )
            }
            Err(err) => Err(vk_err("vkQueuePresentKHR", err)),
        }
    }

    /// 原生状态只选择共享重建原因；当前帧成功或重试语义由 platform 状态机决定。
    pub(super) fn recreate_after_surface_change(
        &mut self,
        operation: &str,
        status: vk::Result,
        reason: crate::platform::presentation::rhi::RhiSurfaceRecreateReason,
    ) -> Result<()> {
        let surface_failure = vk_err(operation, status);
        match self.recreate_swapchain(self.extent, reason) {
            Ok(commit) => commit.complete_frame(surface_failure),
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
        // shutdown 允许借用已 lost 的旧代 owner，但不得再把它暴露给业务入口。
        let device = self.device_lease.as_ref().cloned().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "VulkanContext: shutdown requested without a device lease",
            )
        })?;
        // 已知 lost 的 peer 直接进入只销毁 child 的收口路径，不再调用失效 device/queue。
        let wait_result = if device.is_lost() {
            Err(vk::Result::ERROR_DEVICE_LOST)
        } else {
            // SAFETY: 健康 device 存活；device_wait_idle 无指针参数并排空同 device 全部 queue。
            let result = unsafe { self.device.device_wait_idle() };
            device.observe_wait(result);
            result
        };
        match wait_result {
            Ok(()) if self.present_fences.enabled() => {
                self.present_fences.wait_and_reset_all(&self.device)?;
            }
            Ok(()) => {
                self.present_lifetime
                    .complete_submission(&self.device, &self.swapchain_loader)?;
            }
            Err(vk::Result::ERROR_DEVICE_LOST) => {}
            // 非 LOST 错误不在 match 内处理：下方 accept 统一转换为 typed
            // Error 传播，此空臂只是 Ok 分支的分派占位。
            Err(_) => {}
        }
        accept_device_wait_for_shutdown(wait_result)?;
        // Drawing 资源依赖 Vulkan device，必须在 command pool 与 device 父对象前逆序回收。
        self.rhi_device.shutdown(&self.device);
        // immediate 命令池已释放后再销毁它曾引用的 Surface staging 资源。
        self.surface_readback.release(&self.device);
        // RHI framebuffer 已释放后才能销毁 swapchain image views。
        destroy_image_views(&self.device, &mut self.swapchain_image_views);
        self.acquired_frame = None;
        self.submitted_frame = None;
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

// GPU 验证专用实现位于 tests-src（模块级 include! 保持原作用域与 cfg），
// 仅测试或显式 RUSTFLAGS parity 配置读取；默认生产构建不读取，发布包不携带。
#[cfg(any(test, uix_gpu_parity_vulkan))]
include!("../../../../../../../tests-src/native/presentation/graphics/vulkan/adapter/context/methods_parity_fns.rs");
