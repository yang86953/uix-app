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
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位多目标离屏 blur producer。
BLUR = ROOT / "src/draw/backend/rhi_renderer_blur.rs"
# 定位使用 RhiRendererFrame 的全部核心 producer 源码集合。
PRODUCERS = (
    # 覆盖实体与纹理 quad producer。
    ROOT / "src/draw/backend/rhi_renderer.rs",
    # 覆盖 coverage producer。
    ROOT / "src/draw/backend/rhi_renderer_coverage.rs",
    # 覆盖 gradient producer。
    ROOT / "src/draw/backend/rhi_renderer_gradient.rs",
    # 覆盖混合 painter-order producer。
    ROOT / "src/draw/backend/rhi_renderer_mixed.rs",
    # 覆盖已有纹理采样 producer。
    ROOT / "src/draw/backend/rhi_renderer_sampled.rs",
    # 覆盖 shape producer。
    ROOT / "src/draw/backend/rhi_renderer_shape.rs",
    # 覆盖 shadow producer。
    ROOT / "src/draw/backend/rhi_renderer_shadow.rs",
    # 覆盖 source→scratch→target 的多目标离屏 producer。
    BLUR,
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

    # pass 只暂存命令，target 与独立 load 状态必须由 Frame owner 绑定。
    def test_frame_owns_pass_target_and_load_binding(self) -> None:
        # 读取执行组件源码。
        source = EXECUTION.read_text(encoding="utf-8")
        # pass 结构只能拥有命令序列，不得保存目标或 load。
        pass_start = source.index("struct RhiRendererPass")
        pass_end = source.index("impl RhiRendererPass", pass_start)
        pass_region = source[pass_start:pass_end]
        self.assertRegex(pass_region, r"commands\s*:\s*Vec<FramePlanCommand>")
        self.assertNotIn("RenderTargetRef", pass_region)
        self.assertNotIn("LoadAction", pass_region)
        # Frame 必须提供无目标的命令包构造器。
        self.assertRegex(source, r"fn\s+new_pass\s*\([^)]*\)\s*->\s*RhiRendererPass")
        # push_pass 必须把默认目标与 load 交给 owner 内部绑定。
        push_start = source.index("pub(crate) fn push_pass")
        push_region = source[push_start : push_start + 700]
        self.assertIn("let target = self.render_target();", push_region)
        self.assertIn("self.push_targeted_pass(target, load, pass)", push_region)
        # 真实 RenderPassPlan 只能由 Frame 的私有绑定实现构造和保存。
        targeted_start = source.index("fn push_targeted_pass")
        targeted_region = source[targeted_start : targeted_start + 900]
        self.assertIn("RenderPassPlan::new(target, load)", targeted_region)
        self.assertIn("self.plan.push_pass", targeted_region)
        # 旧计划 accessor 必须彻底删除。
        self.assertNotIn("plan_mut", source)
        # render_target 只能是 Frame 内部绑定实现，不能成为 producer 入口。
        self.assertNotRegex(source, r"pub\(crate\)\s+const\s+fn\s+render_target")

    # 多目标纹理入口必须保持 Offscreen 角色门禁并由 Frame 绑定目标。
    def test_offscreen_multi_target_binding_stays_inside_frame(self) -> None:
        # 读取 Frame owner 与 blur producer。
        execution = EXECUTION.read_text(encoding="utf-8")
        blur = BLUR.read_text(encoding="utf-8")
        # 截取多目标入口，防止其它函数中的相似文本偶然满足断言。
        start = execution.index("pub(crate) fn push_offscreen_pass")
        region = execution[start : start + 1400]
        # Surface/Offscreen 角色判断必须先于任何计划追加。
        gate = region.index("if !matches!(&self.role, RhiRendererFrameRole::Offscreen")
        append = region.index("self.push_targeted_pass")
        self.assertLess(gate, append)
        self.assertIn("Errc::InvalidArgument", region)
        self.assertIn("RenderTargetRef::Texture(target)", region)
        # blur 只能生成命令包并调用 Frame owner，不能拥有真实计划或执行入口。
        self.assertGreaterEqual(blur.count("frame.new_pass()"), 2)
        self.assertIn("frame.push_offscreen_pass(", blur)
        self.assertIn("frame.push_pass(LoadAction::Load, vertical)", blur)
        self.assertIn("frame.execute()", blur)
        self.assertNotIn("RenderPassPlan::new", blur)
        self.assertNotIn("FramePlan::offscreen", blur)
        self.assertNotIn("plan.push_pass", blur)
        self.assertNotIn("execute_plan_without_present", blur)

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

    # Device-only 计划执行入口只能由 RhiRendererFrame 门面调用。
    def test_no_present_execution_entry_is_frame_private(self) -> None:
        # 读取执行组件；一次调用加一次定义应是完整出现集合。
        execution = EXECUTION.read_text(encoding="utf-8")
        self.assertEqual(execution.count("execute_plan_without_present("), 2)
        # 所有 producer 都不得直接取得底层执行入口。
        for path in PRODUCERS:
            source = path.read_text(encoding="utf-8")
            self.assertNotIn(
                "execute_plan_without_present",
                source,
                msg=f"producer bypasses frame execution in {path}",
            )

    # 核心 producer 不得通过旧 accessor 或外部计划参数绕过帧所有权。
    def test_core_producers_do_not_use_old_plan_execution_shape(self) -> None:
        # 逐个读取当前 Renderer producer。
        for path in PRODUCERS:
            source = path.read_text(encoding="utf-8")
            # 文件名必须出现在失败消息中，便于定位回归。
            self.assertNotIn(".plan()", source, msg=f"旧 plan accessor remains in {path}")
            # producer 不得自行构造携带 target/load 的真实 RenderPassPlan。
            self.assertNotIn(".plan_mut()", source, msg=f"旧 plan_mut remains in {path}")
            self.assertNotIn("RenderPassPlan::new", source, msg=f"producer owns pass target in {path}")
            self.assertNotIn("frame.render_target()", source, msg=f"producer owns target selection in {path}")
            # producer 必须通过 Frame owner 的新 pass API 交付命令包。
            self.assertIn("new_pass", source, msg=f"producer lacks new_pass in {path}")
            self.assertIn("push_pass", source, msg=f"producer lacks push_pass in {path}")

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
