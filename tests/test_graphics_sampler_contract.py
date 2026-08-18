# -*- coding: utf-8 -*-
# 说明本文件守卫共享采样语义与两套生产 Adapter 的机械映射。
"""Lock sampler filtering to one cross-adapter RHI contract."""

# 引入单元测试框架。
import unittest
# 引入跨平台路径工具。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]


# 固定 PipelineSampling、SamplerDesc 与原生 Adapter 的单一事实来源。
class GraphicsSamplerContractTests(unittest.TestCase):
    # 共享采样契约必须同时拥有纹理格式与过滤要求。
    def test_shared_sampling_contract_owns_format_and_filter(self) -> None:
        # 读取薄 RHI sampler 描述。
        rhi = (ROOT / "src/native/present/rhi.rs").read_text(encoding="utf-8")
        # 读取共享 pipeline 采样契约。
        pipeline = (ROOT / "src/native/present/rhi/pipeline.rs").read_text(encoding="utf-8")
        # 读取所有生产 Renderer 的 sampler 构造调用。
        renderer = "\n".join(
            # 逐个读取普通图片、coverage、MSDF 与 Blur 资源所有者。
            (ROOT / path).read_text(encoding="utf-8")
            # 固定当前会创建 sampler 的 Drawing 文件集合。
            for path in (
                # 普通图片 sampler 所有者。
                "src/draw/backend/rhi_renderer.rs",
                # coverage sampler 所有者。
                "src/draw/backend/rhi_renderer_coverage.rs",
                # MSDF sampler 所有者。
                "src/draw/backend/rhi_renderer_msdf.rs",
                # Blur sampler 所有者。
                "src/draw/backend/rhi_renderer_blur.rs",
            )
        )
        # SamplerDesc 必须提供命名的线性 clamp 构造器。
        self.assertIn("pub(crate) const fn linear_clamp()", rhi)
        # SamplerDesc 必须提供命名的最近点 clamp 构造器。
        self.assertIn("pub(crate) const fn nearest_clamp()", rhi)
        # 共享层必须以封闭枚举表达过滤模式。
        self.assertIn("enum SamplerFilter", rhi)
        # 过滤枚举必须封闭在 Nearest 与 Linear 两个合法值内。
        self.assertIn("Nearest", rhi)
        # 过滤枚举必须包含 Linear 合法值。
        self.assertIn("Linear", rhi)
        # 共享层必须以封闭枚举表达地址模式。
        self.assertIn("enum SamplerAddressMode", rhi)
        # 地址枚举必须明确包含 ClampToEdge 合法值。
        self.assertIn("ClampToEdge", rhi)
        # 共享层必须以封闭枚举表达 mip 模式。
        self.assertIn("enum SamplerMipMode", rhi)
        # mip 枚举必须明确包含 SingleLevel 合法值。
        self.assertIn("SingleLevel", rhi)
        # sampler 描述必须持有完整的三维类型化语义。
        self.assertIn("filter: SamplerFilter", rhi)
        # sampler 描述必须持有地址模式字段。
        self.assertIn("address_mode: SamplerAddressMode", rhi)
        # sampler 描述必须持有 mip 模式字段。
        self.assertIn("mip_mode: SamplerMipMode", rhi)
        # sampler 描述必须公开 crate 内只读过滤访问器。
        self.assertIn("pub(crate) const fn filter(self) -> SamplerFilter", rhi)
        # sampler 描述必须公开 crate 内只读地址访问器。
        self.assertIn("pub(crate) const fn address_mode(self) -> SamplerAddressMode", rhi)
        # sampler 描述必须公开 crate 内只读 mip 访问器。
        self.assertIn("pub(crate) const fn mip_mode(self) -> SamplerMipMode", rhi)
        # 调用方不得继续用裸布尔值表达过滤语义。
        self.assertNotIn("SamplerDesc { linear:", renderer)
        # PipelineSampling 门禁必须同时接收纹理格式和 sampler 描述。
        self.assertIn("accepts(self, format: TextureFormat, sampler: SamplerDesc)", pipeline)
        # 颜色采样必须通过 matches! 门禁线性过滤枚举。
        self.assertIn("matches!(sampler.filter(), SamplerFilter::Linear)", pipeline)
        # Coverage 必须通过 matches! 门禁最近点过滤枚举。
        self.assertIn("matches!(sampler.filter(), SamplerFilter::Nearest)", pipeline)
        # PipelineSampling 必须额外门禁 ClampToEdge 地址枚举。
        self.assertIn("matches!(sampler.address_mode(), SamplerAddressMode::ClampToEdge)", pipeline)
        # PipelineSampling 必须额外门禁 SingleLevel mip 枚举。
        self.assertIn("matches!(sampler.mip_mode(), SamplerMipMode::SingleLevel)", pipeline)

    # 两个 Adapter 必须保留创建描述并在 draw 前使用同一门禁。
    def test_adapters_preserve_desc_and_validate_before_draw(self) -> None:
        # 读取 OpenGL sampler 资源表。
        opengl_device = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 OpenGL draw Adapter。
        opengl_draw = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs").read_text(encoding="utf-8")
        # 读取 D3D11 sampler 资源表。
        d3d11_device = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 sampler 创建实现。
        d3d11_resources = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_resources.rs").read_text(encoding="utf-8")
        # 读取 D3D11 draw Adapter。
        d3d11_draw = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs").read_text(encoding="utf-8")
        # OpenGL sampler 资源必须保留共享描述。
        self.assertIn("struct OpenGlRhiSampler", opengl_device)
        # OpenGL 资源表必须保存创建时的 SamplerDesc。
        self.assertIn("desc: SamplerDesc", opengl_device)
        # OpenGL 绑定必须把实际描述交给共享绑定门禁。
        self.assertIn("binding.validate_resources(format, sampler_desc)", opengl_device)
        # OpenGL draw 必须按当前 pipeline 取得匹配绑定。
        self.assertIn("sampled_binding_for(pipeline)", opengl_draw)
        # D3D11 sampler 资源必须保留共享描述。
        self.assertIn("struct D3d11RhiSampler", d3d11_device)
        # D3D11 资源表必须保存创建时的 SamplerDesc。
        self.assertIn("desc: SamplerDesc", d3d11_device)
        # D3D11 创建实现必须把同一描述写入资源槽。
        self.assertIn("D3d11RhiSampler {", d3d11_resources)
        # D3D11 绑定必须把实际描述交给共享绑定门禁。
        self.assertIn("binding.validate_resources(format, sampler_desc)", d3d11_resources)
        # D3D11 四类 draw 必须按当前 pipeline 取得匹配绑定。
        self.assertEqual(d3d11_draw.count(".sampled_binding_for(packet.pipeline())"), 4)
        # Adapter draw 不得重复解释共享采样契约。
        self.assertNotIn("contract.sampling.accepts", opengl_draw + d3d11_draw)

    # 原生映射必须保持同一无 mip 的 min/mag 过滤与 clamp 语义。
    def test_native_filter_mapping_has_no_hidden_mip_difference(self) -> None:
        # 读取 OpenGL sampler 原生映射。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 公共导入与 sampler 原生映射。
        d3d11_device = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 sampler 创建实现。
        d3d11_resources = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_resources.rs").read_text(encoding="utf-8")
        # OpenGL 只允许读取共享 filter 访问器作为过滤事实。
        self.assertIn("match desc.filter()", opengl)
        # D3D11 只允许读取共享 filter 访问器作为过滤事实。
        self.assertIn("match (desc.filter(), desc.mip_mode())", d3d11_resources)
        # OpenGL min 与 mag 必须使用同一个共享 filter。
        self.assertIn("glow::TEXTURE_MIN_FILTER, filter", opengl)
        # OpenGL mag 必须复用同一个共享 filter。
        self.assertIn("glow::TEXTURE_MAG_FILTER, filter", opengl)
        # OpenGL 必须把最近点过滤机械映射为 GL_NEAREST。
        self.assertIn("SamplerFilter::Nearest => glow::NEAREST as i32", opengl)
        # OpenGL 必须把线性过滤机械映射为 GL_LINEAR。
        self.assertIn("SamplerFilter::Linear => glow::LINEAR as i32", opengl)
        # OpenGL 必须按共享 address 访问器穷尽映射地址常量。
        self.assertIn("match desc.address_mode()", opengl)
        # OpenGL 必须按共享 mip 访问器穷尽映射 mip 常量。
        self.assertIn("match desc.mip_mode()", opengl)
        # OpenGL 单级 mip 必须把允许的 LOD 两端都投影为第零级。
        self.assertIn("SamplerMipMode::SingleLevel => (0.0, 0.0)", opengl)
        # D3D11 线性映射只能启用 min/mag 线性和 mip point。
        self.assertIn("D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT", d3d11_device + d3d11_resources)
        # D3D11 不得恢复共享契约未声明的三线性 mip 过滤。
        self.assertNotIn("D3D11_FILTER_MIN_MAG_MIP_LINEAR", d3d11_device + d3d11_resources)
        # D3D11 必须按共享 address 访问器穷尽映射地址常量。
        self.assertIn("match desc.address_mode()", d3d11_resources)
        # D3D11 必须按共享 mip 访问器穷尽映射 mip 语义。
        self.assertIn("match desc.mip_mode()", d3d11_resources)
        # D3D11 单级 mip 的最大 LOD 不得继续使用无穷上限。
        self.assertNotIn("MaxLOD: f32::MAX", d3d11_resources)
        # D3D11 单级 mip 必须把允许的 LOD 两端都投影为第零级。
        self.assertIn("SamplerMipMode::SingleLevel => (0.0, 0.0)", d3d11_resources)
        # OpenGL 必须先投影共享地址模式到原生地址变量。
        self.assertIn("let address_mode = match desc.address_mode()", opengl)
        # OpenGL clamp 语义必须机械映射为 GL_CLAMP_TO_EDGE。
        self.assertIn("SamplerAddressMode::ClampToEdge => glow::CLAMP_TO_EDGE as i32", opengl)
        # OpenGL 必须把地址变量复用到 S 轴。
        self.assertIn("glow::TEXTURE_WRAP_S, address_mode", opengl)
        # OpenGL 必须把地址变量复用到 T 轴。
        self.assertIn("glow::TEXTURE_WRAP_T, address_mode", opengl)
        # OpenGL 必须先投影共享 mip 模式到 LOD 变量。
        self.assertIn("let (min_lod, max_lod) = match desc.mip_mode()", opengl)
        # OpenGL 必须把 min_lod 变量传给最小 LOD 参数。
        self.assertIn("glow::TEXTURE_MIN_LOD, min_lod", opengl)
        # OpenGL 必须把 max_lod 变量传给最大 LOD 参数。
        self.assertIn("glow::TEXTURE_MAX_LOD, max_lod", opengl)
        # D3D11 必须先投影共享地址模式到原生地址变量。
        self.assertIn("let native_address = match desc.address_mode()", d3d11_resources)
        # D3D11 clamp 语义必须机械映射为 D3D11_TEXTURE_ADDRESS_CLAMP。
        self.assertIn(
            # 锁定共享枚举到原生常量的唯一映射。
            "SamplerAddressMode::ClampToEdge => D3D11_TEXTURE_ADDRESS_CLAMP",
            # 只在 D3D11 sampler 创建边界内检查。
            d3d11_resources,
        )
        # D3D11 必须把地址变量复用到三个纹理轴。
        self.assertEqual(d3d11_resources.count("AddressU: native_address"), 1)
        # D3D11 必须把地址变量复用到 V 轴。
        self.assertEqual(d3d11_resources.count("AddressV: native_address"), 1)
        # D3D11 必须把地址变量复用到 W 轴。
        self.assertEqual(d3d11_resources.count("AddressW: native_address"), 1)
        # D3D11 必须以 filter 与 mip 联合匹配穷尽过滤映射。
        self.assertIn("match (desc.filter(), desc.mip_mode())", d3d11_resources)
        # D3D11 必须以 mip 模式投影 LOD 变量。
        self.assertIn("let (min_lod, max_lod) = match desc.mip_mode()", d3d11_resources)
        # D3D11 必须消费 min_lod 而不是另造默认下限。
        self.assertIn("MinLOD: min_lod", d3d11_resources)
        # D3D11 必须消费 max_lod 而不是固定无穷上限。
        self.assertIn("MaxLOD: max_lod", d3d11_resources)


# 支持直接运行当前跨后端 sampler 契约。
if __name__ == "__main__":
    # 执行本文件内全部契约测试。
    unittest.main()
