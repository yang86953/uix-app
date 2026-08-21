//! Vulkan GPU-native 与 CPU 规范颜色的最小离屏像素一致性测试。

use std::ffi::CString;

use ash::vk;

use super::*;
use crate::platform::presentation::rhi::{
    DrawBufferBindings, DrawRange, DrawRasterState, DrawSamplingBinding, PipelineKind,
    RhiBufferUpload, RhiExtent, RhiScissor, RhiTextureUpload, RhiViewport, SampledTextureBinding,
    SamplerDesc, TextureFormat,
};

// 把测试浮点载荷编码为共享 RHI 使用的本机字节序。
fn encode_f32(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}

// 按 UNORM8 规范生成 CPU 基准通道。
fn cpu_unorm8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

// 选择一个具有 graphics queue 的物理设备；无 Vulkan 设备的环境允许跳过。
fn select_graphics_device(
    instance: &ash::Instance,
) -> Option<(vk::PhysicalDevice, u32, vk::PhysicalDeviceProperties)> {
    // SAFETY: instance 在本测试完整生命周期内存活。
    let physical_devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
    for physical_device in physical_devices {
        // SAFETY: 物理设备来自同一 instance 的枚举结果。
        let queues =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
        if let Some(index) = queues
            .iter()
            .position(|queue| queue.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        {
            // SAFETY: 物理设备仍由当前 instance 持有。
            let properties = unsafe { instance.get_physical_device_properties(physical_device) };
            return Some((physical_device, index as u32, properties));
        }
    }
    None
}

// 创建用于把 optimal image 复制回 CPU 的紧密 staging buffer。
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
    // SAFETY: 测试 device 存活，size 是固定 4x4 RGBA8 载荷。
    let buffer = unsafe { device.create_buffer(&create, None) }
        .expect("Vulkan parity readback buffer must be created");
    // SAFETY: buffer 刚由同一 device 创建。
    let requirements = unsafe { device.get_buffer_memory_requirements(buffer) };
    let memory_type = find_memory_type(
        instance,
        physical_device,
        requirements.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        "Vulkan parity readback",
    )
    .expect("Vulkan parity readback memory type must exist");
    let allocation = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type);
    // SAFETY: memory type 与 buffer requirements 来自同一 physical device。
    let memory = unsafe { device.allocate_memory(&allocation, None) }
        .expect("Vulkan parity readback memory must be allocated");
    // SAFETY: buffer 与 memory 同属当前 device，allocation 满足 requirements。
    unsafe { device.bind_buffer_memory(buffer, memory, 0) }
        .expect("Vulkan parity readback memory must bind");
    (buffer, memory)
}

