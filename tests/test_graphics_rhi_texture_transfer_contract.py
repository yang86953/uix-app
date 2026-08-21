# -*- coding: utf-8 -*-
# 验证上传、复制与移动只通过共享类型化区域进入两个图形 Adapter。
"""Keep texture transfer regions atomic across native graphics adapters."""

# 启用延迟注解解析，保持契约脚本风格一致。
from __future__ import annotations

# 引入正则表达式以拒绝旧松散字段。
import re
# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口。
RHI = ROOT / "src/platform/presentation/rhi/mod.rs"
# 定位共享纹理区域与传输 Component。
TRANSFER = ROOT / "src/platform/presentation/rhi/transfer.rs"
# 定位 OpenGL 子区域上传 Adapter。
OPENGL_UPLOAD = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_upload.rs"
# 定位 OpenGL 普通复制 Adapter。
OPENGL_COPY = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL 只读 preflight bridge。
OPENGL_RASTER = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL GraphicsDevice host。
OPENGL_HOST = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 OpenGL 重叠安全移动 Adapter。
OPENGL_MOVE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_copy.rs"
# 定位 D3D11 上传、复制与移动 Adapter。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位 FramePlan 的类型化传输门禁。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 FramePlan 到 GraphicsDevice 的唯一执行器。
FRAME_PLAN_EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位共享 texture 资源表与真实描述预检。
TEXTURE_TABLE = ROOT / "src/platform/presentation/rhi/texture_resource_table.rs"


