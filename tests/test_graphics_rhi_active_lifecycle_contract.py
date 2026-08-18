"""锁定 OpenGL RHI 关闭后生命周期门禁的源码顺序契约。"""

# 引入 unittest 断言框架。
import unittest

# 引入稳定的仓库路径计算工具。
from pathlib import Path


# 计算本测试对应的仓库根目录。
ROOT = Path(__file__).resolve().parents[1]


# 读取一个仓内 Rust 源文件。
def source(relative_path: str) -> str:
    # 返回当前工作区中的原始源码文本。
    return (ROOT / relative_path).read_text(encoding="utf-8")


# 提取一个 Rust 函数的完整花括号范围。
def function_body(text: str, function_name: str, occurrence: int = 0) -> str:
    # 定位函数签名，避免只按关键词数量判断。
    marker = f"fn {function_name}("
    # 找到指定序号的函数签名起点。
    start = -1
    # 逐次搜索同名函数以区分 Device 与 Surface 默认实现。
    for _ in range(occurrence + 1):
        # 找到下一处同名函数签名。
        start = text.index(marker, start + 1)
    # 找到函数体开始的左花括号。
    opening = text.index("{", start)
    # 初始化 Rust 花括号嵌套深度。
    depth = 0
    # 从函数体开始逐字符扫描嵌套范围。
    for index in range(opening, len(text)):
        # 读取当前字符。
        character = text[index]
        # 进入一个嵌套代码块时增加深度。
        if character == "{":
            depth += 1
        # 离开一个嵌套代码块时减少深度。
        elif character == "}":
            depth -= 1
            # 顶层函数体闭合时返回精确子串。
            if depth == 0:
                return text[opening : index + 1]
    # 源码结构不完整时让测试明确失败。
    raise AssertionError(f"函数 {function_name} 缺少闭合花括号")


# 断言门禁出现在所有 native/pipeline 调用之前。
def assert_gate_precedes_native(test_case: unittest.TestCase, body: str) -> None:
    # 定位共享 active 门禁调用。
    gate = body.index("rhi_ensure_active()")
    # 收集所有可能触碰 native 或 pipeline 的入口。
    native_markers = (
        "rhi_pipeline_mut(",
        "rhi_pipeline()",
        "rhi_make_current(",
        "rhi_swap_buffers(",
    )
    # 逐一检查每个实际存在的 native 入口。
    for marker in native_markers:
        # 只比较函数体中存在的调用。
        if marker in body:
            # 门禁必须早于该 native/pipeline 调用。
            test_case.assertLess(gate, body.index(marker))


# 断言 D3D11 owner 门禁早于该入口的第一项资源、状态或原生工作。
def assert_d3d_gate_precedes_work(
    test_case: unittest.TestCase, body: str, first_work: str
) -> None:
    # 定位 D3D11 共享 owner 的生命周期门禁。
    gate = body.index("self.ensure_active()?")
    # 定位该入口约定的第一项可失败工作。
    work = body.index(first_work)
    # 关闭错误必须早于所有资源、状态和原生副作用。
    test_case.assertLess(gate, work)


