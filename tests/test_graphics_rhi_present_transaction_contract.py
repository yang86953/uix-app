# -*- coding: utf-8 -*-
# 验证 acquire、submit 与 damage 只能通过共享类型化事务进入两个 Surface Adapter。
"""Keep Surface present validation identical across graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口和 Surface trait。
RHI = ROOT / "src/platform/presentation/rhi/mod.rs"
# 定位共享呈现事务 Component。
TRANSACTION = ROOT / "src/platform/presentation/rhi/present_transaction.rs"
# 定位共享提交序列 Component。
SUBMISSION = ROOT / "src/platform/presentation/rhi/submission.rs"
# 定位 FramePlan 最终执行事务。
FRAME_EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 D3D11 Surface Adapter。
D3D11_SURFACE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs"
# 定位 D3D11 Device 到 Surface 门禁 bridge。
D3D11_SUBMIT = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_submit.rs"
# 定位 OpenGL Surface Adapter。
OPENGL_SURFACE = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 OpenGL pipeline bridge。
OPENGL_BRIDGE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL Device 共享门禁调用点。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"


# 集中锁定跨 Adapter 呈现事务的类型边界和共同错误语义。
class GraphicsRhiPresentTransactionContractTests(unittest.TestCase):
    # Frame、提交与 damage 必须成为字段封闭的单一命令值。
    def test_present_input_is_one_closed_typed_transaction(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取共享呈现事务实现。
        transaction = TRANSACTION.read_text(encoding="utf-8")
        # 组合入口必须装配独立呈现事务 Component。
        self.assertIn("mod present_transaction;", rhi)
        # Surface trait 必须只接收一个不可拆事务。
        self.assertIn(
            "fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()>;",
            rhi,
        )
        # 旧的三个分离参数不得留在 Surface trait。
        self.assertNotIn("frame: SurfaceFrame,\n        submission: SubmissionHandle", rhi)
        # 呈现事务必须显式绑定三项事实。
        self.assertIn("pub(crate) struct RhiPresentTransaction", transaction)
        # frame 字段必须保持私有。
        self.assertIn("    frame: SurfaceFrame,", transaction)
        # submission 字段必须保持私有。
        self.assertIn("    submission: SubmissionHandle,", transaction)
        # damage 字段必须保持私有。
        self.assertIn("    damage: PresentDamage,", transaction)
        # 只有共享门禁可以发布已验证原生输入。
        self.assertIn("pub(crate) struct ValidatedRhiPresent", transaction)

    # 共享门禁必须唯一解释代际和最新提交，Surface 目标由类型固定。
    def test_shared_gate_owns_all_cross_backend_validation(self) -> None:
        # 读取共享呈现事务实现。
        transaction = TRANSACTION.read_text(encoding="utf-8")
        # 读取共享提交序列实现。
        submission = SUBMISSION.read_text(encoding="utf-8")
        # 旧代际必须统一映射为 Surface lost。
        self.assertIn("Errc::GraphicsSurfaceLost", transaction)
        # SurfaceFrame 不得再保存可伪造的 target 字段。
        self.assertNotIn("    target: RenderTargetHandle,", transaction)
        # SurfaceFrame 只允许投影共享 Surface 目标。
        self.assertIn("RenderTargetHandle::surface()", transaction)
        # 提交序列必须提供稳定的检查式入口。
        self.assertIn(
            "pub(crate) fn validate(&self, submission: SubmissionHandle) -> Result<()>",
            submission,
        )
        # 迟到提交诊断不得包含原生 API 名称。
        self.assertIn('"RHI present submission is stale"', submission)
        # 事务门禁必须组合 frame 代际与提交序列校验。
        self.assertIn(".validate_current(current_token)?;", transaction)
        # 事务门禁必须调用共享最新提交规则。
        self.assertIn("submissions.validate(self.submission)?;", transaction)

    # 两个原生 Adapter 必须只提供动态事实并消费验证后 damage。
    def test_adapters_do_not_reimplement_present_gate(self) -> None:
        # 读取 D3D11 Surface Adapter。
        d3d11_surface = D3D11_SURFACE.read_text(encoding="utf-8")
        # 读取 D3D11 提交 bridge。
        d3d11_submit = D3D11_SUBMIT.read_text(encoding="utf-8")
        # 读取 OpenGL Surface Adapter。
        opengl_surface = OPENGL_SURFACE.read_text(encoding="utf-8")
        # 读取 OpenGL pipeline bridge。
        opengl_bridge = OPENGL_BRIDGE.read_text(encoding="utf-8")
        # 读取 OpenGL Device Component。
        opengl_device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # D3D11 Surface 必须接收完整事务。
        self.assertIn("fn present(&mut self, transaction: RhiPresentTransaction)", d3d11_surface)
        # OpenGL Surface 必须接收同一完整事务。
        self.assertIn("fn present(&mut self, transaction: RhiPresentTransaction)", opengl_surface)
        # 两个 Surface Adapter 都不得直接读取 frame 私有事实。
        self.assertNotIn("frame.token", d3d11_surface + opengl_surface)
        # 两个 Surface Adapter 都不得直接读取 target 私有事实。
        self.assertNotIn("frame.target", d3d11_surface + opengl_surface)
        # 两个 Surface Adapter 都不得保留平台命名的迟到提交错误。
        self.assertNotIn("present submission is stale", d3d11_surface + opengl_surface)
        # D3D11 bridge 必须把动态事实交给共享门禁。
        self.assertIn("transaction.validate(", d3d11_submit)
        # OpenGL Device 必须调用同一共享门禁。
        self.assertIn("transaction.validate(", opengl_device)
        # OpenGL bridge 只做 owner-thread 转发。
        self.assertIn(
            ".validate_present(transaction, current_token, present_coherency)",
            opengl_bridge,
        )
        # D3D11 只能把已验证 damage 交给 DXGI 路径。
        self.assertIn("self.present_result(&present)", d3d11_surface)
        # OpenGL 只能把已验证 damage 交给 swap 路径。
        self.assertIn("self.rhi_swap_buffers(present.into_damage())", opengl_surface)

    # FramePlan 必须在唯一最终边界构造不可拆事务。
    def test_frame_plan_constructs_transaction_at_surface_boundary(self) -> None:
        # 读取 FramePlan 执行实现。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # acquire frame 仍必须来自 Surface 角色。
        self.assertIn("let frame = context.surface().acquire()?;", execution)
        # submit 身份仍必须来自 Device 角色。
        self.assertIn("let submission = context.device().submit()?;", execution)
        # 最终 Surface 调用必须原子构造呈现事务。
        self.assertIn(".present(RhiPresentTransaction::new(", execution)
        # 构造顺序必须绑定 acquire frame、submit identity 与最终 damage。
        self.assertIn("frame,\n                // 绑定 Device 刚签发的提交身份。\n                submission,", execution)
        # FramePlan 可以只读 frame 事实，不得访问私有字段。
        self.assertIn("frame.target()", execution)
        # FramePlan 的代际检查也必须使用只读访问器。
        self.assertIn("frame.token()", execution)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
