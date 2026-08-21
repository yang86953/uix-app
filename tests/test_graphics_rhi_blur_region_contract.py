# -*- coding: utf-8 -*-
# 验证 Blur 子区域语义只由共享 Drawing/platform 合同解释一次。
"""Keep Blur source/destination regions and texel steps API-neutral."""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BLUR = ROOT / "src/platform/presentation/rhi/blur.rs"
PIPELINE = ROOT / "src/platform/presentation/rhi/pipeline.rs"
RENDERER = ROOT / "src/draw/backend/rhi_renderer_blur.rs"
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
OPENGL_SHADER = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_shaders.rs"
D3D11_SHADER = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"
D3D11_PIPELINE = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/pipeline.rs"
D3D11_BLUR = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/rhi_blur.rs"
VULKAN_VERTEX = ROOT / "src/native/presentation/graphics/vulkan/adapter/shaders/rhi.vert"
VULKAN_FRAGMENT = ROOT / "src/native/presentation/graphics/vulkan/adapter/shaders/rhi.frag"


class GraphicsRhiBlurRegionContractTests(unittest.TestCase):
    # 源纹理、源域、目标纹理与目标区域必须通过一个值对象建立。
    def test_shared_geometry_is_the_only_region_constructor(self) -> None:
        blur = BLUR.read_text(encoding="utf-8")
        self.assertIn("pub(crate) struct RhiBlurPassGeometry", blur)
        self.assertIn("source_extent: RhiExtent", blur)
        self.assertIn("source: RhiTextureRegionBounds", blur)
        self.assertIn("target_extent: RhiExtent", blur)
        self.assertIn("destination: RhiTextureRegionBounds", blur)
        self.assertIn("if source.extent() != destination.extent()", blur)
        self.assertIn("source.validate_within(source_extent)?", blur)
        self.assertIn("destination.validate_within(target_extent)?", blur)
        self.assertIn("pub(crate) fn vertex_values(self) -> [f32; 24]", blur)
        self.assertIn("let uv_bounds = [", blur)
        self.assertIn("RhiBlurDirection::Horizontal => [inverse_width, 0.0]", blur)
        self.assertIn("RhiBlurDirection::Vertical => [0.0, inverse_height]", blur)
        self.assertNotIn("cfg(target_os", blur)
        self.assertNotIn("cfg(windows)", blur)

    # Drawing 必须从同一几何一次生成 position/UV，并只按方向生成 uniform。
    def test_drawing_reuses_one_geometry_without_api_branches(self) -> None:
        renderer = RENDERER.read_text(encoding="utf-8")
        self.assertEqual(renderer.count("RhiBlurPassGeometry::new("), 1)
        self.assertEqual(renderer.count("geometry.vertex_values()"), 1)
        self.assertIn("RhiBlurDirection::Horizontal", renderer)
        self.assertIn("RhiBlurDirection::Vertical", renderer)
        self.assertIn("geometry.raster_params(direction, tap_radius, weights)", renderer)
        self.assertNotIn("let left = region.x", renderer)
        self.assertNotIn("target_os", renderer)
        self.assertNotIn("GraphicsApi", renderer)

    # 三端只能机械消费 position/UV、采样边界和规范 step，不得保留 region 公式。
    def test_adapters_have_no_private_region_formula(self) -> None:
        pipeline = PIPELINE.read_text(encoding="utf-8")
        opengl_draw = OPENGL_DRAW.read_text(encoding="utf-8")
        opengl = OPENGL_SHADER.read_text(encoding="utf-8")
        d3d11 = D3D11_SHADER.read_text(encoding="utf-8")
        d3d11_pipeline = D3D11_PIPELINE.read_text(encoding="utf-8")
        d3d11_blur = D3D11_BLUR.read_text(encoding="utf-8")
        vulkan_vertex = VULKAN_VERTEX.read_text(encoding="utf-8")
        vulkan_fragment = VULKAN_FRAGMENT.read_text(encoding="utf-8")

        self.assertIn("PipelineVertexLayout::PositionUvF32", pipeline)
        self.assertIn(
            "d3d11_vertex_elements(PipelineVertexLayout::PositionUvF32)",
            d3d11_pipeline,
        )
        self.assertIn("context.IASetInputLayout(&self.layout_blur)", d3d11_blur)
        self.assertIn("v_uv = a_uv;", opengl)
        self.assertIn("o.uv = input.uv;", d3d11)
        self.assertIn("v_uv = a_uv;", vulkan_vertex)
        for fragment in (opengl, d3d11, vulkan_fragment):
            self.assertIn("clamp(", fragment)
            self.assertIn("uv_bounds", fragment)
            self.assertIn("step_taps", fragment)
        for adapter in (
            opengl_draw,
            opengl,
            d3d11,
            d3d11_pipeline,
            d3d11_blur,
            vulkan_vertex,
            vulkan_fragment,
        ):
            self.assertNotIn("u_region", adapter)
            self.assertNotIn("u_sizes", adapter)
            self.assertNotIn("region.xy", adapter)


if __name__ == "__main__":
    unittest.main()
