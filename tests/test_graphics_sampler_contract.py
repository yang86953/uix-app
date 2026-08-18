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
        # 调用方不得继续用裸布尔值表达过滤语义。
        self.assertNotIn("SamplerDesc { linear:", renderer)
        # PipelineSampling 门禁必须同时接收纹理格式和 sampler 描述。
        self.assertIn("accepts(self, format: TextureFormat, sampler: SamplerDesc)", pipeline)
        # 颜色与 MSDF 采样必须共用线性过滤规则。
        self.assertIn("Self::PremultipliedColor | Self::Msdf => sampler.uses_linear_filter()", pipeline)
        # Coverage 必须明确拒绝线性过滤。
        self.assertIn("Self::Coverage => !sampler.uses_linear_filter()", pipeline)

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
        # OpenGL draw 必须把格式与 sampler 描述一起交给共享门禁。
        self.assertIn("Ok((texture.desc.format(), sampler.desc))", opengl_draw)
        # 四类 OpenGL sampled pipeline 都必须调用同一个二参数门禁。
        self.assertEqual(opengl_draw.count("contract.sampling.accepts(format, sampler)"), 4)
        # D3D11 sampler 资源必须保留共享描述。
        self.assertIn("struct D3d11RhiSampler", d3d11_device)
        # D3D11 资源表必须保存创建时的 SamplerDesc。
        self.assertIn("desc: SamplerDesc", d3d11_device)
        # D3D11 创建实现必须把同一描述写入资源槽。
        self.assertIn("D3d11RhiSampler {", d3d11_resources)
        # 四类 D3D11 sampled pipeline 都必须调用同一个二参数门禁。
        self.assertEqual(d3d11_draw.count(".accepts(texture.desc.format(), sampler.desc)"), 4)

    # 原生映射必须保持同一无 mip 的 min/mag 过滤与 clamp 语义。
    def test_native_filter_mapping_has_no_hidden_mip_difference(self) -> None:
        # 读取 OpenGL sampler 原生映射。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 公共导入与 sampler 原生映射。
        d3d11_device = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs").read_text(encoding="utf-8")
        # 读取 D3D11 sampler 创建实现。
        d3d11_resources = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_resources.rs").read_text(encoding="utf-8")
        # 两个 Adapter 都必须只读取共享描述的过滤事实。
        self.assertIn("desc.uses_linear_filter()", opengl)
        # D3D11 同样不得另存过滤策略。
        self.assertIn("desc.uses_linear_filter()", d3d11_resources)
        # OpenGL min 与 mag 必须使用同一个共享 filter。
        self.assertIn("glow::TEXTURE_MIN_FILTER, filter", opengl)
        # OpenGL mag 必须复用同一个共享 filter。
        self.assertIn("glow::TEXTURE_MAG_FILTER, filter", opengl)
        # D3D11 线性映射只能启用 min/mag 线性和 mip point。
        self.assertIn("D3D11_FILTER_MIN_MAG_LINEAR_MIP_POINT", d3d11_device + d3d11_resources)
        # D3D11 不得恢复共享契约未声明的三线性 mip 过滤。
        self.assertNotIn("D3D11_FILTER_MIN_MAG_MIP_LINEAR", d3d11_device + d3d11_resources)
        # OpenGL 两个纹理轴必须保持 clamp-to-edge。
        self.assertGreaterEqual(opengl.count("glow::CLAMP_TO_EDGE as i32"), 2)
        # D3D11 三个纹理轴必须保持相同 clamp 语义。
        self.assertEqual(d3d11_resources.count("D3D11_TEXTURE_ADDRESS_CLAMP"), 3)


# 支持直接运行当前跨后端 sampler 契约。
if __name__ == "__main__":
    # 执行本文件内全部契约测试。
    unittest.main()
