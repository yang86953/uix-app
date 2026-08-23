# -*- coding: utf-8 -*-
"""锁定 GFX-NEXT-18 的 D3D12 十二类原生 pipeline 资源 baseline。"""

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SHARED_PIPELINE = ROOT / "src/platform/presentation/rhi/pipeline.rs"
SHARED_TABLE = ROOT / "src/platform/presentation/rhi/pipeline_resource_table.rs"
D3D_SOURCE = ROOT / "src/native/presentation/graphics/d3d_shader_source.rs"
D3D11_PIPELINE = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline"
D3D12_ROOT = ROOT / "src/native/presentation/graphics/d3d12"
D3D12_DEVICE = D3D12_ROOT / "adapter/context/rhi_device.rs"
D3D12_PIPELINE = D3D12_ROOT / "adapter/pipeline/mod.rs"
D3D12_PASS = D3D12_ROOT / "adapter/context/rhi_device_pass.rs"
D3D12_GRAPHICS = D3D12_ROOT / "adapter/context/graphics.rs"
REGISTRY = ROOT / "src/native/factory/registry_windows.rs"
GRAPHICS_MODULE = ROOT / "src/native/presentation/graphics/mod.rs"
CARGO = ROOT / "Cargo.toml"


def function_range(source: str, signature: str, next_signature: str) -> str:
    """按稳定签名截取一个实现片段。"""
    start = source.index(signature)
    end = source.index(next_signature, start)
    return source[start:end]


def rust_sources(root: Path) -> str:
    """按稳定路径顺序读取一个原生模块的全部 Rust 源。"""
    return "\n".join(
        path.read_text(encoding="utf-8") for path in sorted(root.rglob("*.rs"))
    )


