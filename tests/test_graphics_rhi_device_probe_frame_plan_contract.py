# -*- coding: utf-8 -*-
# 说明本文件锁定 Drawing GPU Module 的启动探针与跨后端预检契约。
"""Static contract checks for the Drawing-owned device probe and FramePlan path."""

# 引入标准单元测试框架。
import unittest
# 引入稳定的跨平台路径类型。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 GraphicsDevice 公开契约。
RHI = ROOT / "src/native/presentation/rhi/mod.rs"
# 定位 native factory 组合根。
REGISTRY = ROOT / "src/native/factory/registry.rs"
# 定位 Drawing GPU Module 的启动探针。
DEVICE_PROBE = ROOT / "src/draw/backend/gpu/device_probe.rs"
# 定位 GPU backend 的构造与启动门禁。
GPU_IMPL = ROOT / "src/draw/backend/gpu/execution/impl_main.rs"
# 定位保留候选 recipe 回退语义的 Drawing bootstrap 循环。
BOOTSTRAP = ROOT / "src/draw/renderer/bootstrap.rs"
# 定位 OpenGL GraphicsDevice Adapter。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 D3D11 GraphicsDevice Adapter。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"


# 提取指定 Rust 函数的完整花括号范围。
def function_body(source: str, function_name: str) -> str:
    # 支持普通函数与带泛型参数的函数签名。
    markers = (f"fn {function_name}(", f"fn {function_name}<")
    # 定位第一个存在的函数签名。
    start = next((source.index(marker) for marker in markers if marker in source), None)
    # 缺少函数时让契约报告明确失败。
    if start is None:
        raise AssertionError(f"函数 {function_name} 缺少签名")
    # 定位函数体起始花括号。
    opening = source.index("{", start)
    # 初始化花括号嵌套深度。
    depth = 0
    # 扫描函数体并匹配闭合花括号。
    for index in range(opening, len(source)):
        # 读取当前源码字符。
        character = source[index]
        # 进入嵌套代码块时增加深度。
        if character == "{":
            depth += 1
        # 离开嵌套代码块时减少深度。
        elif character == "}":
            depth -= 1
            # 顶层函数闭合时返回完整函数体。
            if depth == 0:
                return source[opening : index + 1]
    # 源码结构损坏时明确失败。
    raise AssertionError(f"函数 {function_name} 缺少闭合花括号")


