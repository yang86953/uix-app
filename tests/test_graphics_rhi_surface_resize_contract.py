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
RHI = ROOT / "src/platform/presentation/rhi/mod.rs"
# 定位共享 resize 事务 Component。
TRANSACTION = ROOT / "src/platform/presentation/rhi/resize_transaction.rs"
# 定位 D3D11 Surface Adapter。
D3D11_SURFACE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs"
# 定位 D3D11 context 的唯一状态 owner 与原生方法 Adapter。
D3D11_CONTEXT = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/mod.rs"
D3D11_METHODS = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/methods.rs"
# 定位 D3D11 HRESULT 分类边界。
D3D11_SWAPCHAIN = ROOT / "src/native/presentation/graphics/d3d11/adapter/swapchain.rs"
# 定位 OpenGL 共享 Surface Adapter。
OPENGL_SURFACE = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 EGL 原生 host。
EGL_HOST = ROOT / "src/native/presentation/graphics/opengl/adapter/egl.rs"
# 定位拆分后的 EGL RHI host 实现。
EGL_RHI_HOST = ROOT / "src/native/presentation/graphics/opengl/adapter/egl_rhi.rs"
# 定位 WGL 原生 host。
WGL_HOST = ROOT / "src/native/presentation/graphics/opengl/adapter/wgl_rhi.rs"
# 定位 WGL 原生 owner。
WGL_NATIVE = ROOT / "src/native/presentation/graphics/opengl/adapter/wgl.rs"
# 定位窗口与恢复层已有的零尺寸唯一 owner。
WINDOW_DRIVER = ROOT / "src/app/window/window_driver/driver.rs"
RECOVERY_DRIVER = ROOT / "src/draw/renderer/recovery_driver.rs"


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

    # EGL 与 WGL 只能机械消费共享重建事务，不得拥有第二份代际状态。
    def test_opengl_hosts_only_consume_shared_lifecycle_transaction(self) -> None:
        # 读取 OpenGL host trait。
        opengl = OPENGL_SURFACE.read_text(encoding="utf-8")
        # 读取 EGL owner 与拆分后的 RHI host 实现。
        egl = EGL_HOST.read_text(encoding="utf-8") + EGL_RHI_HOST.read_text(encoding="utf-8")
        # 读取 WGL 实现。
        wgl = WGL_HOST.read_text(encoding="utf-8") + WGL_NATIVE.read_text(
            encoding="utf-8"
        )
        # host trait 必须接收共享生命周期签发的封闭重建事务。
        self.assertIn(
            "recreate: RhiSurfaceRecreateTransaction",
            opengl,
        )
        # OpenGL blanket 必须由共享生命周期开启、提交或回滚事务。
        self.assertIn(".begin_recreate(requested, reason)?", opengl)
        self.assertIn(".commit_recreate(transaction, actual)", opengl)
        self.assertIn(".abort_recreate(transaction)", opengl)
        # EGL 的原生宽高必须来自共享重建事务投影。
        self.assertIn("let (width, height) = recreate.native_size_i32();", egl)
        # WGL 必须从同一封闭事务读取 extent。
        self.assertIn("let extent = recreate.requested();", wgl)
        # 两个平台不得再保存或推进私有 Surface generation。
        self.assertNotIn("surface_generation", egl)
        self.assertNotIn("surface_generation", wgl)
        # Surface token 必须直接来自共享生命周期。
        self.assertIn("self.rhi_surface_lifecycle().token()", opengl)

    # D3D11 必须删除私有 generation/extent，并由共享事务唯一发布 token。
    def test_d3d11_context_owns_only_one_shared_surface_lifecycle(self) -> None:
        context = D3D11_CONTEXT.read_text(encoding="utf-8")
        surface = D3D11_SURFACE.read_text(encoding="utf-8")
        graphics = (
            ROOT
            / "src/native/presentation/graphics/d3d11/adapter/context/graphics.rs"
        ).read_text(encoding="utf-8")

        self.assertIn("surface_lifecycle: RhiSurfaceLifecycle", context)
        self.assertNotIn("surface_generation", context)
        self.assertNotIn("    width: i32,", context)
        self.assertNotIn("    height: i32,", context)
        self.assertNotIn("surface_generation", surface)
        self.assertIn("self.surface_lifecycle.token()", surface)
        self.assertIn("let token = self.surface_lifecycle.token();", graphics)

    # D3D11 编排层必须用 begin/commit/abort 包住机械 DXGI/RTV 动作。
    def test_d3d11_recreate_sequence_is_owned_by_shared_transaction(self) -> None:
        surface = D3D11_SURFACE.read_text(encoding="utf-8")
        methods = D3D11_METHODS.read_text(encoding="utf-8")

        run_start = surface.index("fn run_surface_recreate(")
        run_end = surface.index("fn recover_rejected_frame(", run_start)
        recreate = surface[run_start:run_end]
        self.assertLess(
            recreate.index(".begin_recreate(requested, reason)?"),
            recreate.index("self.recreate_surface_native("),
        )
        self.assertLess(
            recreate.index("self.recreate_surface_native("),
            recreate.index(".commit_recreate(transaction, actual)"),
        )
        self.assertIn(".abort_recreate(transaction)", recreate)
        self.assertIn("recreate: RhiSurfaceRecreateTransaction", methods)
        self.assertIn("let (physical_width, physical_height) = recreate.native_size_i32();", methods)
        self.assertIn(".resize_buffers(physical_width as u32, physical_height as u32)", methods)
        self.assertIn("self.create_rtv_for_extent(previous)", methods)
        self.assertNotIn("RhiSurfaceLifecycle", methods[methods.index("pub(super) fn recreate_surface_native(") : methods.index("pub(super) fn release_rtv(")])
        native_recreate = methods[methods.index("pub(super) fn recreate_surface_native(") : methods.index("pub(super) fn release_rtv(")]
        self.assertNotIn(".generation", native_recreate)
        self.assertNotIn("saturating_add", native_recreate)
        initialize = methods[methods.index("pub(crate) fn create_with_driver(") :]
        self.assertLess(
            initialize.index("RhiSurfaceRecreateReason::Initialize"),
            initialize.index("D3D11CreateDevice("),
        )
        self.assertIn("surface_lifecycle.abort_recreate(surface_initialize)", initialize)
        self.assertIn(".commit_recreate(surface_initialize, initial_extent)?", initialize)

    # SurfaceLost 只做一次同尺寸重建，DeviceLost 保持既有设备级恢复。
    def test_d3d11_surface_recovery_stays_bounded_and_typed(self) -> None:
        surface = D3D11_SURFACE.read_text(encoding="utf-8")
        swapchain = D3D11_SWAPCHAIN.read_text(encoding="utf-8")

        self.assertIn("fn recover_rejected_frame(", surface)
        self.assertIn("self.surface_lifecycle.token().extent", surface)
        self.assertIn("Err(error) if error.code() == Errc::GraphicsSurfaceLost", surface)
        self.assertIn("Errc::GraphicsDeviceLost", swapchain)
        self.assertIn("Errc::GraphicsSurfaceLost", swapchain)
        self.assertNotIn("GraphicsDeviceLost => self.recover_rejected_frame", surface)

    # 零尺寸暂停沿用 WindowDriver/RecoveryDriver，不在 D3D11 中创建替代 Surface。
    def test_zero_extent_owner_remains_window_and_recovery_driver(self) -> None:
        window = WINDOW_DRIVER.read_text(encoding="utf-8")
        recovery = RECOVERY_DRIVER.read_text(encoding="utf-8")
        surface = D3D11_SURFACE.read_text(encoding="utf-8")

        self.assertIn(".suspend(SurfaceSuspendReason::ZeroExtent)", window)
        self.assertIn("if width <= 0 || height <= 0", recovery)
        self.assertIn("return Ok(());", recovery)
        self.assertNotIn("RhiExtent::new(1, 1)", surface)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