class GraphicsD3d12PipelineContractTests(unittest.TestCase):
    """验证共享权威、原生对象、事务登记、生命周期和未激活边界。"""

    def test_all_pipeline_kinds_exhaustively_map_without_a_second_list(self) -> None:
        shared = SHARED_PIPELINE.read_text(encoding="utf-8")
        native = D3D12_PIPELINE.read_text(encoding="utf-8")
        enum_body = shared[shared.index("enum PipelineKind") :]
        enum_body = enum_body[: enum_body.index("}")]
        kinds = tuple(
            re.findall(r"^\s*([A-Z][A-Za-z0-9_]*),\s*$", enum_body, re.MULTILINE)
        )

        self.assertEqual(
            kinds,
            (
                "SolidMesh",
                "TexturedQuad",
                "GradientRect",
                "GlyphCoverageQuad",
                "ShapeRect",
                "ShapeRectAdditive",
                "BoxShadow",
                "TexturedQuadAdditive",
                "BlurPass",
                "MsdfGlyphQuad",
                "Sector",
                "LineSegment",
            ),
        )
        mapping = function_range(native, "fn d3d12_shader_pair(", "fn create_root_signature(")
        for kind in kinds:
            with self.subTest(kind=kind):
                self.assertEqual(
                    len(re.findall(rf"PipelineKind::{kind}(?![A-Za-z0-9_])", mapping)),
                    1,
                )
        self.assertNotIn("PIPELINE_KINDS", native)
        self.assertNotRegex(native, r"const\s+[^\n]*PIPELINE[^\n]*\[")

    def test_pipeline_plan_consumes_the_shared_contract_mechanically(self) -> None:
        shared = SHARED_PIPELINE.read_text(encoding="utf-8")
        native = D3D12_PIPELINE.read_text(encoding="utf-8")
        plan = function_range(
            native,
            "fn from_desc(desc: PipelineDesc)",
            "impl D3d12PipelineFixedState",
        )

        self.assertIn("let contract = desc.kind.contract();", plan)
        self.assertIn("validate_shared_contract(contract)?", plan)
        self.assertIn("d3d12_vertex_elements(contract.vertex)?", plan)
        self.assertIn("D3d12PipelineFixedState::from_contract(contract)", plan)
        for field in (
            "contract.uniform.size_bytes()",
            "contract.sampling",
            "contract.blend.state()",
            "contract.topology",
            "contract.dither",
            "contract.multisample",
            "contract.raster",
            "contract.depth_stencil",
        ):
            with self.subTest(shared_field=field):
                self.assertIn(field, native)
        self.assertIn("pub(crate) struct PipelineContract", shared)
        self.assertNotIn("windows::", shared)
        self.assertNotIn("D3D12", shared)

    def test_d3d11_and_d3d12_compile_one_shared_hlsl_source_set(self) -> None:
        shared = D3D_SOURCE.read_text(encoding="utf-8")
        d3d11 = rust_sources(D3D11_PIPELINE)
        d3d12 = D3D12_PIPELINE.read_text(encoding="utf-8")
        graphics = GRAPHICS_MODULE.read_text(encoding="utf-8")
        cargo = CARGO.read_text(encoding="utf-8")
        source_names = (
            "RECT_HLSL",
            "GLYPH_HLSL",
            "RHI_TEXTURED_PS_HLSL",
            "GRADIENT_HLSL",
            "MESH_HLSL",
            "BLUR_HLSL",
            "SHADOW_HLSL",
            "MSDF_GLYPH_HLSL",
            "SECTOR_HLSL",
        )

        for name in source_names:
            with self.subTest(shader_source=name):
                self.assertEqual(shared.count(f"const {name}: &str ="), 1)
                self.assertNotIn(f"const {name}: &str =", d3d11)
                self.assertNotIn(f"const {name}: &str =", d3d12)
                self.assertIn(name, d3d11)
                self.assertIn(name, d3d12)
        self.assertIn(
            '#[cfg(all(windows, any(feature = "d3d11", feature = "d3d12")))]',
            graphics,
        )
        d3d12_feature = cargo[cargo.index("d3d12 = [") : cargo.index("]", cargo.index("d3d12 = ["))]
        self.assertNotIn('"d3d11"', d3d12_feature)
        self.assertIn('c"vs_4_0"', d3d11)
        self.assertIn('c"ps_4_0"', d3d11)
        self.assertIn('c"vs_5_1"', d3d12)
        self.assertIn('c"ps_5_1"', d3d12)

    def test_complete_native_resources_exist_before_binding_registration(self) -> None:
        native = D3D12_PIPELINE.read_text(encoding="utf-8")
        device = D3D12_DEVICE.read_text(encoding="utf-8")
        create_resource = function_range(
            native,
            "pub(crate) fn create(device:",
            "pub(crate) fn retain_after_undrained_drop",
        )
        create_binding = function_range(
            device,
            "fn create_pipeline(\n",
            "fn destroy_buffer(&mut self, buffer:",
        )

        for owner in (
            "root_signature: ID3D12RootSignature",
            "vertex_shader: ID3DBlob",
            "pixel_shader: ID3DBlob",
            "input_layout: Vec<D3D12_INPUT_ELEMENT_DESC>",
            "fixed_state: D3d12PipelineFixedState",
            "bgra8: ID3D12PipelineState",
            "rgba8: ID3D12PipelineState",
        ):
            self.assertIn(owner, native)
        for native_call in (
            "D3D12SerializeRootSignature(",
            "CreateRootSignature(0, bytes)",
            "D3DCompile(",
            "CreateGraphicsPipelineState(&desc)",
            "DXGI_FORMAT_B8G8R8A8_UNORM",
            "DXGI_FORMAT_R8G8B8A8_UNORM",
        ):
            self.assertIn(native_call, native)
        self.assertLess(create_resource.index("D3d12PipelinePlan::from_desc(desc)?"), create_resource.index("create_root_signature("))
        self.assertLess(create_resource.index("create_root_signature("), create_resource.index("D3d12PipelineStateVariants::create("))
        self.assertLess(create_binding.index("D3d12PipelineResource::create(device, desc)?"), create_binding.index("self.pipelines.insert(desc.kind, resource)"))
        self.assertEqual(
            SHARED_TABLE.read_text(encoding="utf-8").count(
                "pub(crate) struct RhiPipelineResourceTable"
            ),
            1,
        )

    def test_root_signature_and_fixed_state_follow_contract_values(self) -> None:
        native = D3D12_PIPELINE.read_text(encoding="utf-8")
        root = function_range(native, "fn create_root_signature(", "fn descriptor_table_parameter(")

        self.assertIn("D3D12_ROOT_PARAMETER_TYPE_CBV", root)
        self.assertIn("ShaderRegister: 0", root)
        self.assertIn("D3D12_DESCRIPTOR_RANGE_TYPE_SRV", root)
        self.assertIn("D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER", root)
        self.assertIn("match contract.sampling", root)
        self.assertIn("PipelineSampling::None => {}", root)
        self.assertIn("PipelineSampling::PremultipliedColor", root)
        self.assertIn("PipelineSampling::Coverage", root)
        self.assertIn("PipelineSampling::Msdf", root)
        self.assertIn("d3d12_blend_desc(contract)", native)
        self.assertIn("d3d12_rasterizer_desc(contract)", native)
        self.assertIn("d3d12_depth_stencil_desc(contract)", native)
        self.assertIn("contract.multisample.sample_mask()", native)
        self.assertIn("d3d12_topology_type(contract.topology)", native)

    def test_destroy_shutdown_and_undrained_drop_keep_unique_reverse_lifecycle(self) -> None:
        native = D3D12_PIPELINE.read_text(encoding="utf-8")
        device = D3D12_DEVICE.read_text(encoding="utf-8")
        resource = function_range(
            native,
            "pub(crate) struct D3d12PipelineResource {",
            "impl D3d12PipelinePlan",
        )
        destroy = function_range(
            device,
            "fn destroy_pipeline(&mut self, pipeline:",
            "pub(super) fn shutdown(&mut self)",
        )
        shutdown = function_range(
            device,
            "pub(super) fn shutdown(&mut self)",
            "pub(super) fn retain_after_undrained_drop",
        )
        retain = function_range(
            device,
            "pub(super) fn retain_after_undrained_drop",
            "impl GraphicsDevice for D3d12Context",
        )

        self.assertIn("self.pipelines.take(pipeline)?", destroy)
        self.assertLess(shutdown.index("self.pipelines.drain_reverse()"), shutdown.index("self.samplers.drain_reverse()"))
        self.assertLess(shutdown.index("self.samplers.drain_reverse()"), shutdown.index("self.textures.drain_reverse()"))
        self.assertLess(shutdown.index("self.textures.drain_reverse()"), shutdown.index("self.buffers.drain_reverse()"))
        self.assertIn("pipeline.retain_after_undrained_drop()", retain)
        # Rust 按字段声明顺序析构，锁定单项内部的原生依赖逆序。
        drop_order = (
            "variants: D3d12PipelineStateVariants",
            "vertex_shader: ID3DBlob",
            "pixel_shader: ID3DBlob",
            "input_layout: Vec<D3D12_INPUT_ELEMENT_DESC>",
            "fixed_state: D3d12PipelineFixedState",
            "contract: PipelineContract",
            "root_signature: ID3D12RootSignature",
        )
        positions = [resource.index(field) for field in drop_order]
        self.assertEqual(positions, sorted(positions))
        self.assertNotIn("impl Drop for D3d12PipelineResource", native)
        self.assertNotIn("ManuallyDrop", resource)
        self.assertIn("std::mem::forget(self)", native)

    def test_draw_registry_and_upper_source_remain_inactive(self) -> None:
        device = D3D12_DEVICE.read_text(encoding="utf-8")
        graphics = D3D12_GRAPHICS.read_text(encoding="utf-8")
        registry = REGISTRY.read_text(encoding="utf-8")
        pass_source = D3D12_PASS.read_text(encoding="utf-8")
        upper = "\n".join(
            rust_sources(ROOT / directory)
            for directory in ("src/app", "src/ui", "src/draw")
        )

        for capability in (
            "sampled_textures: true",
            "premultiplied_alpha_blend: true",
            "additive_blend: true",
        ):
            self.assertIn(capability, device)
        self.assertIn("self.rhi_device.preflight_draw_resources(packet)", device)
        self.assertIn("self.draw_rhi_packet(packet)", device)
        self.assertNotIn("resource_stage_deferred", device)
        self.assertNotIn("DrawInstanced", D3D12_PIPELINE.read_text(encoding="utf-8"))
        self.assertNotIn("DrawIndexedInstanced", D3D12_PIPELINE.read_text(encoding="utf-8"))
        self.assertNotIn("DrawInstanced", pass_source)
        self.assertIn('"D3D12 thin RHI is not implemented"', graphics)
        self.assertNotIn("GraphicsApi::D3d12", registry)
        for token in ("D3d12", "Direct3D12", "GraphicsApi::D3d12"):
            self.assertNotIn(token, upper)

    def test_touched_code_files_stay_below_limit(self) -> None:
        touched = (
            GRAPHICS_MODULE,
            D3D_SOURCE,
            D3D11_PIPELINE / "mod.rs",
            D3D11_PIPELINE / "pipeline.rs",
            D3D11_PIPELINE / "rhi_sector.rs",
            D3D12_ROOT / "adapter/mod.rs",
            D3D12_DEVICE,
            D3D12_PIPELINE,
            Path(__file__),
        )
        for path in touched:
            with self.subTest(path=path):
                self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500)


if __name__ == "__main__":
    unittest.main()
