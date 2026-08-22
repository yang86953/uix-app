//! D3D12 薄 RHI 的 render-target、pass、clear 与提交事务。
//!
//! 本模块只把 platform 唯一 pass/submission 合同机械编码为 D3D12 命令；
//! draw 由同级专用模块消费活动目标事实，组合入口继续保持未激活。

use super::*;

use crate::platform::presentation::rhi::{RhiExtent, RhiViewport, UIX_COLOR_CLEAR_CONTRACT};
use ::windows::Win32::Foundation::RECT;
use ::windows::Win32::Graphics::Direct3D12::D3D12_VIEWPORT;
use ::windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

// 标识一个原生目标的状态提交位置，不复制共享 RenderTargetHandle 状态机。
#[derive(Clone, Copy, PartialEq, Eq)]
enum D3d12RhiTargetOwner {
    // Surface 状态提交回 context 唯一 backbuffer 槽位。
    Surface(usize),
    // Texture 状态提交回唯一共享资源表中的原生 owner。
    Texture(TextureHandle),
}

// 保存全部共享门禁完成后才能进入命令列表的原生目标计划。
pub(super) struct D3d12RhiRenderTarget {
    // 保留共享目标身份，供 pass 与 Adapter 私有 owner 一致性检查。
    target: RenderTargetHandle,
    // 指向成功提交后唯一允许发布状态的位置。
    owner: D3d12RhiTargetOwner,
    // 命令执行与 fence 完成前保持原生资源存活。
    native: ID3D12Resource,
    // 保持 RTV descriptor heap 存活，避免 CPU handle 悬空。
    rtv_heap_owner: ID3D12DescriptorHeap,
    // 只保存已经由存活 heap 证明的 CPU descriptor handle。
    rtv: D3D12_CPU_DESCRIPTOR_HANDLE,
    // 保存共享状态机使用的目标范围。
    extent: RhiExtent,
    // 保存当前目标的共享颜色格式，供 draw 唯一选择兼容 PSO。
    format: TextureFormat,
    // 冻结命令录制开始前已经提交的原生状态。
    before: D3D12_RESOURCE_STATES,
    // 冻结本 pass 结束后要在 submit 成功后发布的原生状态。
    after: D3D12_RESOURCE_STATES,
    // 保存共享 viewport 投影得到的完整 D3D12 viewport。
    viewport: D3D12_VIEWPORT,
    // 保存共享左上原点 scissor 投影得到的完整原生矩形。
    scissor: RECT,
}

impl D3d12RhiRenderTarget {
    // 返回当前原生目标对应的共享不透明身份。
    pub(super) const fn target(&self) -> RenderTargetHandle {
        self.target
    }

    // 返回 draw adapter 选择 PSO 时唯一允许读取的活动目标格式。
    pub(super) const fn format(&self) -> TextureFormat {
        self.format
    }

    // 未知 GPU 状态下泄漏最后一份资源与 descriptor 引用，禁止提前释放。
    pub(super) fn retain_after_undrained_drop(self) {
        std::mem::forget(self.native);
        std::mem::forget(self.rtv_heap_owner);
    }
}

impl D3d12RhiDevice {
    // Surface present 只读借用本 Component 唯一提交序列。
    pub(in super::super) const fn submissions(&self) -> &RhiSubmissionSequence {
        &self.submission_sequence
    }