# 集中锁定纹理传输几何的所有权与跨后端机械映射。
class GraphicsRhiTextureTransferContractTests(unittest.TestCase):
    # RHI 必须用区域与传输值对象替代命令中的松散坐标字段。
    def test_rhi_owns_atomic_region_and_transfer_types(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取共享传输 Component。
        transfer = TRANSFER.read_text(encoding="utf-8")
        # 组合入口必须重导出类型化上传区域。
        self.assertIn("RhiTextureRegion, RhiTextureRegionBounds", rhi)
        # 组合入口必须重导出类型化传输几何。
        self.assertIn("RhiTextureTransfer,", rhi)
        # 纹理上传 trait 只能接收绑定资源、区域与载荷的完整命令。
        self.assertIn("_upload: RhiTextureUpload<'_>", rhi)
        # 旧的横纵偏移参数不得留在 trait。
        self.assertNotIn("_destination_x: u32", rhi)
        # 共享 Component 必须定义类型化原点。
        self.assertIn("pub(crate) struct RhiTextureOrigin", transfer)
        # 共享 Component 必须定义不可拆区域。
        self.assertIn("pub(crate) struct RhiTextureRegion", transfer)
        # 共享 Component 必须定义唯一传输尺寸。
        self.assertIn("pub(crate) struct RhiTextureTransfer", transfer)
        # 复制命令必须持有完整传输对象。
        self.assertEqual(transfer.count("transfer: RhiTextureTransfer"), 4)
        # 旧的公开松散坐标字段不得重新出现。
        self.assertNotIn("pub(crate) source_x:", transfer)
        # 旧的公开宽高字段不得重新出现。
        self.assertNotIn("pub(crate) width:", transfer)

    # 上传路径必须共享边界与紧密载荷布局算法。
    def test_upload_adapters_consume_shared_region_bounds(self) -> None:
        # 读取共享传输 Component。
        transfer = TRANSFER.read_text(encoding="utf-8")
        # 读取 OpenGL 上传实现。
        opengl = OPENGL_UPLOAD.read_text(encoding="utf-8")
        # 读取 D3D11 上传实现。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # checked 远端边界必须只由共享区域拥有。
        self.assertIn(".checked_add(self.extent.width)", transfer)
        # 紧密载荷布局也必须由共享已验证区域拥有。
        self.assertIn("pub(crate) fn tight_payload_layout(", transfer)
        # OpenGL 必须先用资源描述验证完整上传命令。
        self.assertIn("upload.validate(texture.desc)?", opengl)
        # OpenGL 必须消费共享有符号投影。
        self.assertIn("bounds.native_origin_and_size_i32()", opengl)
        # OpenGL 不得维护私有区域加法。
        self.assertNotIn("checked_add", opengl)
        # D3D11 必须先用资源描述验证同一种完整上传命令。
        self.assertIn("upload.validate(resource.desc)?", d3d11)
        # D3D11 必须消费共享无符号矩形。
        self.assertIn("bounds.native_rect_u32()", d3d11)
        # D3D11 不得维护私有上传区域加法。
        self.assertNotIn("destination_x.checked_add", d3d11)

    # 普通复制与重叠移动必须机械消费同一个传输事实。
    def test_copy_and_move_adapters_share_typed_transfer(self) -> None:
        # 读取 OpenGL 普通复制实现。
        opengl_copy = OPENGL_COPY.read_text(encoding="utf-8")
        # 读取 OpenGL 移动实现。
        opengl_move = OPENGL_MOVE.read_text(encoding="utf-8")
        # 读取 OpenGL 只读 preflight bridge。
        opengl_raster = OPENGL_RASTER.read_text(encoding="utf-8")
        # 读取 OpenGL GraphicsDevice host。
        opengl_host = OPENGL_HOST.read_text(encoding="utf-8")
        # 读取 D3D11 复制与移动实现。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 读取 FramePlan 验证实现。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取 FramePlan 的 activate 前传输预检执行器。
        execution = FRAME_PLAN_EXECUTION.read_text(encoding="utf-8")
        # 读取共享 texture 资源表。
        texture_table = TEXTURE_TABLE.read_text(encoding="utf-8")
        # 读取薄 RHI 的只读 preflight 入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 两个普通复制 Adapter 都必须消费共享验证边界。
        self.assertIn("let bounds = copy.validate_transfer", opengl_copy)
        # D3D11 普通复制必须消费同一个验证入口。
        self.assertIn("let bounds = copy.validate_transfer", d3d11)
        # 两个重叠移动 Adapter 都必须调用共享 scratch 拆分。
        self.assertIn("movement.through_scratch(scratch)", opengl_move)
        # D3D11 必须执行相同的 scratch 拆分。
        self.assertIn("movement.through_scratch(scratch)", d3d11)
        # FramePlan 只能从完整传输对象读取唯一尺寸。
        self.assertEqual(frame_plan.count("transfer().extent().is_valid()"), 2)
        # 共享 texture 资源表必须先解析真实描述再调用 copy/move 校验。
        self.assertIn("pub(crate) fn validate_copy(&self, copy: TextureCopy)", texture_table)
        self.assertIn("pub(crate) fn validate_move(&self, movement: TextureMove)", texture_table)
        self.assertIn("copy.validate_transfer(source, destination)", texture_table)
        self.assertIn("movement.validate_transfer(source, destination)", texture_table)
        # GraphicsDevice 必须提供两个只读传输 preflight 入口。
        self.assertIn("fn preflight_texture_copy(&self, _copy: TextureCopy)", rhi)
        self.assertIn("fn preflight_texture_move(&self, _movement: TextureMove)", rhi)
        # 执行器必须在 activate 前调用独立的传输验证阶段。
        self.assertIn("self.validate_transfers(steps)?;", execution)
        self.assertIn("self.device.preflight_texture_copy(*copy)?;", execution)
        self.assertIn("self.device.preflight_texture_move(*movement)?;", execution)
        # 两类资源 preflight 都必须早于 Device 激活。
        self.assertLess(
            execution.index("self.validate_transfers(steps)?;"),
            execution.index("self.device.activate()?;"),
        )
        # OpenGL Device 必须把两种 preflight 机械委托给共享资源表。
        self.assertIn("self.textures.validate_copy(copy)", opengl_copy)
        self.assertIn("self.textures.validate_move(movement)", opengl_copy)
        # D3D11 Device 必须委托同一个共享资源表。
        self.assertIn("self.rhi_device.textures.validate_copy(copy)", d3d11)
        self.assertIn("self.rhi_device.textures.validate_move(movement)", d3d11)
        # OpenGL raster bridge 必须保持两个只读入口。
        self.assertIn("pub(crate) fn rhi_preflight_texture_copy(&self", opengl_raster)
        self.assertIn("pub(crate) fn rhi_preflight_texture_move(&self", opengl_raster)
        # OpenGL host 必须先检查 owner，再只读访问 pipeline。
        copy_preflight = opengl_host.split("fn preflight_texture_copy", maxsplit=1)[1]
        # 截取 copy preflight 函数体。
        copy_preflight = copy_preflight.split("    }", maxsplit=1)[0]
        # copy preflight 不得恢复 native context。
        self.assertIn("self.rhi_ensure_active()?", copy_preflight)
        self.assertIn("self.rhi_pipeline().rhi_preflight_texture_copy(copy)", copy_preflight)
        self.assertNotIn("rhi_make_current", copy_preflight)
        # move preflight 使用独立的只读函数体。
        move_preflight = opengl_host.split("fn preflight_texture_move", maxsplit=1)[1]
        # 截取 move preflight 函数体。
        move_preflight = move_preflight.split("    }", maxsplit=1)[0]
        # move preflight 同样不得触碰 native context。
        self.assertIn("self.rhi_ensure_active()?", move_preflight)
        self.assertIn("self.rhi_pipeline().rhi_preflight_texture_move(movement)", move_preflight)
        self.assertNotIn("rhi_make_current", move_preflight)
        # Adapter 中不得再次读取旧的松散传输字段。
        old_fields = re.compile(
            # 匹配 copy 或 movement 后的任意旧坐标和尺寸字段。
            r"\b(?:copy|movement)\.(?:source_x|source_y|destination_x|destination_y|width|height)\b"
        )
        # 三个 Adapter 源码都必须没有旧字段访问。
        self.assertIsNone(old_fields.search(opengl_copy + opengl_move + d3d11))


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
