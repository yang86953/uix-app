# -*- coding: utf-8 -*-
# 验证 FramePlan 在 Surface acquire 前完成共享 Device 资源预检。
"""Keep read-only FramePlan resource checks ahead of Surface acquisition."""

# 引入标准单元测试框架。
import unittest

# 引入稳定的跨平台路径类型。
from pathlib import Path


# 定位当前仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 FramePlan 资源预检与原生命令执行边界。
FRAME_EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 overlay backdrop 快照与恢复的统一计划边界。
BACKDROP_EXECUTION = ROOT / "src/draw/backend/gpu/backend/render_backend_backdrop.rs"
# 定位 FramePlan scope 与顶层步骤校验实现。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位原生 lowering 的 move-only 执行 helper。
RHI_LOWERING = ROOT / "src/draw/backend/gpu/rhi_lowering.rs"
# 定位 FrameEncoder 的 move-only 执行 helper。
RHI_FRAME = ROOT / "src/draw/backend/gpu/backend/rhi_frame.rs"
# 定位 Surface scroll 的 move-only 执行入口。
RHI_SURFACE_SCROLL = ROOT / "src/draw/backend/gpu/backend/rhi_surface_scroll.rs"


# 提取指定 Rust 函数的完整花括号范围。
def function_body(text: str, function_name: str) -> str:
    # 构造非泛型函数的精确签名标记。
    marker = f"fn {function_name}("
    # 泛型方法需要在名称后先出现类型参数。
    if marker not in text:
        # 切换为泛型函数的精确签名标记。
        marker = f"fn {function_name}<"
    # 定位函数签名，避免被相似长名称误匹配。
    start = text.index(marker)
    # 定位函数体开始的左花括号。
    opening = text.index("{", start)
    # 初始化 Rust 花括号嵌套深度。
    depth = 0
    # 从函数体开始逐字符扫描。
    for index in range(opening, len(text)):
        # 读取当前字符。
        character = text[index]
        # 进入嵌套代码块时增加深度。
        if character == "{":
            # 记录新进入的花括号层级。
            depth += 1
        # 离开嵌套代码块时减少深度。
        elif character == "}":
            # 记录当前代码块已经结束。
            depth -= 1
            # 顶层函数闭合时返回精确源码范围。
            if depth == 0:
                # 包含闭合花括号，便于完整断言函数责任。
                return text[opening : index + 1]
    # 源码结构损坏时让契约测试明确失败。
    raise AssertionError(f"函数 {function_name} 缺少闭合花括号")


