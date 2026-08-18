//! D3D11 薄 RHI 的 texture、pipeline、sampler 和绑定资源生命周期。

#![allow(dead_code)]

// 复用父模块的 D3D11 context、RHI 类型和错误转换辅助。
use super::*;

// 为 D3D11 context 提供资源生命周期的 inherent helper，trait 入口在父模块统一转发。
impl D3d11Context {
    // 创建可采样且尽可能可作为 render target 的 D3D11 texture。
    pub(super) fn rhi_create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        // 拒绝零尺寸或超过共同原生值域的资源。
        if !desc.extent.is_valid() {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("D3d11 RHI texture extent is invalid"));
        }
        // 取得格式事实映射。
        let (format, _bytes_per_pixel, renderable) = D3d11RhiDevice::texture_format(desc.format);
        // 为 texture 选择 shader resource 和可选 render target 绑定。
        let mut bind_flags = D3D11_BIND_SHADER_RESOURCE.0 as u32;
        // 只有颜色格式进入 render target 绑定。
        if renderable {
            // 增加 render target 绑定旗标。
            bind_flags |= D3D11_BIND_RENDER_TARGET.0 as u32;
        }
        // 准备 D3D11 texture 描述。
        let native_desc = D3D11_TEXTURE2D_DESC {
            Width: desc.extent.width,
            Height: desc.extent.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: ::windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT(format),
            SampleDesc: ::windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind_flags,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        // 为 CreateTexture2D 准备空初始数据。
        let mut native = None;
        // SAFETY: device 属于当前 owner thread，描述只包含本函数验证过的
        // 尺寸、格式和绑定旗标，输出槽在调用期间保持有效。
        unsafe {
            self.device
                .CreateTexture2D(&native_desc, None, Some(&mut native))
                .map_err(|error| d3d_error("ID3D11Device::CreateTexture2D(rhi)", error))?;
        }
        // 拒绝驱动返回的空纹理。
        let native =
            native.ok_or_else(|| rhi_platform("D3d11 RHI CreateTexture2D returned no texture"))?;
        // 创建 shader resource view。
        let mut srv = None;
        // SAFETY: native texture 由同一 device 刚刚创建，SRV 描述与 texture
        // 格式一致，输出槽在调用期间保持有效。
        unsafe {
            self.device
                .CreateShaderResourceView(&native, None, Some(&mut srv))
                .map_err(|error| d3d_error("ID3D11Device::CreateShaderResourceView(rhi)", error))?;
        }
        // 拒绝驱动返回的空 SRV。
        let srv =
            srv.ok_or_else(|| rhi_platform("D3d11 RHI CreateShaderResourceView returned no view"))?;
        // 为颜色纹理创建 render target view。
        let rtv =
            if renderable {
                // 准备可选 RTV 输出槽。
                let mut rtv = None;
                // SAFETY: renderable 只对声明了 render target 绑定的颜色格式为真。
                unsafe {
                    self.device
                        .CreateRenderTargetView(&native, None, Some(&mut rtv))
                        .map_err(|error| {
                            d3d_error("ID3D11Device::CreateRenderTargetView(rhi)", error)
                        })?;
                }
                // 驱动必须返回颜色纹理 RTV。
                Some(rtv.ok_or_else(|| {
                    rhi_platform("D3d11 RHI CreateRenderTargetView returned no view")
                })?)
            } else {
                // 覆盖率纹理保持 sampled-only。
                None
            };
        // 记录资源及其通用描述。
        self.rhi_device.textures.push(Some(D3d11RhiTexture {
            native,
            rtv,
            srv,
            extent: desc.extent,
            format: desc.format,
        }));
        // 计算刚刚追加的资源句柄。
        let raw = self.rhi_device.textures.len() as u64;
        // 返回 opaque texture handle。
        Ok(TextureHandle::from_raw(raw))
    }

    // 创建当前 D3D11 Adapter 已经具备 shader ABI 的封闭 pipeline。
    pub(super) fn rhi_create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineHandle> {
        // 保存封闭通用语义，不把原生 shader 对象暴露给通用层。
        self.rhi_device
            .pipelines
            .push(Some(D3d11RhiPipeline { kind: desc.kind }));
        // 计算刚刚追加的 pipeline 句柄。
        let raw = self.rhi_device.pipelines.len() as u64;
        // 返回 opaque pipeline handle。
        Ok(PipelineHandle::from_raw(raw))
    }

    // 创建带 clamp 地址模式的 D3D11 sampler。
    pub(super) fn rhi_create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        // 线性和点采样都保持边界 clamp，避免图片 quad 越界取样。
        let native_desc = D3D11_SAMPLER_DESC {
            // 根据通用 sampler 事实选择过滤模式。
            Filter: if desc.uses_linear_filter() {
                // UIX texture 固定单 mip，线性契约只覆盖 min/mag，不启用隐式三线性过滤。
                D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT
            } else {
                D3D11_FILTER_MIN_MAG_MIP_POINT
            },
            // U/V/W 三个轴统一采用 clamp。
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            // 不使用比较采样和各向异性扩展。
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: 0.0,
            MaxLOD: f32::MAX,
        };
        // 为 CreateSamplerState 准备空输出槽。
        let mut native = None;
        // SAFETY: device 属于当前 owner thread，描述只包含固定的 sampler
        // 事实，输出槽在调用期间保持有效。
        unsafe {
            self.device
                .CreateSamplerState(&native_desc, Some(&mut native))
                .map_err(|error| d3d_error("ID3D11Device::CreateSamplerState(rhi)", error))?;
        }
        // 拒绝驱动返回的空 sampler。
        let native = native
            .ok_or_else(|| rhi_platform("D3d11 RHI CreateSamplerState returned no sampler"))?;
        // 保存 sampler 资源。
        self.rhi_device.samplers.push(Some(D3d11RhiSampler {
            // 保持原生 sampler state 生命周期。
            native,
            // 保留创建描述供共享 PipelineSampling 在 draw 前核对。
            desc,
        }));
        // 计算刚刚追加的 sampler 句柄。
        let raw = self.rhi_device.samplers.len() as u64;
        // 返回 opaque sampler handle。
        Ok(SamplerHandle::from_raw(raw))
    }

    // 销毁 pipeline 资源槽。
    pub(super) fn rhi_destroy_pipeline(&mut self, pipeline: PipelineHandle) -> Result<()> {
        // 解析资源身份并检查是否已销毁。
        let index = pipeline
            .raw()
            .checked_sub(1)
            .ok_or_else(|| rhi_invalid("pipeline handle is null"))? as usize;
        // 读取资源槽。
        let Some(slot) = self.rhi_device.pipelines.get_mut(index) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("pipeline handle is stale"));
        };
        // 拒绝重复销毁。
        if slot.is_none() {
            // 返回稳定的状态错误。
            return Err(rhi_invalid("pipeline handle was already destroyed"));
        }
        // 清空资源槽，让旧句柄立即失效。
        *slot = None;
        // 返回成功。
        Ok(())
    }

    // 销毁 sampler 资源槽。
    pub(super) fn rhi_destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        // 解析资源身份并检查是否已销毁。
        let index = sampler
            .raw()
            .checked_sub(1)
            .ok_or_else(|| rhi_invalid("sampler handle is null"))? as usize;
        // 读取资源槽。
        let Some(slot) = self.rhi_device.samplers.get_mut(index) else {
            // 返回稳定的参数错误。
            return Err(rhi_invalid("sampler handle is stale"));
        };
        // 拒绝重复销毁。
        if slot.is_none() {
            // 返回稳定的状态错误。
            return Err(rhi_invalid("sampler handle was already destroyed"));
        }
        // 清空资源槽，让旧句柄立即失效。
        *slot = None;
        // 清理共享 pass 中可能残留的 sampler 绑定身份。
        self.rhi_device.pass.unbind_sampler(sampler);
        // 返回成功。
        Ok(())
    }

    // 绑定当前 pass 的采样纹理和 sampler。
    pub(super) fn rhi_bind_sampled_texture(
        &mut self,
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 验证纹理和 sampler 句柄仍然有效。
        self.rhi_device.texture(binding.texture())?;
        self.rhi_device.sampler(binding.sampler())?;
        // 由共享状态机统一验证 pass 和目标反馈环后原子记录绑定。
        self.rhi_device.pass.bind_sampled_texture(binding)?;
        // 返回绑定成功。
        Ok(())
    }
}
