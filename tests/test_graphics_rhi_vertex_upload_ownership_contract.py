# -*- coding: utf-8 -*-
# 说明本文件锁定每个 FramePlan Draw 的类型化顶点内容所有权。
"""Keep vertex initialization owned by the FramePlan pass."""

# 引入标准单元测试框架。
import unittest
# 引入路径解析工具。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 FramePlan 顶点顺序验证。
VALIDATION = ROOT / "src/draw/backend/frame_plan_validation.rs"
# 定位共享顶点 payload 工厂。
RENDERER = ROOT / "src/draw/backend/rhi_renderer.rs"
# 定位独立 Gradient lowering。
GRADIENT = ROOT / "src/draw/backend/rhi_renderer_gradient.rs"
# 定位独立 Shape lowering。
SHAPE = ROOT / "src/draw/backend/rhi_renderer_shape.rs"
# 定位独立 Shadow lowering。
SHADOW = ROOT / "src/draw/backend/rhi_renderer_shadow.rs"
# 定位 mixed painter-order lowering。
MIXED = ROOT / "src/draw/backend/rhi_renderer_mixed.rs"
# 定位类型化命令到薄 Device 的唯一执行边界。
FRAME_EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 OpenGL ES Buffer Adapter。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 D3D11 Buffer Adapter。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs"


# 截取一个稳定函数范围，避免其它函数偶然满足断言。
def function_range(source: str, start_marker: str, end_marker: str) -> str:
    # 找到目标函数或方法的起点。
    start = source.index(start_marker)
    # 找到目标范围的结束标记。
    end = source.index(end_marker, start)
    # 返回目标函数的精确源码。
    return source[start:end]


