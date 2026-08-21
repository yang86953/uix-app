//! Vulkan GPU-native 对 API 无关全图元规范场景的离屏执行与回读 harness。

use std::ffi::{CStr, CString};

use ash::vk;

use super::*;
use crate::draw::backend::rhi_renderer::consistency::{
    CONSISTENCY_BACKGROUND, CONSISTENCY_EXTENT, ConsistencyScene, canonical_scenes,
    validate_canonical_scenes,
};
use crate::platform::presentation::rhi::{
    DrawBufferBindings, DrawRange, DrawRasterState, DrawSamplingBinding, PipelineKind,
    RhiBufferUpload, RhiTextureUpload, RhiViewport, SampledTextureBinding, TextureFormat,
};

// 保存一个规范场景机械映射后的 Vulkan RHI 身份；不拥有任何像素语义。
struct NativeSceneResources {
    pipeline: PipelineBinding,
    vertex: BufferHandle,
    uniform: BufferHandle,
    sampled: Option<(TextureHandle, SamplerHandle)>,
}

// 选择一个具有 graphics queue 的真实物理设备；无 Vulkan 环境时显式报告跳过。
fn select_graphics_device(
    instance: &ash::Instance,
) -> Option<(vk::PhysicalDevice, u32, vk::PhysicalDeviceProperties)> {
    // SAFETY: instance 在本 harness 的完整生命周期内存活。
    let physical_devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
    for physical_device in physical_devices {
        // SAFETY: physical_device 来自同一 instance 的真实枚举结果。
        let queues =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        if let Some(index) = queues
            .iter()
            .position(|queue| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        {
            // SAFETY: physical_device 仍由当前 instance 持有。
            let properties = unsafe { instance.get_physical_device_properties(physical_device) };
            return Some((physical_device, index as u32, properties));
        }
    }
    None
}

// 创建用于把离屏 RGBA8 target 复制回 CPU 的紧密 staging buffer。
fn create_readback_buffer(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    size: vk::DeviceSize,
) -> (vk::Buffer, vk::DeviceMemory) {
    let create = vk::BufferCreateInfo::default()
        .size(size)
        .usage(vk::BufferUsageFlags::TRANSFER_DST)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    // SAFETY: device 存活，size 来自固定规范画布的精确字节数。
    let buffer = unsafe { device.create_buffer(&create, None) }
        .expect("Vulkan consistency readback buffer must be created");
    // SAFETY: buffer 刚由同一 device 创建。
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type = find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        "Vulkan consistency readback",
    )
    .expect("Vulkan consistency readback memory type must exist");
    let allocation = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type);
    // SAFETY: memory type 与 buffer requirements 来自同一 physical device。
    let memory = unsafe { device.allocate_memory(&allocation, None) }
        .expect("Vulkan consistency readback memory must be allocated");
    // SAFETY: buffer 与 memory 同属当前 device，allocation 满足 requirements。
    unsafe { device.bind_buffer_memory(buffer, memory, 0) }
        .expect("Vulkan consistency readback memory must bind");
    (buffer, memory)
}

// 记录纹理到紧密 staging buffer 的转换与复制。
fn record_readback(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    destination: vk::Buffer,
) {
    let range = vk::ImageSubresourceRange::default()
        .aspect_mask(vk::ImageAspectFlags::COLOR)
        .base_mip_level(0)
        .level_count(1)
        .base_array_layer(0)
        .layer_count(1);
    let barrier = vk::ImageMemoryBarrier::default()
        .src_access_mask(vk::AccessFlags::SHADER_READ)
        .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
        .old_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
        .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(range);
    let copy = vk::BufferImageCopy::default()
        .buffer_offset(0)
        .buffer_row_length(0)
        .buffer_image_height(0)
        .image_subresource(
            vk::ImageSubresourceLayers::default()
                .aspect_mask(vk::ImageAspectFlags::COLOR)
                .mip_level(0)
                .base_array_layer(0)
                .layer_count(1),
        )
        .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
        .image_extent(vk::Extent3D {
            width: CONSISTENCY_EXTENT.width,
            height: CONSISTENCY_EXTENT.height,
            depth: 1,
        });
    // SAFETY: command buffer 正在录制；image 与 buffer 的用途和范围均匹配。
    unsafe {
        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            std::slice::from_ref(&barrier),
        );
        device.cmd_copy_image_to_buffer(
            command_buffer,
            image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            destination,
            std::slice::from_ref(&copy),
        );
    }
}

