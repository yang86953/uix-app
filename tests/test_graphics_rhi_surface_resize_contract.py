# -*- coding: utf-8 -*-
# 验证 Surface resize 只能通过共享类型化事务进入 D3D11 与 OpenGL Adapter。
"""Keep Surface resize validation identical across graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口。
RHI = ROOT / "src/native/present/rhi.rs"
# 定位共享 resize 事务 Component。
TRANSACTION = ROOT / "src/native/present/rhi/resize_transaction.rs"
# 定位 D3D11 Surface Adapter。
D3D11_SURFACE = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi.rs"
# 定位 OpenGL 共享 Surface Adapter。
OPENGL_SURFACE = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 EGL 原生 host。
EGL_HOST = ROOT / "src/native/presentation/graphics/opengl/platform/egl.rs"
# 定位拆分后的 EGL RHI host 实现。
EGL_RHI_HOST = ROOT / "src/native/presentation/graphics/opengl/platform/egl_rhi.rs"
# 定位 WGL 原生 host。
WGL_HOST = ROOT / "src/native/presentation/graphics/opengl/platform/wgl_rhi.rs"


# 集中锁定跨 Adapter resize 的前置值域和成功后 token 语义。
class GraphicsRhiSurfaceResizeContractTests(unittest.TestCase):
    # Surface trait 必须保留薄输入，同时由独立事务 Component 解释动态状态。
    def test_resize_uses_one_closed_typed_transaction(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取共享事务实现。
        transaction = TRANSACTION.read_text(encoding="utf-8")
        # 组合入口必须装配独立 resize 事务 Component。
        self.assertIn("mod resize_transaction;", rhi)
        # Surface trait 仍只暴露 API 无关的 extent 和 token。
        self.assertIn(
            "fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken>;",
            rhi,
        )
        # 共享事务必须冻结旧 token 与请求 extent。
        self.assertIn("pub(crate) struct RhiSurfaceResizeTransaction", transaction)
        # 旧 token 字段不得暴露给 Adapter 改写。
        self.assertIn("    previous: SurfaceToken,", transaction)
        # 请求字段必须保持私有。
        self.assertIn("    requested: RhiExtent,", transaction)
        # 原生宽高投影必须由事务封闭保存。
        self.assertIn("    native_width: i32,", transaction)
        # 原生高度必须服从同一投影。
        self.assertIn("    native_height: i32,", transaction)

    # 共享门禁必须唯一解释无效输入与成功后的 token 约束。
    def test_shared_gate_owns_resize_preconditions_and_postconditions(self) -> None:
        # 读取共享事务实现。
        transaction = TRANSACTION.read_text(encoding="utf-8")
        # 非法 extent 必须统一映射为参数错误。
        self.assertIn("Errc::InvalidArgument", transaction)
        # 前置诊断必须保持 API 无关。
        self.assertIn('"RHI surface resize extent is invalid"', transaction)
        # 成功结果必须精确匹配请求 extent。
        self.assertIn("if current.extent != self.requested", transaction)
        # generation 回退必须被拒绝。
        self.assertIn("if current.generation < self.previous.generation", transaction)
        # 物理尺寸变化必须推进 generation。
        self.assertIn("&& current.generation == self.previous.generation", transaction)
        # 后置条件失败必须统一归类为平台实现错误。
        self.assertGreaterEqual(transaction.count("Errc::PlatformError"), 3)

    # D3D11 与 OpenGL Surface 入口必须调用同一前置和后置门禁。
    def test_surface_adapters_share_the_same_resize_gate(self) -> None:
        # 读取 D3D11 Surface Adapter。
        d3d11 = D3D11_SURFACE.read_text(encoding="utf-8")
        # 读取 OpenGL Surface Adapter。
        opengl = OPENGL_SURFACE.read_text(encoding="utf-8")
        # 两个入口都必须创建共享 resize 事务。
        self.assertIn("RhiSurfaceResizeTransaction::validate(extent, self.token())?", d3d11)
        # OpenGL blanket 实现必须使用完全相同的门禁调用。
        self.assertIn("RhiSurfaceResizeTransaction::validate(extent, self.token())?", opengl)
        # D3D11 必须在 DXGI helper 成功后执行共享后置验证。
        self.assertIn("resize.complete(self.token())", d3d11)
        # OpenGL 必须在 EGL 或 WGL host 成功后执行共享后置验证。
        self.assertIn("resize.complete(self.token())", opengl)
        # D3D11 不得保留平台私有的非法 extent 诊断。
        self.assertNotIn("D3d11 RHI surface extent is invalid", d3d11)
        # OpenGL 不得保留平台私有的非法 extent 诊断。
        self.assertNotIn("OpenGL RHI surface extent", opengl)

    # EGL 与 WGL 只能机械消费已验证事务，不得重新解释无符号输入。
    def test_opengl_hosts_only_consume_validated_resize(self) -> None:
        # 读取 OpenGL host trait。
        opengl = OPENGL_SURFACE.read_text(encoding="utf-8")
        # 读取 EGL owner 与拆分后的 RHI host 实现。
        egl = EGL_HOST.read_text(encoding="utf-8") + EGL_RHI_HOST.read_text(encoding="utf-8")
        # 读取 WGL 实现。
        wgl = WGL_HOST.read_text(encoding="utf-8")
        # host trait 必须接收封闭事务而非裸 extent。
        self.assertIn(
            "fn rhi_resize_surface(&mut self, resize: RhiSurfaceResizeTransaction)",
            opengl,
        )
        # EGL 必须从事务读取唯一 extent。
        self.assertIn("let extent = resize.extent();", egl)
        # EGL 的原生宽高必须来自事务投影。
        self.assertIn("let (width, height) = resize.native_size_i32();", egl)
        # WGL 必须从同一事务读取 extent。
        self.assertIn("let extent = resize.extent();", wgl)
        # EGL 不得保留第二套无效尺寸错误文本。
        self.assertNotIn("EGL RHI surface extent is invalid", egl)
        # WGL 不得接收裸 RhiExtent resize 参数。
        self.assertNotIn("fn rhi_resize_surface(&mut self, extent: RhiExtent)", wgl)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