# 验证 Drawing System 负责探针编排，Adapter 只实现共享原语。
class GraphicsRhiDeviceProbeFramePlanContractTests(unittest.TestCase):
    # 启动探针必须脱离 native RHI trait 与 factory 组合根。
    def test_probe_ownership_and_factory_boundary(self) -> None:
        # 读取共享 RHI、factory、Drawing probe 与 GPU 构造源码。
        rhi = RHI.read_text(encoding="utf-8")
        registry = REGISTRY.read_text(encoding="utf-8")
        probe = DEVICE_PROBE.read_text(encoding="utf-8")
        gpu_impl = GPU_IMPL.read_text(encoding="utf-8")
        # 新 probe 文件必须存在并导出设备探针入口。
        self.assertTrue(DEVICE_PROBE.is_file())
        self.assertIn("fn probe_device", probe)
        # GraphicsDevice trait 不得再拥有 probe 编排入口。
        trait_start = rhi.index("pub(crate) trait GraphicsDevice")
        # 以相邻 GraphicsSurface trait 作为共享契约区域的结束边界。
        trait_end = rhi.index("pub(crate) trait GraphicsSurface", trait_start)
        # 只截取 GraphicsDevice trait，避免其它模块的同名方法干扰断言。
        trait = rhi[trait_start:trait_end]
        # trait 内不得出现旧式 probe 方法声明。
        self.assertNotIn("fn probe(", trait)
        self.assertNotIn("probe_device", rhi)
        # native factory 不得调用旧 probe 或新 probe。
        self.assertNotIn(".probe(", registry)
        self.assertNotIn("probe_device", registry)
        # GPU-only 构造必须调用 Drawing-owned probe。
        constructor = function_body(gpu_impl, "new_gpu_only")
        self.assertIn("probe_device", constructor)
        # probe 失败分支必须交给统一 owner 清理 helper。
        self.assertIn("shutdown_gpu_owner_with_error", constructor)
        # 提取清理 helper，验证资源失败的因果链保持完整。
        shutdown = function_body(gpu_impl, "shutdown_gpu_owner_with_error")
        # helper 必须执行 owner 检查式关闭。
        self.assertIn("try_shutdown", shutdown)
        # 清理失败必须链接原始 probe 主错误。
        self.assertIn("with_source", shutdown)

    # probe 必须只通过离屏 FramePlan 执行共享预检与命令。
    def test_probe_uses_offscreen_frame_plan_and_forbids_direct_device_commands(self) -> None:
        # 读取 Drawing-owned probe 源码。
        probe = DEVICE_PROBE.read_text(encoding="utf-8")
        # 提取 probe_device 函数，避免其它辅助代码满足断言。
        body = function_body(probe, "probe_device")
        # 锁定离屏计划、资源命令、render pass 与统一执行入口。
        required = (
            "FramePlan::offscreen()",
            "push_move",
            "RenderPassPlan",
            "FramePlanCommand::UploadVertex",
            "FramePlanCommand::UploadUniform",
            "push_pass",
            "execute_offscreen_on_device",
        )
        # 逐项确认 probe 的命令路径完整。
        for fragment in required:
            # 缺少任一片段都表示绕过共享 FramePlan 契约。
            self.assertIn(fragment, body)
        # probe 函数体不得直接编排底层 render/pass/submit 命令。
        forbidden = (
            ".begin_render_pass(",
            ".set_viewport(",
            ".set_scissor(",
            ".clear_rect(",
            ".draw(",
            ".move_texture_region(",
            ".end_render_pass(",
            ".submit(",
        )
        # 逐项确认所有原生命令均被统一执行器封装。
        for fragment in forbidden:
            # 直接调用会让 Drawing 越过 FramePlan 执行边界。
            self.assertNotIn(fragment, body)
        # probe 文件不得取得 Surface 或承担 Surface 生命周期。
        for fragment in ("GraphicsSurface", "GraphicsContextRhi", "acquire(", "present(", "resize("):
            # 这些角色只能属于 Surface/Present Module。
            self.assertNotIn(fragment, probe)

    # Drawing probe 失败必须继续沿用 renderer candidate 回退循环。
    def test_probe_failure_remains_a_renderer_candidate_failure(self) -> None:
        # 读取 GPU 构造与 bootstrap 候选循环源码。
        gpu_impl = GPU_IMPL.read_text(encoding="utf-8")
        # 读取候选循环与失败阶段映射。
        bootstrap = BOOTSTRAP.read_text(encoding="utf-8")
        # GPU 构造必须把 probe 失败作为构造错误返回。
        constructor = function_body(gpu_impl, "new_gpu_only")
        # probe 错误不得被记录后伪装成成功 backend。
        self.assertIn("return Err(shutdown_gpu_owner_with_error(&mut gpu_ctx, error));", constructor)
        # 截取唯一 recipe 候选循环。
        candidates = function_body(bootstrap, "bootstrap_renderer_with_candidates")
        # 每个 native owner 都必须进入统一 renderer 装配入口。
        self.assertIn("assemble_renderer(owner, width, height)", candidates)
        # 只检查 renderer 装配失败之后的分支，避免由 context-create continue 偶然满足。
        assembly_failure = candidates[candidates.index("let renderer = match assemble_renderer") :]
        # probe 所属的 renderer-create 失败必须记录具体阶段。
        self.assertIn("failure.stage.probe_stage()", assembly_failure)
        # 当前候选失败后必须继续尝试下一 recipe。
        self.assertIn("continue;", assembly_failure)
        # GpuBackend 构造错误必须映射为 renderer_create 而非 native context_create。
        self.assertIn("Self::Create => ProbeStage::RendererCreate", bootstrap)

    # OpenGL 与 D3D11 Adapter 必须消费同一组共享预检与原生命令能力。
    def test_both_adapters_implement_shared_preflight_and_submission_contract(self) -> None:
        # 读取两个真实 GraphicsDevice 实现文件。
        adapters = {
            "OpenGL": OPENGL.read_text(encoding="utf-8"),
            "D3D11": D3D11.read_text(encoding="utf-8"),
        }
        # 两类 Adapter 都必须实现共享只读预检入口。
        required = ("preflight_buffer_upload", "preflight_draw_resources", "preflight_texture_move")
        # 两类 Adapter 都必须实现完整 pass 生命周期与提交。
        lifecycle = ("begin_render_pass", "draw", "end_render_pass", "submit")
        # 分别锁定每个 Adapter 的 trait 实现区域。
        for name, source in adapters.items():
            # Adapter 必须显式实现 GraphicsDevice。
            self.assertIn("GraphicsDevice for", source, name)
            # 预检和生命周期函数必须实际存在于 Adapter 文件。
            for fragment in required + lifecycle:
                # 缺失表示后端未消费共享契约。
                self.assertIn(f"fn {fragment}", source, f"{name}: {fragment}")


# 允许直接运行本契约文件进行局部诊断。
if __name__ == "__main__":
    # 运行本文件声明的静态契约。
    unittest.main()
