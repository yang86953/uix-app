# -*- coding: utf-8 -*-
"""锁定 GFX-NEXT-19 的 D3D12 DrawPacket 原生纵向切片。"""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SHARED_RHI = ROOT / "src/platform/presentation/rhi"
D3D12 = ROOT / "src/native/presentation/graphics/d3d12/adapter"
DEVICE = D3D12 / "context/rhi_device.rs"
DRAW = D3D12 / "context/rhi_device_draw.rs"
PASS = D3D12 / "context/rhi_device_pass.rs"
PIPELINE = D3D12 / "pipeline/mod.rs"
GRAPHICS = D3D12 / "context/graphics.rs"
REGISTRY = ROOT / "src/native/factory/registry_windows.rs"


def function_range(source: str, signature: str, next_signature: str) -> str:
    """按稳定签名截取一个实现片段。"""
    start = source.index(signature)
    end = source.index(next_signature, start)
    return source[start:end]


def without_line_comments(source: str) -> str:
    """删除行注释，让否定断言只检查真实代码。"""
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


class GraphicsD3d12DrawContractTests(unittest.TestCase):
    """验证共享门禁、原生绑定、命令编码、存活期和未激活边界。"""

    def test_draw_reuses_shared_packet_resource_and_pass_authorities(self) -> None:
        draw = DRAW.read_text(encoding="utf-8")
        device = DEVICE.read_text(encoding="utf-8")

        for authority in (
            "DrawPacket",
            "RhiBufferResourceTable",
            "RhiPassState",
            "SampledTextureBinding",
            "DrawRange",
        ):
            self.assertIn(authority, draw + device)
        preflight = function_range(
            draw,
            "pub(super) fn preflight_draw_resources(",
            "fn validate_sampled_resources(",
        )
        self.assertLess(
            preflight.index("self.pipelines.get(packet.pipeline())?"),
            preflight.index("self.buffers.validate_draw(packet)?"),
        )
        self.assertIn("self.validate_sampled_resources(binding)?", preflight)
        sampled = function_range(
            draw,
            "fn validate_sampled_resources(",
            "fn stage_draw(",
        )
        self.assertIn("binding.validate_resources(texture.desc.format(), sampler.desc)", sampled)
        for side_effect in ("SetDescriptorHeaps", "SetPipelineState", "DrawInstanced"):
            self.assertNotIn(side_effect, preflight)
        self.assertIn("self.rhi_device.preflight_draw_resources(packet)", device)

    def test_all_read_only_gates_precede_every_native_draw_side_effect(self) -> None:
        draw = DRAW.read_text(encoding="utf-8")
        execute = function_range(
            draw,
            "pub(super) fn draw_rhi_packet(",
            "fn validate_buffer_native(",
        )
        self.assertLess(execute.index("stage_draw(packet"), execute.index("record_draw_commands("))
        stage = function_range(draw, "fn stage_draw(", "fn stage_sampled_resources(")
        for gate in (
            "self.pass.require_open()?",
            "self.preflight_draw_resources(packet)?",
            "self.pass.validate_draw_raster(packet.raster())?",
            ".native_binding(contract, target.format())?",
            "validate_buffer_native(vertex, BufferUsage::Vertex)?",
            "validate_buffer_native(uniform, BufferUsage::Uniform)?",
            "self.stage_sampled_resources(binding)",
        ):
            self.assertIn(gate, stage)
        for side_effect in (
            "SetDescriptorHeaps",
            "SetGraphicsRootSignature",
            "SetPipelineState",
            "IASetVertexBuffers",
            "DrawInstanced",
            "DrawIndexedInstanced",
        ):
            self.assertNotIn(side_effect, without_line_comments(stage))

    def test_texture_owns_real_shader_visible_srv_and_sampling_is_stateless(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        draw = DRAW.read_text(encoding="utf-8")
        texture = function_range(device, "struct D3d12RhiTexture", "impl RhiTextureResource")
        create_srv = function_range(
            device,
            "fn create_texture_srv_heap(",
            "fn create_texture_rtv_heap(",
        )
        self.assertIn("srv_heap: ID3D12DescriptorHeap", texture)
        self.assertIn("srv_cpu: D3D12_CPU_DESCRIPTOR_HANDLE", texture)
        self.assertIn("D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV", create_srv)
        self.assertIn("D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE", create_srv)
        self.assertLess(create_srv.index("desc.validate()?"), create_srv.index("CreateDescriptorHeap("))
        self.assertLess(create_srv.index("CreateDescriptorHeap("), create_srv.index("CreateShaderResourceView("))

        sampled = function_range(
            draw,
            "fn stage_sampled_resources(",
            "impl D3d12Context",
        )
        for gate in (
            "self.pass.validate_sampled_texture(binding.texture())?",
            "self.validate_sampled_resources(binding)?",
            "effective != D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE",
            "validate_texture_native(texture)?",
            "validate_descriptor_heap(",
        ):
            self.assertIn(gate, sampled)
        self.assertNotIn("history", without_line_comments(sampled).lower())

    def test_pipeline_exposes_only_target_selected_root_and_pso_binding(self) -> None:
        pipeline = PIPELINE.read_text(encoding="utf-8")
        binding = function_range(
            pipeline,
            "pub(crate) fn native_binding(",
            "pub(crate) fn retain_after_undrained_drop",
        )
        self.assertIn("self.contract != contract", binding)
        self.assertIn("TextureFormat::Bgra8Unorm => self.variants.bgra8.clone()", binding)
        self.assertIn("TextureFormat::Rgba8Unorm => self.variants.rgba8.clone()", binding)
        self.assertIn("TextureFormat::R8Unorm", binding)
        self.assertIn("root_signature: self.root_signature.clone()", binding)
        for parameter in (
            "ROOT_PARAMETER_CBV_B0",
            "ROOT_PARAMETER_SRV_T0",
            "ROOT_PARAMETER_SAMPLER_S0",
        ):
            self.assertIn(parameter, pipeline)
        self.assertNotIn("PipelineKind::", DRAW.read_text(encoding="utf-8"))

    def test_views_roots_raster_and_draw_range_encode_mechanically(self) -> None:
        draw = DRAW.read_text(encoding="utf-8")
        record = function_range(draw, "fn record_draw_commands(", "fn draw_invalid(")
        ordered = (
            "SetDescriptorHeaps",
            "SetGraphicsRootSignature",
            "SetPipelineState",
            "IASetPrimitiveTopology",
            "IASetVertexBuffers",
            "IASetIndexBuffer",
            "RSSetViewports",
            "RSSetScissorRects",
            "SetGraphicsRootConstantBufferView",
        )
        positions = [record.index(call) for call in ordered]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("SetGraphicsRootDescriptorTable(srv_parameter, sampled.srv_gpu)", record)
        self.assertIn("SetGraphicsRootDescriptorTable(sampler_parameter, sampled.sampler_gpu)", record)
        self.assertIn("plan.range.index_binding().is_some()", record)
        self.assertIn("DrawIndexedInstanced(", record)
        self.assertIn("DrawInstanced(", record)
        self.assertIn("plan.range.index_count()", record)
        self.assertIn("plan.range.first_index()", record)
        self.assertIn("plan.range.vertex_count()", record)
        self.assertIn("plan.range.first_vertex()", record)
        self.assertIn("D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST", draw)
        self.assertIn("raster", draw)
        self.assertIn(".viewport()", draw)
        self.assertIn(".native_size_i32()", draw)
        self.assertIn("raster.scissor()", draw)

    def test_draw_references_survive_until_fence_and_unknown_state_leaks(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        draw = DRAW.read_text(encoding="utf-8")
        pass_source = PASS.read_text(encoding="utf-8")
        submit = pass_source[pass_source.index("pub(super) fn rhi_submit(") :]
        execute = function_range(
            draw,
            "pub(super) fn draw_rhi_packet(",
            "fn validate_buffer_native(",
        )
        self.assertIn("pending_draw_resources: Vec<rhi_device_draw::D3d12RhiDrawResources>", device)
        self.assertLess(execute.index("record_draw_commands("), execute.index("pending_draw_resources"))
        self.assertLess(submit.index("self.execute_recording_and_wait()?"), submit.index("submission_sequence.issue()"))
        self.assertLess(submit.index("submission_sequence.issue()"), submit.index("pending_draw_resources.clear()"))
        self.assertIn("draw.retain_after_undrained_drop()", device)
        for owner in (
            "D3d12PipelineNativeBinding",
            "vertex: ID3D12Resource",
            "uniform: ID3D12Resource",
            "index: Option<ID3D12Resource>",
            "texture: ID3D12Resource",
            "srv_heap: ID3D12DescriptorHeap",
            "sampler_heap: ID3D12DescriptorHeap",
        ):
            self.assertIn(owner, draw)

    def test_buffer_upload_versions_prevent_later_draw_data_aliasing(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        owner = device[device.index("impl D3d12RhiDevice {") :]
        update = function_range(
            owner,
            "fn update_buffer(&mut self, device:",
            "fn preflight_buffer_upload(",
        )
        self.assertLess(update.index("upload.validate(desc)?"), update.index("create_committed_resource("))
        self.assertLess(update.index("native.Map("), update.index("native.Unmap("))
        self.assertLess(update.index("native.Unmap("), update.index("self.buffers.get_mut("))
        self.assertIn(".native = native", update)

    def test_capabilities_enable_only_real_draw_while_production_entry_stays_closed(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        graphics = GRAPHICS.read_text(encoding="utf-8")
        registry = REGISTRY.read_text(encoding="utf-8")
        upper = "\n".join(
            path.read_text(encoding="utf-8")
            for directory in (ROOT / "src/app", ROOT / "src/ui", ROOT / "src/draw")
            for path in directory.rglob("*.rs")
        )
        for capability in (
            "sampled_textures: true",
            "premultiplied_alpha_blend: true",
            "additive_blend: true",
            "clear_rect: true",
            "render_to_texture: true",
            "scissor: true",
        ):
            self.assertIn(capability, device)
        self.assertIn('"D3D12 thin RHI is not implemented"', graphics)
        self.assertNotIn("GraphicsApi::D3d12", registry)
        for token in ("D3d12", "Direct3D12", "GraphicsApi::D3d12"):
            self.assertNotIn(token, upper)

    def test_no_second_hlsl_or_shared_contract_and_files_stay_below_limit(self) -> None:
        draw = DRAW.read_text(encoding="utf-8")
        for forbidden in (
            "const PIPELINE",
            "struct DrawPacket",
            "struct PipelineContract",
            "struct RhiPassState",
            "VSMain",
            "PSMain",
            "HLSL",
        ):
            self.assertNotIn(forbidden, without_line_comments(draw))
        self.assertEqual(
            sum(
                path.read_text(encoding="utf-8").count("pub(crate) struct DrawPacket")
                for path in SHARED_RHI.rglob("*.rs")
            ),
            1,
        )
        for path in (DEVICE, DRAW, PASS, PIPELINE, Path(__file__)):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


if __name__ == "__main__":
    unittest.main()
