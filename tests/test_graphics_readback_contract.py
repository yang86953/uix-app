#!/usr/bin/env python3
"""锁定跨 Adapter surface 回读格式、时序与公开验收入口。"""

# 引入标准单元测试框架。
import unittest
# 引入资产哈希算法，锁定跨平台字体输入。
import hashlib
# 引入稳定的仓库路径读取类型。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]


# 覆盖 Drawing、RHI、Renderer 与演示验收之间的单向契约。
class GraphicsReadbackContractTests(unittest.TestCase):
    # 主演示必须在 Application 组合根安装同一正文与 CJK 字体资产。
    def test_main_demo_bundles_one_deterministic_cjk_font(self) -> None:
        # 读取主演示唯一运行组合根。
        demo = (ROOT / "demo/uix-lang-demo/src/main.rs").read_text(encoding="utf-8")
        # 定位随应用分发的单一 SC 字体资产。
        font = ROOT / "assets/fonts/NotoSansCJKsc-Regular.otf"
        # 所有启动模式必须在窗口创建前经过统一字体配置函数。
        self.assertIn("let app = with_deterministic_fonts(app);", demo)
        # 组合根必须使用公开 FontBundle，而不是平台字体路径。
        self.assertIn('FontBundle::new(', demo)
        # 编译期资产路径必须指向仓库内同一文件。
        self.assertIn('include_bytes!("../../../assets/fonts/NotoSansCJKsc-Regular.otf")', demo)
        # 资产必须存在且非空，避免 include 路径被空占位替代。
        self.assertGreater(font.stat().st_size, 0)
        # 固定官方提交对应的字节哈希，禁止依赖同名但度量不同的字体版本。
        self.assertEqual(
            # 对完整二进制资产计算发布哈希。
            hashlib.sha256(font.read_bytes()).hexdigest(),
            # Noto CJK 官方提交 f8d1575 中 SC Regular OTF 的规范哈希。
            "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b",
        )

    # 校验原生 Adapter 只能返回统一范围与像素格式。
    def test_rhi_readback_normalizes_region_rows_and_channels(self) -> None:
        # 读取薄 RHI 共享结果契约。
        rhi = (ROOT / "src/native/presentation/rhi/mod.rs").read_text(encoding="utf-8")
        # 读取 OpenGL 私有通道与行序适配。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/pipeline2.rs").read_text(encoding="utf-8")
        # 读取 D3D11 surface Adapter。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi.rs").read_text(encoding="utf-8")
        # 共享结果必须显式保存请求区域与规范像素。
        self.assertIn("struct RhiSurfaceReadback", rhi)
        # 所有 Adapter 必须先执行同一范围验证。
        self.assertIn("RhiSurfaceReadback::validate_region(region, self.token().extent)?", d3d11)
        # OpenGL 固定 RGBA 字节必须显式转换为数值 AARRGGBB。
        self.assertIn("normalize_rgba_readback_pixels(&mut pixels)", opengl)
        # OpenGL 底部原点行序必须转换成顶部原点。
        self.assertIn("reverse_readback_rows(&mut pixels", opengl)

    # 校验回读严格发生在最终 composite submit 与 surface present 之间。
    def test_final_frame_readback_runs_between_submit_and_present(self) -> None:
        # 读取唯一 FramePlan surface 执行事务。
        plan = (ROOT / "src/draw/backend/frame_plan_execution.rs").read_text(encoding="utf-8")
        # 读取 retained texture 到 swapchain 的最终合成边界。
        submit = (ROOT / "src/draw/backend/gpu/backend/rhi_surface_final.rs").read_text(encoding="utf-8")
        # 截取带观察钩子的 FramePlan 方法。
        method = plan[
            # 从新事务入口开始。
            plan.index("pub(crate) fn execute_on_context_with_before_present(") :
            # 到后续 offscreen 执行入口前结束。
            plan.index("pub(crate) fn execute_offscreen_on_device")
        ]
        # 定位唯一 Device submit。
        submit_index = method.index("let submission = context.device().submit()?")
        # 只在 submit 之后定位观察钩子调用，排除函数名和参数声明。
        hook_index = method.index("\n        before_present(", submit_index)
        # device submit 必须先于观察钩子。
        self.assertLess(submit_index, hook_index)
        # 观察钩子必须先于最终 surface present。
        self.assertLess(hook_index, method.index(".present(RhiPresentTransaction::new"))
        # 最终 retained composite 必须使用该精确钩子入口。
        self.assertIn("execute_sampled_quads_with_present_hook", submit)
        # 回读钩子必须只消费窄 Surface 角色，而非重新取得组合 context。
        self.assertIn("GpuBackend::try_readback(surface)", submit)

    # 校验应用只取得 Drawing 快照票据，不接触任何原生图形对象。
    def test_public_test_port_and_demo_assert_all_header_edges(self) -> None:
        # 读取公开应用句柄测试入口。
        handle = (ROOT / "src/app/application/app_handle.rs").read_text(encoding="utf-8")
        # 读取规范 Drawing 快照契约。
        contract = (ROOT / "src/draw/backend/contract.rs").read_text(encoding="utf-8")
        # 读取真实主演示像素验收。
        demo = (ROOT / "demo/uix-lang-demo/src/graphics_readback.rs").read_text(encoding="utf-8")
        # AppHandle 只能返回一次性票据。
        self.assertIn("pub fn request_surface_readback_for_test(&self) -> Result<SurfaceReadbackTicket>", handle)
        # Drawing 快照必须公开固定的 AARRGGBB 语义。
        self.assertIn("pub struct SurfaceReadback", contract)
        # 演示验收必须覆盖四条边，不能只抽样顶部。
        for edge in ('("top",', '("bottom",', '("left",', '("right",'):
            # 每个结构样本都必须存在。
            self.assertIn(edge, demo)
        # 专用运行入口必须提供机器可识别的成功标记。
        self.assertIn("UIX_GRAPHICS_READBACK_OK", demo)
        # 同一真实 surface 验收必须覆盖固定字体的 glyph coverage 路径。
        self.assertIn("glyph-coverage", demo)
        # 成功证据必须输出可由真实 Windows 与 Linux 比较的文本区域哈希。
        self.assertIn("text_hash={text_hash:016X}", demo)
        # 回读结果必须携带共享 FramePlan 已成功执行的 API 无关移动证据。
        self.assertIn("moved_readback.executed_texture_moves == 0", demo)
        # 成功标记必须明确包含纹理移动像素与执行路径验收。
        self.assertIn("glyph-coverage,texture-move-pixels,texture-move", demo)
        # 像素验收必须比较移动后目标与初始帧四十像素之外的来源。
        self.assertIn("validate_scroll_move_pixels(&initial_readback, &moved_readback)", demo)
        # 固定样本必须明确表达四十逻辑像素的来源与目标关系。
        self.assertIn('(\"upper-center\", 100, 398, 438)', demo)
        # 受控状态更新必须经 UI 队列触发正常声明协调，不能直接调用 Renderer。
        self.assertIn("scroll_offset.set(Point::new(0.0, 40.0))", demo)
        # 同一真实窗口必须分别观察初始帧和移动帧。
        self.assertGreaterEqual(demo.count("request_surface_readback_for_test()"), 2)

    # 校验 Drawing 回读证据来自通用 GPU FramePlan，而不是 Adapter 自报成功。
    def test_texture_move_evidence_is_recorded_after_shared_plan_execution(self) -> None:
        # 读取 Drawing 层公开的测试快照契约。
        contract = (ROOT / "src/draw/backend/contract.rs").read_text(encoding="utf-8")
        # 读取主表面滚动到共享 FramePlan 的 lowering 边界。
        scroll = (ROOT / "src/draw/backend/gpu/backend/rhi_surface_scroll.rs").read_text(encoding="utf-8")
        # 读取最终 surface 回读附加执行证据的唯一边界。
        final = (ROOT / "src/draw/backend/gpu/backend/rhi_surface_final.rs").read_text(encoding="utf-8")
        # 公开结果只能携带 API 无关的执行次数。
        self.assertIn("pub executed_texture_moves: usize", contract)
        # 计数必须发生在共享 FramePlan 成功执行之后。
        self.assertLess(
            # 先定位统一计划执行调用。
            scroll.index("plan.execute_offscreen_on_device(context)?;"),
            # 再定位成功后的计数发布。
            scroll.index("executed_texture_moves_in_frame = self"),
        )
        # 最终回读必须附加冻结的共享执行证据。
        self.assertIn("with_executed_texture_moves(executed_texture_moves)", final)
        # Adapter 文件不得拥有或修改该测试计数，避免形成平台平行真相。
        for adapter in (
            # 检查 OpenGL Adapter 实现目录。
            ROOT / "src/native/presentation/graphics/opengl",
            # 检查 D3D11 Adapter 实现目录。
            ROOT / "src/native/presentation/graphics/d3d11",
        ):
            # 合并该 Adapter 下全部 Rust 源码进行边界断言。
            source = "\n".join(path.read_text(encoding="utf-8") for path in adapter.rglob("*.rs"))
            # 原生实现不能维护另一份移动成功计数。
            self.assertNotIn("executed_texture_moves_in_frame", source)

    # 校验 retained Drawing 复用与最终 Surface damage 能力保持正交。
    def test_retained_scroll_capability_is_not_gated_by_present_coherency(self) -> None:
        # 读取 Device 原语到 Drawing 私有能力的唯一投影。
        projection = (ROOT / "src/draw/backend/gpu/capabilities.rs").read_text(encoding="utf-8")
        # 读取 Drawing backend 对外能力组合边界。
        backend = (ROOT / "src/draw/backend/gpu/backend/render_backend.rs").read_text(encoding="utf-8")
        # 读取 OpenGL ES Device Adapter 的事实能力声明。
        opengl = (ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs").read_text(encoding="utf-8")
        # 读取 D3D11 Device Adapter 的事实能力声明。
        d3d11 = (ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs").read_text(encoding="utf-8")
        # Device 的可选纹理移动事实必须直接进入 Drawing 私有投影。
        self.assertIn("rhi_texture_region_move: capabilities.texture_region_move", projection)
        # 两个生产 Adapter 都必须显式声明它们实际实现的共享原语。
        for adapter in (opengl, d3d11):
            # 每套 Adapter 都只能开启同一个薄 RHI capability 字段。
            self.assertIn("capabilities.texture_region_move = true;", adapter)
        # 截取场景层读取的最终 backend 能力方法。
        capabilities = backend[
            # 从 RenderBackend 能力入口开始。
            backend.index("fn capabilities(&self) -> BackendCapabilities") :
            # 在 resize 生命周期入口前结束。
            backend.index("fn resize(&mut self", backend.index("fn capabilities(&self) -> BackendCapabilities"))
        ]
        # Drawing 局部重绘必须由 retained target 自己证明。
        self.assertIn("let supports_retained_redraw = self.rhi_renderer.is_some()", capabilities)
        # 最终交换链 coherency 不得反向关闭 retained texture 的局部更新。
        self.assertNotIn("present_coherency", capabilities)
        # 滚动复用必须同时要求 retained 像素与 Device TextureMove 原语。
        self.assertIn(
            # 锁定两个正交事实的唯一组合表达式。
            "capabilities.scroll_memmove = supports_retained_redraw && supports_texture_region_move;",
            # 在完整 backend 源码中检查能力组合函数。
            backend,
        )

    # 校验 backdrop blur 由同一上层程序驱动两个原生 Adapter 并产生像素证据。
    def test_backdrop_blur_visual_is_api_neutral_and_readback_driven(self) -> None:
        # 读取独立真窗程序的 capability 配置。
        cargo = (ROOT / "demo/backdrop-visual/Cargo.toml").read_text(encoding="utf-8")
        # 读取真窗程序唯一组合根。
        demo = (ROOT / "demo/backdrop-visual/src/main.rs").read_text(encoding="utf-8")
        # Windows 与 Linux Adapter 必须由同一依赖目标共同编译。
        self.assertIn('"d3d11", "opengles"', cargo)
        # 测试控制面必须保持可选，不能污染普通视觉程序。
        self.assertIn('test-harness = ["uix/test-harness"]', cargo)
        # 上层组合根不能硬编码任何具体图形 API。
        self.assertNotIn(".graphics_backend(", demo)
        # 自动模式必须从最终 surface 关系验证真实边界混色。
        self.assertIn("validate_blurred_stripes(&readback)", demo)
        # runner 必须取得稳定成功标记与规范像素格式。
        self.assertIn("UIX_BACKDROP_READBACK_OK", demo)


# 支持直接运行本契约测试文件。
if __name__ == "__main__":
    # 执行本文件内的全部测试。
    unittest.main()
