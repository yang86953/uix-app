"""Vulkan Adapter 消费 platform thin RHI 固定状态的源码契约。"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
ADAPTER = ROOT / "src/native/presentation/graphics/vulkan/adapter/mod.rs"
MAPPING = ROOT / "src/native/presentation/graphics/vulkan/adapter/rhi.rs"


class VulkanRhiMappingContractTests(unittest.TestCase):
    """锁定第二阶段首个里程碑的单一契约所有权。"""

    def test_vulkan_adapter_owns_only_native_mapping(self) -> None:
        adapter = ADAPTER.read_text(encoding="utf-8")
        mapping = MAPPING.read_text(encoding="utf-8")

        self.assertIn("mod rhi;", adapter)
        self.assertIn("crate::platform::presentation::rhi", mapping)
        self.assertNotRegex(
            mapping,
            re.compile(
                r"(?:pub(?:\([^)]*\))?\s+)?enum\s+"
                r"(?:PipelineKind|PipelineBlend|TextureFormat|IndexFormat|SamplerDesc)\b"
            ),
        )

    def test_pipeline_state_is_derived_from_shared_contract(self) -> None:
        mapping = MAPPING.read_text(encoding="utf-8")

        self.assertIn("fn from_contract(contract: PipelineContract)", mapping)
        self.assertRegex(mapping, re.compile(r"contract\s*\.vertex\s*\.attributes\(\)"))
        self.assertIn("contract.blend.state()", mapping)
        self.assertIn("primitive_topology(contract.topology)", mapping)
        self.assertIn("contract.multisample.sample_mask()", mapping)

    def test_closed_shared_formats_have_explicit_vulkan_mappings(self) -> None:
        mapping = MAPPING.read_text(encoding="utf-8")

        expected = (
            "TextureFormat::Bgra8Unorm => vk::Format::B8G8R8A8_UNORM",
            "TextureFormat::Rgba8Unorm => vk::Format::R8G8B8A8_UNORM",
            "TextureFormat::R8Unorm => vk::Format::R8_UNORM",
            "IndexFormat::Uint32 => vk::IndexType::UINT32",
            "PipelinePrimitiveTopology::TriangleList => vk::PrimitiveTopology::TRIANGLE_LIST",
            "SamplerAddressMode::ClampToEdge => vk::SamplerAddressMode::CLAMP_TO_EDGE",
        )
        for item in expected:
            self.assertIn(item, mapping)


if __name__ == "__main__":
    unittest.main()
