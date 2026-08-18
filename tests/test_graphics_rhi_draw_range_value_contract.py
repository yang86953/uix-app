# -*- coding: utf-8 -*-
# 验证 DrawRange 只使用 OpenGL 与 D3D11 都能无损表达的数量和起点。
"""Keep draw range values inside the shared native ABI domain."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 DrawRange 值域契约。
SHARED = ROOT / "src/native/present/rhi/draw_packet.rs"
# 定位 FramePlan 统一门禁。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 OpenGL 有符号原生命令映射。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 无符号原生命令映射。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device_draw.rs"


# 集中验证共享值域、FramePlan 和两个 Adapter 的同一门禁。
class GraphicsRhiDrawRangeValueContractTests(unittest.TestCase):
    # 共享范围必须拥有数量与起点的 checked 有符号投影。
    def test_shared_range_owns_native_value_domain(self) -> None:
        # 读取共享 DrawRange 源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 元素数量必须由共享 helper 收敛到 i32。
        self.assertIn("const fn count_i32(count: u32) -> Option<i32>", shared)
        # 零或超过 i32 上限的数量都必须被拒绝。
        self.assertIn("count == 0 || count > i32::MAX as u32", shared)
        # 首顶点必须由独立 checked helper 收敛到 i32。
        self.assertIn("const fn vertex_first_i32(first: u32) -> Option<i32>", shared)
        # 完整范围必须通过共同值域门禁。
        self.assertIn("pub(crate) const fn is_valid(self) -> bool", shared)
        # 索引起点必须继续使用格式步长计算 checked 字节偏移。
        self.assertIn("binding.format().byte_offset(first).is_some()", shared)
        # Rust 边界测试必须锁定溢出拒绝行为。
        self.assertIn("fn draw_range_rejects_native_value_overflow()", shared)

    # FramePlan 与两个 Adapter 必须复用同一个有效范围判断。
    def test_all_boundaries_share_one_range_gate(self) -> None:
        # 读取 FramePlan 门禁源码。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取 OpenGL Adapter 源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 Adapter 源码。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # FramePlan 必须在进入 Adapter 前拒绝无效范围。
        self.assertIn("!packet.has_valid_range()", frame_plan)
        # OpenGL 必须防御绕过 FramePlan 的直接 Device 调用。
        self.assertIn("!range.is_valid()", opengl)
        # D3D11 必须使用完全相同的防御门禁。
        self.assertIn("!range.is_valid()", d3d11)

    # OpenGL 不得再把 u32 draw 参数直接截断为 i32。
    def test_opengl_uses_checked_signed_projections(self) -> None:
        # 读取 OpenGL draw 编码源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 索引数量必须使用共享 checked 投影。
        self.assertIn(".index_count_i32()", opengl)
        # 顶点数量必须使用共享 checked 投影。
        self.assertIn(".vertex_count_i32()", opengl)
        # 首顶点必须使用共享 checked 投影。
        self.assertIn(".first_vertex_i32()", opengl)
        # 三个范围参数都不得继续使用未经验证的直接转换。
        self.assertNotIn("range.index_count() as i32", opengl)
        # 首顶点不得继续使用未经验证的直接转换。
        self.assertNotIn("range.first_vertex() as i32", opengl)
        # 顶点数量不得继续使用未经验证的直接转换。
        self.assertNotIn("range.vertex_count() as i32", opengl)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
