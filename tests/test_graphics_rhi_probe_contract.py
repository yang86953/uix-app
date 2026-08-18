# -*- coding: utf-8 -*-
# 说明本文件锁定共享 GraphicsDevice probe 的启动、原子失败和 ABI 契约。
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
# 定位共享 RHI trait 与 probe 模块。
RHI = ROOT / "src/native/present/rhi.rs"
# 定位共享 probe 实现。
PROBE = ROOT / "src/native/present/rhi/probe.rs"
# 定位 OpenGL GraphicsDevice host。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 D3D11 GraphicsDevice 实现。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs"


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
    # 启动 probe 必须由共享 GraphicsDevice trait 统一编排。
    def test_bootstrap_probe_is_shared_by_both_adapters(self) -> None:
        # 读取共享与两类 Adapter 源码。
        rhi = RHI.read_text(encoding="utf-8")
        probe = PROBE.read_text(encoding="utf-8")
        opengl = OPENGL.read_text(encoding="utf-8")
        d3d11 = D3D11.read_text(encoding="utf-8")
        # GraphicsDevice 默认入口必须调用共享 probe 模块。
        device_probe = function_region(rhi, "fn probe(&mut self)")
        self.assertIn("probe::probe_device(self)", device_probe)
        # 共享模块必须提供以 GraphicsDevice 为边界的 probe_device。
        self.assertRegex(probe, r"fn\s+probe_device\s*<[^>]*GraphicsDevice")
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
        # scope 必须追踪 pass 状态与五类资源。
        for resource in ("pass", "Buffer", "Texture", "Sampler", "Pipeline"):
            self.assertRegex(probe, rf"(?i)(scope|resources|cleanup).*{resource}|{resource}.*(scope|resources|cleanup)")
        # cleanup 必须继续处理每类资源并保留首个错误。
        self.assertTrue(contains_any(probe, ("cleanup_all", "cleanup", "drain")))
        self.assertRegex(probe, r"(?s)(first_error|primary_error|main_error).{0,700}(cleanup|destroy)")
        self.assertRegex(probe, r"(?s)(cleanup|destroy).{0,700}(first_error|primary_error|main_error)")

    # Solid uniform 必须使用共享 ABI 编码，并固定最小 1x1 viewport。
    def test_solid_probe_uses_shared_encoded_uniform(self) -> None:
        # 读取共享 probe 源码。
        probe = PROBE.read_text(encoding="utf-8")
        # probe 必须通过共享值对象编码 native-endian uniform。
        self.assertIn("RhiMeshRasterParams::new", probe)
        self.assertIn("encode_ne_bytes()", probe)
        # 最小 target 与 viewport 均必须锁定为 1x1。
        self.assertRegex(probe, r"RhiExtent::new\(1,\s*1\)")
        self.assertRegex(probe, r"width:\s*1\.0[\s\S]{0,160}height:\s*1\.0")
        # 禁止回退到 SolidMesh 大小的零初始化 uniform。
        self.assertNotRegex(probe, r"vec!\s*\[\s*0\s*;[^\]]*PipelineKind::SolidMesh")


# 允许直接运行本契约文件进行局部诊断。
if __name__ == "__main__":
    # 运行本文件声明的静态契约。
    unittest.main()
