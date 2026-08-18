# -*- coding: utf-8 -*-
# 验证 Buffer 描述与上传只通过共享值对象进入 OpenGL 与 D3D11 Adapter。
"""Keep buffer creation and upload semantics identical across graphics adapters."""

# 启用延迟注解解析，保持契约脚本风格一致。
from __future__ import annotations

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口。
RHI = ROOT / "src/native/present/rhi.rs"
# 定位共享 Buffer Component。
BUFFER = ROOT / "src/native/present/rhi/buffer.rs"
# 定位共享 Buffer 资源表 Component。
BUFFER_TABLE = ROOT / "src/native/present/rhi/buffer_resource_table.rs"
# 定位 FramePlan 命令闭集。
FRAME_PLAN = ROOT / "src/draw/backend/frame_plan.rs"
# 定位 FramePlan 到 Device 的唯一执行边界。
FRAME_EXECUTION = ROOT / "src/draw/backend/frame_plan_execution.rs"
# 定位 OpenGL Buffer Adapter。
OPENGL = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL owner-thread bridge。
OPENGL_BRIDGE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi.rs"
# 定位 OpenGL GraphicsDevice host。
OPENGL_HOST = ROOT / "src/native/presentation/graphics/opengl/rhi_host.rs"
# 定位 D3D11 Buffer Adapter。
D3D11 = ROOT / "src/native/presentation/graphics/d3d11/platform/context/rhi_device.rs"