    // 在任何命令列表副作用前把共享 extent 投影为完整 viewport 与左上原点 scissor。
    fn render_geometry(extent: RhiExtent) -> Result<(D3D12_VIEWPORT, RECT)> {
        let viewport = RhiViewport {
            width: extent.width as f32,
            height: extent.height as f32,
        };
        let (width, height) = viewport.native_size_i32().ok_or_else(|| {
            Error::new(
                Errc::InvalidArgument,
                "D3D12 RHI render target viewport is invalid",
            )
        })?;
        let scissor = RhiScissor {
            x: 0,
            y: 0,
            width,
            height,
        };
        if !scissor.fits_within(extent) {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3D12 RHI render target scissor is invalid",
            ));
        }
        let (left, top, right, bottom) = scissor.native_rect().ok_or_else(|| {
            Error::new(
                Errc::InvalidArgument,
                "D3D12 RHI render target rectangle is invalid",
            )
        })?;
        Ok((
            D3D12_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: width as f32,
                Height: height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            },
            RECT {
                left,
                top,
                right,
                bottom,
            },
        ))
    }

    // 返回同一原生目标在当前未提交批次中的最新状态，避免复制资源状态权威。
    fn effective_target_state(
        &self,
        owner: D3d12RhiTargetOwner,
        committed: D3D12_RESOURCE_STATES,
    ) -> D3D12_RESOURCE_STATES {
        self.pending_targets
            .iter()
            .rev()
            .find(|pending| pending.owner == owner)
            .map(|pending| pending.after)
            .unwrap_or(committed)
    }

    // 返回 sampled texture 在当前未提交批次中已经由命令序列建立的最新状态。
    pub(super) fn effective_sampled_texture_state(
        &self,
        texture: TextureHandle,
        committed: D3D12_RESOURCE_STATES,
    ) -> D3D12_RESOURCE_STATES {
        self.effective_target_state(D3d12RhiTargetOwner::Texture(texture), committed)
    }

    // 解析可渲染 texture，并在 descriptor、格式与范围全部验证后克隆原生 owner。
    fn stage_texture_target(
        &self,
        target: RenderTargetHandle,
        texture: TextureHandle,
    ) -> Result<D3d12RhiRenderTarget> {
        let resolved = self.resolve_render_target(texture)?;
        if resolved != target {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3D12 RHI texture target identity is invalid",
            ));
        }
        let resource = self.textures.get(texture)?;
        if !resource.desc.format().supports_render_target() {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3D12 RHI texture target format is not renderable",
            ));
        }
        let rtv_heap_owner = resource.rtv_heap.as_ref().cloned().ok_or_else(|| {
            platform_error("D3d12Context: renderable RHI texture has no RTV descriptor")
        })?;
        // SAFETY: native 为存活二维纹理，GetDesc 是只读查询。
        let native_desc = unsafe { resource.native.GetDesc() };
        if native_desc.Format != texture_format(resource.desc.format())
            || native_desc.Width != u64::from(resource.desc.extent().width)
            || native_desc.Height != resource.desc.extent().height
        {
            return Err(platform_error(
                "D3d12Context: RHI texture target native description diverged",
            ));
        }
        let extent = resource.desc.extent();
        let (viewport, scissor) = Self::render_geometry(extent)?;
        // SAFETY: 单槽 RTV heap 由计划克隆并保持存活，起始 handle 指向唯一 descriptor。
        let rtv = unsafe { rtv_heap_owner.GetCPUDescriptorHandleForHeapStart() };
        let owner = D3d12RhiTargetOwner::Texture(texture);
        Ok(D3d12RhiRenderTarget {
            target: resolved,
            owner,
            native: resource.native.clone(),
            rtv_heap_owner,
            rtv,
            extent,
            format: resource.desc.format(),
            before: self.effective_target_state(owner, resource.state),
            after: D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            viewport,
            scissor,
        })
    }

    // 待提交批次继续拥有 texture target 时禁止销毁其共享身份。
    pub(super) fn validate_pending_texture_destroy(&self, texture: TextureHandle) -> Result<()> {
        let pending = self
            .pending_targets
            .iter()
            .any(|target| target.owner == D3d12RhiTargetOwner::Texture(texture));
        if pending {
            return Err(Error::new(
                Errc::InvalidState,
                "RHI texture is referenced by an unsubmitted render pass",
            ));
        }
        Ok(())
    }

    // 在 GPU 执行前证明全部状态提交目标仍指向存活的唯一 owner。
    fn validate_pending_target_commits(
        &self,
        back_buffer_states: &[D3D12_RESOURCE_STATES],
    ) -> Result<()> {
        for target in &self.pending_targets {
            match target.owner {
                D3d12RhiTargetOwner::Surface(index) => {
                    if index >= back_buffer_states.len() {
                        return Err(platform_error(
                            "D3d12Context: pending Surface target index is invalid",
                        ));
                    }
                }
                D3d12RhiTargetOwner::Texture(texture) => {
                    self.textures.get(texture)?;
                }
            }
        }
        Ok(())
    }

    // 只在 GPU 执行并等待成功后发布本批次的原生资源状态。
    fn commit_pending_target_states(
        &mut self,
        back_buffer_states: &mut [D3D12_RESOURCE_STATES],
    ) -> Result<()> {
        for index in 0..self.pending_targets.len() {
            let owner = self.pending_targets[index].owner;
            let after = self.pending_targets[index].after;
            match owner {
                D3d12RhiTargetOwner::Surface(buffer_index) => {
                    back_buffer_states[buffer_index] = after;
                }
                D3d12RhiTargetOwner::Texture(texture) => {
                    self.textures.get_mut(texture)?.state = after;
                }
            }
        }
        Ok(())
    }

    // present 前禁止任何打开或尚未提交的 D3D12 pass 绕过提交事务。
    pub(in super::super) fn require_present_ready(&self) -> Result<()> {
        self.pass.require_closed()?;
        if self.active_target.is_some()
            || !self.pending_targets.is_empty()
            || !self.pending_draw_resources.is_empty()
        {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI present requires a successfully submitted render pass",
            ));
        }
        Ok(())
    }
}