// 记录 texture 到紧密 staging buffer 的转换与复制。
fn record_readback(
    device: &ash::Device,
    command_buffer: vk::CommandBuffer,
    image: vk::Image,
    destination: vk::Buffer,
    extent: RhiExtent,
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
            width: extent.width,
            height: extent.height,
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

pub(super) fn run_gpu_parity_test() {
    // 无 Vulkan loader 的构建环境不伪造失败；有 loader 时必须完整执行真实 GPU 路径。
    let Ok(entry) = (unsafe { ash::Entry::load() }) else {
        return;
    };
    let app_name = CString::new("uix-vulkan-parity").expect("static app name must be valid");
    let app = vk::ApplicationInfo::default()
        .application_name(&app_name)
        .api_version(vk::API_VERSION_1_0);
    let instance_create = vk::InstanceCreateInfo::default().application_info(&app);
    // SAFETY: 不启用扩展或 layer，create info 的借用覆盖同步调用。
    let instance = unsafe { entry.create_instance(&instance_create, None) }
        .expect("Vulkan parity instance must be created");
    let Some((physical_device, queue_family, properties)) = select_graphics_device(&instance)
    else {
        // SAFETY: instance 尚未创建任何 child。
        unsafe { instance.destroy_instance(None) };
        return;
    };
    let priorities = [1.0f32];
    let queue_create = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family)
        .queue_priorities(&priorities);
    let device_create =
        vk::DeviceCreateInfo::default().queue_create_infos(std::slice::from_ref(&queue_create));
    // SAFETY: queue family 来自当前 physical device 的真实属性。
    let device = unsafe { instance.create_device(physical_device, &device_create, None) }
        .expect("Vulkan parity device must be created");
    // SAFETY: device 创建时请求了 queue index 0。
    let queue = unsafe { device.get_device_queue(queue_family, 0) };
    let pool_create = vk::CommandPoolCreateInfo::default()
        .queue_family_index(queue_family)
        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
    // SAFETY: queue family 属于当前 device。
    let command_pool = unsafe { device.create_command_pool(&pool_create, None) }
        .expect("Vulkan parity command pool must be created");
    let command_allocate = vk::CommandBufferAllocateInfo::default()
        .command_pool(command_pool)
        .level(vk::CommandBufferLevel::PRIMARY)
        .command_buffer_count(1);
    // SAFETY: command pool 存活且属于当前 device。
    let command_buffer = unsafe { device.allocate_command_buffers(&command_allocate) }
        .expect("Vulkan parity command buffer must be allocated")[0];
    let fence_create = vk::FenceCreateInfo::default();
    // SAFETY: device 存活且 create info 无外部借用。
    let fence = unsafe { device.create_fence(&fence_create, None) }
        .expect("Vulkan parity fence must be created");

    let extent = RhiExtent::new(4, 4);
    let mut rhi =
        VulkanRhiDevice::new(properties.limits.min_uniform_buffer_offset_alignment.max(1));
    let vertices = encode_f32(&[0.0, 0.0, 4.0, 0.0, 4.0, 4.0, 0.0, 0.0, 4.0, 4.0, 0.0, 4.0]);
    let expected_left = [0.25f32, 0.5, 0.75, 1.0];
    let expected_right = [0.75f32, 0.25, 0.5, 1.0];
    let left_uniforms = encode_f32(&[
        extent.width as f32,
        extent.height as f32,
        0.0,
        0.0,
        expected_left[0],
        expected_left[1],
        expected_left[2],
        expected_left[3],
    ]);
    let right_uniforms = encode_f32(&[
        extent.width as f32,
        extent.height as f32,
        0.0,
        0.0,
        expected_right[0],
        expected_right[1],
        expected_right[2],
        expected_right[3],
    ]);
    let textured_vertices = encode_f32(&[
        0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 4.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 4.0, 4.0,
        1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 4.0, 4.0, 1.0, 1.0,
        1.0, 1.0, 1.0, 1.0, 0.0, 4.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ]);
    let sampled_uniforms = encode_f32(&[extent.width as f32, extent.height as f32, 0.0, 0.0]);
    let vertex = rhi
        .create_buffer(
            &instance,
            physical_device,
            &device,
            BufferDesc::vertex(vertices.len(), 8),
        )
        .expect("Vulkan parity vertex buffer must be created");
    let uniform = rhi
        .create_buffer(
            &instance,
            physical_device,
            &device,
            BufferDesc::uniform(left_uniforms.len()),
        )
        .expect("Vulkan parity uniform buffer must be created");
    let textured_vertex = rhi
        .create_buffer(
            &instance,
            physical_device,
            &device,
            BufferDesc::vertex(textured_vertices.len(), 32),
        )
        .expect("Vulkan parity textured vertex buffer must be created");
    let sampled_uniform = rhi
        .create_buffer(
            &instance,
            physical_device,
            &device,
            BufferDesc::uniform(sampled_uniforms.len()),
        )
        .expect("Vulkan parity sampled uniform buffer must be created");
    rhi.update_buffer(&device, RhiBufferUpload::new(vertex, &vertices))
        .expect("Vulkan parity vertices must upload");
    rhi.update_buffer(&device, RhiBufferUpload::new(uniform, &left_uniforms))
        .expect("Vulkan parity left uniforms must upload");
    rhi.update_buffer(
        &device,
        RhiBufferUpload::new(textured_vertex, &textured_vertices),
    )
    .expect("Vulkan parity textured vertices must upload");
    rhi.update_buffer(
        &device,
        RhiBufferUpload::new(sampled_uniform, &sampled_uniforms),
    )
    .expect("Vulkan parity sampled uniforms must upload");
    let texture = rhi
        .create_texture(
            &instance,
            physical_device,
            &device,
            TextureDesc::new(extent, TextureFormat::Rgba8Unorm),
        )
        .expect("Vulkan parity target must be created");
    let sampled_texture = rhi
        .create_texture(
            &instance,
            physical_device,
            &device,
            TextureDesc::new(RhiExtent::new(1, 1), TextureFormat::Rgba8Unorm),
        )
        .expect("Vulkan parity sampled texture must be created");
    let sampled_pixel = [32u8, 64, 128, 255];
    rhi.update_texture(
        &instance,
        physical_device,
        &device,
        queue,
        queue_family,
        RhiTextureUpload::full(sampled_texture, RhiExtent::new(1, 1), &sampled_pixel),
    )
    .expect("Vulkan parity sampled texture must upload");
    let sampler = rhi
        .create_sampler(&device, SamplerDesc::linear_clamp())
        .expect("Vulkan parity sampler must be created");
    let pipeline_kinds = [
        PipelineKind::SolidMesh,
        PipelineKind::TexturedQuad,
        PipelineKind::GradientRect,
        PipelineKind::GlyphCoverageQuad,
        PipelineKind::ShapeRect,
        PipelineKind::ShapeRectAdditive,
        PipelineKind::BoxShadow,
        PipelineKind::TexturedQuadAdditive,
        PipelineKind::BlurPass,
        PipelineKind::MsdfGlyphQuad,
        PipelineKind::Sector,
    ];
    let pipelines = pipeline_kinds.map(|kind| {
        rhi.create_pipeline(&device, PipelineDesc { kind })
            .expect("every Vulkan UI pipeline resource must be created")
    });
    let pipeline = pipelines[0];
    let textured_pipeline = pipelines[1];
    rhi.prepare_frame(&device, command_buffer)
        .expect("Vulkan parity frame must prepare");
    let target = rhi
        .textures
        .resolve_render_target(texture)
        .expect("Vulkan parity target identity must resolve");
    let native_target = rhi
        .texture_target(texture)
        .expect("Vulkan parity native target must resolve");
    rhi.begin_render_pass(
        &device,
        command_buffer,
        target,
        native_target,
        LoadAction::Clear(RhiColor::transparent()),
    )
    .expect("Vulkan parity render pass must begin");
    let (target_format, render_pass) = rhi
        .frame
        .pipeline_target()
        .expect("Vulkan parity active pipeline target must resolve");
    for binding in pipelines {
        rhi.pipelines
            .get_mut(binding)
            .expect("Vulkan parity pipeline identity must resolve")
            .materialize(&device, target_format, render_pass)
            .expect("every Vulkan UI graphics pipeline must materialize on the real device");
    }
    let left_packet = DrawPacket::new(
        pipeline,
        DrawBufferBindings::new(vertex, uniform),
        DrawSamplingBinding::none(),
        DrawRasterState::new(
            RhiViewport {
                width: extent.width as f32,
                height: extent.height as f32,
            },
            Some(RhiScissor {
                x: 0,
                y: 0,
                width: 1,
                height: extent.height as i32,
            }),
        ),
        DrawRange::vertices(6),
    );
    rhi.draw(
        &instance,
        physical_device,
        &device,
        command_buffer,
        left_packet,
    )
    .expect("Vulkan parity left draw must encode");
    // 同一 Uniform 资源在前一 draw 已录制后再次更新；上传快照池必须冻结左侧颜色。
    rhi.update_buffer(&device, RhiBufferUpload::new(uniform, &right_uniforms))
        .expect("Vulkan parity right uniforms must upload");
    let right_packet = DrawPacket::new(
        pipeline,
        DrawBufferBindings::new(vertex, uniform),
        DrawSamplingBinding::none(),
        DrawRasterState::new(
            RhiViewport {
                width: extent.width as f32,
                height: extent.height as f32,
            },
            Some(RhiScissor {
                x: 1,
                y: 0,
                width: 1,
                height: extent.height as i32,
            }),
        ),
        DrawRange::vertices(6),
    );
    rhi.draw(
        &instance,
        physical_device,
        &device,
        command_buffer,
        right_packet,
    )
    .expect("Vulkan parity right draw must encode");
    let textured_packet = DrawPacket::new(
        textured_pipeline,
        DrawBufferBindings::new(textured_vertex, sampled_uniform),
        DrawSamplingBinding::sampled(SampledTextureBinding::for_pipeline(
            sampled_texture,
            sampler,
            textured_pipeline,
        )),
        DrawRasterState::new(
            RhiViewport {
                width: extent.width as f32,
                height: extent.height as f32,
            },
            Some(RhiScissor {
                x: 2,
                y: 0,
                width: 2,
                height: extent.height as i32,
            }),
        ),
        DrawRange::vertices(6),
    );
    rhi.draw(
        &instance,
        physical_device,
        &device,
        command_buffer,
        textured_packet,
    )
    .expect("Vulkan parity textured draw must encode");
    rhi.end_render_pass(&device, command_buffer)
        .expect("Vulkan parity render pass must end");
    rhi.finish_recording(&device, command_buffer)
        .expect("Vulkan parity frame must finish");
    let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
    // SAFETY: command buffer 已结束，fence 尚未提交。
    unsafe { device.queue_submit(queue, std::slice::from_ref(&submit), fence) }
        .expect("Vulkan parity draw must submit");
    // SAFETY: fence 覆盖唯一 draw submission。
    unsafe { device.wait_for_fences(&[fence], true, u64::MAX) }
        .expect("Vulkan parity draw must complete");

    let byte_count = u64::from(extent.width) * u64::from(extent.height) * 4;
    let (readback, readback_memory) =
        create_readback_buffer(&instance, physical_device, &device, byte_count);
    // SAFETY: 前一提交已完成，command buffer 与 fence 都可重置复用。
    unsafe {
        device
            .reset_command_buffer(command_buffer, vk::CommandBufferResetFlags::empty())
            .expect("Vulkan parity command buffer must reset");
        device
            .reset_fences(&[fence])
            .expect("Vulkan parity fence must reset");
    }
    let begin =
        vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    // SAFETY: command buffer 已重置且当前没有其它录制者。
    unsafe { device.begin_command_buffer(command_buffer, &begin) }
        .expect("Vulkan parity readback must begin");
    let image = rhi
        .textures
        .get(texture)
        .expect("Vulkan parity target must remain alive")
        .image();
    record_readback(&device, command_buffer, image, readback, extent);
    // SAFETY: readback 命令完整且不在 render pass 内。
    unsafe { device.end_command_buffer(command_buffer) }
        .expect("Vulkan parity readback must finish");
    let submit = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
    // SAFETY: command buffer 已结束，fence 已重置。
    unsafe { device.queue_submit(queue, std::slice::from_ref(&submit), fence) }
        .expect("Vulkan parity readback must submit");
    // SAFETY: fence 覆盖唯一 readback submission。
    unsafe { device.wait_for_fences(&[fence], true, u64::MAX) }
        .expect("Vulkan parity readback must complete");
    // SAFETY: readback memory 为 HOST_VISIBLE，映射范围与复制字节数相同。
    let mapped = unsafe {
        device
            .map_memory(readback_memory, 0, byte_count, vk::MemoryMapFlags::empty())
            .expect("Vulkan parity readback memory must map")
    };
    // SAFETY: 映射范围至少含 byte_count 字节且在 unmap 前保持有效。
    let pixels = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), byte_count as usize) };
    let cpu_left = expected_left.map(cpu_unorm8);
    let cpu_right = expected_right.map(cpu_unorm8);
    let cpu_sampled = sampled_pixel;
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        let x = index % extent.width as usize;
        let cpu_reference = match x {
            0 => cpu_left,
            1 => cpu_right,
            _ => cpu_sampled,
        };
        for (actual, reference) in pixel.iter().zip(cpu_reference) {
            assert!(
                actual.abs_diff(reference) <= 1,
                "Vulkan pixel {pixel:?} differs from CPU reference {cpu_reference:?}",
            );
        }
    }
    // SAFETY: 映射指针不再使用；GPU 已完成全部命令。
    unsafe {
        device.unmap_memory(readback_memory);
        device.destroy_buffer(readback, None);
        device.free_memory(readback_memory, None);
        device
            .queue_wait_idle(queue)
            .expect("Vulkan parity queue must idle");
    }
    rhi.shutdown(&device);
    // SAFETY: 所有 RHI child 已销毁，随后按父子顺序释放测试对象。
    unsafe {
        device.destroy_fence(fence, None);
        device.destroy_command_pool(command_pool, None);
        device.destroy_device(None);
        instance.destroy_instance(None);
    }
}

#[cfg(test)]
#[test]
fn solid_mesh_matches_cpu_unorm_reference_on_real_vulkan_device() {
    run_gpu_parity_test();
}
