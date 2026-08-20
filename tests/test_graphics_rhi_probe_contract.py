# -*- coding: utf-8 -*-
# 说明本文件锁定 Drawing GPU Module probe 的启动、FramePlan 和 ABI 契约。
"""Static contract checks for the shared graphics RHI bootstrap probe."""

# 引入标准单元测试框架。
import unittest
# 引入正则表达式工具。
import re
# 引入路径解析工具。
from pathlib import Path
# 引入类型标注。
from typing import Iterable


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 RHI trait。
RHI = ROOT / "src/native/presentation/rhi/mod.rs"
# 定位 Drawing GPU Module probe 实现。
PROBE = ROOT / "src/draw/backend/gpu/device_probe.rs"
# 定位 native factory 组合根。
REGISTRY = ROOT / "src/native/factory/registry.rs"
# 定位 GPU backend 启动门禁。
GPU_IMPL = ROOT / "src/draw/backend/gpu/backend/impl_main.rs"
# 定位 OpenGL GraphicsDevice host。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 D3D11 GraphicsDevice 实现。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"


# 从候选词中匹配一个稳定的共享 scope 名称，避免锁死局部变量名。
def contains_any(source: str, needles: Iterable[str]) -> bool:
    # 只要一个候选契约存在即视为匹配。
    return any(needle in source for needle in needles)


# 只截取 impl/函数附近的源码，避免 Adapter 其它方法偶然满足断言。
def function_region(source: str, marker: str, limit: int = 9000) -> str:
    # 定位共享入口或实现声明。
    start = source.index(marker)
    # 返回有限范围以保持契约抗局部重排能力。
    return source[start : start + limit]


# 验证共享 RHI 作为 System、Adapter 作为 Component 的单向依赖。
class GraphicsRhiProbeContractTests(unittest.TestCase):
    # 启动 probe 必须由 Drawing GPU Module 统一编排。
    def test_bootstrap_probe_is_shared_by_both_adapters(self) -> None:
        # 读取共享与两类 Adapter 源码。
        rhi = RHI.read_text(encoding="utf-8")
        probe = PROBE.read_text(encoding="utf-8")
        opengl = OPENGL.read_text(encoding="utf-8")
        d3d11 = D3D11.read_text(encoding="utf-8")
        # Drawing 模块必须提供以 GraphicsDevice 为边界的 probe_device。
        self.assertRegex(probe, r"fn\s+probe_device\s*<[^>]*GraphicsDevice")
        # GraphicsDevice trait 不得再编排 probe。
        trait_start = rhi.index("pub(crate) trait GraphicsDevice")
        trait_end = rhi.index("pub(crate) trait GraphicsSurface", trait_start)
        self.assertNotIn("fn probe(", rhi[trait_start:trait_end])
        # native registry 不得再编排 probe。
        registry = REGISTRY.read_text(encoding="utf-8")
        self.assertNotIn(".probe(", registry)
        self.assertNotIn("probe_device", registry)
        # GPU backend 构造必须调用 Drawing-owned probe。
        gpu_impl = GPU_IMPL.read_text(encoding="utf-8")
        self.assertIn("probe_device", function_region(gpu_impl, "fn new_gpu_only"))
        # 两类生产 Adapter 都必须实现同一个 GraphicsDevice trait。
        self.assertIn("impl<T> GraphicsDevice for T", opengl)
        self.assertIn("impl GraphicsDevice for D3d11Context", d3d11)
        # Adapter 不得各自复制 bootstrap probe 入口或调用共享 probe 实现。
        self.assertNotIn("probe::probe_device", opengl)
        self.assertNotIn("probe::probe_device", d3d11)
        self.assertNotRegex(opengl, r"fn\s+probe\s*\(")
        self.assertNotRegex(d3d11, r"fn\s+probe\s*\(")

    # 共享 probe 必须以唯一 scope 追踪资源，并在失败时清理全部资源。
    def test_probe_scope_cleanup_is_first_error_wins_and_complete(self) -> None:
        # 读取共享 probe 源码。
        probe = PROBE.read_text(encoding="utf-8")
        # 必须存在唯一的资源/失败 scope，而不是散落的 Adapter 清理策略。
        self.assertTrue(contains_any(probe, ("ProbeScope", "ProbeResources", "ProbeCleanup")))
        self.assertTrue(contains_any(probe, ("first_error", "primary_error", "main_error")))
        # scope 只必须追踪四类资源，pass 生命周期属于 FramePlan。
        for resource in ("Buffer", "Texture", "Sampler", "Pipeline"):
            self.assertRegex(probe, rf"(?i)(scope|resources|cleanup).*{resource}|{resource}.*(scope|resources|cleanup)")
        # pass 状态不得作为 ProbeScope 字段保存。
        scope = probe[probe.index("struct ProbeScope") : probe.index("impl ProbeScope")]
        self.assertNotIn("pass", scope)
        # cleanup 必须继续处理每类资源并保留首个错误。
        self.assertTrue(contains_any(probe, ("cleanup_all", "cleanup", "drain")))
        self.assertRegex(probe, r"(?s)(first_error|primary_error|main_error).{0,700}(cleanup|destroy)")
        self.assertRegex(probe, r"(?s)(cleanup|destroy).{0,700}(first_error|primary_error|main_error)")

    # Solid 与 Shape probe 必须使用共享 FramePlan payload，并固定最小 1x1 viewport。
    def test_solid_probe_uses_shared_encoded_uniform(self) -> None:
        # 读取共享 probe 源码。
        probe = PROBE.read_text(encoding="utf-8")
        # probe 必须通过共享顶点与 uniform payload 构造 FramePlan 命令。
        self.assertIn("FrameVertexPayload", probe)
        self.assertIn("FrameUniformPayload::Mesh", probe)
        self.assertIn("FrameUniformPayload::Shape", probe)
        self.assertIn("FramePlan::offscreen()", probe)
        self.assertIn("push_pass", probe)
        self.assertIn("execute_offscreen_on_device", probe)
        # probe 不得直接编码或更新 buffer 字节。
        self.assertNotIn("encode_ne_bytes()", probe)
        self.assertNotIn("update_buffer(", probe)
        # 最小 target 与 viewport 均必须锁定为 1x1。
        self.assertRegex(probe, r"RhiExtent::new\(1,\s*1\)")
        self.assertRegex(probe, r"width:\s*1\.0[\s\S]{0,160}height:\s*1\.0")
        # 禁止回退到 SolidMesh 大小的零初始化 uniform。
        self.assertNotRegex(probe, r"vec!\s*\[\s*0\s*;[^\]]*PipelineKind::SolidMesh")


# 允许直接运行本契约文件进行局部诊断。
if __name__ == "__main__":
    # 运行本文件声明的静态契约。
    unittest.main()
