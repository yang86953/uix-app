# -*- coding: utf-8 -*-
# 验证 OpenGL 原生资源在插入共享资源表前失败时由唯一创建事务完整回滚。
"""Keep pre-registration OpenGL resource failures leak-free."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 OpenGL RHI Device 创建事务。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL framebuffer helper。
OPENGL_RESOURCES = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_resources.rs"


# 集中锁定 texture 与 framebuffer 在登记前的单一 owner 和逆序回滚。
class GraphicsRhiResourceCreationRollbackContractTests(unittest.TestCase):
    # framebuffer 失败时必须先释放未登记 texture 再返回原始错误。
    def test_texture_creation_rolls_back_before_resource_table_insert(self) -> None:
        # 读取 OpenGL Device 创建实现。
        device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 截取 texture 创建事务，避免销毁路径的删除调用干扰断言。
        creation = device.split("pub(super) fn create_texture", maxsplit=1)[1].split(
            "pub(super) fn create_sampler",
            maxsplit=1,
        )[0]
        # framebuffer 创建必须由显式 match 拥有失败分支。
        self.assertIn("match Self::create_framebuffer(gl, native)", creation)
        # 失败分支必须取得原始错误。
        self.assertIn("Err(error) =>", creation)
        # 尚未登记的 texture 必须由当前事务删除。
        self.assertIn("gl.delete_texture(native);", creation)
        # 删除后必须原样返回 framebuffer 错误。
        self.assertIn("return Err(error);", creation)
        # 删除必须发生在资源表 insert 之前。
        self.assertLess(
            creation.index("gl.delete_texture(native);"),
            creation.index("self.textures.insert"),
        )
        # 旧的问号直返不得重新引入泄漏路径。
        self.assertNotIn("Some(Self::create_framebuffer(gl, native)?)", creation)

    # framebuffer helper 必须只回滚自己创建的对象，避免 texture 重复删除。
    def test_framebuffer_helper_keeps_its_own_rollback_boundary(self) -> None:
        # 读取 framebuffer helper。
        resources = OPENGL_RESOURCES.read_text(encoding="utf-8")
        # 不完整 framebuffer 必须由 helper 自己删除。
        self.assertIn("gl.delete_framebuffer(framebuffer);", resources)
        # helper 借用 texture，不得抢夺并删除调用方 owner。
        self.assertNotIn("delete_texture", resources)
        # helper 必须保留稳定平台错误分类。
        self.assertIn("Errc::PlatformError", resources)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