# 声明生命周期门禁契约测试集合。
class GraphicsRhiActiveLifecycleContractTest(unittest.TestCase):
    # 检查 shared OpenGL host 的每个可变入口先执行 active 门禁。
    def test_shared_host_device_and_surface_entries_are_gated(self) -> None:
        # 读取 shared host 实现。
        host = source("src/native/presentation/graphics/opengl/rhi_host.rs")
        # 截取 Device blanket impl，避免同名 Surface 方法误满足断言。
        device_host = host[: host.index("// 将共享 surface 生命周期转发给 WGL/EGL context。")]
        # 截取 Surface blanket impl，避免同名 Device 方法误满足断言。
        surface_host = host[host.index("// 将共享 surface 生命周期转发给 WGL/EGL context。") :]
        # 列出所有必须先门禁的 Device 入口。
        device_entries = (
            "activate", "maintain", "inject_device_lost_for_test", "create_buffer",
            "update_buffer", "create_texture", "resolve_render_target",
            "preflight_texture_copy", "preflight_texture_move",
            # Draw 只读资源预检也必须先检查 OpenGL owner。
            "preflight_draw_resources", "update_texture", "create_sampler",
            "create_pipeline", "destroy_buffer", "destroy_texture", "destroy_sampler",
            "destroy_pipeline", "begin_render_pass", "set_viewport", "set_scissor",
            "clear_rect", "bind_sampled_texture", "draw", "copy_texture",
            "move_texture_region", "end_render_pass", "submit",
        )
        # 逐一检查 Device 入口存在且先执行门禁。
        for entry in device_entries:
            # 提取当前入口函数体。
            body = function_body(device_host, entry)
            # 检查门禁早于 pipeline/native 调用。
            assert_gate_precedes_native(self, body)
        # 列出所有必须先门禁的 Surface 入口。
        surface_entries = (
            "acquire", "resize", "read_surface_pixels", "present", "test_present",
            "maintain", "inject_surface_lost_for_test",
        )
        # 逐一检查 Surface 入口存在且先执行门禁。
        for entry in surface_entries:
            # 提取当前入口函数体。
            body = function_body(surface_host, entry)
            # Surface 的测试入口必须先门禁再返回未实现。
            self.assertIn("rhi_ensure_active()", body)
            # 检查门禁早于所有 native host 调用。
            assert_gate_precedes_native(self, body)

    # 检查 EGL shutdown_started 发布顺序与所有权门禁。
    def test_egl_shutdown_and_borrow_gate_order(self) -> None:
        # 读取 EGL owner 与拆分后的 RHI 实现。
        egl = source("src/native/presentation/graphics/opengl/platform/egl.rs")
        egl_rhi = source("src/native/presentation/graphics/opengl/platform/egl_rhi.rs")
        # shutdown_started 必须存在并在 cleanup 前发布。
        self.assertIn("shutdown_started: bool", egl)
        # 构造成功的 EGL owner 必须从 active 状态开始。
        self.assertIn("shutdown_started: false", egl)
        # 提取 shutdown 事务函数体。
        shutdown = function_body(egl, "shutdown_result")
        # 关闭事实发布必须早于第一个原生 cleanup 调用。
        self.assertLess(shutdown.index("self.shutdown_started = true"), shutdown.index("self.egl"))
        # cleanup 重试不能调用只允许业务 RHI 使用的 active 门禁。
        self.assertNotIn("ensure_rhi_active", shutdown)
        # 截取 EGL owner 的唯一 active 判定。
        active = function_body(egl, "ensure_rhi_active")
        # 部分或完整 shutdown 的每个不可逆状态都必须使 owner 失活。
        for marker in (
            "self.shutdown_started", "self.shutdown", "self.context_destroyed",
            "self.surface_destroyed", "self.display_terminated", "self.egl_window.is_null()",
        ):
            # active helper 必须读取当前关闭状态事实。
            self.assertIn(marker, active)
        # EGL 失活必须统一为稳定 InvalidState。
        self.assertIn("Errc::InvalidState", active)
        # make_current 必须先调用 inherent active helper。
        current = function_body(egl, "make_current_result")
        self.assertLess(current.index("self.ensure_rhi_active()?"), current.index("self.egl"))
        # recipe owner 借出组合 context 前必须先门禁。
        context = function_body(egl_rhi, "rhi_context")
        self.assertLess(context.index("self.ensure_rhi_active()?"), context.index("Ok(self)"))
        # OpenGL host 实现必须委托同一 inherent helper。
        self.assertIn("self.ensure_rhi_active()", function_body(egl_rhi, "rhi_ensure_active"))

    # 检查 WGL shutdown_started 发布顺序与所有权门禁。
    def test_wgl_shutdown_and_borrow_gate_order(self) -> None:
        # 读取 WGL owner 与拆分后的生命周期实现。
        wgl = source("src/native/presentation/graphics/opengl/platform/wgl.rs")
        wgl_graphics = source("src/native/presentation/graphics/opengl/platform/wgl_graphics.rs")
        wgl_rhi = source("src/native/presentation/graphics/opengl/platform/wgl_rhi.rs")
        # shutdown_started 必须存在并在原生 cleanup 前发布。
        self.assertIn("shutdown_started: bool", wgl)
        # 构造成功的 WGL owner 必须从 active 状态开始。
        self.assertIn("shutdown_started: false", wgl)
        # 提取 shutdown 事务函数体。
        shutdown = function_body(wgl, "shutdown_result", 2)
        # 关闭事实发布必须早于第一个原生 cleanup 调用。
        self.assertLess(shutdown.index("self.shutdown_started = true"), shutdown.index("wglMakeCurrent"))
        # cleanup 重试不能调用只允许业务 RHI 使用的 active 门禁。
        self.assertNotIn("ensure_rhi_active", shutdown)
        # 截取 WGL owner 的唯一 active 判定。
        active = function_body(wgl, "ensure_rhi_active")
        # 关闭事务与两个关键原生句柄共同决定 WGL owner 是否存活。
        for marker in ("self.shutdown_started", "self.hdc.is_null()", "self.hglrc.is_null()"):
            # active helper 必须读取当前关闭状态事实。
            self.assertIn(marker, active)
        # WGL 失活必须统一为稳定 InvalidState。
        self.assertIn("Errc::InvalidState", active)
        # make_current 必须先调用 inherent active helper。
        current = function_body(wgl, "make_current_result")
        self.assertLess(current.index("self.ensure_rhi_active()?"), current.index("wglMakeCurrent"))
        # recipe owner 借出组合 context 前必须先门禁。
        context = function_body(wgl_graphics, "rhi_context")
        self.assertLess(context.index("self.ensure_rhi_active()?"), context.index("Ok(self)"))
        # OpenGL host 实现必须委托同一 inherent helper。
        self.assertIn("self.ensure_rhi_active()", function_body(wgl_rhi, "rhi_ensure_active"))

    # 检查 D3D11 仍保留同一 active 生命周期对照。
    def test_d3d11_active_gate_remains_cross_backend_baseline(self) -> None:
        # 读取 D3D11 context 生命周期实现。
        d3d = source("src/native/presentation/graphics/d3d11/platform/context/graphics.rs")
        # 读取 D3D11 Device 注入实现。
        d3d_device = source("src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs")
        # 读取 D3D11 健康维护 helper，保持门禁责任集中在 owner helper。
        d3d_health = source("src/native/presentation/graphics/d3d11/platform/context/rhi_health.rs")
        # 读取 D3D11 Surface 注入实现。
        d3d_surface = source("src/native/presentation/graphics/d3d11/platform/context/rhi.rs")
        # D3D11 rhi_context 必须先执行既有 active 门禁。
        context = function_body(d3d, "rhi_context")
        # 共享跨后端契约要求门禁早于借出 self。
        self.assertLess(context.index("self.ensure_active()?"), context.index("Ok(self)"))
        # Device 故障注入必须先通过 active owner 门禁。
        device_injection = function_body(d3d_device, "inject_device_lost_for_test")
        # Device 门禁必须存在。
        self.assertIn("self.ensure_active()?", device_injection)
        # Device 门禁必须早于故障标志写入组件。
        self.assertLess(
            device_injection.index("self.ensure_active()?"),
            device_injection.index("arm_rhi_device_lost_for_test()"),
        )
        # 列出必须在任何资源、状态或 native 工作前检查 owner 的 Device 入口。
        d3d_device_entries = (
            ("activate", "Ok(())"),
            ("create_buffer", "desc.validate()?"),
            ("update_buffer", "self.rhi_device.buffer("),
            ("create_texture", "self.rhi_create_texture("),
            ("resolve_render_target", "self.rhi_device.textures.resolve_render_target("),
            ("preflight_texture_copy", "self.rhi_device.textures.validate_copy("),
            ("preflight_texture_move", "self.rhi_device.textures.validate_move("),
            # Draw 资源预检必须先门禁，再读取真实 pipeline 与 Buffer 表。
            ("preflight_draw_resources", "self.rhi_device.pipeline("),
            ("create_pipeline", "self.rhi_create_pipeline("),
            ("create_sampler", "self.rhi_create_sampler("),
            ("update_texture", "self.rhi_device.texture("),
            ("destroy_buffer", "self.rhi_device.buffers.take("),
            ("destroy_texture", "self.rhi_device.pass.validate_texture_destroy("),
            ("destroy_pipeline", "self.rhi_destroy_pipeline("),
            ("destroy_sampler", "self.rhi_destroy_sampler("),
            ("begin_render_pass", "self.rhi_device.pass.require_closed("),
            ("bind_sampled_texture", "self.rhi_bind_sampled_texture("),
            ("set_viewport", "self.rhi_set_viewport("),
            ("set_scissor", "self.rhi_set_scissor("),
            ("clear_rect", "self.rhi_clear_rect("),
            ("draw", "self.draw_rhi_packet("),
            ("copy_texture", "self.rhi_device.pass.require_closed("),
            ("move_texture_region", "self.rhi_device.pass.require_closed("),
            ("end_render_pass", "self.end_render_pass_impl("),
            ("submit", "self.submit_impl("),
        )
        # 逐一验证 D3D11 Device 入口均先执行 active 门禁。
        for entry, first_work in d3d_device_entries:
            # 提取当前 D3D11 Device 入口函数体。
            body = function_body(d3d_device, entry)
            # 检查门禁存在且早于该入口的第一项实际工作。
            assert_d3d_gate_precedes_work(self, body, first_work)
        # 提取 D3D11 健康维护 helper 的函数体。
        health = function_body(d3d_health, "maintain_rhi_device")
        # 健康维护仍由 helper 自己拥有 active 门禁，避免 Device 重复实现。
        self.assertIn("self.ensure_active()?", health)
        # helper 门禁必须早于后续健康查询或 native 工作。
        self.assertLess(
            health.index("self.ensure_active()?"),
            health.index("self.device.GetDeviceRemovedReason"),
        )
        # Surface 故障注入必须先通过 active owner 门禁。
        surface_injection = function_body(d3d_surface, "inject_surface_lost_for_test")
        # Surface 门禁必须存在。
        self.assertIn("self.ensure_active()?", surface_injection)
        # Surface 门禁必须早于故障标志写入。
        self.assertLess(
            surface_injection.index("self.ensure_active()?"),
            surface_injection.index("self.rhi_surface_lost_for_test = true"),
        )
        # 读取 D3D11 Surface 实现作为 native 调用前门禁对照。
        surface = source("src/native/presentation/graphics/d3d11/platform/context/rhi.rs")
        # 当前 D3D11 的全部 native Surface 入口都必须保持 active 门禁。
        for entry in ("acquire", "resize", "read_surface_pixels", "present", "test_present"):
            # 截取对应 Surface 入口。
            body = function_body(surface, entry)
            # D3D11 门禁必须出现在任何具体 Surface 工作之前。
            self.assertIn("self.ensure_active()?", body)


# 允许直接以 unittest 模块方式执行本文件。
if __name__ == "__main__":
    # 运行本文件声明的契约测试。
    unittest.main()
