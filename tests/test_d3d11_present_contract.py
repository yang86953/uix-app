# 导入标准单元测试框架。
import unittest
# 导入跨平台路径类型。
from pathlib import Path

# 保存仓库根目录，避免测试依赖启动工作目录。
ROOT = Path(__file__).resolve().parents[1]


# 固化 D3D11 swapchain 与对外 present 能力的一致性边界。
class D3d11PresentContractTests(unittest.TestCase):
    # 验证所有 D3D11 能力消费者都从同一 swapchain 事实投影。
    def test_discard_swapchain_has_one_present_capability_source(self) -> None:
        # 读取交换链创建与事实契约模块。
        swapchain = (ROOT / "src/native/presentation/graphics/d3d11/platform/swapchain.rs").read_text(encoding="utf-8")
        # 读取静态 recipe 能力投影所在的平台适配模块。
        platform = (ROOT / "src/native/presentation/graphics/d3d11/platform/mod.rs").read_text(encoding="utf-8")
        # 读取 coherency 投影所在的 context 方法模块。
        methods = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/methods.rs").read_text(encoding="utf-8")
        # 读取薄 RHI 能力投影。
        rhi_device = (ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs").read_text(encoding="utf-8")
        # 唯一事实必须由 swapchain 模块提供，按 tracked 与 legacy 两个形态固化。
        self.assertIn("pub(crate) const fn tracked_swap_chain_contract()", swapchain)
        self.assertIn("pub(crate) const fn legacy_swap_chain_contract()", swapchain)
        # 两种形态必须通过同一 contract() 分发读取冻结事实。
        self.assertIn("Self::Tracked(_) => tracked_swap_chain_contract(),", swapchain)
        self.assertIn("Self::Legacy(_) => legacy_swap_chain_contract(),", swapchain)
        # 当前生产交换链必须继续使用 DISCARD。
        self.assertIn("swap_effect: DXGI_SWAP_EFFECT_DISCARD", swapchain)
        # DISCARD 只能提供完整提交 coherency。
        self.assertIn("present_coherency: PresentCoherency::FullOnly", swapchain)
        # DISCARD 不得宣称 compositor 脏矩形能力。
        self.assertIn("partial_present: false", swapchain)
        # descriptor 缓冲数量必须消费同一事实。
        self.assertIn("BufferCount: contract.buffer_count", swapchain)
        # descriptor 交换效果必须消费同一事实。
        self.assertIn("SwapEffect: contract.swap_effect", swapchain)
        # context 的 coherency 投影必须读取同一 swapchain 契约。
        self.assertIn("self.swap_chain.contract().present_coherency", methods)
        # 静态 recipe 能力必须消费同一 swapchain 事实。
        self.assertIn("let caps = context_caps(ctx.present_coherency());", platform)
        # 平台适配不得保留第二份 FullOnly 硬编码。
        self.assertNotIn("PresentCoherency::FullOnly", platform)
        # 薄 RHI 必须从同一事实投影 partial-present 能力。
        self.assertIn("capabilities.partial_present = self.swap_chain.contract().partial_present;", rhi_device)


# 支持直接执行本测试文件。
if __name__ == "__main__":
    # 运行本文件声明的全部契约测试。
    unittest.main()
