//! D3D11 薄 RHI 的 texture、pipeline、sampler 和绑定资源生命周期。

#![allow(dead_code)]

// 复用父模块的 D3D11 context、RHI 类型和错误转换辅助。
use super::*;

// 为 D3D11 context 提供资源生命周期的 inherent helper，trait 入口在父模块统一转发。
impl D3d11Context {
    // 创建可采样且尽可能可作为 render target 的 D3D11 texture。
    pub(super) fn rhi_create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        // 先通过两个 Adapter 共用的非空二维原生值域门禁。
        desc.validate()?;
        // 取得格式事实映射。
        let format = D3d11RhiDevice::texture_format(desc.format());
        // 颜色目标能力只由共享格式闭集决定。
        let renderable = desc.format().supports_render_target();
        // 为 texture 选择 shader resource 和可选 render target 绑定。
        let mut bind_flags = D3D11_BIND_SHADER_RESOURCE.0 as u32;
        // 只有颜色格式进入 render target 绑定。
        if renderable {
            // 增加 render target 绑定旗标。
            bind_flags |= D3D11_BIND_RENDER_TARGET.0 as u32;
        }
        // 准备 D3D11 texture 描述。
        let native_desc = D3D11_TEXTURE2D_DESC {
            Width: desc.extent().width,
            Height: desc.extent().height,
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
        // 由共享资源表原子登记原生对象、视图与通用描述。
        Ok(self.rhi_device.textures.insert(D3d11RhiTexture {
            native,
            rtv,
            srv,
            // Adapter 只保存唯一共享描述，不再复制尺寸和格式字段。
            desc,
        }))
    }

    // 创建当前 D3D11 Adapter 已经具备 shader ABI 的封闭 pipeline。
    pub(super) fn rhi_create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        // 由共享 pipeline 表登记空的 D3D11 资源占位并签发 binding。
        Ok(self.rhi_device.pipelines.insert(desc.kind, ()))
    }

    // 创建带 clamp 地址模式的 D3D11 sampler。
    pub(super) fn rhi_create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        // 穷尽映射过滤与 mip 组合，禁止 Adapter 私自选择未声明状态。
        let native_filter = match (desc.filter(), desc.mip_mode()) {
            // 最近点单级采样在三个维度都使用 point 选择。
            (SamplerFilter::Nearest, SamplerMipMode::SingleLevel) => D3D11_FILTER_MIN_MAG_MIP_POINT,
            // 线性单级采样只对 min/mag 插值，mip 维度保持 point。
            (SamplerFilter::Linear, SamplerMipMode::SingleLevel) => {
                D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT
            }
        };
        // 穷尽映射二维纹理地址语义。
        let native_address = match desc.address_mode() {
            // 当前唯一地址模式机械映射为 D3D11 clamp。
            SamplerAddressMode::ClampToEdge => D3D11_TEXTURE_ADDRESS_CLAMP,
        };
        // 穷尽映射单级纹理允许的原生 LOD 范围。
        let (min_lod, max_lod) = match desc.mip_mode() {
            // 两个端点都冻结为第零级，禁止未来层级被隐式观察。
            SamplerMipMode::SingleLevel => (0.0, 0.0),
        };
        // 使用已经从共享描述投影出的完整原生 sampler 状态。
        let native_desc = D3D11_SAMPLER_DESC {
            // 过滤模式来自共享 filter 与 mip 的联合投影。
            Filter: native_filter,
            // U/V/W 三个轴统一采用 clamp。
            AddressU: native_address,
            // V 轴复用同一二维地址事实。
            AddressV: native_address,
            // 二维纹理不读取 W，但仍复用同一封闭地址事实。
            AddressW: native_address,
            // 不使用比较采样和各向异性扩展。
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0; 4],
            MinLOD: min_lod,
            // 最大 LOD 与最小 LOD 同时锁定第零级。
            MaxLOD: max_lod,
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
        // 由共享资源表原子登记 sampler 资源。
        Ok(self.rhi_device.samplers.insert(D3d11RhiSampler {
            // 保持原生 sampler state 生命周期。
            native,
            // 保留创建描述供共享 PipelineSampling 在 draw 前核对。
            desc,
        }))
    }

    // 销毁 pipeline 资源槽。
    pub(super) fn rhi_destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        // 由共享 pipeline 表检查 kind 后检查式取出占位资源。
        self.rhi_device.pipelines.take(pipeline)?;
        // 返回成功。
        Ok(())
    }

    // 销毁 sampler 资源槽。
    pub(super) fn rhi_destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        // 由共享资源表检查式取出 sampler 及其原生状态。
        self.rhi_device.samplers.take(sampler)?;
        // 清理共享 pass 中可能残留的 sampler 绑定身份。
        self.rhi_device.pass.unbind_sampler(sampler);
        // 返回成功。
        Ok(())
    }

    // 只读验证 sampled binding 指向的真实纹理与 sampler 资源。
    pub(super) fn rhi_validate_sampled_binding(
        // 只读借用 D3D11 owner，避免预检触碰原生状态。
        &self,
        // 接收共享 pipeline 语义绑定。
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 解析真实 texture 资源并结束资源表借用。
        let texture = self.rhi_device.texture(binding.texture())?;
        // 解析真实 sampler 资源并结束资源表借用。
        let sampler = self.rhi_device.sampler(binding.sampler())?;
        // 复制共享纹理格式值，避免校验持有资源表借用。
        let format = texture.desc.format();
        // 复制共享 sampler 描述值，避免校验持有资源表借用。
        let sampler_desc = sampler.desc;
        // 委托共享绑定契约验证资源描述与 pipeline sampling 语义。
        binding.validate_resources(format, sampler_desc)
    }

    // 绑定当前 pass 的采样纹理和 sampler。
    pub(super) fn rhi_bind_sampled_texture(
        &mut self,
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 先复用只读资源真实性与采样语义预检。
        self.rhi_validate_sampled_binding(binding)?;
        // 由共享状态机统一验证 pass 和目标反馈环后原子记录绑定。
        self.rhi_device.pass.bind_sampled_texture(binding)?;
        // 返回绑定成功。
        Ok(())
    }
}
