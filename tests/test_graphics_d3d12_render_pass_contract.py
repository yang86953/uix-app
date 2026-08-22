# -*- coding: utf-8 -*-
"""锁定 GFX-NEXT-17 的 D3D12 render-target/pass/clear/submit 纵向切片。"""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONTEXT = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/mod.rs"
METHODS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/methods.rs"
DEVICE = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_device.rs"
PASS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_device_pass.rs"
PIPELINE = ROOT / "src/native/presentation/graphics/d3d12/adapter/pipeline/mod.rs"
SURFACE = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_surface.rs"
GRAPHICS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/graphics.rs"
REGISTRY = ROOT / "src/native/factory/registry_windows.rs"


def function_range(source: str, signature: str, next_signature: str) -> str:
    """按稳定签名截取一个实现片段。"""
    start = source.index(signature)
    end = source.index(next_signature, start)
    return source[start:end]


def without_line_comments(source: str) -> str:
    """删除行注释，让否定断言只检查真实代码。"""
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


class GraphicsD3d12RenderPassContractTests(unittest.TestCase):
    """验证共享权威、原生编码、提交顺序、能力和未激活边界。"""

    def test_unique_resource_component_owns_shared_pass_and_submission(self) -> None:
        context = CONTEXT.read_text(encoding="utf-8")
        device = DEVICE.read_text(encoding="utf-8")
        pass_source = PASS.read_text(encoding="utf-8")

        self.assertEqual(
            context.count("rhi_device: rhi_device::D3d12RhiDevice"), 1
        )
        self.assertNotIn("rhi_submissions", context)
        self.assertEqual(device.count("    pass: RhiPassState,"), 1)
        self.assertEqual(
            device.count("    submission_sequence: RhiSubmissionSequence,"), 1
        )
        self.assertIn("pass: RhiPassState::new()", device)
        self.assertIn(
            "submission_sequence: RhiSubmissionSequence::new()", device
        )
        for shared_type in (
            "RenderTargetHandle",
            "LoadAction",
            "RhiColor",
            "RhiScissor",
        ):
            self.assertIn(shared_type, device + pass_source)
        self.assertNotIn("struct D3d12PassState", pass_source)
        self.assertNotIn("struct D3d12Submission", pass_source)

    def test_surface_and_texture_targets_are_gated_before_command_side_effects(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        pass_source = PASS.read_text(encoding="utf-8")
        texture = function_range(
            pass_source,
            "fn stage_texture_target(",
            "pub(super) fn validate_pending_texture_destroy(",
        )
        surface = function_range(
            pass_source,
            "fn stage_surface_target(",
            "pub(super) fn rhi_begin_render_pass(",
        )
        begin = function_range(
            pass_source,
            "pub(super) fn rhi_begin_render_pass(",
            "pub(super) fn rhi_clear_rect(",
        )
        create_rtv = function_range(
            device,
            "fn create_texture_rtv_heap(",
            "fn create_committed_resource(",
        )

        self.assertIn("rtv_heap: Option<ID3D12DescriptorHeap>", device)
        self.assertIn("D3D12_DESCRIPTOR_HEAP_TYPE_RTV", create_rtv)
        self.assertIn("CreateRenderTargetView(texture, None, rtv)", create_rtv)
        self.assertLess(
            texture.index("self.resolve_render_target(texture)?"),
            texture.index("resource.native.clone()"),
        )
        self.assertLess(
            texture.index("supports_render_target()"),
            texture.index("GetCPUDescriptorHandleForHeapStart()"),
        )
        self.assertLess(
            texture.index("native_desc.Format != texture_format"),
            texture.index("resource.native.clone()"),
        )
        self.assertLess(
            surface.index("self.surface_lifecycle.ensure_active()?"),
            surface.index("self.back_buffers[self.frame_index].clone()"),
        )
        self.assertLess(
            surface.index("native_desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM"),
            surface.index("self.rtv_handle(self.frame_index)"),
        )
        for gate in (
            "self.ensure_healthy()?",
            "self.rhi_device.pass.require_closed()?",
            "self.stage_surface_target()?",
            "self.rhi_device.stage_texture_target(target, texture)?",
            "self.rhi_device.pass.begin(target, native.extent, load)?",
        ):
            self.assertLess(begin.index(gate), begin.index("reset_rhi_command_list("))
        self.assertLess(
            begin.index("reset_rhi_command_list("),
            begin.index("record_transition("),
        )
        self.assertLess(
            begin.index("validate_color_clear_contract()?"),
            begin.index("reset_rhi_command_list("),
        )
        self.assertLess(begin.index("record_transition("), begin.index("RSSetViewports("))
        self.assertLess(begin.index("RSSetViewports("), begin.index("RSSetScissorRects("))
        self.assertLess(begin.index("RSSetScissorRects("), begin.index("OMSetRenderTargets("))
        self.assertIn("if let LoadAction::Clear(color) = load", begin)
        self.assertIn("ClearRenderTargetView(native.rtv", begin)

    def test_clear_rect_uses_shared_top_left_geometry_before_native_clear(self) -> None:
        pass_source = PASS.read_text(encoding="utf-8")
        clear = function_range(
            pass_source,
            "pub(super) fn rhi_clear_rect(",
            "pub(super) fn rhi_end_render_pass(",
        )

        self.assertLess(
            clear.index("self.rhi_device.pass.validate_clear(color, scissor)?"),
            clear.index("validate_color_clear_contract()?"),
        )
        self.assertLess(
            clear.index("validate_color_clear_contract()?"),
            clear.index(".native_rect()"),
        )
        self.assertLess(
            clear.index(".native_rect()"),
            clear.index("ClearRenderTargetView("),
        )
        self.assertIn("let rect = RECT", clear)
        self.assertIn("Some(std::slice::from_ref(&rect))", clear)
        self.assertNotIn("bottom_origin_y", clear)

    def test_end_and_submit_publish_only_after_gpu_success(self) -> None:
        pass_source = PASS.read_text(encoding="utf-8")
        end = function_range(
            pass_source,
            "pub(super) fn rhi_end_render_pass(",
            "pub(super) fn rhi_submit(",
        )
        submit = pass_source[pass_source.index("pub(super) fn rhi_submit(") :]

        self.assertLess(end.index("pass.require_open()?"), end.index("OMSetRenderTargets("))
        self.assertLess(end.index("OMSetRenderTargets("), end.index("record_transition("))
        self.assertLess(end.index("record_transition("), end.index("pass.end()?"))
        self.assertLess(end.index("pass.end()?"), end.index("pending_targets.push(target)"))
        self.assertLess(
            submit.index("pass.require_closed()?"),
            submit.index("validate_pending_target_commits("),
        )
        self.assertLess(
            submit.index("validate_pending_target_commits("),
            submit.index("self.execute_recording_and_wait()?"),
        )
        self.assertLess(
            submit.index("self.execute_recording_and_wait()?"),
            submit.index("commit_pending_target_states("),
        )
        self.assertLess(
            submit.index("commit_pending_target_states("),
            submit.index("submission_sequence.issue()"),
        )
        self.assertLess(
            submit.index("submission_sequence.issue()"),
            submit.index("pending_targets.clear()"),
        )
        self.assertIn("target.retain_after_undrained_drop()", DEVICE.read_text(encoding="utf-8"))

    def test_present_consumes_only_the_shared_latest_submission(self) -> None:
        surface = SURFACE.read_text(encoding="utf-8")
        pass_source = PASS.read_text(encoding="utf-8")
        present = surface[surface.index("fn present(") :]
        d3d12_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "src/native/presentation/graphics/d3d12").rglob("*.rs")
        )

        self.assertIn("self.pass.require_closed()?", pass_source)
        self.assertIn("!self.pending_targets.is_empty()", pass_source)
        self.assertIn("if self.recording", pass_source)
        self.assertLess(
            present.index("self.rhi_require_present_ready()?"),
            present.index(
                "transaction.validate(current, coherency, self.rhi_device.submissions())?"
            ),
        )
        self.assertLess(
            present.index(
                "transaction.validate(current, coherency, self.rhi_device.submissions())?"
            ),
            present.index("self.present_result(&present)"),
        )
        self.assertEqual(without_line_comments(d3d12_sources).count(".issue()"), 1)
        self.assertNotIn("SubmissionHandle::from_raw", without_line_comments(d3d12_sources))

    def test_capabilities_and_inactive_boundaries_remain_honest(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        graphics = GRAPHICS.read_text(encoding="utf-8")
        registry = REGISTRY.read_text(encoding="utf-8")
        pass_source = PASS.read_text(encoding="utf-8")
        upper_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for directory in (ROOT / "src/app", ROOT / "src/ui", ROOT / "src/draw")
            for path in directory.rglob("*.rs")
        )

        for enabled in ("clear_rect: true", "render_to_texture: true", "scissor: true"):
            self.assertIn(enabled, device)
        for enabled in (
            "sampled_textures: true",
            "premultiplied_alpha_blend: true",
            "additive_blend: true",
        ):
            self.assertIn(enabled, device)
        self.assertIn("self.rhi_device.preflight_draw_resources(packet)", device)
        self.assertIn("self.draw_rhi_packet(packet)", device)
        self.assertNotIn("resource_stage_deferred", device)
        self.assertIn("self.rhi_device.create_pipeline(&self.device, desc)", device)
        self.assertIn("self.rhi_device.destroy_pipeline(pipeline)", device)
        for forbidden in ("create_pipeline", "DrawInstanced", "DrawIndexedInstanced"):
            self.assertNotIn(forbidden, pass_source)
        self.assertIn('"D3D12 thin RHI is not implemented"', graphics)
        self.assertIn("Errc::NotImplemented", graphics)
        self.assertNotIn("GraphicsApi::D3d12", registry)
        for upper_token in ("D3d12", "Direct3D12", "GraphicsApi::D3d12"):
            self.assertNotIn(upper_token, upper_sources)

    def test_touched_files_stay_below_limit(self) -> None:
        for path in (
            CONTEXT,
            METHODS,
            DEVICE,
            PASS,
            PIPELINE,
            SURFACE,
            GRAPHICS,
            Path(__file__),
        ):
            self.assertLess(len(path.read_text(encoding="utf-8").splitlines()), 1500, path)


if __name__ == "__main__":
    unittest.main()
