# -*- coding: utf-8 -*-
"""锁定 GFX-NEXT-15..19 的 D3D12 资源、draw、提交与未激活边界。"""

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONTEXT = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/mod.rs"
METHODS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/methods.rs"
DEVICE = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_device.rs"
PASS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/rhi_device_pass.rs"
PIPELINE = ROOT / "src/native/presentation/graphics/d3d12/adapter/pipeline/mod.rs"
GRAPHICS = ROOT / "src/native/presentation/graphics/d3d12/adapter/context/graphics.rs"
REGISTRY = ROOT / "src/native/factory/registry_windows.rs"
RESOURCE_TABLE = ROOT / "src/platform/presentation/rhi/resource_table.rs"
TRANSFER = ROOT / "src/platform/presentation/rhi/transfer.rs"


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
        self.assertIn(
            "RhiPipelineResourceTable<D3d12PipelineResource>", device
        )
        self.assertIn("pass: RhiPassState", device)
        self.assertIn("submission_sequence: RhiSubmissionSequence", device)
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
        self.assertIn("shadow: Vec<u8>", device)
        self.assertIn("impl RhiBufferResource for D3d12RhiBuffer", device)
        self.assertIn("struct D3d12RhiTexture", device)
        self.assertIn("impl RhiTextureResource for D3d12RhiTexture", device)
        self.assertIn("state: D3D12_RESOURCE_STATES", device)
        self.assertIn("rtv_heap: Option<ID3D12DescriptorHeap>", device)
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
            "fn update_buffer(&mut self, device:",
        )
        update_buffer = function_range(
            owner,
            "fn update_buffer(&mut self, device:",
            "fn preflight_buffer_upload(&self, upload:",
        )
        zero_shadow = function_range(
            device,
            "fn zeroed_buffer_shadow(",
            "fn clone_buffer_shadow(",
        )
        clone_shadow = function_range(
            device,
            "fn clone_buffer_shadow(",
            "fn create_buffer_upload_version(",
        )
        upload_version = function_range(
            device,
            "fn create_buffer_upload_version(",
            "fn create_upload_buffer(",
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

        self.assertLess(create_buffer.index("desc.validate()?"), create_buffer.index("zeroed_buffer_shadow("))
        self.assertLess(create_buffer.index("zeroed_buffer_shadow("), create_buffer.index("create_buffer_upload_version("))
        self.assertLess(create_buffer.index("create_buffer_upload_version("), create_buffer.index("self.buffers.insert("))
        self.assertLess(update_buffer.index("self.buffers.get("), update_buffer.index("upload.validate("))
        self.assertLess(update_buffer.index("upload.validate("), update_buffer.index("clone_buffer_shadow("))
        self.assertLess(update_buffer.index("clone_buffer_shadow("), update_buffer.index("copy_from_slice(data)"))
        self.assertLess(update_buffer.index("copy_from_slice(data)"), update_buffer.index("create_buffer_upload_version("))
        self.assertLess(update_buffer.index("create_buffer_upload_version("), update_buffer.index("std::mem::replace("))
        self.assertNotIn(".native =", update_buffer)
        self.assertNotIn(".shadow =", update_buffer)
        self.assertLess(zero_shadow.index("try_reserve_exact(size)"), zero_shadow.index("shadow.resize(size, 0)"))
        self.assertLess(clone_shadow.index("try_reserve_exact(current.len())"), clone_shadow.index("extend_from_slice(current)"))
        self.assertLess(upload_version.index("shadow.len() != desc.size_bytes()"), upload_version.index("create_committed_resource("))
        self.assertLess(upload_version.index("create_committed_resource("), upload_version.index("native.Map("))
        self.assertLess(upload_version.index("shadow.as_ptr()"), upload_version.rindex("native.Unmap("))
        self.assertIn("mapped.cast::<u8>(), shadow.len()", upload_version)
        self.assertIn("End: shadow.len()", upload_version)
        self.assertLess(create_texture.index("desc.validate()?"), create_texture.index("create_committed_resource("))
        self.assertLess(upload_texture.index("self.textures.get("), upload_texture.index("upload.validate("))
        self.assertLess(upload_texture.index("upload.validate("), upload_texture.index("GetCopyableFootprints("))
        self.assertLess(upload_texture.index("upload.validate("), upload_texture.index("create_upload_buffer("))
        self.assertLess(upload_texture.index("upload.validate("), upload_texture.index("staging.Map("))
        self.assertLess(create_sampler.index("match (desc.filter(), desc.mip_mode())"), create_sampler.index("CreateDescriptorHeap("))
        self.assertLess(create_sampler.index("match desc.address_mode()"), create_sampler.index("CreateSampler("))
        self.assertLess(trait_upload.index("stage_texture_upload("), trait_upload.index("begin_rhi_transfer_commands()?"))
        self.assertLess(trait_upload.index("stage_texture_upload("), trait_upload.index("CopyTextureRegion("))

    # draw 闭环后只开启已有真实实现支撑的能力。
    def test_preflight_and_capability_snapshot_are_honest(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")

        self.assertIn("self.buffers.validate_upload(upload)", device)
        self.assertIn("self.textures.validate_copy(copy)", device)
        self.assertIn("self.validate_texture_move(movement).map(|_| ())", device)
        self.assertIn("dynamic_buffers: true", device)
        self.assertIn("texture_upload: true", device)
        self.assertIn("texture_copy: true", device)
        self.assertIn("texture_region_move: true", device)
        self.assertIn("clear_rect: true", device)
        self.assertIn("render_to_texture: true", device)
        self.assertIn("scissor: true", device)
        for enabled in (
            "sampled_textures: true",
            "premultiplied_alpha_blend: true",
            "additive_blend: true",
        ):
            self.assertIn(enabled, device)
        for operation, delegation in (
            ("preflight_draw_resources", "self.rhi_device.preflight_draw_resources(packet)"),
            ("draw", "self.draw_rhi_packet(packet)"),
        ):
            start = device.index(f"fn {operation}(", device.index("impl GraphicsDevice"))
            body = device[start : device.index("\n    }", start)]
            self.assertIn("self.ensure_healthy()?", body)
            self.assertIn(delegation, body)
            self.assertNotIn("resource_stage_deferred", body)
        for operation, delegation in (
            ("create_pipeline", "self.rhi_device.create_pipeline(&self.device, desc)"),
            ("destroy_pipeline", "self.rhi_device.destroy_pipeline(pipeline)"),
        ):
            start = device.index(f"fn {operation}(", device.index("impl GraphicsDevice"))
            body = device[start : device.index("\n    }", start)]
            self.assertIn("self.ensure_healthy()?", body)
            self.assertIn(delegation, body)
            self.assertIn("self.observe_rhi_result", body)
            self.assertNotIn("resource_stage_deferred", body)
        self.assertIn("RhiPipelineResourceTable", device)
        self.assertIn("RhiPassState", device)
        self.assertIn("RhiSubmissionSequence", device)

    # 共享 copy 验证必须先于 native 命令，GPU 成功必须先于两端状态提交。
    def test_texture_copy_validation_recording_and_commit_order(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        owner = device[device.index("impl D3d12RhiDevice {") :]
        stage = function_range(
            owner,
            "fn stage_texture_copy(&self, copy:",
            "fn validate_texture_move(&self, movement:",
        )
        commit = function_range(
            owner,
            "fn commit_texture_copy(",
            "fn resolve_render_target(&self, texture:",
        )
        trait_device = device[device.index("impl GraphicsDevice for D3d12Context {") :]
        execute = function_range(
            trait_device,
            "fn copy_texture(&mut self, copy:",
            "fn move_texture_region(&mut self, movement:",
        )
        native = function_range(
            device,
            "fn record_texture_copy_commands(",
            "impl D3d12Context {",
        )

        self.assertLess(stage.index("self.textures.get(copy.source())?"), stage.index("copy.validate_transfer("))
        self.assertLess(stage.index("self.textures.get(copy.destination())?"), stage.index("copy.validate_transfer("))
        self.assertLess(stage.index("copy.validate_transfer("), stage.index("source.native.clone()"))
        self.assertLess(stage.index("copy.validate_transfer("), stage.index("destination.native.clone()"))
        self.assertLess(execute.index("stage_texture_copy(copy)"), execute.index("begin_rhi_transfer_commands()?"))
        self.assertLess(execute.index("stage_texture_copy(copy)"), execute.index("record_texture_copy_commands("))
        self.assertNotIn("resource_stage_deferred", execute)

        copy_call = native.index("CopyTextureRegion(")
        self.assertLess(native.index("D3D12_RESOURCE_STATE_COPY_SOURCE"), copy_call)
        self.assertLess(native.index("D3D12_RESOURCE_STATE_COPY_DEST"), copy_call)
        self.assertIn("Some(&source_box)", native)
        self.assertGreater(
            native.index("D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE", copy_call),
            copy_call,
        )

        self.assertLess(execute.index("pending_gpu_resources"), execute.index("execute_recording_and_wait()"))
        self.assertLess(execute.index("execute_recording_and_wait()"), execute.index("pending_gpu_resources.truncate("))
        self.assertLess(execute.index("execute_recording_and_wait()"), execute.index("commit_texture_copy("))
        self.assertLess(commit.index("self.textures.get(source)?"), commit.index("self.textures.get_mut(source)?"))
        self.assertLess(commit.index("self.textures.get(destination)?"), commit.index("self.textures.get_mut(destination)?"))

    # move 必须先共享验证，跨纹理复用 copy，同纹理严格按 scratch 两段执行并检查清理。
    def test_texture_move_reuses_copy_and_checks_scratch_cleanup(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        owner = device[device.index("impl D3d12RhiDevice {") :]
        validation = function_range(
            owner,
            "fn validate_texture_move(&self, movement:",
            "fn commit_texture_copy(",
        )
        trait_device = device[device.index("impl GraphicsDevice for D3d12Context {") :]
        movement = function_range(
            trait_device,
            "fn move_texture_region(&mut self, movement:",
            "fn end_render_pass(&mut self)",
        )

        self.assertLess(validation.index("self.textures.get(movement.source())?"), validation.index("movement.validate_transfer("))
        self.assertLess(validation.index("self.textures.get(movement.destination())?"), validation.index("movement.validate_transfer("))
        self.assertLess(movement.index("validate_texture_move(movement)"), movement.index("create_texture(&self.device"))
        self.assertIn("return self.copy_texture(movement.into_copy())", movement)
        self.assertLess(movement.index("create_texture(&self.device"), movement.index("movement.through_scratch(scratch)"))
        self.assertLess(movement.index("movement.through_scratch(scratch)"), movement.index("copy_texture(to_scratch)"))
        self.assertLess(movement.index("copy_texture(to_scratch)"), movement.index("copy_texture(from_scratch)"))
        self.assertLess(movement.index("copy_texture(from_scratch)"), movement.index("self.rhi_device.destroy_texture(scratch)"))
        self.assertIn("match (operation, cleanup)", movement)
        self.assertIn("Err(primary_error.with_source(cleanup_error))", movement)
        self.assertIn("(Err(primary_error), Ok(()))", movement)
        self.assertIn("(Ok(()), Err(cleanup_error))", movement)
        self.assertIn("(Ok(()), Ok(()))", movement)
        self.assertNotIn("resource_stage_deferred", movement)

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
        self.assertGreaterEqual(device.count("drain_reverse()"), 8)
        self.assertIn("pipeline.retain_after_undrained_drop()", device)
        self.assertIn("std::mem::forget(sampler.heap)", device)
        self.assertIn("std::mem::forget(texture.native)", device)
        self.assertIn("std::mem::forget(buffer.native)", device)

    # 真实提交只能由共享序列签发，但组合入口与 registry 继续保持未激活。
    def test_submission_registry_and_recipe_entry_remain_inactive(self) -> None:
        device = DEVICE.read_text(encoding="utf-8")
        pass_source = PASS.read_text(encoding="utf-8")
        graphics = GRAPHICS.read_text(encoding="utf-8")
        registry = REGISTRY.read_text(encoding="utf-8")
        transfer = TRANSFER.read_text(encoding="utf-8")
        d3d12_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for path in (ROOT / "src/native/presentation/graphics/d3d12").rglob("*.rs")
        )
        upper_sources = "\n".join(
            path.read_text(encoding="utf-8")
            for directory in (ROOT / "src/app", ROOT / "src/ui", ROOT / "src/draw")
            for path in directory.rglob("*.rs")
        )

        self.assertIn("impl GraphicsDevice for D3d12Context", device)
        self.assertIn('"D3D12 thin RHI is not implemented"', graphics)
        self.assertIn("Errc::NotImplemented", graphics)
        self.assertNotIn("GraphicsApi::D3d12", registry)
        self.assertEqual(without_line_comments(d3d12_sources).count(".issue()"), 1)
        self.assertLess(
            pass_source.index("self.execute_recording_and_wait()?"),
            pass_source.index("submission_sequence.issue()"),
        )
        self.assertNotIn("SubmissionHandle::from_raw", without_line_comments(d3d12_sources))
        for upper_token in ("D3d12", "Direct3D12", "GraphicsApi::D3d12"):
            self.assertNotIn(upper_token, upper_sources)
        for shared_contract in (
            "pub(crate) struct TextureCopy",
            "pub(crate) struct TextureMove",
            "pub(crate) struct RhiTextureTransferBounds",
        ):
            self.assertEqual(transfer.count(shared_contract), 1)
            self.assertNotIn(shared_contract, device)

    # 所有阶段触及文件继续满足单文件上限。
    def test_touched_files_stay_below_limit(self) -> None:
        for path in (
            CONTEXT,
            METHODS,
            DEVICE,
            PASS,
            PIPELINE,
            GRAPHICS,
            TRANSFER,
            Path(__file__),
        ):
            self.assertLess(
                len(path.read_text(encoding="utf-8").splitlines()),
                1500,
                path,
            )


if __name__ == "__main__":
    unittest.main()
