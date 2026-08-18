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