# 集中验证共享顶点上传所有权和四类静态 unit quad 路径。
class GraphicsRhiVertexUploadOwnershipContractTests(unittest.TestCase):
    # 缺失顶点上传必须在 Adapter 前被 FramePlan 拒绝。
    def test_validation_requires_preceding_typed_vertex_upload(self) -> None:
        # 读取共享验证器源码。
        validation = VALIDATION.read_text(encoding="utf-8")
        # 截取 draw upload 验证函数。
        validator = function_range(validation, "pub(super) fn validate_draw_uploads", "    // 当前 draw 的所有类型化资源事实")
        # 验证器必须把最近顶点上传提升为必需事实。
        self.assertIn("latest_vertex.ok_or_else", validator)
        # 验证器必须保留布局匹配门禁。
        self.assertIn("data.layout() != contract.vertex", validator)
        # 错误必须明确要求类型化顶点上传。
        self.assertIn("FramePlan draw must follow a typed vertex upload", validator)

    # 静态 unit quad 必须由共享类型化工厂提供并按 payload 大小创建 buffer。
    def test_static_resources_create_without_direct_upload(self) -> None:
        # 读取共享 renderer 与四个资源创建函数源码。
        renderer = RENDERER.read_text(encoding="utf-8")
        gradient = GRADIENT.read_text(encoding="utf-8")
        shape = SHAPE.read_text(encoding="utf-8")
        shadow = SHADOW.read_text(encoding="utf-8")
        mixed = MIXED.read_text(encoding="utf-8")
        # 共享 renderer 必须拥有唯一 position-float2 unit quad 工厂。
        self.assertIn("pub(super) fn unit_quad_vertex_payload", renderer)
        self.assertIn("FrameVertexPayload::position_f32x2", renderer)
        # Gradient 创建函数只按 typed payload 大小分配，不直接上传。
        gradient_resources = function_range(renderer, "fn ensure_gradient_resources", "    // 构造所有静态 float2 unit quad")
        self.assertIn("unit_vertices.size_bytes()", gradient_resources)
        self.assertNotIn("device.update_buffer", gradient_resources)
        # Shape 创建函数只按 typed payload 大小分配，不直接上传。
        shape_resources = function_range(shape, "pub(super) fn ensure_shape_resources", "    // 将一个 shape 的固定常量")
        self.assertIn("unit_vertices.size_bytes()", shape_resources)
        self.assertNotIn("device.update_buffer", shape_resources)
        # Shadow 创建函数只按 typed payload 大小分配，不直接上传。
        shadow_resources = function_range(shadow, "pub(super) fn ensure_shadow_resources", "    // 将一个 Shadow 的固定常量")
        self.assertIn("unit_vertices.size_bytes()", shadow_resources)
        self.assertNotIn("device.update_buffer", shadow_resources)
        # Sector 创建函数只按 typed payload 大小分配，不直接上传。
        sector_resources = function_range(mixed, "fn ensure_sector_resources", "// 为通用 renderer 提供一个混合操作")
        self.assertIn("unit_vertices.size_bytes()", sector_resources)
        self.assertNotIn("device.update_buffer", sector_resources)

    # 独立与 mixed lowering 都必须在 Draw 前发出 typed UploadVertex。
    def test_standalone_and_mixed_plans_upload_static_vertices(self) -> None:
        # 读取四类 lowering 源码。
        gradient = GRADIENT.read_text(encoding="utf-8")
        shape = SHAPE.read_text(encoding="utf-8")
        shadow = SHADOW.read_text(encoding="utf-8")
        mixed = MIXED.read_text(encoding="utf-8")
        # 独立 Gradient 必须显式上传 unit quad。
        self.assertIn("FramePlanCommand::UploadVertex", gradient)
        self.assertIn("RhiRenderer::unit_quad_vertex_payload()", gradient)
        # 独立 Shape 必须显式上传 unit quad。
        self.assertIn("FramePlanCommand::UploadVertex", shape)
        self.assertIn("RhiRenderer::unit_quad_vertex_payload()", shape)
        # 独立 Shadow 必须显式上传 unit quad。
        self.assertIn("FramePlanCommand::UploadVertex", shadow)
        self.assertIn("RhiRenderer::unit_quad_vertex_payload()", shadow)
        # mixed 必须按实际资源槽为 Gradient、Shape、Shadow、Sector 上传。
        self.assertGreaterEqual(mixed.count("FramePlanCommand::UploadVertex"), 4)
        self.assertGreaterEqual(mixed.count("RhiRenderer::unit_quad_vertex_payload()"), 4)
        # mixed 的静态上传必须出现在原始 operation loop 之前。
        pass_start = mixed.index("let mut pass = RenderPassPlan::new")
        operation_loop = mixed.index("for (index, operation)", pass_start)
        self.assertLess(mixed.index("if let Some((_, vertex_buffer, _)) = gradient_resources", pass_start), operation_loop)
        # 四类静态资源必须都通过同一个共享 payload 工厂。
        self.assertIn("if let Some((_, _, vertex_buffer, _)) = shape_resources", mixed)
        self.assertIn("if let Some((_, vertex_buffer, _)) = shadow_resources", mixed)
        self.assertIn("if let Some((_, vertex_buffer, _)) = sector_resources", mixed)

    # 两个生产 Adapter 都必须消费同一 FramePlan 类型化上传，不能依赖各自创建初态。
    def test_opengl_and_d3d11_consume_the_shared_plan_upload(self) -> None:
        # 读取共享执行边界与两个生产 Adapter 的 Buffer 实现。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 读取 OpenGL ES Buffer Adapter。
        opengl = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 D3D11 Buffer Adapter。
        d3d11 = D3D11_DEVICE.read_text(encoding="utf-8")
        # 截取共享命令分派函数。
        command_execution = function_range(execution, "fn execute_command", "    // 把通用 target 引用解析")
        # 类型化顶点必须在唯一执行边界编码。
        self.assertIn("FramePlanCommand::UploadVertex { buffer, data }", command_execution)
        # 编码后的顶点只通过薄 Device update_buffer 原语交付。
        self.assertIn(".update_buffer(RhiBufferUpload::new(*buffer, &bytes))", command_execution)
        # 截取 OpenGL ES Buffer 创建路径。
        opengl_create = function_range(opengl, "pub(super) fn create_buffer", "    // 更新 Buffer 从零开始")
        # OpenGL ES 创建期 CPU 镜像为零，不能成为 FramePlan 的顶点事实。
        self.assertIn("data: vec![0; desc.size_bytes()]", opengl_create)
        # 截取 D3D11 Buffer 创建路径。
        d3d11_create = function_range(d3d11, "fn create_buffer", "    // 将紧密排列的数据写入")
        # D3D11 创建期没有初始数据，证明两个 Adapter 初态不能充当共享契约。
        self.assertIn("CreateBuffer(&native_desc, None", d3d11_create)
        # 截取 OpenGL ES Buffer 更新路径。
        opengl_update = function_range(opengl, "pub(super) fn update_buffer", "    // 创建 sampled texture")
        # OpenGL ES Adapter 必须机械消费共享载荷。
        self.assertIn("buffer_sub_data_u8_slice", opengl_update)
        # 截取 D3D11 Buffer 更新路径。
        d3d11_update = function_range(d3d11, "fn update_buffer", "    // 创建可采样且尽可能可作为 render target")
        # D3D11 Adapter 必须机械消费同一共享载荷。
        self.assertIn("UpdateSubresource", d3d11_update)


# 支持直接执行本文件定义的精确契约测试。
if __name__ == "__main__":
    # 运行当前文件中的全部测试。
    unittest.main()