# 锁定只读预检、Surface 事务与原生命令执行的职责分离。
class GraphicsRhiFramePlanResourcePreflightContractTests(unittest.TestCase):
    # 四类资源检查必须集中在唯一只读 Component。
    def test_resource_preflight_owns_all_read_only_device_checks(self) -> None:
        # 读取 FramePlan 执行 Module 的当前源码。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 截取资源预检 Component 到原生命令执行器之间的完整范围。
        preflight = execution[
            execution.index("struct FramePlanResourcePreflight") :
            execution.index("pub(super) struct FramePlanExecutor")
        ]
        # 资源预检只能持有只读 Device 借用。
        self.assertIn("device: &'device D", preflight)
        # Surface 与 Offscreen 必须使用封闭 scope 变体。
        self.assertIn("SurfaceAllowed", preflight)
        # 离屏作用域也必须由同一封闭枚举表达。
        self.assertIn("OffscreenOnly", preflight)
        # 提取唯一预检运行入口。
        run = function_body(preflight, "run")
        # 预检必须按 target、transfer、upload、draw、sampled 的固定顺序执行。
        validators = (
            # 首先验证目标身份和能力。
            "self.validate_targets(steps)?",
            # 随后验证 pass 外纹理传输。
            "self.validate_transfers(steps)?",
            # 随后验证每条类型化 Buffer 上传。
            "self.validate_buffer_uploads(steps)?",
            # 随后验证 Draw 的 pipeline 与 Buffer 资源。
            "self.validate_draw_resources(steps)?",
            # 最后验证 sampled texture 与 sampler。
            "self.validate_sampled_bindings(steps)",
        )
        # 保存前一验证阶段的位置以检查严格顺序。
        previous = -1
        # 逐项验证每个阶段存在且顺序稳定。
        for validator in validators:
            # 定位当前验证阶段。
            current = run.index(validator)
            # 当前阶段必须晚于前一阶段。
            self.assertGreater(current, previous)
            # 推进顺序游标。
            previous = current
        # target 预检必须解析真实 texture render-target 能力。
        self.assertIn("self.device.resolve_render_target(texture)?", preflight)
        # copy 预检必须进入共享 GraphicsDevice 只读入口。
        self.assertIn("self.device.preflight_texture_copy(*copy)?", preflight)
        # move 预检必须进入共享 GraphicsDevice 只读入口。
        self.assertIn("self.device.preflight_texture_move(*movement)?", preflight)
        # 顶点、索引与 Uniform 上传都必须进入共享只读预检入口。
        self.assertEqual(preflight.count("self.device.preflight_buffer_upload("), 3)
        # Draw 预检必须进入共享 GraphicsDevice 只读入口。
        self.assertIn("self.device.preflight_draw_resources(*packet)?", preflight)
        # sampled 预检必须进入共享 GraphicsDevice 只读入口。
        self.assertIn("self.device.preflight_sampled_binding(*binding)?", preflight)
        # 只读 Component 不得激活 Device。
        self.assertNotIn(".activate()", preflight)
        # 只读 Component 不得取得或呈现 Surface。
        self.assertNotIn(".acquire()", preflight)
        # 只读 Component 不得越权执行最终 present。
        self.assertNotIn(".present(", preflight)

    # Surface 事务必须先预检，再 acquire，最后执行原生命令。
    def test_surface_preflight_precedes_acquire_and_executor(self) -> None:
        # 读取 FramePlan 执行 Module 的当前源码。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 提取完整 Surface 帧事务函数。
        surface = function_body(execution, "execute_on_context_with_before_present")
        # 定义 Surface 只读资源预检调用。
        preflight = "FramePlanResourcePreflight::surface(context.device_ref()).run(&self.steps)?"
        # 定义唯一 Surface acquire 调用。
        acquire = "context.surface().acquire()?"
        # 定义 acquire 后的原生命令执行器调用。
        executor = "FramePlanExecutor::for_surface(context.device(), frame.target()).execute(&self.steps)?"
        # Surface 事务必须包含三个明确阶段。
        self.assertIn(preflight, surface)
        # Surface 事务必须包含唯一 acquire。
        self.assertIn(acquire, surface)
        # Surface 事务必须包含 acquire 后的唯一执行器。
        self.assertIn(executor, surface)
        # 只读 Device 资源预检必须严格早于 Surface acquire。
        self.assertLess(surface.index(preflight), surface.index(acquire))
        # Surface acquire 必须严格早于需要 acquired target 的执行器。
        self.assertLess(surface.index(acquire), surface.index(executor))
        # preflight 与 acquire 之间不得提前激活 Device。
        before_acquire = surface[: surface.index(acquire)]
        # 原生激活只能由 acquire 后的 Executor 负责。
        self.assertNotIn(".activate()", before_acquire)
        # 最终 present 仍必须留在同一 Surface 事务内。
        self.assertIn(".present(RhiPresentTransaction::new(", surface)

    # Offscreen 必须复用同一预检且永不取得 Surface 角色。
    def test_offscreen_reuses_preflight_before_executor(self) -> None:
        # 读取 FramePlan 执行 Module 的当前源码。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 提取完整 Offscreen 执行入口。
        offscreen = function_body(execution, "execute_offscreen_on_device")
        # 定义离屏只读资源预检调用。
        preflight = "FramePlanResourcePreflight::offscreen(device).run(&self.steps)?"
        # 定义离屏原生命令执行器调用。
        executor = "FramePlanExecutor::offscreen(device).execute(&self.steps)?"
        # Offscreen 必须包含共享预检和唯一执行器。
        self.assertIn(preflight, offscreen)
        # Offscreen 必须进入同一原生命令执行器。
        self.assertIn(executor, offscreen)
        # 离屏预检必须严格早于原生命令执行器。
        self.assertLess(offscreen.index(preflight), offscreen.index(executor))
        # 离屏入口不得取得 Surface image。
        self.assertNotIn(".acquire()", offscreen)
        # 离屏入口不得调用 Surface present。
        self.assertNotIn(".present(", offscreen)

    # overlay backdrop 快照与恢复必须复用离屏 FramePlan 执行边界。
    def test_overlay_backdrop_reuses_offscreen_frame_plan_boundary(self) -> None:
        # 读取 GPU backend backdrop helper 的当前源码。
        backdrop = BACKDROP_EXECUTION.read_text(encoding="utf-8")
        # 分别提取快照和恢复 helper 的完整函数体。
        functions = {
            # 快照复制必须走离屏计划。
            "create_rhi_overlay_backdrop": function_body(
                backdrop, "create_rhi_overlay_backdrop"
            ),
            # 恢复复制必须走离屏计划。
            "restore_rhi_overlay_backdrop": function_body(
                backdrop, "restore_rhi_overlay_backdrop"
            ),
        }
        # 两个 helper 都必须共享同一组计划构造、复制和执行契约。
        required_fragments = (
            # 计划必须明确声明离屏作用域。
            "FramePlan::offscreen()",
            # 复制命令必须进入类型化计划。
            "push_copy(",
            # 计划必须通过唯一离屏设备执行边界提交。
            "execute_offscreen_on_device(device)",
        )
        # 直接设备命令和 Surface 事务命令不得泄漏到 helper。
        forbidden_fragments = (
            # 禁止绕过 FramePlan 直接复制纹理。
            "device.copy_texture(",
            # 禁止绕过 FramePlan 直接提交。
            "device.submit(",
            # 禁止取得 Surface。
            ".acquire(",
            # 禁止呈现 Surface。
            ".present(",
        )
        # 分别锁定两个 helper 的复用与越权调用边界。
        for function_name, body in functions.items():
            # 每个 helper 都必须包含全部离屏计划片段。
            with self.subTest(function_name=function_name):
                # 逐项断言计划构造、复制和执行均存在。
                for fragment in required_fragments:
                    # 任何缺失都表示 helper 绕过了统一执行边界。
                    self.assertIn(fragment, body)
                # 每个 helper 都不得直接触碰设备或 Surface 事务命令。
                for fragment in forbidden_fragments:
                    # 防止快照或恢复形成第二套执行路径。
                    self.assertNotIn(fragment, body)

    # FramePlan 必须区分空计划、Surface 计划与非空离屏资源计划。
    def test_frame_plan_validate_scopes_empty_and_resource_only_rules(self) -> None:
        # 读取 FramePlan scope 与 validate Component 的当前源码。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 截取唯一顶层 validate 函数，避免锁定局部变量名。
        validate = function_body(frame_plan, "validate")
        # 空计划必须在任何 scope 下被拒绝。
        self.assertIn("self.steps.is_empty()", validate)
        # Surface scope 必须单独保留至少一个 render pass 的约束。
        self.assertIn("FramePlanScope::Surface", validate)
        # Surface 规则必须检查顶层 Pass 步骤。
        self.assertIn("FramePlanStep::Pass", validate)
        # 资源步骤必须覆盖 copy 与 move 两种纯离屏计划。
        self.assertIn("FramePlanStep::Copy", validate)
        self.assertIn("FramePlanStep::Move", validate)
        # 压缩空白后检查 Surface 分支而非全局无条件 pass 门禁。
        compact_validate = "".join(validate.split())
        # Surface 规则必须在 scope 条件内检查 render pass。
        self.assertIn("FramePlanScope::Surface", compact_validate)
        # 禁止恢复对所有 scope 无条件要求 pass 的旧规则。
        self.assertNotIn("if!self.steps.iter().any", compact_validate)

    # 三个 move-only lowering helper 必须只复用离屏 FramePlan 资源边界。
    def test_move_only_helpers_avoid_placeholder_render_pass(self) -> None:
        # 按文件和函数名读取三个独立 lowering Component 的源码。
        helpers = {
            # Canvas2D 原生 scroll lowering 的 move 提交 helper。
            "rhi_lowering::execute_texture_move": function_body(
                RHI_LOWERING.read_text(encoding="utf-8"), "execute_texture_move"
            ),
            # FrameEncoder scroll lowering 的 move 提交 helper。
            "rhi_frame::execute_frame_texture_move": function_body(
                RHI_FRAME.read_text(encoding="utf-8"), "execute_frame_texture_move"
            ),
            # Surface scroll 批量移动的执行入口。
            "rhi_surface_scroll::try_apply_rhi_surface_scroll_copies": function_body(
                RHI_SURFACE_SCROLL.read_text(encoding="utf-8"),
                "try_apply_rhi_surface_scroll_copies",
            ),
        }
        # 每个 helper 都必须声明、追加并执行离屏 move 计划。
        required_fragments = (
            # move-only 操作必须使用离屏 scope。
            "FramePlan::offscreen()",
            # 每条纹理移动必须进入类型化计划。
            "push_move(",
            # 计划必须通过统一离屏设备边界执行。
            "execute_offscreen_on_device(",
        )
        # move-only helper 不得伪造 render pass 或 viewport 占位。
        forbidden_fragments = (
            # 禁止引入 RenderPassPlan 占位目标。
            "RenderPassPlan",
            # 禁止追加空 pass 满足旧校验。
            "push_pass",
            # 禁止追加仅用于占位的 viewport。
            "SetViewport",
        )
        # 逐个锁定 helper 的纯资源步骤契约。
        for function_name, body in helpers.items():
            # 每个 helper 都必须满足相同的离屏边界。
            with self.subTest(function_name=function_name):
                # 断言离屏计划构造、move 追加和执行均存在。
                for fragment in required_fragments:
                    # 缺失任一片段都表示绕过统一资源执行路径。
                    self.assertIn(fragment, body)
                # 断言没有遗留的 render pass 占位实现。
                for fragment in forbidden_fragments:
                    # 防止 move-only 计划重新引入无意义的绘制步骤。
                    self.assertNotIn(fragment, body)

    # Executor 只能拥有 activate 后的原生命令责任。
    def test_executor_does_not_repeat_resource_preflight(self) -> None:
        # 读取 FramePlan 执行 Module 的当前源码。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 提取 Executor 的唯一 execute 函数。
        executor = function_body(execution, "execute")
        # Executor 必须先激活 owner context。
        self.assertIn("self.device.activate()?", executor)
        # Executor 必须在激活后执行健康维护。
        self.assertIn("self.device.maintain()?", executor)
        # 激活必须严格早于健康维护。
        self.assertLess(
            executor.index("self.device.activate()?"),
            executor.index("self.device.maintain()?"),
        )
        # 健康维护必须严格早于第一条 FramePlan 命令。
        self.assertLess(
            executor.index("self.device.maintain()?"),
            executor.index("for step in steps"),
        )
        # Executor 不得重新执行任一只读资源预检。
        for validator in (
            # 目标资源预检归属只读 Component。
            "validate_targets",
            # 传输资源预检归属只读 Component。
            "validate_transfers",
            # Buffer 上传资源预检归属只读 Component。
            "validate_buffer_uploads",
            # Draw 资源预检归属只读 Component。
            "validate_draw_resources",
            # sampled 资源预检归属只读 Component。
            "validate_sampled_bindings",
        ):
            # 每个验证方法都必须从 Executor 主体消失。
            with self.subTest(validator=validator):
                # 防止 Surface 与 Offscreen 执行重复检查同一资源。
                self.assertNotIn(validator, executor)
        # Executor 不得越权取得 Surface image。
        self.assertNotIn(".acquire()", executor)
        # Executor 不得越权执行最终 present。
        self.assertNotIn(".present(", executor)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行本文件声明的契约测试。
    unittest.main()
