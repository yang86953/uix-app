"""Vulkan Shader ABI 与 Pipeline 资源生命周期源码契约。"""

from pathlib import Path
import struct
import unittest


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "src/native/presentation/graphics/vulkan/adapter"
DEVICE = ADAPTER / "context/rhi_device.rs"
PIPELINE = ADAPTER / "context/rhi_pipeline.rs"
VERTEX = ADAPTER / "shaders/rhi.vert"
FRAGMENT = ADAPTER / "shaders/rhi.frag"
SPV = ADAPTER / "shaders/spv"
GENERATOR = ROOT / "scripts/compile_vulkan_rhi_shaders.py"


class VulkanRhiPipelineContractTests(unittest.TestCase):
    """锁定共享 PipelineKind 到 Vulkan 原生资源的单向映射。"""

    def test_pipeline_identity_uses_platform_resource_table(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("RhiPipelineResourceTable<VulkanRhiPipeline>", device)
        self.assertIn("VulkanRhiPipeline::create(device, desc.kind)?", device)
        self.assertIn("self.pipelines.insert(desc.kind, resource)", device)
        self.assertIn("self.pipelines.take(binding)?", device)
        self.assertNotIn("HashMap", device)

    def test_all_shared_pipeline_kinds_have_one_shader_pair(self) -> None:
        pipeline = PIPELINE.read_text(encoding="utf-8")
        expected_kinds = (
            "SolidMesh",
            "TexturedQuad",
            "TexturedQuadAdditive",
            "GradientRect",
            "GlyphCoverageQuad",
            "ShapeRect",
            "ShapeRectAdditive",
            "BoxShadow",
            "BlurPass",
            "MsdfGlyphQuad",
            "Sector",
            "LineSegment",
        )

        for kind in expected_kinds:
            self.assertIn(f"PipelineKind::{kind}", pipeline)
        self.assertIn("VulkanPipelineState::from_contract(contract)?", pipeline)

    def test_descriptor_abi_has_one_uniform_and_optional_sampled_binding(self) -> None:
        pipeline = PIPELINE.read_text(encoding="utf-8")

        self.assertIn("binding(0)", pipeline)
        self.assertIn("vk::DescriptorType::UNIFORM_BUFFER", pipeline)
        self.assertIn("binding(1)", pipeline)
        self.assertIn("vk::DescriptorType::COMBINED_IMAGE_SAMPLER", pipeline)
        self.assertIn("matches!(sampling, PipelineSampling::None)", pipeline)

    def test_shader_sources_share_std140_uniform_and_set_zero(self) -> None:
        vertex = VERTEX.read_text(encoding="utf-8")
        fragment = FRAGMENT.read_text(encoding="utf-8")

        for source in (vertex, fragment):
            self.assertIn("set = 0, binding = 0, std140", source)
            self.assertNotIn("#ifdef _WIN32", source)
            self.assertNotIn("target_os", source)
        self.assertIn("set = 0, binding = 1", fragment)

    def test_committed_spirv_assets_have_vulkan_magic(self) -> None:
        expected_assets = {
            "mesh.vert.spv",
            "sampled.vert.spv",
            "gradient.vert.spv",
            "shape.vert.spv",
            "shadow.vert.spv",
            "blur.vert.spv",
            "msdf.vert.spv",
            "sector.vert.spv",
            "line.vert.spv",
            "mesh.frag.spv",
            "textured.frag.spv",
            "coverage.frag.spv",
            "gradient.frag.spv",
            "shape.frag.spv",
            "shadow.frag.spv",
            "blur.frag.spv",
            "msdf.frag.spv",
            "sector.frag.spv",
            "line.frag.spv",
        }

        actual_assets = {path.name for path in SPV.glob("*.spv")}
        self.assertEqual(actual_assets, expected_assets)
        generator = GENERATOR.read_text(encoding="utf-8")
        for asset in expected_assets:
            self.assertIn(f'"{asset}"', generator)
            data = (SPV / asset).read_bytes()
            self.assertGreater(len(data), 20)
            self.assertEqual(struct.unpack_from("<I", data)[0], 0x07230203)
        self.assertIn('"--target-env=vulkan1.0"', generator)
        self.assertIn('require_tool("spirv-val")', generator)


if __name__ == "__main__":
    unittest.main()
