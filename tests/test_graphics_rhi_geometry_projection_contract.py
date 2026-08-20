# -*- coding: utf-8 -*-
# 验证目标几何值只通过共享 checked 投影进入 OpenGL 与 D3D11。
"""Keep native geometry projections identical across graphics adapters."""

# 启用延迟注解解析，保持契约脚本风格一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享目标几何 Component。
GEOMETRY = ROOT / "src/native/presentation/rhi/geometry.rs"
# 定位共享 pass 状态机。
PASS_STATE = ROOT / "src/native/presentation/rhi/pass_state.rs"
# 定位 OpenGL 状态映射。
OPENGL_STATE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL texture 上传映射。
OPENGL_UPLOAD = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_upload.rs"
# 定位 D3D11 状态映射。
D3D11_STATE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_state.rs"
# 定位 D3D11 局部清理映射。
D3D11_CLEAR = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_clear.rs"
# 定位最终 surface damage 合成边界。
SURFACE_COMPOSITE = ROOT / "src/draw/backend/gpu/backend/rhi_surface_composite.rs"


# 集中验证共享原生值域与两个 Adapter 的机械投影。
class GraphicsRhiGeometryProjectionContractTests(unittest.TestCase):
    # 共享 Component 必须拥有全部 checked 几何投影。
    def test_shared_geometry_owns_native_projections(self) -> None:
        # 读取共享几何源码。
        geometry = GEOMETRY.read_text(encoding="utf-8")
        # extent 必须统一提供有符号原生尺寸。
        self.assertIn("pub(crate) const fn native_size_i32(self)", geometry)
        # viewport 必须复用同名整像素投影。
        self.assertEqual(geometry.count("fn native_size_i32(self)"), 2)
        # scissor 必须统一提供 checked 四边。
        self.assertIn("pub(crate) const fn native_rect(self)", geometry)
        # 底部原点 Y 必须由共享值对象换算。
        self.assertIn("pub(crate) const fn bottom_origin_y(self", geometry)
        # 远端边界必须使用 checked 加法。
        self.assertIn("self.x.checked_add(self.width)", geometry)
        # 目标方向换算必须使用 checked 减法。
        self.assertIn("extent_height.checked_sub(bottom)", geometry)

    # pass、OpenGL 与 D3D11 必须复用同一有效目标值域。
    def test_pass_and_adapters_share_extent_and_viewport_domain(self) -> None:
        # 读取共享 pass 状态机。
        pass_state = PASS_STATE.read_text(encoding="utf-8")
        # 读取 OpenGL 状态编码。
        opengl = OPENGL_STATE.read_text(encoding="utf-8")
        # 读取 D3D11 状态编码。
        d3d11 = D3D11_STATE.read_text(encoding="utf-8")
        # pass 起点必须拒绝共同原生值域之外的 extent。
        self.assertIn("if !extent.is_valid()", pass_state)
        # OpenGL viewport 必须消费共享整像素投影。
        self.assertIn(".native_size_i32()", opengl)
        # OpenGL 不得直接截断 viewport。
        self.assertNotIn("viewport.width as i32", opengl)
        # D3D11 完整 scissor 必须消费共享 extent 投影。
        self.assertIn(".native_size_i32()", d3d11)
        # D3D11 不得直接截断完整目标尺寸。
        self.assertNotIn("extent.width as i32", d3d11)

    # 两个 Adapter 必须消费共享矩形与方向投影。
    def test_scissor_and_upload_paths_have_no_private_overflow_rules(self) -> None:
        # 读取 OpenGL 状态编码。
        opengl = OPENGL_STATE.read_text(encoding="utf-8")
        # 读取 OpenGL texture 上传编码。
        upload = OPENGL_UPLOAD.read_text(encoding="utf-8")
        # 读取 D3D11 状态编码。
        d3d11 = D3D11_STATE.read_text(encoding="utf-8")
        # 读取 D3D11 clear 编码。
        clear = D3D11_CLEAR.read_text(encoding="utf-8")
        # 读取最终 surface damage 合成源码。
        composite = SURFACE_COMPOSITE.read_text(encoding="utf-8")
        # OpenGL surface Y 必须消费共享底部原点投影。
        self.assertIn(".bottom_origin_y(extent)", opengl)
        # OpenGL 不得自行执行未检查的目标高度减法。
        self.assertNotIn("extent.height as i32 -", opengl)
        # OpenGL texture region 必须消费共享区域原点与尺寸投影。
        self.assertIn(".native_origin_and_size_i32()", upload)
        # D3D11 状态与 clear 都必须消费相同完整矩形。
        self.assertIn(".native_rect()", d3d11)
        # 局部 clear 必须复用同一投影。
        self.assertIn(".native_rect()", clear)
        # D3D11 不得用饱和加法隐藏 scissor 溢出。
        self.assertNotIn("saturating_add", d3d11)
        # surface damage 必须复用共享 scissor 目标边界。
        self.assertIn(".fits_within(extent)", composite)
        # Drawing 边界不得饱和修正超大 extent。
        self.assertNotIn("min(i32::MAX as u32)", composite)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