// 把一个 API 无关场景机械创建为 Vulkan RHI 资源，不解释其期望像素。
fn create_scene_resources(
    rhi: &mut VulkanRhiDevice,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    queue: vk::Queue,
    queue_family: u32,
    scene: &ConsistencyScene,
) -> NativeSceneResources {
    let vertex_bytes = scene.vertex.encode_ne_bytes();
    let uniform_bytes = scene.uniform.encode_ne_bytes();
    let contract = scene.kind.contract();
    let vertex = rhi
        .create_buffer(
            instance,
            physical_device,
            device,
            BufferDesc::vertex(vertex_bytes.len(), contract.vertex.stride_bytes()),
        )
        .expect("Vulkan consistency vertex buffer must be created");
    let uniform = rhi
        .create_buffer(
            instance,
            physical_device,
            device,
            BufferDesc::uniform(uniform_bytes.len()),
        )
        .expect("Vulkan consistency uniform buffer must be created");
    rhi.update_buffer(device, RhiBufferUpload::new(vertex, &vertex_bytes))
        .expect("Vulkan consistency vertices must upload");
    rhi.update_buffer(device, RhiBufferUpload::new(uniform, &uniform_bytes))
        .expect("Vulkan consistency uniforms must upload");
    let pipeline = rhi
        .create_pipeline(device, PipelineDesc { kind: scene.kind })
        .expect("Vulkan consistency pipeline must be created");
    let sampled = scene.texture.as_ref().map(|source| {
        let texture = rhi
            .create_texture(
                instance,
                physical_device,
                device,
                TextureDesc::new(source.extent, source.format),
            )
            .expect("Vulkan consistency sampled texture must be created");
        rhi.update_texture(
            instance,
            physical_device,
            device,
            queue,
            queue_family,
            RhiTextureUpload::full(texture, source.extent, &source.bytes),
        )
        .expect("Vulkan consistency sampled texture must upload");
        let sampler = rhi
            .create_sampler(device, source.sampler)
            .expect("Vulkan consistency sampler must be created");
        (texture, sampler)
    });
    NativeSceneResources {
        pipeline,
        vertex,
        uniform,
        sampled,
    }
}

// 编码一个真实 draw；PipelineKind、ABI、采样和容差均只来自共享规范场景。
fn draw_scene(
    rhi: &mut VulkanRhiDevice,
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    scene: &ConsistencyScene,
    native: &NativeSceneResources,
) {
    let sampling = native
        .sampled
        .map_or_else(DrawSamplingBinding::none, |(texture, sampler)| {
            DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
                texture,
                sampler,
                native.pipeline,
            ))
        });
    let packet = DrawPacket::new(
        native.pipeline,
        DrawBufferBindings::new(native.vertex, native.uniform),
        sampling,
        DrawRasterState::new(
            RhiViewport {
                width: CONSISTENCY_EXTENT.width as f32,
                height: CONSISTENCY_EXTENT.height as f32,
            },
            scene.scissor,
        ),
        DrawRange::vertices(
            scene
                .vertex
                .vertex_count()
                .expect("canonical scene vertex count must be complete"),
        ),
    );
    rhi.draw(instance, physical_device, device, command_buffer, packet)
        .unwrap_or_else(|error| panic!("{} Vulkan draw failed: {error}", scene.name));
}

// 用共享采样点逐类证明 shader 已执行；复杂图元只检查稳定内外/边界不变量。
fn validate_readback(scenes: &[ConsistencyScene], pixels: &[u8]) {
    let row_bytes = CONSISTENCY_EXTENT.width as usize * 4;
    for scene in scenes {
        for sample in &scene.samples {
            let offset = sample.y as usize * row_bytes + sample.x as usize * 4;
            let actual: [u8; 4] = pixels[offset..offset + 4]
                .try_into()
                .expect("canonical sample must address one RGBA pixel");
            assert!(
                sample.accepts(actual),
                "{} {} at ({}, {}): actual {actual:?}, expected {:?}..={:?}, tolerance {:?}({})",
                scene.name,
                sample.semantic,
                sample.x,
                sample.y,
                sample.minimum,
                sample.maximum,
                sample.tolerance,
                sample.tolerance.amount(),
            );
        }
    }
}

