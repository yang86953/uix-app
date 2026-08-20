# -*- coding: utf-8 -*-
# 验证原语拓扑只由共享 PipelineContract 拥有，两个 Adapter 只能机械翻译。
"""Keep primitive topology identical across the OpenGL and D3D11 adapters."""

# 启用延迟注解解析，保持测试与其余契约脚本一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位共享 pipeline 契约。
SHARED = ROOT / "src/native/presentation/rhi/pipeline.rs"
# 定位 OpenGL Adapter draw 编码。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_draw.rs"
# 定位 D3D11 pipeline 统一状态绑定。
D3D11_MODULE = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline/mod.rs"
# 定位 D3D11 draw 分派边界。
D3D11_DRAW = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_draw.rs"
# 定位不得继续私自绑定拓扑的 shader helper 目录。
D3D11_HELPERS = ROOT / "src/native/presentation/graphics/d3d11/adapter/pipeline"


# 集中验证共享拓扑所有权和两个原生翻译入口。
class GraphicsRhiPrimitiveTopologyContractTests(unittest.TestCase):
    # 全部现有 pipeline 必须通过同一类型化拓扑契约构造。
    def test_pipeline_contract_has_one_primitive_topology_owner(self) -> None:
        # 读取共享 pipeline 契约源码。
        shared = SHARED.read_text(encoding="utf-8")
        # 共享层必须定义封闭原语拓扑类型。
        self.assertIn("pub(crate) enum PipelinePrimitiveTopology", shared)
        # 完整 pipeline 契约必须拥有原语拓扑。
        self.assertIn("pub(crate) topology: PipelinePrimitiveTopology", shared)
        # 公共二维构造边界必须冻结 TriangleList。
        self.assertIn("topology: PipelinePrimitiveTopology::TriangleList", shared)
        # 每个现有 pipeline 都必须进入唯一二维公共构造边界。
        self.assertEqual(shared.count("=> ui_2d_pipeline_contract("), 11)

    # OpenGL indexed 与 non-indexed draw 必须复用一次共享拓扑翻译。
    def test_opengl_draws_use_shared_primitive_topology(self) -> None:
        # 读取 OpenGL draw 编码源码。
        opengl = OPENGL.read_text(encoding="utf-8")
        # OpenGL 必须通过穷尽函数翻译共享拓扑。
        self.assertIn("fn gl_primitive_topology(topology: PipelinePrimitiveTopology)", opengl)
        # OpenGL 拓扑必须来自当前 pipeline 契约。
        self.assertIn("gl_primitive_topology(contract.topology)", opengl)
        # indexed draw 必须使用翻译后的同一枚举。
        self.assertIn("gl.draw_elements(\n                    primitive_topology,", opengl)
        # non-indexed draw 必须使用翻译后的同一枚举。
        self.assertIn("gl.draw_arrays(primitive_topology, first_vertex, vertex_count)", opengl)
        # GL_TRIANGLES 只能存在于共享拓扑翻译函数中。
        self.assertEqual(opengl.count("glow::TRIANGLES"), 1)

    # D3D11 必须在统一分派边界绑定共享拓扑。
    def test_d3d11_binds_shared_primitive_topology_once(self) -> None:
        # 读取 D3D11 pipeline 统一状态绑定源码。
        module = D3D11_MODULE.read_text(encoding="utf-8")
        # 读取 D3D11 draw 分派源码。
        draw = D3D11_DRAW.read_text(encoding="utf-8")
        # D3D11 必须通过穷尽函数翻译共享拓扑。
        self.assertIn("fn d3d11_primitive_topology(topology: PipelinePrimitiveTopology)", module)
        # 统一绑定入口必须设置翻译后的 IA 拓扑。
        self.assertIn("context.IASetPrimitiveTopology(d3d11_primitive_topology(topology))", module)
        # draw 分派必须把当前 pipeline 的共享拓扑交给统一入口。
        self.assertIn("contract.topology,", draw)
        # 收集所有薄 RHI shader helper 源码。
        helpers = "\n".join(path.read_text(encoding="utf-8") for path in sorted(D3D11_HELPERS.glob("rhi_*.rs")))
        # shader helper 不得再次分散绑定原语拓扑。
        self.assertNotIn("IASetPrimitiveTopology", helpers)
        # shader helper 不得继续私自选择 TriangleList。
        self.assertNotIn("D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST", helpers)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
