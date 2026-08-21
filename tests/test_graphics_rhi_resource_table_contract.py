# -*- coding: utf-8 -*-
# 验证四类原生资源只通过共享类型化槽位状态机分配、查询和销毁。
"""Keep opaque resource slot semantics identical across graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口与句柄定义。
RHI = ROOT / "src/platform/presentation/rhi/mod.rs"
# 定位共享资源表 Component。
RESOURCE_TABLE = ROOT / "src/platform/presentation/rhi/resource_table.rs"
# 定位 D3D11 资源设备实现。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"
# 定位 D3D11 拆分资源生命周期实现。
D3D11_RESOURCES = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device_resources.rs"
# 定位 OpenGL 资源设备实现。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL 资源查询实现。
OPENGL_RESOURCES = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_resources.rs"


# 集中锁定跨 Adapter 资源身份与槽位生命周期的唯一所有者。
class GraphicsRhiResourceTableContractTests(unittest.TestCase):
    # 四类资源句柄必须显式进入共享表契约。
    def test_resource_handles_are_typed_for_one_shared_table(self) -> None:
        # 读取薄 RHI 组合入口。
        rhi = RHI.read_text(encoding="utf-8")
        # 组合入口必须装配独立资源表 Component。
        self.assertIn("mod resource_table;", rhi)
        # 两个 Adapter 必须取得同一表类型和句柄约束。
        self.assertIn("RhiResourceHandle, RhiResourceTable", rhi)
        # Buffer 必须声明稳定资源种类。
        self.assertIn('opaque_resource_handle!(BufferHandle, "buffer")', rhi)
        # Texture 必须声明稳定资源种类。
        self.assertIn('opaque_resource_handle!(TextureHandle, "texture")', rhi)
        # Sampler 必须声明稳定资源种类。
        self.assertIn('opaque_resource_handle!(SamplerHandle, "sampler")', rhi)
        # Pipeline 必须声明稳定资源种类。
        self.assertIn('opaque_resource_handle!(PipelineHandle, "pipeline")', rhi)
        # RenderTarget 与 Submission 不属于资源槽位，不得错误进入资源表。
        self.assertNotIn("opaque_resource_handle!(RenderTargetHandle", rhi)

    # 共享表必须唯一拥有分配、查询、销毁和逆序关闭语义。
    def test_shared_table_owns_slot_state_transitions(self) -> None:
        # 读取共享资源表实现。
        table = RESOURCE_TABLE.read_text(encoding="utf-8")
        # 槽位必须保持私有，调用方只能使用类型化方法。
        self.assertIn("slots: Vec<Option<T>>", table)
        # 新身份必须由共享 insert 从一开始签发。
        self.assertIn("pub(crate) fn insert(&mut self, resource: T) -> H", table)
        # 只读查询必须进入共享门禁。
        self.assertIn("pub(crate) fn get(&self, handle: H) -> Result<&T>", table)
        # 可变查询必须进入同一共享门禁。
        self.assertIn("pub(crate) fn get_mut(&mut self, handle: H) -> Result<&mut T>", table)
        # 销毁必须检查式取出资源所有权。
        self.assertIn("pub(crate) fn take(&mut self, handle: H) -> Result<T>", table)
        # 原生 owner 关闭必须复用共享逆序清空。
        self.assertIn("pub(crate) fn drain_reverse", table)
        # 统一表必须区分零句柄诊断。
        self.assertIn('format!("RHI {} handle is null", H::KIND)', table)
        # 统一表必须区分陈旧句柄诊断。
        self.assertIn('format!("RHI {} handle is stale", H::KIND)', table)
        # 统一表必须区分重复销毁诊断。
        self.assertIn('format!("RHI {} handle was already destroyed", H::KIND)', table)

    # 两个 Adapter 必须只保存共享表并机械调用其状态转换。
    def test_adapters_do_not_reimplement_slot_semantics(self) -> None:
        # 读取 D3D11 主资源设备。
        d3d11 = D3D11_DEVICE.read_text(encoding="utf-8")
        # 读取 D3D11 拆分资源实现。
        d3d11_resources = D3D11_RESOURCES.read_text(encoding="utf-8")
        # 读取 OpenGL 主资源设备。
        opengl = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 OpenGL 资源查询实现。
        opengl_resources = OPENGL_RESOURCES.read_text(encoding="utf-8")
        # Buffer、Texture、Pipeline 与 Sampler 必须各自由对应共享表持有。
        table_types = (
            # Buffer 表额外拥有真实描述验证。
            "RhiBufferResourceTable<",
            # Texture 表额外拥有格式、目标与传输验证。
            "RhiTextureResourceTable<",
            # Pipeline 表额外拥有句柄与 kind 绑定验证。
            "RhiPipelineResourceTable<",
            # Sampler 尚无额外描述关系，直接复用通用类型化表。
            "RhiResourceTable<SamplerHandle",
        )
        # 两个 Adapter 必须装配完全相同的四类资源表角色。
        for table_type in table_types:
            # 每种类型化资源表在每个 Adapter 中必须只有一个 owner。
            with self.subTest(table_type=table_type):
                # D3D11 不得缺失或复制任何资源表角色。
                self.assertEqual(d3d11.count(table_type), 1)
                # OpenGL 必须保持与 D3D11 相同的资源表分层。
                self.assertEqual(opengl.count(table_type), 1)
        # 两个 Adapter 不得直接保存可漂移的 Option 槽位向量。
        self.assertNotIn("Vec<Option<", d3d11 + opengl)
        # 两个 Adapter 不得自行执行一基到零基的身份投影。
        self.assertNotIn("checked_sub(1)", d3d11 + d3d11_resources + opengl_resources)
        # D3D11 创建与销毁必须调用共享 insert/take。
        self.assertIn(".buffers.insert(D3d11RhiBuffer", d3d11)
        # D3D11 纹理生命周期必须调用同一表方法。
        self.assertIn(".textures.insert(D3d11RhiTexture", d3d11_resources)
        # OpenGL 创建必须调用共享 insert。
        self.assertIn("self.textures.insert(OpenGlRhiTexture", opengl)
        # OpenGL 销毁必须调用共享 take。
        self.assertIn("self.textures.take(handle)?", opengl)
        # OpenGL owner 关闭必须调用共享逆序 drain。
        self.assertEqual(opengl.count(".drain_reverse()"), 4)


# 支持直接执行这一精确契约测试。
if __name__ == "__main__":
    # 运行当前文件定义的契约测试。
    unittest.main()