pub(super) fn run_gpu_parity_test() {
    // 无 loader 的编译环境不伪造 GPU 结果；真实验证日志会明确打印设备。
    let Ok(entry) = (unsafe { ash::Entry::load() }) else {
        eprintln!("Vulkan consistency skipped: loader unavailable");
        return;
    };
    let app_name = CString::new("uix-vulkan-consistency").expect("static app name must be valid");
    let app = vk::ApplicationInfo::default()
        .application_name(&app_name)
        .api_version(vk::API_VERSION_1_0);
    let instance_create = vk::InstanceCreateInfo::default().application_info(&app);
    // SAFETY: 不启用扩展或 layer，create info 的借用覆盖同步调用。
    let instance = unsafe { entry.create_instance(&instance_create, None) }
        .expect("Vulkan consistency instance must be created");
    let Some((physical_device, queue_family, properties)) = select_graphics_device(&instance)
    else {
        eprintln!("Vulkan consistency skipped: graphics device unavailable");
        // SAFETY: instance 尚未创建任何 child。
        unsafe { instance.destroy_instance(None) };
        return;
    };
    // SAFETY: Vulkan 固定长度 device_name 由驱动保证以 NUL 结束。
    let device_name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }.to_string_lossy();
    eprintln!(
        "Vulkan consistency device: {device_name}; vendor=0x{:04x}; device=0x{:04x}; api={}.{}.{}",
        properties.vendor_id,
        properties.device_id,
        vk::api_version_major(properties.api_version),
        vk::api_version_minor(properties.api_version),
        vk::api_version_patch(properties.api_version),
    );

    let priorities = [1.0f32];
    let queue_create = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family)
        .queue_priorities(&priorities);
    let device_create =
        vk::DeviceCreateInfo::default().queue_create_infos(std::slice::from_ref(&queue_create));
    // SAFETY: queue family 来自当前 physical device 的真实属性。
    let device = unsafe { instance.create_device(physical_device, &device_create, None) }
        .expect("Vulkan consistency device must be created");
    // SAFETY: device 创建时请求了 queue index 0。
    let queue = unsafe { device.get_device_queue(queue_family, 0) };
    let pool_create = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    // SAFETY: queue family 属于当前 device。
    let command_pool = unsafe { device.create_command_pool(&pool_create, None) }
        .expect("Vulkan consistency command pool must be created");
    let command_allocate = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    // SAFETY: command pool 存活且属于当前 device。
    let command_buffer = unsafe { device.allocate_command_buffers(&command_allocate) }
        .expect("Vulkan consistency command buffer must be allocated")[0];
    // SAFETY: device 存活且 create info 无外部借用。
    let fence = unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None) }
        .expect("Vulkan consistency fence must be created");

    let scenes = canonical_scenes();
    validate_canonical_scenes(&scenes).expect("shared consistency architecture gate must pass");
    let mut rhi =
        VulkanRhiDevice::new(properties.limits.min_uniform_buffer_offset_alignment.max(1));
    let target = rhi
        .create_texture(
            &instance,
            physical_device,
            &device,
            TextureDesc::new(CONSISTENCY_EXTENT, TextureFormat::Rgba8Unorm),
        )
        .expect("Vulkan consistency target must be created");
    let resources = scenes
        .iter()
        .map(|scene| {
            create_scene_resources(
                &mut rhi,
                &instance,
                physical_device,
                &device,
                queue,
                queue_family,
                scene,
            )
        })
        .collect::<Vec<_>>();

    rhi.prepare_frame(&device, command_buffer)
        .expect("Vulkan consistency frame must prepare");
    let target_identity = rhi
        .textures
        .resolve_render_target(target)
        .expect("Vulkan consistency target identity must resolve");
    let native_target = rhi
        .texture_target(target)
        .expect("Vulkan consistency native target must resolve");
    let clear =
        RhiColor::from_straight_rgba(CONSISTENCY_BACKGROUND.map(|value| value as f32 / 255.0));
    rhi.begin_render_pass(
        &device,
        command_buffer,
        target_identity,
        native_target,
        LoadAction::Clear(clear),
    )
    .expect("Vulkan consistency render pass must begin");
    // Replace blur 先建立全目标基线，其余十类再覆盖各自隔离区域。
    let draw_order = scenes
        .iter()
        .enumerate()
        .filter(|(_, scene)| scene.kind == PipelineKind::BlurPass)
        .chain(
            scenes
                .iter()
                .enumerate()
                .filter(|(_, scene)| scene.kind != PipelineKind::BlurPass),
        );
    for (index, scene) in draw_order {
        draw_scene(
            &mut rhi,
            &instance,
            physical_device,
            &device,
            command_buffer,
            scene,
            &resources[index],
        );
    }
    rhi.end_render_pass(&device, command_buffer)
        .expect("Vulkan consistency render pass must end");
    rhi.finish_recording(&device, command_buffer)
        .expect("Vulkan consistency frame must finish");
    let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
    // SAFETY: command buffer 已结束，fence 尚未提交。
    unsafe { device.queue_submit(queue, std::slice::from_ref(&submit), fence) }
        .expect("Vulkan consistency draws must submit");
    // SAFETY: fence 覆盖唯一 draw submission。
    unsafe { device.wait_for_fences(&[fence], true, u64::MAX) }
        .expect("Vulkan consistency draws must complete");

    let byte_count = u64::from(CONSISTENCY_EXTENT.width) * u64::from(CONSISTENCY_EXTENT.height) * 4;
    let (readback, readback_memory) =
        create_readback_buffer(&instance, physical_device, &device, byte_count);
    // SAFETY: 前一提交已完成，command buffer 与 fence 都可重置复用。
    unsafe {
        device
            .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
            .expect("Vulkan consistency command buffer must reset");
        device
            .reset_fences(&[fence])
            .expect("Vulkan consistency fence must reset");
    }
    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    // SAFETY: command buffer 已重置且当前没有其它录制者。
    unsafe { device.begin_command_buffer(command_buffer, &begin) }
        .expect("Vulkan consistency readback must begin");
    let image = rhi
        .textures
        .get(target)
        .expect("Vulkan consistency target must remain alive")
        .image();
    record_readback(&device, command_buffer, image, readback);
    // SAFETY: readback 命令完整且不在 render pass 内。
    unsafe { device.end_command_buffer(command_buffer) }
        .expect("Vulkan consistency readback must finish");
    let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
    // SAFETY: command buffer 已结束，fence 已重置。
    unsafe { device.queue_submit(queue, std::slice::from_ref(&submit), fence) }
        .expect("Vulkan consistency readback must submit");
    // SAFETY: fence 覆盖唯一 readback submission。
    unsafe { device.wait_for_fences(&[fence], true, u64::MAX) }
        .expect("Vulkan consistency readback must complete");
    // SAFETY: readback memory 为 HOST_VISIBLE，映射范围与复制字节数相同。
    let mapped = unsafe {
        device
            .map_memory(readback_memory, 0, byte_count, vk::MemoryMapFlags::empty())
            .expect("Vulkan consistency readback memory must map")
    };
    // SAFETY: 映射范围至少含 byte_count 字节且在 unmap 前保持有效。
    let pixels = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), byte_count as usize) };
    validate_readback(&scenes, pixels);
    eprintln!(
        "Vulkan consistency verified: {} PipelineKind, {} sampled invariants",
        scenes.len(),
        scenes
            .iter()
            .map(|scene| scene.samples.len())
            .sum::<usize>(),
    );

    // SAFETY: 映射指针不再使用；GPU 已完成全部命令。
    unsafe {
        device.unmap_memory(readback_memory);
        device.destroy_buffer(readback, None);
        device.free_memory(readback_memory, None);
        device
            .queue_wait_idle(queue)
            .expect("Vulkan consistency queue must idle");
    }
    rhi.shutdown(&device);
    // SAFETY: 所有 RHI child 已销毁，随后按父子顺序释放 harness 对象。
    unsafe {
        device.destroy_fence(fence, None);
        device.destroy_command_pool(command_pool, None);
        device.destroy_device(None);
        instance.destroy_instance(None);
    }
}

#[cfg(test)]
#[test]
fn every_pipeline_matches_shared_consistency_scene_on_real_vulkan_device() {
    run_gpu_parity_test();
}
