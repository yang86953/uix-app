# -*- coding: utf-8 -*-
# 验证 D3D11 局部清理能力声明与 ClearView 执行共用同一冻结接口事实。
"""Keep clear-rect capability truthful across graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 D3D11 Device Adapter。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位 D3D11 局部清理 Component。
D3D11_CLEAR = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_clear.rs"
# 定位 D3D11 context 构造边界。
D3D11_CONSTRUCTION = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/methods.rs"
# 定位 OpenGL Device capability 实现。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"


# 集中锁定 capability 事实不得与原生执行支持性分叉。
class GraphicsRhiClearCapabilityContractTests(unittest.TestCase):
    # D3D11 Device 必须在构造时冻结可选的 D3D11.1 context。
    def test_d3d11_device_owns_one_frozen_clear_interface(self) -> None:
        # 读取 D3D11 Device Adapter。
        device = D3D11_DEVICE.read_text(encoding="utf-8")
        # 读取 context 构造边界。
        construction = D3D11_CONSTRUCTION.read_text(encoding="utf-8")
        # Device owner 必须保存可选 D3D11.1 interface。
        self.assertIn("clear_context: Option<", device)
        # 构造器必须接收已经创建的 immediate context。
        self.assertIn("pub(super) fn new(", device)
        # 可选接口只能在 Device 构造中查询一次。
        self.assertIn("clear_context: rhi_device_clear::query_clear_context(context)", device)
        # context 组合根必须在移动原生 context 前建立 RHI Device。
        self.assertIn("let rhi_device = D3d11RhiDevice::new(&context);", construction)
        # 组合根必须接管同一个已冻结 Device owner。
        self.assertIn("        rhi_device,", construction)

    # D3D11 capability 必须投影冻结接口而不是无条件承诺。
    def test_d3d11_capability_reads_the_frozen_interface(self) -> None:
        # 读取 D3D11 Device Adapter。
        device = D3D11_DEVICE.read_text(encoding="utf-8")
        # capability 必须从同一接口 accessor 读取支持性。
        self.assertIn(
            "capabilities.clear_rect = self.rhi_device.clear_rect_context().is_some();",
            device,
        )
        # D3D11 不得再无条件声明局部清理能力。
        self.assertNotIn("capabilities.clear_rect = true;", device)
        # accessor 必须只借用构造时保存的 COM owner。
        self.assertIn("self.clear_context.as_ref()", device)

    # D3D11 执行路径必须消费 capability 使用的同一接口 owner。
    def test_d3d11_clear_execution_reuses_the_frozen_interface(self) -> None:
        # 读取 D3D11 局部清理 Component。
        clear = D3D11_CLEAR.read_text(encoding="utf-8")
        # 构造查询必须使用唯一的 D3D11.1 QueryInterface。
        self.assertIn("context.cast::<ID3D11DeviceContext1>().ok()", clear)
        # ClearView 执行必须借用 Device 保存的 accessor。
        self.assertIn(".clear_rect_context()", clear)
        # 执行阶段不得再次从 immediate context 查询接口。
        execution = clear.split("pub(super) fn rhi_clear_rect", maxsplit=1)[1]
        # 后半段只允许消费冻结接口，不再包含 COM cast。
        self.assertNotIn(".cast::<ID3D11DeviceContext1>()", execution)
        # 直接误调用仍必须产生 typed capability 缺失错误。
        self.assertIn(
            'rhi_not_implemented("clear_rect requires ID3D11DeviceContext1")',
            execution,
        )

    # OpenGL 已实现的局部清理能力必须保持明确声明。
    def test_opengl_clear_capability_remains_implemented(self) -> None:
        # 读取 OpenGL RHI bridge。
        opengl = OPENGL_DEVICE.read_text(encoding="utf-8")
        # GLES scissor clear 仍稳定提供局部清理原语。
        self.assertIn("capabilities.clear_rect = true;", opengl)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
