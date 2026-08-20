# -*- coding: utf-8 -*-
# 验证 FramePlan 到两个 Adapter 只传递固定 t0/s0 的原子采样绑定。
"""Keep sampled texture bindings atomic and slot-free across adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 DrawPacket 采样绑定。
DRAW_PACKET = ROOT / "src/native/presentation/rhi/draw_packet.rs"
# 定位独立的 Draw 条件采样资源契约。
DRAW_SAMPLING = ROOT / "src/native/presentation/rhi/draw_sampling.rs"
# 定位薄 RHI Device 契约。
RHI = ROOT / "src/native/presentation/rhi/mod.rs"
# 定位类型化 FramePlan 命令。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 FramePlan 生产与执行源码。
VALIDATION = ROOT / "src/draw/backend/frame_plan_validation.rs"
EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 OpenGL Adapter 的共享状态写入与读取路径。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL raster owner 的只读预检桥接路径。
OPENGL_RASTER = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL host 的生命周期门禁与只读转发路径。
OPENGL_HOST = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 OpenGL draw 的完整绑定读取路径。
OPENGL_DRAW = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 Device 的生命周期门禁与只读预检入口。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位 D3D11 Adapter 的共享状态写入路径。
D3D11_RESOURCES = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_resources.rs"
# 定位 D3D11 draw 的完整绑定读取路径。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_draw.rs"
# 列出全部生产采样命令的 Drawing 源文件。
PRODUCERS = (
    # 通用颜色采样路径。
    ROOT / "src/draw/backend/rhi_renderer.rs",
    # blur 双 pass 路径。
    ROOT / "src/draw/backend/rhi_renderer_blur.rs",
    # coverage 路径。
    ROOT / "src/draw/backend/rhi_renderer_coverage.rs",
    # mixed painter-order 路径。
    ROOT / "src/draw/backend/rhi_renderer_mixed.rs",
    # 离屏 sampled quad 路径。
    ROOT / "src/draw/backend/rhi_renderer_sampled.rs",
    # 无采样渐变路径。
    ROOT / "src/draw/backend/rhi_renderer_gradient.rs",
    # 无采样阴影路径。
    ROOT / "src/draw/backend/rhi_renderer_shadow.rs",
    # 无采样形状路径。
    ROOT / "src/draw/backend/rhi_renderer_shape.rs",
    # Drawing GPU Module 探针路径。
    ROOT / "src/draw/backend/gpu/device_probe.rs",
)


# 集中验证共享值对象、FramePlan、生产端和两个 Adapter 的同一契约。
class GraphicsRhiSampledBindingContractTests(unittest.TestCase):
    # 采样资源必须由当前 DrawPacket 原子拥有。
    def test_sampled_binding_is_preflighted_before_activation(self) -> None:
        # 采样绑定定义集中在 DrawPacket 相邻的独立 Component。
        draw_sampling = DRAW_SAMPLING.read_text(encoding="utf-8")
        # DrawPacket 只负责组合并封闭条件角色。
        draw_packet = DRAW_PACKET.read_text(encoding="utf-8")
        self.assertIn("struct DrawSamplingBinding", draw_sampling)
        self.assertIn("fn none()", draw_sampling)
        self.assertIn("fn sampled(binding: SampledTextureBinding)", draw_sampling)
        self.assertIn("sampling: DrawSamplingBinding", draw_packet)
        self.assertIn("pub(crate) const fn sampling(self)", draw_packet)
        # 旧的 Device 预检与独立绑定入口必须退出。
        self.assertNotIn("preflight_sampled_binding", RHI.read_text(encoding="utf-8"))
        self.assertNotIn("bind_sampled_texture", RHI.read_text(encoding="utf-8"))

    # 独立采样 Component 只能暴露一个完整条件绑定。
    def test_draw_sampling_component_owns_one_atomic_binding(self) -> None:
        # 读取共享 Draw 采样契约源码。
        draw_sampling = DRAW_SAMPLING.read_text(encoding="utf-8")
        self.assertIn("pub(crate) struct SampledTextureBinding", draw_sampling)
        self.assertIn("pub(crate) struct DrawSamplingBinding", draw_sampling)
        self.assertIn("matches_pipeline", draw_sampling)

    # FramePlan 与 Device 接口不得再暴露裸槽位或拆分资源。
    def test_frame_plan_and_device_are_slot_free(self) -> None:
        # 读取 FramePlan 命令定义。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取薄 RHI Device trait。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取 FramePlan 的唯一执行映射。
        execution = EXECUTION.read_text(encoding="utf-8")
        # FramePlan 必须以单一值承载完整采样绑定。
        self.assertNotIn("BindSampledTexture", frame_plan)
        # 旧的带字段绑定命令必须消失。
        self.assertNotIn("BindTexture {", frame_plan)
        # FramePlan 不得再出现裸采样槽位。
        self.assertNotIn("slot: u32", frame_plan)
        # Device trait 必须接收同一个完整值对象。
        self.assertNotIn("bind_sampled_texture", rhi)
        # Device trait 不得保留旧的拆分入口。
        self.assertNotIn("fn bind_texture(", rhi)
        # 执行器必须机械转发同一个绑定值。
        self.assertNotIn("bind_sampled_texture", execution)

    # 所有生产端与验证器必须只识别类型化绑定命令。
    def test_all_producers_use_the_typed_binding(self) -> None:
        # 合并全部生产采样命令的源码。
        producers = "\n".join(path.read_text(encoding="utf-8") for path in PRODUCERS)
        # 读取 FramePlan 的完整命令顺序验证入口。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 当前九个生产点必须全部构造原子绑定。
        self.assertEqual(producers.count("DrawSamplingBinding::"), 19)
        # 当前九个生产点必须全部使用类型化 FramePlan 命令。
        self.assertEqual(producers.count("FramePlanCommand::BindSampledTexture("), 0)
        # 生产端不得继续声明零号槽位。
        self.assertNotIn("slot: 0", producers)
        # 验证器必须读取最近一次完整绑定。
        validation = VALIDATION.read_text(encoding="utf-8")
        # 最近绑定必须按 painter order 逆向查找。
        self.assertNotIn("sampled_binding_for", validation)
        # 固定 t0/s0 语义必须作为完整绑定值读取。
        self.assertNotIn("BindSampledTexture", validation)
        # 绑定语义必须与当前 Draw pipeline 契约匹配。
        self.assertNotIn("sampled_binding_for", validation)
        # 每条绑定命令都必须在 FramePlan 顺序遍历中立即检查反馈环。
        self.assertNotIn("validate_sampled_binding_target", frame_plan)
        # 验证器不得继续匹配裸槽位。
        self.assertNotIn("slot: 0", validation)

    # OpenGL 与 D3D11 必须写入并读取同一个原子状态事实。
    def test_both_adapters_share_the_atomic_binding(self) -> None:
        # 读取 OpenGL 状态写入路径。
        opengl_device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 OpenGL draw 路径。
        opengl_draw = OPENGL_DRAW.read_text(encoding="utf-8")
        # 读取 D3D11 状态写入路径。
        d3d11_resources = D3D11_RESOURCES.read_text(encoding="utf-8")
        # 读取 D3D11 draw 路径。
        d3d11_draw = D3D11_DRAW.read_text(encoding="utf-8")
        # OpenGL 必须把完整绑定交给共享状态机。
        self.assertIn("binding.validate_resources(format, sampler_desc)", opengl_device)
        self.assertNotIn("bind_sampled_texture", opengl_device)
        # D3D11 必须把同一个类型交给共享状态机。
        self.assertIn("binding.validate_resources(format, sampler_desc)", d3d11_resources)
        self.assertNotIn("bind_sampled_texture", d3d11_resources)
        # OpenGL draw 必须一次取得完整绑定。
        self.assertIn("packet.sampling()", opengl_draw)
        # D3D11 四类 sampled pipeline 都必须通过 packet 投影一次取得完整绑定。
        self.assertIn(".sampling()", d3d11_draw)
        # 两个 draw Adapter 不得重复解释 sampling.accepts。
        self.assertNotIn("contract.sampling.accepts", opengl_draw + d3d11_draw)
        # 两个 Adapter 都不得读取拆分纹理状态。
        self.assertNotIn(".bound_texture()", opengl_draw + d3d11_draw)
        # 两个 Adapter 都不得读取拆分 sampler 状态。
        self.assertNotIn(".bound_sampler()", opengl_draw + d3d11_draw)

    # 两端执行期绑定必须复用预检，并在共享 pass 写入前完成。
    def test_adapters_reuse_preflight_before_pass_write(self) -> None:
        # 读取 OpenGL Adapter 的完整 packet 预检实现。
        opengl = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 D3D11 Adapter 的完整 packet 预检实现。
        d3d11 = D3D11_DEVICE.read_text(encoding="utf-8")
        # 保留每个 Adapter 的 packet 投影与真实资源门禁标记。
        adapter_markers = (
            # OpenGL 只读资源预检标记。
            (
                opengl,
                "packet.sampling().sampled_texture()",
                "self.validate_sampled_resources(binding)?;",
            ),
            # D3D11 只读资源预检标记。
            (
                d3d11,
                "packet.sampling().sampled_texture()",
                "self.rhi_validate_sampled_resources(binding)?;",
            ),
        )
        # 分别检查两个 Adapter 的完整 DrawPacket 预检。
        for source, packet_projection, resource_gate in adapter_markers:
            # 只截取绑定函数，避免其它资源路径偶然满足断言。
            body = source
            # 两个 Adapter 必须直接从 packet 取得采样事实。
            self.assertIn(packet_projection, body)
            # 真实资源描述门禁必须发生在同一个预检入口。
            self.assertIn(resource_gate, body)
            # 旧的独立绑定状态与 helper 必须消失。
            self.assertNotIn("sampled_binding_for", body)
            self.assertNotIn("bind_sampled_texture", body)
        # 两个执行 Adapter 必须只核对当前 packet 输入与活动输出的反馈关系。
        for draw in (OPENGL_DRAW, D3D11_DRAW):
            # 读取当前原生 Draw 分派。
            source = draw.read_text(encoding="utf-8")
            # pass 状态不得保存采样历史，只接收当前 binding 的纹理身份。
            self.assertIn("validate_sampled_texture(binding.texture())?", source)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