# 集中锁定 Buffer 事实所有权、共同值域与跨后端机械映射。
class GraphicsRhiBufferContractTests(unittest.TestCase):
    # Draw 真实 Buffer 角色必须由共享表在 Device activate 前预检。
    def test_draw_resources_are_preflighted_before_activation(self) -> None:
        # 读取共享 Buffer 资源表。
        table = BUFFER_TABLE.read_text(encoding="utf-8")
        # 读取薄 RHI Device 契约。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取 FramePlan 执行器顺序。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 读取 OpenGL Device、bridge 与 host。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 OpenGL 只读 bridge。
        opengl_bridge = OPENGL_BRIDGE.read_text(encoding="utf-8")
        # 读取 OpenGL owner-thread host。
        opengl_host = OPENGL_HOST.read_text(encoding="utf-8")
        # 读取 D3D11 Device Adapter。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 资源表必须拥有真实描述读取和 Draw 委托入口。
        self.assertIn("pub(crate) trait RhiBufferResource", table)
        self.assertIn("pub(crate) struct RhiBufferResourceTable", table)
        self.assertIn("pub(crate) fn validate_draw", table)
        self.assertIn("packet.validate_resources", table)
        # GraphicsDevice 必须公开只读 Draw 资源预检入口。
        self.assertIn("fn preflight_draw_resources(&self, _packet: DrawPacket)", rhi)
        # 执行器必须定义 Draw 预检函数并在 activate 前调用。
        self.assertIn("fn validate_draw_resources", execution)
        self.assertIn("self.device.preflight_draw_resources(*packet)?", execution)
        # Draw 资源预检必须严格早于 Device 激活。
        self.assertLess(
            # 定位 FramePlan 的共享 Draw 资源预检阶段。
            execution.index("self.validate_draw_resources(steps)?"),
            # 定位首个允许原生副作用的 Device 激活入口。
            execution.index("self.device.activate()?"),
        )
        # OpenGL 必须保存共享 Buffer 表并先验证真实 pipeline 绑定。
        self.assertIn("RhiBufferResourceTable<OpenGlRhiBuffer>", opengl)
        # pipeline 身份校验必须早于 Buffer 角色与容量校验。
        self.assertLess(
            # 定位共享 pipeline 身份校验。
            opengl.index("self.pipelines.get(packet.pipeline)?"),
            # 定位共享 Buffer 角色与容量校验。
            opengl.index("self.buffers.validate_draw(packet)"),
        )
        # OpenGL bridge 必须保持只读 preflight 调用链。
        self.assertIn("pub(crate) fn rhi_preflight_draw_resources(&self", opengl_bridge)
        # bridge 只能机械委托共享 Device。
        self.assertIn("self.rhi.preflight_draw_resources(packet)", opengl_bridge)
        # OpenGL host 必须先检查 owner 且不得为只读预检恢复 context。
        opengl_preflight = opengl_host.split("fn preflight_draw_resources", maxsplit=1)[1].split("    }", maxsplit=1)[0]
        # host 必须首先检查 owner 生命周期。
        self.assertIn("self.rhi_ensure_active()?", opengl_preflight)
        # host 只能通过只读 pipeline bridge 进入资源预检。
        self.assertIn("self.rhi_pipeline().rhi_preflight_draw_resources(packet)", opengl_preflight)
        # 只读资源检查不得恢复原生 OpenGL context。
        self.assertNotIn("rhi_make_current", opengl_preflight)
        # D3D11 必须使用同一共享表并先验证真实 pipeline 绑定。
        self.assertIn("RhiBufferResourceTable<D3d11RhiBuffer>", d3d11)
        # D3D11 的 pipeline 身份校验同样必须早于 Buffer 预检。
        self.assertLess(
            # 定位 D3D11 的共享 pipeline 查询。
            d3d11.index("self.rhi_device.pipeline(packet.pipeline)?"),
            # 定位 D3D11 的共享 Buffer 表委托。
            d3d11.index("self.rhi_device.buffers.validate_draw(packet)"),
        )

    # RHI 必须用封闭描述与上传值对象替代松散字段和参数。
    def test_rhi_owns_typed_buffer_description_and_upload(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取共享 Buffer Component。
        buffer = BUFFER.read_text(encoding="utf-8")
        # 读取 FramePlan 命令闭集。
        frame_plan = FRAME_PLAN.read_text(encoding="utf-8")
        # 读取 FramePlan 执行边界。
        execution = FRAME_EXECUTION.read_text(encoding="utf-8")
        # 组合入口必须重导出描述、用途与上传值对象。
        self.assertIn("BufferDesc, BufferUsage, RhiBufferUpload", rhi)
        # Device 原语只能接收不可拆的上传命令。
        self.assertIn("_upload: RhiBufferUpload<'_>", rhi)
        # 共享 Component 必须拥有封闭 Buffer 描述。
        self.assertIn("pub(crate) struct BufferDesc", buffer)
        # 描述字段不得重新向调用方公开。
        self.assertNotIn("pub(crate) size_bytes:", buffer)
        # 顶点、索引与 Uniform 必须通过用途化构造器创建。
        for constructor in ("fn vertex(", "fn index(", "fn uniform("):
            # 每一种用途都必须有独立封闭入口。
            self.assertIn(constructor, buffer)
        # 上传命令必须同时保存目标身份与不可变载荷。
        self.assertIn("pub(crate) struct RhiBufferUpload<'a>", buffer)
        # FramePlan 顶点命令不再暴露从未使用的裸字节偏移。
        self.assertNotIn("offset: usize", frame_plan)
        # 顶点与 Uniform 都只能在唯一执行边界组装上传值对象。
        self.assertEqual(execution.count("RhiBufferUpload::new(*buffer, &bytes)"), 2)

    # 两个 Adapter 创建资源前必须消费同一描述门禁与原生投影。
    def test_adapters_share_buffer_creation_domain(self) -> None:
        # 读取共享 Buffer Component。
        buffer = BUFFER.read_text(encoding="utf-8")
        # 读取 OpenGL Adapter。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 D3D11 Adapter。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 共同容量上限必须由共享有符号原生值域决定。
        self.assertIn("self.size_bytes > i32::MAX as usize", buffer)
        # Uniform 的十六字节 ABI 必须由共享门禁拥有。
        self.assertIn("self.size_bytes.is_multiple_of(16)", buffer)
        # 元素容量与步长对齐必须由共享门禁拥有。
        self.assertIn("self.size_bytes % self.stride_bytes as usize", buffer)
        # 两个 Adapter 都必须在创建原生资源前验证描述。
        self.assertIn("let native_desc = desc.validate()?;", opengl)
        # D3D11 必须调用同一个共享验证入口。
        self.assertIn("let native = desc.validate()?;", d3d11)
        # OpenGL 容量只能来自共享有符号投影。
        self.assertIn("native_desc.size_bytes_i32()", opengl)
        # D3D11 容量只能来自共享无符号投影。
        self.assertIn("native.size_bytes_u32()", d3d11)
        # OpenGL 资源表必须保存完整共享描述。
        self.assertIn("desc: BufferDesc", opengl)
        # OpenGL 资源表不得继续复制容量字段。
        self.assertNotIn("size_bytes: usize,", opengl)
        # D3D11 资源表也必须保存完整共享描述。
        self.assertIn("desc: BufferDesc", d3d11)
        # D3D11 资源表不得继续复制步长字段。
        self.assertNotIn("stride_bytes: u32,", d3d11)
        # Adapter 不得继续维护各自的容量上限。
        self.assertNotIn("desc.size_bytes == 0", opengl + d3d11)
        # Adapter 不得继续私有解释 Uniform 对齐。
        self.assertNotIn("desc.size_bytes % 16", opengl + d3d11)

    # 两个 Adapter 更新资源时必须机械消费同一上传验证结果。
    def test_adapters_share_upload_range_and_uniform_semantics(self) -> None:
        # 读取共享 Buffer Component。
        buffer = BUFFER.read_text(encoding="utf-8")
        # 读取 OpenGL Adapter。
        opengl = OPENGL.read_text(encoding="utf-8")
        # 读取 OpenGL bridge。
        opengl_bridge = OPENGL_BRIDGE.read_text(encoding="utf-8")
        # 读取 D3D11 Adapter。
        d3d11 = D3D11.read_text(encoding="utf-8")
        # 空上传拒绝必须由共享门禁拥有。
        self.assertIn("if self.data.is_empty()", buffer)
        # 上传验证必须独立复用描述门禁，禁止非法描述参与原生窄化。
        self.assertEqual(buffer.count("desc.validate()?;"), 1)
        # Uniform 整块替换规则必须由共享门禁拥有。
        self.assertIn("self.data.len() != desc.size_bytes", buffer)
        # OpenGL 必须先解析句柄再验证同一上传值对象。
        self.assertIn("upload.validate(buffer.desc)?", opengl)
        # D3D11 必须消费相同验证入口。
        self.assertIn("upload.validate(resource.desc)?", d3d11)
        # OpenGL 上传固定从零开始，不再窄化调用方偏移。
        self.assertIn("buffer_sub_data_u8_slice(glow::ARRAY_BUFFER, 0, data)", opengl)
        # D3D11 box 右边界必须来自共享已验证投影。
        self.assertIn("right: validated.size_bytes_u32()", d3d11)
        # 两个 Adapter 都不得重新计算上传末端。
        self.assertNotIn("checked_add(data.len())", opengl + d3d11)
        # 两个 Adapter 都不得直接窄化上传偏移或末端。
        self.assertNotIn("offset as i32", opengl)
        # D3D11 不得直接窄化上传末端。
        self.assertNotIn("end as u32", d3d11)
        # owner-thread bridge 必须保持上传命令原子性。
        self.assertIn("rhi.update_buffer(gl, upload)", opengl_bridge)


# 支持直接执行该精确契约测试。
if __name__ == "__main__":
    # 运行本文件定义的测试。
    unittest.main()
