# -*- coding: utf-8 -*-
# 说明本文件锁定 Renderer 帧对 FramePlan 的唯一所有权与跨后端消费契约。
"""Static checks for the RhiRendererFrame FramePlan ownership boundary."""

# 引入标准单元测试框架。
import unittest
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Renderer 帧执行组件。
EXECUTION = ROOT / "src/draw/backend/rhi_renderer_execution.rs"
# 定位 OpenGL 统一 RHI host。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 D3D11 统一 RHI device。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs"
# 定位核心 producer 源码集合。
PRODUCERS = (
    ROOT / "src/draw/backend/rhi_renderer.rs",
    ROOT / "src/draw/backend/rhi_renderer_execution.rs",
    ROOT / "src/draw/backend/rhi_renderer_coverage.rs",
    ROOT / "src/draw/backend/rhi_renderer_gradient.rs",
    ROOT / "src/draw/backend/rhi_renderer_mixed.rs",
    ROOT / "src/draw/backend/rhi_renderer_sampled.rs",
    ROOT / "src/draw/backend/rhi_renderer_shape.rs",
    ROOT / "src/draw/backend/rhi_renderer_shadow.rs",
)


# 验证 Renderer System 通过唯一帧 Module 组织计划与 Adapter。
class RendererFramePlanOwnerContractTests(unittest.TestCase):
    # RhiRendererFrame 必须直接拥有唯一 FramePlan。
    def test_frame_owns_plan_and_construction_initializes_scope(self) -> None:
        # 读取执行组件源码。
        source = EXECUTION.read_text(encoding="utf-8")
        # 结构字段必须明确声明计划所有权。
        self.assertRegex(source, r"struct\s+RhiRendererFrame[\s\S]{0,1800}\bplan:\s*FramePlan")
        # Surface 构造必须建立带 token 与 damage 的计划。
        self.assertIn("FramePlan::new(context.surface_ref().token(), damage)", source)
        # Offscreen 构造必须建立 Device-only 计划。
        self.assertIn("plan: FramePlan::offscreen()", source)

    # 帧只暴露当前计划的可变窄入口，不泄露独立计划副本。
    def test_plan_mut_is_the_only_plan_accessor(self) -> None:
        # 读取执行组件源码。
        source = EXECUTION.read_text(encoding="utf-8")
        # 必须存在返回内部可变计划的入口。
        self.assertRegex(source, r"fn\s+plan_mut\s*\([^)]*\)\s*->\s*&mut\s+FramePlan")
        # 不得存在返回独立或只读计划的同名 accessor。
        self.assertNotRegex(source, r"fn\s+plan\s*\(")

    # execute 必须从 self.plan 借用，且签名不得接收外部计划。
    def test_execute_consumes_self_plan(self) -> None:
        # 读取执行组件源码并截取 execute 实现。
        source = EXECUTION.read_text(encoding="utf-8")
        start = source.index("pub(crate) fn execute")
        region = source[start : start + 4200]
        # execute 只能使用无参数签名。
        self.assertRegex(region, r"pub\(crate\) fn execute\s*\(\s*&mut self\s*\)")
        # 实现必须从当前帧借用唯一计划。
        self.assertIn("let plan = &self.plan;", region)
        # 禁止保留旧的外部计划参数或调用形态。
        self.assertNotRegex(region, r"execute\s*\([^)]*FramePlan")

    # 核心 producer 不得通过旧 accessor 或外部计划参数绕过帧所有权。
    def test_core_producers_do_not_use_old_plan_execution_shape(self) -> None:
        # 逐个读取当前 Renderer producer。
        for path in PRODUCERS:
            source = path.read_text(encoding="utf-8")
            # 文件名必须出现在失败消息中，便于定位回归。
            self.assertNotIn(".plan()", source, msg=f"旧 plan accessor remains in {path}")
            # 禁止旧的 execute(&plan) 调用形态。
            self.assertNotRegex(source, r"\.execute\s*\(\s*&\s*plan\s*\)", msg=f"旧 execute(&plan) remains in {path}")

    # 两类 Adapter 都必须只实现统一 DrawPacket 消费入口。
    def test_adapters_consume_draw_packet_without_frame_dependency(self) -> None:
        # 读取两类 Adapter 源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 两个 Adapter 都必须实现统一 draw 签名。
        for name, source in (("OpenGL", opengl), ("D3D11", d3d11)):
            # draw 参数必须是共享 DrawPacket。
            self.assertRegex(source, r"fn\s+draw\s*\(\s*&mut self,\s*packet:\s*DrawPacket\s*\)", msg=f"{name} lacks unified DrawPacket draw")
            # Adapter 不得新增 FramePlan 或 RendererFrame 类型依赖。
            self.assertNotRegex(source, r"use [^;]*(?:FramePlan|RhiRendererFrame)", msg=f"{name} imports frame owner")
            self.assertNotRegex(source, r"(?:FramePlan|RhiRendererFrame)\s*[<(]", msg=f"{name} directly depends on frame owner")


# 允许直接运行本契约文件进行局部诊断。
if __name__ == "__main__":
    # 运行本文件声明的静态契约。
    unittest.main()
