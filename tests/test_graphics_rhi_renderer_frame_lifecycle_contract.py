# -*- coding: utf-8 -*-
# 验证 Renderer FramePlan 执行帧具有显式且不可重入的一次性生命周期。
"""Keep RhiRendererFrame execution single-use without hiding post-execution cleanup."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Renderer 帧执行 Component。
RENDERER_FRAME = ROOT / "src/draw/backend/rhi_renderer_execution.rs"


# 集中锁定 Pending/Consumed 状态与执行前门禁。
class GraphicsRhiRendererFrameLifecycleContractTests(unittest.TestCase):
    # 两个封闭角色必须共享同一私有执行状态类型。
    def test_frame_roles_have_pending_consumed_state(self) -> None:
        # 读取 Renderer 帧实现。
        source = RENDERER_FRAME.read_text(encoding="utf-8")
        # Renderer 门面必须保持字段私有的窄 struct。
        self.assertIn("pub(crate) struct RhiRendererFrame", source)
        # Surface 与 Offscreen 角色必须继续由私有封闭 enum 表达。
        self.assertIn("enum RhiRendererFrameRole", source)
        # 状态类型必须同时表达未执行与已消费。
        self.assertIn("enum RhiRendererFrameExecutionState", source)
        # 执行状态只属于当前 Component，不得扩散为 crate 公共契约。
        self.assertNotIn("pub(crate) enum RhiRendererFrameExecutionState", source)
        self.assertIn("Pending", source)
        self.assertIn("Consumed", source)
        # 两个构造器都必须从 Pending 开始。
        self.assertGreaterEqual(
            source.count("execution_state: RhiRendererFrameExecutionState::Pending"),
            2,
        )

    # 执行资格必须在角色 match 前消费，避免任何原生副作用后的重复门禁。
    def test_begin_execution_precedes_role_dispatch(self) -> None:
        # 读取 Renderer 帧实现。
        source = RENDERER_FRAME.read_text(encoding="utf-8")
        # 定位公开执行入口。
        execute_start = source.index("pub(crate) fn execute(&mut self, plan: &FramePlan)")
        # 只检查该入口的局部源码，避免误匹配其它 match。
        execute_body = source[execute_start : execute_start + 900]
        # 统一 helper 必须出现在角色分派之前。
        self.assertLess(
            execute_body.index("self.begin_execution()?"),
            execute_body.index("match &mut self.role"),
        )

    # 重复调用必须返回 InvalidState，并保留执行后的 Device 清理入口。
    def test_reexecution_error_and_device_borrow_remain_typed(self) -> None:
        # 读取 Renderer 帧实现。
        source = RENDERER_FRAME.read_text(encoding="utf-8")
        # 第二次执行必须使用稳定 InvalidState 分类。
        self.assertIn("Errc::InvalidState", source)
        # 执行后仍必须保留统一 Device 窄借用方法。
        self.assertIn("pub(crate) fn device(&mut self) -> &mut dyn GraphicsDevice", source)
        # 生命周期失败不得改成消费式 self。
        self.assertIn("pub(crate) fn execute(&mut self, plan: &FramePlan)", source)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
