# -*- coding: utf-8 -*-
# 验证 Surface 与 texture render target 使用封闭类型而非跨 Adapter 裸值哨兵。
"""Keep render target identity typed across FramePlan and native adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口。
RHI = ROOT / "src/platform/presentation/rhi/mod.rs"
# 定位封闭 render target Component。
TARGET = ROOT / "src/platform/presentation/rhi/render_target.rs"
# 定位共享 texture 资源表。
TEXTURE_TABLE = ROOT / "src/platform/presentation/rhi/texture_resource_table.rs"
# 定位 acquired Surface frame 契约。
PRESENT = ROOT / "src/platform/presentation/rhi/present_transaction.rs"
# 定位共享 pass 状态机。
PASS_STATE = ROOT / "src/platform/presentation/rhi/pass_state.rs"
# 定位类型化 FramePlan。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 Renderer 封闭 Surface/Offscreen 帧。
RENDERER_FRAME = ROOT / "src/draw/backend/rhi_renderer_execution.rs"
# 定位 D3D11 Device Adapter。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位 D3D11 context 组合入口。
D3D11_CONTEXT = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/mod.rs"
# 定位 OpenGL Device Adapter。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL raster bridge。
OPENGL_RASTER = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL host bridge。
OPENGL_HOST = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"


# 集中锁定目标种类、FramePlan 投影和 Adapter 解析的唯一类型边界。
class GraphicsRhiRenderTargetContractTests(unittest.TestCase):
    # 共享值对象必须用互斥变体隔离 Surface 与 texture 资源值域。
    def test_render_target_is_a_closed_sum_type(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取封闭目标实现。
        target = TARGET.read_text(encoding="utf-8")
        # 组合入口必须装配独立 RenderTarget Component。
        self.assertIn("mod render_target;", rhi)
        # 目标身份必须是封闭枚举而非裸整数 tuple struct。
        self.assertIn("pub(crate) enum RenderTargetHandle", target)
        # Surface 必须是无载荷独立变体。
        self.assertIn("    Surface,", target)
        # Texture 变体必须直接携带类型化资源句柄。
        self.assertIn("    Texture(TextureHandle),", target)
        # 共享构造器必须提供明确 Surface 身份。
        self.assertIn("pub(crate) const fn surface() -> Self", target)
        # texture 提升必须接收共享 TextureDesc 并返回可失败结果。
        self.assertIn(
            "pub(super) fn for_texture(texture: TextureHandle, desc: TextureDesc) -> Result<Self>",
            target,
        )
        # 测试 fixture 必须使用显式测试入口。
        self.assertIn("pub(crate) const fn for_test(texture: TextureHandle) -> Self", target)
        # 目标 Component 不得恢复裸值构造入口。
        self.assertNotIn("pub(crate) const fn from_raw", target)
        # 目标 Component 不得定义跨资源值域的 raw 投影。
        self.assertNotIn("fn raw", target)

    # SurfaceFrame 与 FramePlan 必须分别只能表达自己的目标角色。
    def test_surface_and_frame_plan_keep_target_roles_typed(self) -> None:
        # 读取 acquired Surface frame 契约。
        present = PRESENT.read_text(encoding="utf-8")
        # 读取 FramePlan 目标定义。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取 Renderer 封闭执行帧。
        renderer_frame = RENDERER_FRAME.read_text(encoding="utf-8")
        # SurfaceFrame 不得保存调用方可替换的 target 字段。
        self.assertNotIn("    target: RenderTargetHandle,", present)
        # SurfaceFrame 的目标投影必须固定为 Surface 变体。
        self.assertIn("RenderTargetHandle::surface()", present)
        # FramePlan 的 Texture 变体必须直接保存 TextureHandle。
        self.assertIn("Texture(TextureHandle)", frame_plan)
        # FramePlan 不得再保存可表示 Surface 的泛化 handle。
        self.assertNotIn("Texture(RenderTargetHandle)", frame_plan)
        # Renderer Offscreen 变体必须直接拥有 TextureHandle。
        self.assertIn("target: TextureHandle", renderer_frame)
        # 唯一 Device 边界负责把 texture 提升为 render target。
        execution = (
            # 定位 FramePlan 执行器。
            ROOT / "src/draw/backend/frame_plan_execution.rs"
            # 读取当前源码。
        ).read_text(encoding="utf-8")
        # FramePlan 执行器必须先向 Device 查询已验证目标身份。
        self.assertIn("self.device.resolve_render_target(texture)?", execution)
        # 目标校验必须先于任何 owner-context 激活。
        self.assertLess(
            execution.index("self.validate_targets(steps)?"),
            execution.index("self.device.activate()?"),
        )
        # 执行目标解析也必须复用 Device 的共享能力入口。
        self.assertIn("self.device.resolve_render_target(texture)", execution)
        # 共享资源表必须在真实 TextureDesc 上执行目标能力提升。
        resource_table = TEXTURE_TABLE.read_text(encoding="utf-8")
        self.assertIn("pub(crate) trait RhiTextureResource", resource_table)
        self.assertIn("resolve_render_target", resource_table)
        self.assertIn("resource.desc()", resource_table)
        # GraphicsDevice 必须声明目标解析而非让 FramePlan 构造裸目标。
        rhi = RHI.read_text(encoding="utf-8")
        self.assertIn("fn resolve_render_target(&self, _texture: TextureHandle)", rhi)

    # 共享 pass 状态必须通过类型投影判断反馈环。
    def test_pass_state_compares_texture_identity_without_raw_values(self) -> None:
        # 读取共享 pass 状态机。
        pass_state = PASS_STATE.read_text(encoding="utf-8")
        # 输出目标与 sampled source 必须比较同一 TextureHandle。
        self.assertIn("active.target.texture() == Some(binding.texture())", pass_state)
        # 资源销毁保护也必须使用同一类型投影。
        self.assertIn("active.target.texture() == Some(texture)", pass_state)
        # 共享 pass 不得再比较 target 与 texture 的裸整数。
        self.assertNotIn("target.raw()", pass_state)

    # 两个 Adapter 必须机械解析封闭目标，不得保留 Surface 哨兵。
    def test_adapters_do_not_define_or_decode_surface_sentinels(self) -> None:
        # 读取 D3D11 Adapter 与组合入口。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 读取 D3D11 context 模块。
        d3d11_context = D3D11_CONTEXT.read_text(encoding="utf-8")
        # 读取 OpenGL Adapter 与 host。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 OpenGL raster bridge。
        opengl_raster = OPENGL_RASTER.read_text(encoding="utf-8")
        # 读取 OpenGL host bridge。
        opengl_host = OPENGL_HOST.read_text(encoding="utf-8")
        # 两个 Adapter 和 host 都不得定义旧 Surface 裸值常量。
        self.assertNotIn(
            "RHI_SURFACE_TARGET_RAW",
            d3d11 + d3d11_context + opengl + opengl_host,
        )
        # D3D11 必须显式读取目标种类。
        self.assertIn("if target.is_surface()", d3d11)
        # D3D11 texture 分支必须使用类型化投影。
        self.assertIn("let texture_handle = target", d3d11)
        # OpenGL 必须按 texture 投影匹配默认 framebuffer 或资源 framebuffer。
        self.assertIn("match target.texture()", opengl)
        # OpenGL 坐标方向也必须读取同一目标种类。
        self.assertIn("if target.is_surface()", opengl)
        # 两个 Adapter 不得反向构造 TextureHandle。
        self.assertNotIn("TextureHandle::from_raw(target", d3d11 + opengl)
        # 两个 Adapter 不得读取 render target 裸值。
        self.assertNotIn("target.raw()", d3d11 + opengl)

    # 两个 Adapter 必须复用共享 texture 资源表签发目标身份。
    def test_adapters_delegate_render_target_capability_to_shared_table(self) -> None:
        # 读取 D3D11 Adapter。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 读取 OpenGL Device Adapter。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 OpenGL raster bridge。
        opengl_raster = OPENGL_RASTER.read_text(encoding="utf-8")
        # 读取 OpenGL host bridge。
        opengl_host = OPENGL_HOST.read_text(encoding="utf-8")
        # 两个 Adapter 都必须使用唯一的类型化 texture 资源表。
        self.assertIn("RhiTextureResourceTable<D3d11RhiTexture>", d3d11)
        # OpenGL 也不得保留平行 texture 资源表。
        self.assertIn("RhiTextureResourceTable<OpenGlRhiTexture>", opengl)
        # 两个原生 texture 资源都必须投影冻结的共享描述。
        self.assertIn("impl RhiTextureResource for D3d11RhiTexture", d3d11)
        # OpenGL 资源必须满足相同的共享描述契约。
        self.assertIn("impl RhiTextureResource for OpenGlRhiTexture", opengl)
        # D3D11 resolver 必须直接委托其共享表。
        self.assertIn("self.rhi_device.textures.resolve_render_target(texture)", d3d11)
        # OpenGL Device resolver 也必须直接委托共享表。
        self.assertIn("self.textures.resolve_render_target(texture)", opengl)
        # OpenGL raster bridge 必须保持只读 resolver。
        self.assertIn("pub(crate) fn rhi_resolve_render_target", opengl_raster)
        # OpenGL host 必须先检查 owner 生命周期。
        resolver = opengl_host.split("fn resolve_render_target", maxsplit=1)[1]
        # 截取 resolver 函数，避免后续有副作用的方法干扰断言。
        resolver = resolver.split("    }", maxsplit=1)[0]
        # resolver 必须拒绝已关闭 owner。
        self.assertIn("self.rhi_ensure_active()?", resolver)
        # resolver 只能经过只读 raster bridge。
        self.assertIn("self.rhi_pipeline().rhi_resolve_render_target(texture)", resolver)
        # 目标能力查询不得隐式激活原生 OpenGL context。
        self.assertNotIn("rhi_make_current", resolver)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
