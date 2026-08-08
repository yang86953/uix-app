use super::*;

impl D3d12Context {
    pub(crate) fn new(native_window: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_window.is_null() {
            return Err(platform_error("D3d12Context: native window handle is null"));
        }
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| d3d12_error("CreateDXGIFactory2", error))?;
        match Self::create_with_factory(
            native_window,
            width,
            height,
            factory.clone(),
            D3d12DriverKind::Hardware,
        ) {
            Ok(context) => Ok(context),
            Err(hardware_error) => {
                tracing::warn!(
                    "D3d12Context: hardware device unavailable; retrying with WARP: {}",
                    hardware_error.what()
                );
                Self::create_with_factory(
                    native_window,
                    width,
                    height,
                    factory,
                    D3d12DriverKind::Warp,
                )
                .map_err(|warp_error| {
                    platform_error(format!(
                        "D3d12Context: hardware and WARP creation failed; hardware=[{}]; warp=[{}]",
                        hardware_error.what(),
                        warp_error.what()
                    ))
                })
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_driver(
        native_window: *mut c_void,
        width: i32,
        height: i32,
        driver: D3d12DriverKind,
    ) -> Result<Self> {
        if native_window.is_null() {
            return Err(platform_error("D3d12Context: native window handle is null"));
        }
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| d3d12_error("CreateDXGIFactory2", error))?;
        Self::create_with_factory(native_window, width, height, factory, driver)
    }

    pub(super) fn create_with_factory(
        native_window: *mut c_void,
        width: i32,
        height: i32,
        factory: IDXGIFactory4,
        driver: D3d12DriverKind,
    ) -> Result<Self> {
        let drawable = win_surface::drawable_size(native_window, width, height);
        let (adapter, device, adapter_info) = match driver {
            D3d12DriverKind::Hardware => select_hardware_adapter(&factory)?,
            D3d12DriverKind::Warp => select_warp_adapter(&factory)?,
        };
        let queue_desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        let queue: ID3D12CommandQueue = unsafe { device.CreateCommandQueue(&queue_desc) }
            .map_err(|error| d3d12_error("ID3D12Device::CreateCommandQueue", error))?;
        let desc = swap_chain_desc(drawable.width, drawable.height);
        let swap_chain1 = unsafe {
            factory.CreateSwapChainForHwnd(
                &queue,
                HWND(native_window),
                &desc,
                None,
                None::<&IDXGIOutput>,
            )
        }
        .map_err(|error| d3d12_error("IDXGIFactory4::CreateSwapChainForHwnd", error))?;
        unsafe { factory.MakeWindowAssociation(HWND(native_window), DXGI_MWA_NO_ALT_ENTER) }
            .map_err(|error| d3d12_error("IDXGIFactory4::MakeWindowAssociation", error))?;
        let swap_chain: IDXGISwapChain3 = swap_chain1
            .cast()
            .map_err(|error| d3d12_error("IDXGISwapChain1::cast<IDXGISwapChain3>", error))?;

        let rtv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            NumDescriptors: FRAME_COUNT as u32,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };
        let rtv_heap: ID3D12DescriptorHeap = unsafe { device.CreateDescriptorHeap(&rtv_heap_desc) }
            .map_err(|error| d3d12_error("ID3D12Device::CreateDescriptorHeap(RTV)", error))?;
        let rtv_stride =
            unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) };

        let mut allocators = Vec::with_capacity(FRAME_COUNT);
        for _ in 0..FRAME_COUNT {
            let allocator: ID3D12CommandAllocator =
                unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }
                    .map_err(|error| d3d12_error("ID3D12Device::CreateCommandAllocator", error))?;
            allocators.push(allocator);
        }
        let command_list: ID3D12GraphicsCommandList = unsafe {
            device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &allocators[0],
                None::<&ID3D12PipelineState>,
            )
        }
        .map_err(|error| d3d12_error("ID3D12Device::CreateCommandList", error))?;
        unsafe { command_list.Close() }
            .map_err(|error| d3d12_error("ID3D12GraphicsCommandList::Close(initial)", error))?;
        let command_list_base: ID3D12CommandList = command_list
            .cast()
            .map_err(|error| d3d12_error("command list cast", error))?;
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }
            .map_err(|error| d3d12_error("ID3D12Device::CreateFence", error))?;
        let fence_event = unsafe { CreateEventW(None, false, false, None) }
            .map_err(|error| d3d12_error("CreateEventW(fence)", error))?;

        let mut context = Self {
            hwnd: native_window,
            _factory: factory,
            _adapter: adapter,
            device,
            queue,
            swap_chain,
            rtv_heap,
            rtv_stride,
            back_buffers: Vec::with_capacity(FRAME_COUNT),
            back_buffer_states: [D3D12_RESOURCE_STATE_PRESENT; FRAME_COUNT],
            allocators,
            command_list,
            command_list_base,
            fence,
            fence_event: Some(fence_event),
            fence_values: [0; FRAME_COUNT],
            next_fence_value: 1,
            frame_index: 0,
            recording: false,
            pending_gpu_resources: Vec::new(),
            adapter_info,
            logical_width: drawable.logical_width,
            logical_height: drawable.logical_height,
            width: drawable.width,
            height: drawable.height,
            fault: None,
            shutdown: false,
        };
        context.rebuild_back_buffers()?;
        tracing::info!(
            "D3d12Context: created {}x{} flip-discard swapchain; {}",
            drawable.width,
            drawable.height,
            context.adapter_info.diagnostic_summary()
        );
        Ok(context)
    }

    pub(super) fn rtv_handle(&self, index: usize) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        let mut handle = unsafe { self.rtv_heap.GetCPUDescriptorHandleForHeapStart() };
        handle.ptr += index * self.rtv_stride as usize;
        handle
    }

    pub(super) fn rebuild_back_buffers(&mut self) -> Result<()> {
        let mut back_buffers = Vec::with_capacity(FRAME_COUNT);
        for index in 0..FRAME_COUNT {
            let buffer: ID3D12Resource = unsafe { self.swap_chain.GetBuffer(index as u32) }
                .map_err(|error| d3d12_error("IDXGISwapChain::GetBuffer", error))?;
            unsafe {
                self.device
                    .CreateRenderTargetView(&buffer, None, self.rtv_handle(index));
            }
            back_buffers.push(buffer);
        }
        self.back_buffers = back_buffers;
        self.back_buffer_states = [D3D12_RESOURCE_STATE_PRESENT; FRAME_COUNT];
        self.frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;
        Ok(())
    }

    pub(super) fn ensure_healthy(&self) -> Result<()> {
        if let Some(fault) = self.fault.as_ref() {
            return Err(Error::new(
                Errc::InvalidState,
                format!("D3d12Context is faulted: {fault}"),
            ));
        }
        if self.shutdown {
            return Err(Error::new(Errc::InvalidState, "D3d12Context is shut down"));
        }
        Ok(())
    }

    pub(super) fn latch_fault(&mut self, operation: &str, error: &Error) {
        if self.fault.is_none() {
            self.fault = Some(format!("{operation}: {}", error.what()));
        }
    }

    pub(super) fn wait_for_fence(&self, value: u64) -> Result<()> {
        if value == 0 || unsafe { self.fence.GetCompletedValue() } >= value {
            return Ok(());
        }
        let event = self
            .fence_event
            .ok_or_else(|| platform_error("D3d12Context: fence event is closed"))?;
        unsafe { self.fence.SetEventOnCompletion(value, event) }
            .map_err(|error| d3d12_error("ID3D12Fence::SetEventOnCompletion", error))?;
        let wait = unsafe { WaitForSingleObject(event, INFINITE) };
        if wait != WAIT_OBJECT_0 {
            return Err(platform_error(format!(
                "D3d12Context: fence wait returned {wait:?}"
            )));
        }
        Ok(())
    }

    pub(super) fn signal(&mut self) -> Result<u64> {
        let value = self.next_fence_value;
        self.next_fence_value = self.next_fence_value.saturating_add(1);
        unsafe { self.queue.Signal(&self.fence, value) }
            .map_err(|error| d3d12_error("ID3D12CommandQueue::Signal", error))?;
        Ok(value)
    }

    pub(super) fn wait_for_gpu(&mut self) -> Result<()> {
        let value = self.signal()?;
        self.wait_for_fence(value)
    }

    pub(super) fn begin_commands(&mut self) -> Result<()> {
        self.ensure_healthy()?;
        if self.recording {
            return Ok(());
        }
        if self.frame_index >= self.back_buffers.len() || self.frame_index >= self.allocators.len()
        {
            let error = platform_error(format!(
                "D3d12Context: invalid frame resources index={} buffers={} allocators={}",
                self.frame_index,
                self.back_buffers.len(),
                self.allocators.len()
            ));
            self.latch_fault("begin_commands", &error);
            return Err(error);
        }
        if let Err(error) = self.wait_for_fence(self.fence_values[self.frame_index]) {
            self.latch_fault("wait_for_frame", &error);
            return Err(error);
        }
        let allocator = &self.allocators[self.frame_index];
        if let Err(error) = unsafe { allocator.Reset() } {
            let error = d3d12_error("ID3D12CommandAllocator::Reset", error);
            self.latch_fault("reset allocator", &error);
            return Err(error);
        }
        let reset_result = unsafe {
            self.command_list
                .Reset(allocator, None::<&ID3D12PipelineState>)
        };
        if let Err(error) = reset_result {
            let error = d3d12_error("ID3D12GraphicsCommandList::Reset", error);
            self.latch_fault("reset command list", &error);
            return Err(error);
        }
        let state = self.back_buffer_states[self.frame_index];
        if state != D3D12_RESOURCE_STATE_RENDER_TARGET {
            record_transition(
                &self.command_list,
                &self.back_buffers[self.frame_index],
                state,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );
            self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_RENDER_TARGET;
        }
        let rtv = self.rtv_handle(self.frame_index);
        unsafe {
            self.command_list
                .OMSetRenderTargets(1, Some(&rtv), true, None);
        }
        self.recording = true;
        Ok(())
    }

    pub(super) fn execute_recording(&mut self) -> Result<()> {
        if !self.recording {
            return Ok(());
        }
        if let Err(error) = unsafe { self.command_list.Close() } {
            let error = d3d12_error("ID3D12GraphicsCommandList::Close", error);
            self.latch_fault("close command list", &error);
            return Err(error);
        }
        unsafe {
            self.queue
                .ExecuteCommandLists(&[Some(self.command_list_base.clone())]);
        }
        self.recording = false;
        Ok(())
    }

    pub(super) fn execute_recording_and_wait(&mut self) -> Result<()> {
        self.execute_recording()?;
        if let Err(error) = self.wait_for_gpu() {
            self.latch_fault("execute_recording_and_wait", &error);
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn transition_current_buffer_to_present_and_wait(&mut self) -> Result<()> {
        self.begin_commands()?;
        let buffer_index = self.frame_index;
        let state = self.back_buffer_states[buffer_index];
        record_transition(
            &self.command_list,
            &self.back_buffers[buffer_index],
            state,
            D3D12_RESOURCE_STATE_PRESENT,
        );
        self.back_buffer_states[buffer_index] = D3D12_RESOURCE_STATE_PRESENT;
        self.execute_recording_and_wait()
    }

    pub(super) fn present_result(&mut self) -> Result<()> {
        self.begin_commands()?;
        let buffer_index = self.frame_index;
        let state = self.back_buffer_states[buffer_index];
        record_transition(
            &self.command_list,
            &self.back_buffers[buffer_index],
            state,
            D3D12_RESOURCE_STATE_PRESENT,
        );
        self.back_buffer_states[buffer_index] = D3D12_RESOURCE_STATE_PRESENT;
        self.execute_recording()?;

        let present = unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)) };
        let fence_value = match self.signal() {
            Ok(value) => value,
            Err(error) => {
                self.latch_fault("present signal", &error);
                return Err(error);
            }
        };
        self.fence_values[buffer_index] = fence_value;
        self.frame_index = unsafe { self.swap_chain.GetCurrentBackBufferIndex() } as usize;
        self.latch_present_result(
            present
                .ok()
                .map_err(|error| d3d12_error("IDXGISwapChain::Present", error)),
        )
    }

    /// A failed DXGI Present leaves the submitted command list in an uncertain
    /// display state. Keep the context alive only long enough for terminal
    /// cleanup; all later recording, resize, and readback work must fail.
    pub(crate) fn latch_present_result(&mut self, result: Result<()>) -> Result<()> {
        if let Err(error) = &result {
            self.latch_fault("present", error);
        }
        result
    }

    pub(crate) fn resize_result(&mut self, width: i32, height: i32) -> Result<()> {
        self.ensure_healthy()?;
        let drawable = win_surface::drawable_size(self.hwnd, width, height);
        if drawable.width == self.width && drawable.height == self.height {
            return Ok(());
        }
        // ResizeBuffers requires every reference released. Normalize the current
        // buffer to PRESENT first so a failed resize can safely rebuild and keep
        // the same state tracking for the original swapchain buffers.
        self.transition_current_buffer_to_present_and_wait()?;
        let old_width = self.width;
        let old_height = self.height;
        self.back_buffers.clear();
        let resize_result = unsafe {
            self.swap_chain.ResizeBuffers(
                FRAME_COUNT as u32,
                drawable.width as u32,
                drawable.height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG(0),
            )
        };
        if let Err(error) = resize_result {
            let resize_error = d3d12_error("IDXGISwapChain::ResizeBuffers", error);
            self.width = old_width;
            self.height = old_height;
            if let Err(rebuild_error) = self.rebuild_back_buffers() {
                let combined = platform_error(format!(
                    "{}; restoring old back buffers also failed: {}",
                    resize_error.what(),
                    rebuild_error.what()
                ));
                self.latch_fault("resize recovery", &combined);
                return Err(combined);
            }
            return Err(resize_error);
        }
        self.logical_width = drawable.logical_width;
        self.logical_height = drawable.logical_height;
        self.width = drawable.width;
        self.height = drawable.height;
        self.fence_values = [0; FRAME_COUNT];
        if let Err(error) = self.rebuild_back_buffers() {
            self.latch_fault("rebuild resized back buffers", &error);
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn read_pixels_result(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> Result<Vec<u32>> {
        let x0 = x.clamp(0, self.width);
        let y0 = y.clamp(0, self.height);
        let x1 = x.saturating_add(width).clamp(x0, self.width);
        let y1 = y.saturating_add(height).clamp(y0, self.height);
        let read_w = x1 - x0;
        let read_h = y1 - y0;
        if read_w <= 0 || read_h <= 0 {
            return Ok(Vec::new());
        }

        self.ensure_healthy()?;
        if self.frame_index >= self.back_buffers.len() {
            return Err(platform_error(format!(
                "D3d12Context: readback frame index {} has only {} buffers",
                self.frame_index,
                self.back_buffers.len()
            )));
        }
        let buffer = self.back_buffers[self.frame_index].clone();
        let desc = unsafe { buffer.GetDesc() };
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total_bytes = 0u64;
        unsafe {
            self.device.GetCopyableFootprints(
                &desc,
                0,
                1,
                0,
                Some(&mut footprint),
                None,
                None,
                Some(&mut total_bytes),
            );
        }
        let readback = create_readback_buffer(&self.device, total_bytes)?;
        self.begin_commands()?;
        let previous_state = self.back_buffer_states[self.frame_index];
        record_transition(
            &self.command_list,
            &buffer,
            previous_state,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        );
        self.back_buffer_states[self.frame_index] = D3D12_RESOURCE_STATE_COPY_SOURCE;
        let mut source = texture_copy_location_subresource(&buffer);
        let mut destination = texture_copy_location_footprint(&readback, footprint);
        unsafe {
            self.command_list
                .CopyTextureRegion(&destination, 0, 0, 0, &source, None);
        }
        release_copy_location(&mut source);
        release_copy_location(&mut destination);
        record_transition(
            &self.command_list,
            &buffer,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
            previous_state,
        );
        self.back_buffer_states[self.frame_index] = previous_state;
        self.pending_gpu_resources.push(readback.clone());
        self.execute_recording_and_wait()?;
        self.pending_gpu_resources.pop();

        let read_range = D3D12_RANGE {
            Begin: 0,
            End: total_bytes as usize,
        };
        let mut mapped = std::ptr::null_mut();
        unsafe { readback.Map(0, Some(&read_range), Some(&mut mapped)) }
            .map_err(|error| d3d12_error("ID3D12Resource::Map(readback)", error))?;
        if mapped.is_null() {
            unsafe { readback.Unmap(0, None) };
            return Err(platform_error("D3d12Context: readback Map returned null"));
        }
        let pixels = copy_mapped_bgra_rows(
            mapped.cast(),
            footprint.Offset as usize,
            footprint.Footprint.RowPitch as usize,
            x0 as usize,
            y0 as usize,
            read_w as usize,
            read_h as usize,
        );
        let written = D3D12_RANGE { Begin: 0, End: 0 };
        unsafe { readback.Unmap(0, Some(&written)) };
        Ok(pixels)
    }

    pub(super) fn drain_for_shutdown(&mut self) -> Result<()> {
        let recording_error = if !self.recording {
            None
        } else if self.fault.is_none() {
            self.execute_recording().err()
        } else {
            match unsafe { self.command_list.Close() } {
                Ok(()) => {
                    self.recording = false;
                    None
                }
                Err(error) => Some(d3d12_error(
                    "ID3D12GraphicsCommandList::Close(discard faulted recording)",
                    error,
                )),
            }
        };
        let value = match self.signal() {
            Ok(value) => value,
            Err(signal_error) => {
                let known = self.fence_values.iter().copied().max().unwrap_or(0);
                let known_wait = self.wait_for_fence(known).err();
                let device_removed = unsafe { self.device.GetDeviceRemovedReason() }.err();
                return Err(platform_error(format!(
                    "D3d12Context: shutdown could not signal a terminal fence: {}; recording_close={}; known_fence_wait={}; device_removed_reason={}",
                    signal_error.what(),
                    recording_error
                        .as_ref()
                        .map(|error| error.what())
                        .unwrap_or_else(|| "ok".to_string()),
                    known_wait
                        .as_ref()
                        .map(|error| error.what())
                        .unwrap_or_else(|| "ok".to_string()),
                    device_removed
                        .as_ref()
                        .map(ToString::to_string)
                        .as_deref()
                        .unwrap_or("none")
                )));
            }
        };
        if let Err(wait_error) = self.wait_for_fence(value) {
            let device_removed = unsafe { self.device.GetDeviceRemovedReason() }.err();
            return Err(platform_error(format!(
                "D3d12Context: shutdown terminal fence wait failed: {}; recording_close={}; device_removed_reason={}",
                wait_error.what(),
                recording_error
                    .as_ref()
                    .map(|error| error.what())
                    .unwrap_or_else(|| "ok".to_string()),
                device_removed
                    .as_ref()
                    .map(ToString::to_string)
                    .as_deref()
                    .unwrap_or("none")
            )));
        }
        if let Some(recording_error) = recording_error {
            return Err(platform_error(format!(
                "D3d12Context: terminal fence drained but the command list could not close: {}",
                recording_error.what()
            )));
        }
        Ok(())
    }

    pub(super) fn retain_gpu_objects_after_undrained_drop(&self) {
        std::mem::forget(self._factory.clone());
        std::mem::forget(self._adapter.clone());
        std::mem::forget(self.device.clone());
        std::mem::forget(self.queue.clone());
        std::mem::forget(self.swap_chain.clone());
        std::mem::forget(self.rtv_heap.clone());
        for buffer in &self.back_buffers {
            std::mem::forget(buffer.clone());
        }
        for allocator in &self.allocators {
            std::mem::forget(allocator.clone());
        }
        for resource in &self.pending_gpu_resources {
            std::mem::forget(resource.clone());
        }
        std::mem::forget(self.command_list.clone());
        std::mem::forget(self.command_list_base.clone());
        std::mem::forget(self.fence.clone());
    }

    pub(crate) fn shutdown_result(&mut self) -> Result<()> {
        if self.shutdown {
            return Ok(());
        }
        if let Err(error) = self.drain_for_shutdown() {
            self.latch_fault("shutdown drain", &error);
            return Err(error);
        }
        self.pending_gpu_resources.clear();
        self.back_buffers.clear();
        if let Some(event) = self.fence_event.take() {
            unsafe {
                let _ = CloseHandle(event);
            }
        }
        self.shutdown = true;
        Ok(())
    }
}
