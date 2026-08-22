# -*- coding: utf-8 -*-
"""锁定 GFX-NEXT-15 的 D3D12 唯一资源 owner 与未激活边界。"""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONTEXT = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/mod.rs"
METHODS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/methods.rs"
DEVICE = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_device.rs"
GRAPHICS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/graphics.rs"
REGISTRY = ROOT / "src/native/factory/registry_windows.rs"
RESOURCE_TABLE = ROOT / "src/platform/presentation/rhi/resource_table.rs"


def function_range(source: str, signature: str, next_signature: str) -> str:
    """按稳定签名截取一个实现片段。"""
    start = source.index(signature)
    end = source.index(next_signature, start)
    return source[start:end]


def without_line_comments(source: str) -> str:
    """删除行注释，让否定断言只检查真实代码。"""
    return "\n".join(line.split("//", 1)[0] for line in source.splitlines())


class GraphicsD3d12ResourceOwnerContractTests(unittest.TestCase):
    """验证资源身份、原生所有权、门禁、能力和关闭边界同源。"""

    # Context 只能组合一个直接复用 platform 表的资源 Component。
    def test_context_owns_one_platform_backed_resource_owner(self) -> None:
        context = CONTEXT.read_text(encoding="utf-8")
        methods = METHODS.read_text(encoding="utf-8")
        device = DEVICE.read_text(encoding="utf-8")

        self.assertEqual(
            context.count("rhi_device: rhi_device::D3d12RhiDevice"), 1
        )
        self.assertEqual(
            methods.count("rhi_device: rhi_device::D3d12RhiDevice::new()"), 1
        )
        self.assertIn("RhiBufferResourceTable<D3d12RhiBuffer>", device)
        self.assertIn("RhiTextureResourceTable<D3d12RhiTexture>", device)
        self.assertIn(
            "RhiResourceTable<SamplerHandle, D3d12RhiSampler>", device
        )
        self.assertNotIn("HashMap", device)
        self.assertNotIn("Vec<Option", device)
        self.assertEqual(
            RESOURCE_TABLE.read_text(encoding="utf-8").count(
                "pub(crate) struct RhiResourceTable"
            ),
            1,
        )

    # 每类句柄都必须由共享表登记真实原生对象与冻结描述后签发。
    def test_native_resources_and_frozen_descriptions_back_every_handle(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("struct D3d12RhiBuffer", device)
        self.assertIn("native: ID3D12Resource", device)
        self.assertIn("impl RhiBufferResource for D3d12RhiBuffer", device)
        self.assertIn("struct D3d12RhiTexture", device)
        self.assertIn("impl RhiTextureResource for D3d12RhiTexture", device)
        self.assertIn("state: D3D12_RESOURCE_STATES", device)
        self.assertIn("struct D3d12RhiSampler", device)
        self.assertIn("heap: ID3D12DescriptorHeap", device)
        self.assertIn("cpu: D3D12_CPU_DESCRIPTOR_HANDLE", device)
        self.assertGreaterEqual(device.count("desc: BufferDesc"), 1)
        self.assertGreaterEqual(device.count("desc: TextureDesc"), 1)
        self.assertGreaterEqual(device.count("desc: SamplerDesc"), 1)
        for insertion in (
            "self.buffers.insert(D3d12RhiBuffer",
            "self.textures.insert(D3d12RhiTexture",
            "self.samplers.insert(D3d12RhiSampler",
        ):
            self.assertIn(insertion, device)
        for checked_destroy in (
            "self.buffers.take(buffer)?",
            "self.textures.take(texture)?",
            "self.samplers.take(sampler)?",
        ):
            self.assertIn(checked_destroy, device)
        code = without_line_comments(device)
        self.assertNotIn("from_resource_raw", code)
        self.assertNotIn("BufferHandle::", code)
        self.assertNotIn("TextureHandle::", code)
        self.assertNotIn("SamplerHandle::", code)

    # 共享描述、身份、范围、用途和采样语义必须先于所有原生副作用。
    def test_shared_gates_precede_com_map_and_copy_side_effects(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        owner = device[device.index("impl D3d12RhiDevice {") :]
        create_buffer = function_range(
            owner,
            "fn create_buffer(&mut self, device:",
            "fn update_buffer(&self, upload:",
        )
        update_buffer = function_range(
            owner,
            "fn update_buffer(&self, upload:",
            "fn preflight_buffer_upload(&self, upload:",
        )
        create_texture = function_range(
            owner,
            "fn create_texture(\n",
            "fn stage_texture_upload<'a>(",
        )
        upload_texture = function_range(
            owner,
            "fn stage_texture_upload<'a>(",
            "fn commit_texture_upload(&mut self, texture:",
        )
        create_sampler = function_range(
            owner,
            "fn create_sampler(\n",
            "fn destroy_buffer(&mut self, buffer:",
        )
        trait_upload = function_range(
            device[device.index("impl GraphicsDevice for D3d12Context {") :],
            "fn update_texture(&mut self, upload:",
            "fn create_sampler(&mut self, desc:",
        )

        self.assertLess(create_buffer.index("desc.validate()?"), create_buffer.index("create_committed_resource("))
        self.assertLess(update_buffer.index("self.buffers.get("), update_buffer.index("upload.validate("))
        self.assertLess(update_buffer.index("upload.validate("), update_buffer.index(".Map("))
        self.assertLess(create_texture.index("desc.validate()?"), create_texture.index("create_committed_resource("))
        self.assertLess(upload_texture.index("self.textures.get("), upload_texture.index("upload.validate("))
        self.assertLess(upload_texture.index("upload.validate("), upload_texture.index("GetCopyableFootprints("))
        self.assertLess(upload_texture.index("upload.validate("), upload_texture.index("create_upload_buffer("))
        self.assertLess(upload_texture.index("upload.validate("), upload_texture.index("staging.Map("))
        self.assertLess(create_sampler.index("match (desc.filter(), desc.mip_mode())"), create_sampler.index("CreateDescriptorHeap("))
        self.assertLess(create_sampler.index("match desc.address_mode()"), create_sampler.index("CreateSampler("))
        self.assertLess(trait_upload.index("stage_texture_upload("), trait_upload.index("begin_rhi_transfer_commands()?"))
        self.assertLess(trait_upload.index("stage_texture_upload("), trait_upload.index("CopyTextureRegion("))

    # 相关预检必须只调用共享表，能力快照不得把这些预检冒充执行能力。
    def test_preflight_and_capability_snapshot_are_honest(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("self.buffers.validate_upload(upload)", device)
        self.assertIn("self.textures.validate_copy(copy)", device)
        self.assertIn("self.textures.validate_move(movement)", device)
        self.assertIn("dynamic_buffers: true", device)
        self.assertIn("texture_upload: true", device)
        for missing in (
            "texture_copy: false",
            "texture_region_move: false",
            "clear_rect: false",
            "sampled_textures: false",
            "render_to_texture: false",
            "scissor: false",
            "premultiplied_alpha_blend: false",
            "additive_blend: false",
        ):
            self.assertIn(missing, device)
        for operation in (
            "preflight_draw_resources",
            "create_pipeline",
            "destroy_pipeline",
            "begin_render_pass",
            "clear_rect",
            "draw",
            "copy_texture",
            "move_texture_region",
            "end_render_pass",
            "submit",
        ):
            start = device.index(f"fn {operation}(", device.index("impl GraphicsDevice"))
            body = device[start : device.index("\n    }", start)]
            self.assertIn("self.ensure_healthy()?", body)
            self.assertIn("resource_stage_deferred", body)
        self.assertNotIn("RhiPipelineResourceTable", device)
        self.assertNotIn("RhiPassState", device)
        self.assertNotIn("RhiSubmissionSequence", device)

    # GPU 完成必须先于资源 owner 排空，失败 Drop 还必须保留在途原生引用。
    def test_checked_shutdown_drains_or_retains_the_resource_owner(self) -> None:
        methods = METHODS.read_text(encoding="utf-8")
        device = DEVICE.read_text(encoding="utf-8")
        shutdown = methods[methods.index("pub(crate) fn shutdown_result(") :]
        retain = function_range(
            methods,
            "pub(super) fn retain_gpu_objects_after_undrained_drop(",
            "pub(crate) fn shutdown_result(",
        )

        self.assertLess(shutdown.index("self.drain_for_shutdown()"), shutdown.index("self.rhi_device.shutdown()"))
        self.assertLess(shutdown.index("self.rhi_device.shutdown()"), shutdown.index("self.pending_gpu_resources.clear()"))
        self.assertLess(shutdown.index("self.rhi_device.shutdown()"), shutdown.index("self.back_buffers.clear()"))
        self.assertLess(shutdown.index("self.rhi_device.shutdown()"), shutdown.index("close_owned_fence_event_with("))
        self.assertLess(shutdown.index("close_owned_fence_event_with("), shutdown.index("self.shutdown = true"))
        self.assertLess(retain.index("self.rhi_device.retain_after_undrained_drop()"), retain.index("forget(self.device.clone())"))
        self.assertGreaterEqual(device.count("drain_reverse()"), 6)
        self.assertIn("std::mem::forget(sampler.heap)", device)
        self.assertIn("std::mem::forget(texture.native)", device)
        self.assertIn("std::mem::forget(buffer.native)", device)

    # 本阶段只能形成未激活的 Device 类型形状，不得签发 submission 或进入 registry。
    def test_submission_registry_and_recipe_entry_remain_inactive(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        graphics = GRAPHICS.read_text(encoding="utf-8")
        registry = REGISTRY.read_text(encoding="utf-8")
        d3d12_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "src/native/presentation/graphics/d3d12").rglob("*.rs")
        )

        self.assertIn("impl GraphicsDevice for D3d12Context", device)
        self.assertIn('"D3D12 thin RHI is not implemented"', graphics)
        self.assertIn("Errc::NotImplemented", graphics)
        self.assertNotIn("GraphicsApi::D3d12", registry)
        self.assertNotIn(".issue()", without_line_comments(d3d12_sources))
        self.assertNotIn("Ok(SubmissionHandle", without_line_comments(device))

    # 所有阶段触及文件继续满足单文件上限。
    def test_touched_files_stay_below_limit(self) -> None:
        for path in (CONTEXT, METHODS, DEVICE, GRAPHICS, Path(__file__)):
            self.assertLess(
                len(path.read_text(encoding="utf-8").splitlines()),
                1500,
                path,
            )


if __name__ == "__main__":
    unittest.main()
