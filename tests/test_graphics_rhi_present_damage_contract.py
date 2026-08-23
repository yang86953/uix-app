# -*- coding: utf-8 -*-
# 验证 Surface 能力、物理范围与最终 damage 只在共享 RHI 门禁中结合。
"""Keep present-damage normalization out of native graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Drawing 核心的最终 damage 规范。
CORE_DAMAGE = ROOT / "src/core/damage/mod.rs"
# 定位共享 Surface 呈现事务。
TRANSACTION = ROOT / "src/platform/presentation/rhi/present_transaction.rs"
# 定位 D3D11 Surface 与提交 bridge。
D3D11_SURFACE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs"
# 定位 D3D11 共享门禁转发。
D3D11_SUBMIT = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_submit.rs"
# 定位 D3D11 swapchain Adapter。
D3D11_SWAPCHAIN = ROOT / "src/native/presentation/graphics/d3d11/adapter/swapchain.rs"
# 定位 D3D11 context 呈现收口。
D3D11_METHODS = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/methods.rs"
# 定位 OpenGL Surface Adapter。
OPENGL_SURFACE = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 OpenGL owner-thread bridge。
OPENGL_BRIDGE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL 共享呈现门禁调用点。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"


# 集中锁定跨后端最终 damage 的唯一解释边界。
class GraphicsRhiPresentDamageContractTests(unittest.TestCase):
    # Drawing 核心必须拥有矩形上限、合并与 Surface 范围规则。
    def test_shared_damage_contract_owns_normalization(self) -> None:
        # 读取共享 damage 实现。
        damage = CORE_DAMAGE.read_text(encoding="utf-8")
        # 读取共享呈现事务。
        transaction = TRANSACTION.read_text(encoding="utf-8")
        # 最终 damage 必须提供按 Surface 规范化的唯一入口。
        self.assertIn("pub(crate) fn normalize_for_surface(", damage)
        # 矩形数量与合并必须复用 core 的唯一规则。
        self.assertIn("normalize_present_rects(rects)", damage)
        # FullOnly 必须在进入原生 Adapter 前降级。
        self.assertIn("coherency == PresentCoherency::FullOnly", damage)
        # 范围检查必须使用右下开区间物理边界。
        self.assertIn("x + rect_width <= surface_width", damage)
        self.assertIn("y + rect_height <= surface_height", damage)
        # 事务门禁必须在发布 ValidatedRhiPresent 前规范化。
        self.assertIn("self.damage.normalize_for_surface(", transaction)
        # 门禁必须同时消费 Surface 保留能力。
        self.assertIn("present_coherency: PresentCoherency", transaction)

    # 两个 Surface Adapter 必须把同一 capability 事实交给共享门禁。
    def test_surface_capability_flows_into_shared_gate(self) -> None:
        # 读取 D3D11 Surface 和提交 bridge。
        d3d11_surface = D3D11_SURFACE.read_text(encoding="utf-8")
        d3d11_submit = D3D11_SUBMIT.read_text(encoding="utf-8")
        # 读取 OpenGL Surface、bridge 和 Device 门禁。
        opengl_surface = OPENGL_SURFACE.read_text(encoding="utf-8")
        opengl_bridge = OPENGL_BRIDGE.read_text(encoding="utf-8")
        opengl_device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 两个 Surface 都必须从自己的 profile 冻结 coherency。
        for source in (d3d11_surface, opengl_surface):
            # capability 是唯一权威事实。
            self.assertIn("self.surface_capabilities().present_coherency", source)
        # D3D11 bridge 必须把 coherency 交给事务门禁。
        self.assertIn("present_coherency: PresentCoherency", d3d11_submit)
        self.assertIn("present_coherency,", d3d11_submit)
        # OpenGL bridge 和 Device 也必须保留同一动态事实。
        self.assertIn("present_coherency: PresentCoherency", opengl_bridge)
        self.assertIn("present_coherency: PresentCoherency", opengl_device)
        self.assertIn("present_coherency,", opengl_device)

    # D3D11 Adapter 只能消费已验证事务并机械映射 RECT。
    def test_d3d11_adapter_only_projects_validated_damage(self) -> None:
        # 读取 swapchain Adapter 与 context 收口。
        swapchain = D3D11_SWAPCHAIN.read_text(encoding="utf-8")
        methods = D3D11_METHODS.read_text(encoding="utf-8")
        # swapchain 必须以封闭已验证值作为入口。
        self.assertIn("present: &ValidatedRhiPresent", swapchain)
        self.assertIn("present: &ValidatedRhiPresent", methods)
        # Adapter 只保留最终矩形到 RECT 的机械投影。
        self.assertIn("fn native_dirty_rects(damage: &PresentDamage) -> Vec<RECT>", swapchain)
        self.assertIn("right: x + width", swapchain)
        self.assertIn("bottom: y + height", swapchain)
        # 平台私有的数量上限与几何门禁必须完全移除。
        self.assertNotIn("MAX_D3D11_PRESENT_RECTS", swapchain)
        self.assertNotIn("validated_dirty_rects", swapchain)
        self.assertNotIn("checked_add", swapchain)

    # 两个 Surface Adapter 都必须在共享校验成功后才改变原生呈现状态。
    def test_shared_present_gate_precedes_native_state_changes(self) -> None:
        # 读取 OpenGL 与 D3D11 的最终 Surface 呈现入口。
        opengl = OPENGL_SURFACE.read_text(encoding="utf-8")
        # D3D11 作为同一共享事务的跨后端时序对照。
        d3d11 = D3D11_SURFACE.read_text(encoding="utf-8")
        # 截取 OpenGL 的单次 present 实现，避免其它生命周期入口干扰顺序断言。
        opengl_start = opengl.index("fn present(&mut self, transaction: RhiPresentTransaction)")
        # OpenGL present 在 test_present 前结束。
        opengl_present = opengl[
            opengl_start : opengl.index("fn test_present", opengl_start)
        ]
        # 共享事务校验必须早于 native current context 切换。
        self.assertLess(
            opengl_present.index("rhi_validate_present("),
            opengl_present.index("rhi_make_current()"),
        )
        # native 交换只能消费共享门禁发布的结果。
        self.assertLess(
            opengl_present.index("rhi_make_current()"),
            opengl_present.index("rhi_swap_buffers("),
        )
        # 截取 D3D11 的单次 present 实现作为同一失败时序对照。
        d3d11_start = d3d11.index("fn present(&mut self, transaction: RhiPresentTransaction)")
        # D3D11 present 同样在 test_present 前结束。
        d3d11_present = d3d11[
            d3d11_start : d3d11.index("fn test_present", d3d11_start)
        ]
        # D3D11 必须继续先校验事务再恢复 swapchain target。
        self.assertLess(
            d3d11_present.index("validate_present_impl("),
            d3d11_present.index("bind_swapchain_target()"),
        )
        # 原生 Present 只接收共享校验后的封闭值。
        self.assertLess(
            d3d11_present.index("bind_swapchain_target()"),
            d3d11_present.index("present_result(&present)"),
        )


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
