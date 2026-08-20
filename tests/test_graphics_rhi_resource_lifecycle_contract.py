# -*- coding: utf-8 -*-
# 验证活动 render target 的销毁规则只由共享 pass 生命周期解释。
"""Keep texture destruction lifecycle identical across graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 render-pass 状态机。
PASS_STATE = ROOT / "src/native/presentation/rhi/pass_state.rs"
# 定位 D3D11 资源 Device Adapter。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位 OpenGL 资源 Device Adapter。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"


# 集中锁定 texture 销毁的共享生命周期边界。
class GraphicsRhiResourceLifecycleContractTests(unittest.TestCase):
    # 共享 pass 状态必须提供检查式销毁门禁。
    def test_pass_state_owns_active_target_destroy_rule(self) -> None:
        # 读取 API 无关的 pass 状态实现。
        state = PASS_STATE.read_text(encoding="utf-8")
        # 销毁前检查必须返回统一 typed Result。
        self.assertIn(
            "pub(crate) fn validate_texture_destroy(&self, texture: TextureHandle) -> Result<()>",
            state,
        )
        # 活动目标必须通过封闭 texture 投影比较。
        self.assertIn("active.target.texture() == Some(texture)", state)
        # 调用顺序违例必须统一为 InvalidState。
        self.assertIn('"RHI cannot destroy the active render target"', state)
        self.assertIn("return Err(invalid_state(", state)
        # 旧布尔探测不得继续把规则交给 Adapter 解释。
        self.assertNotIn("fn references_target", state)

    # 两个 Adapter 必须调用同一门禁且不构造平台错误。
    def test_adapters_share_the_checked_destroy_gate(self) -> None:
        # 读取两套生产 Device Adapter。
        d3d11 = D3D11_DEVICE.read_text(encoding="utf-8")
        opengl = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 两个 Adapter 必须只调用共享检查式入口。
        self.assertIn("pass.validate_texture_destroy(texture)?;", d3d11)
        self.assertIn("pass.validate_texture_destroy(handle)?;", opengl)
        # Adapter 不得继续读取布尔目标引用事实。
        self.assertNotIn("references_target", d3d11 + opengl)
        # 平台命名的销毁错误必须完全消失。
        self.assertNotIn("D3d11 RHI cannot destroy the active render target", d3d11)
        self.assertNotIn("OpenGL RHI cannot destroy active target", opengl)

    # 资源表与 sampled binding 更新必须保留检查、销毁、提交的顺序。
    def test_destroy_order_is_gate_take_then_unbind(self) -> None:
        # 读取两套生产 Device Adapter。
        sources = (
            D3D11_DEVICE.read_text(encoding="utf-8"),
            OPENGL_DEVICE.read_text(encoding="utf-8"),
        )
        # 分别检查每个 Adapter 的单调顺序。
        for source in sources:
            # 共享门禁必须最先执行。
            gate = source.index("validate_texture_destroy")
            # 只在门禁通过后从资源表取出 texture。
            take = source.index("textures.take", gate)
            # 只在资源成功取出后清除 sampled binding。
            unbind = source.index("unbind_texture", take)
            # 锁定检查、资源销毁、共享状态提交的先后关系。
            self.assertLess(gate, take)
            self.assertLess(take, unbind)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