// 验证 D3D12 ClearRenderTargetView 能机械表达当前唯一共享清理状态。
fn validate_color_clear_contract() -> Result<()> {
    if !UIX_COLOR_CLEAR_CONTRACT.is_uix_contract() {
        return Err(Error::new(
            Errc::InvalidArgument,
            "D3D12 RHI color clear contract is unsupported",
        ));
    }
    Ok(())
}

impl D3d12Context {
    // 把 context 命令录制事实与资源 Component 的 pass/批次门禁合并为 present 前置条件。
    pub(in super::super) fn rhi_require_present_ready(&self) -> Result<()> {
        self.rhi_device.require_present_ready()?;
        if self.recording {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI present cannot interrupt command recording",
            ));
        }
        Ok(())
    }

    // 在共享门禁全部完成后解析当前 Surface backbuffer 的原生目标计划。
    fn stage_surface_target(&self) -> Result<D3d12RhiRenderTarget> {
        self.surface_lifecycle.ensure_active()?;
        if self.frame_index >= self.back_buffers.len()
            || self.frame_index >= self.back_buffer_states.len()
            || self.frame_index >= self.allocators.len()
            || self.rtv_stride == 0
        {
            return Err(platform_error(
                "D3d12Context: RHI Surface target resources are invalid",
            ));
        }
        let native = self.back_buffers[self.frame_index].clone();
        // SAFETY: 当前 backbuffer 由 context 保持存活，GetDesc 是只读查询。
        let native_desc = unsafe { native.GetDesc() };
        let extent = self.surface_lifecycle.token().extent;
        if native_desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || native_desc.Width != u64::from(extent.width)
            || native_desc.Height != extent.height
        {
            return Err(platform_error(
                "D3d12Context: RHI Surface target native description diverged",
            ));
        }
        let (viewport, scissor) = D3d12RhiDevice::render_geometry(extent)?;
        let owner = D3d12RhiTargetOwner::Surface(self.frame_index);
        Ok(D3d12RhiRenderTarget {
            target: RenderTargetHandle::surface(),
            owner,
            native,
            rtv_heap_owner: self.rtv_heap.clone(),
            rtv: self.rtv_handle(self.frame_index),
            extent,
            format: TextureFormat::Bgra8Unorm,
            before: self
                .rhi_device
                .effective_target_state(owner, self.back_buffer_states[self.frame_index]),
            after: D3D12_RESOURCE_STATE_PRESENT,
            viewport,
            scissor,
        })
    }

    // 开始 Surface 或 texture pass；所有共享/原生只读门禁均早于 Reset 与命令编码。
    pub(super) fn rhi_begin_render_pass(
        &mut self,
        target: RenderTargetHandle,
        load: LoadAction,
    ) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        if self.recording && self.rhi_device.pending_targets.is_empty() {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI render pass cannot join unrelated command recording",
            ));
        }
        let native = if target.is_surface() {
            self.stage_surface_target()?
        } else {
            let texture = target.texture().ok_or_else(|| {
                Error::new(
                    Errc::InvalidArgument,
                    "D3D12 RHI render target identity is invalid",
                )
            })?;
            self.rhi_device.stage_texture_target(target, texture)?
        };
        if native.target != target {
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3D12 RHI render target does not match its resource owner",
            ));
        }
        if matches!(load, LoadAction::Clear(_)) {
            validate_color_clear_contract()?;
        }
        // 共享状态机最后验证 extent 与 Load/Clear，并建立唯一活动 pass 事实。
        self.rhi_device.pass.begin(target, native.extent, load)?;
        if !self.recording
            && let Err(error) = self.reset_rhi_command_list("RHI render pass")
        {
            self.rhi_device.pass.reset();
            return Err(error);
        }
        record_transition(
            &self.command_list,
            &native.native,
            native.before,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );
        // SAFETY: viewport/scissor 来自共享范围投影，RTV 与资源/heap 由 native 计划共同保持存活。
        unsafe {
            self.command_list
                .RSSetViewports(std::slice::from_ref(&native.viewport));
            self.command_list
                .RSSetScissorRects(std::slice::from_ref(&native.scissor));
            self.command_list
                .OMSetRenderTargets(1, Some(&native.rtv), true, None);
        }
        if let LoadAction::Clear(color) = load {
            // SAFETY: pass 已验证预乘颜色，RTV 属于当前 device，None 表示清理完整目标。
            unsafe {
                self.command_list
                    .ClearRenderTargetView(native.rtv, &color.components(), None);
            }
        }
        self.rhi_device.active_target = Some(native);
        Ok(())
    }

    // 在当前 pass 内按共享左上原点矩形编码 D3D12 局部清理。
    pub(super) fn rhi_clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.validate_clear(color, scissor)?;
        validate_color_clear_contract()?;
        let (left, top, right, bottom) = scissor
            .native_rect()
            .ok_or_else(|| Error::new(Errc::InvalidArgument, "D3D12 RHI clear rect is invalid"))?;
        let target = self.rhi_device.active_target.as_ref().ok_or_else(|| {
            Error::new(Errc::InvalidState, "D3D12 RHI clear has no active target")
        })?;
        if target.target != self.rhi_device.pass.target()? || !self.recording {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI clear target is not recording",
            ));
        }
        let rect = RECT {
            left,
            top,
            right,
            bottom,
        };
        // SAFETY: pass/颜色/矩形与原生 RTV 已在本调用内完成全部门禁。
        unsafe {
            self.command_list.ClearRenderTargetView(
                target.rtv,
                &color.components(),
                Some(std::slice::from_ref(&rect)),
            );
        }
        Ok(())
    }

    // 显式结束当前 pass，并编码输出目标的最终原生状态。
    pub(super) fn rhi_end_render_pass(&mut self) -> Result<()> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_open()?;
        let target =
            self.rhi_device.active_target.as_ref().ok_or_else(|| {
                Error::new(Errc::InvalidState, "D3D12 RHI pass has no active target")
            })?;
        if target.target != self.rhi_device.pass.target()? || !self.recording {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI pass target is not recording",
            ));
        }
        // SAFETY: command list 正在录制；零目标显式解除当前 RTV 绑定。
        unsafe { self.command_list.OMSetRenderTargets(0, None, false, None) };
        record_transition(
            &self.command_list,
            &target.native,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            target.after,
        );
        // 原生命令已完整编码后才关闭共享 pass，仍不发布任何原生状态。
        self.rhi_device.pass.end()?;
        let target =
            self.rhi_device.active_target.take().ok_or_else(|| {
                Error::new(Errc::InvalidState, "D3D12 RHI pass target disappeared")
            })?;
        self.rhi_device.pending_targets.push(target);
        Ok(())
    }

    // 执行已关闭 pass 的命令批次，等待 GPU 后才发布状态并签发提交身份。
    pub(super) fn rhi_submit(&mut self) -> Result<SubmissionHandle> {
        self.ensure_healthy()?;
        self.rhi_device.pass.require_closed()?;
        if self.rhi_device.active_target.is_some()
            || self.rhi_device.pending_targets.is_empty()
            || !self.recording
        {
            return Err(Error::new(
                Errc::InvalidState,
                "D3D12 RHI submit requires at least one ended render pass",
            ));
        }
        // 所有状态发布目标必须在 Close/ExecuteCommandLists 前保持有效。
        self.rhi_device
            .validate_pending_target_commits(&self.back_buffer_states)?;
        // Close、ExecuteCommandLists、Signal 与 fence wait 全部成功后才可建立提交事实。
        self.execute_recording_and_wait()?;
        if let Err(error) = self
            .rhi_device
            .commit_pending_target_states(&mut self.back_buffer_states)
        {
            self.latch_fault("commit RHI render target states", &error);
            return Err(error);
        }
        let submission = match self.rhi_device.submission_sequence.issue() {
            Ok(submission) => submission,
            Err(error) => {
                self.latch_fault("issue RHI submission", &error);
                return Err(error);
            }
        };
        // 状态与身份都成功发布后，按录制引用的逆序释放 draw 与目标 COM owner。
        self.rhi_device.pending_draw_resources.clear();
        self.rhi_device.pending_targets.clear();
        Ok(submission)
    }
}
