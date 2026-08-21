# -*- coding: utf-8 -*-
# 验证纹理描述和上传命令由共享 RHI 独占解释，两个 Adapter 只做原生映射。
"""Keep texture resource and upload semantics typed across graphics adapters."""

# 引入标准单元测试框架。
import unittest
# 引入跨平台路径处理。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位薄 RHI 组合入口。
RHI = ROOT / "src/platform/presentation/rhi/mod.rs"
# 定位共享纹理资源契约。
TEXTURE = ROOT / "src/platform/presentation/rhi/texture.rs"
# 定位共享纹理上传契约。
TRANSFER = ROOT / "src/platform/presentation/rhi/transfer.rs"
# 定位 OpenGL 资源表与上传 Adapter。
OPENGL_DEVICE = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device.rs"
# 定位 OpenGL 原生上传 Adapter。
OPENGL_UPLOAD = ROOT / "src/native/presentation/graphics/opengl/raster/rhi_device_upload.rs"
# 定位 D3D11 资源表与上传 Adapter。
D3D11_DEVICE = ROOT / "src/native/presentation/graphics/d3d11/adapter/context/rhi_device.rs"


# 集中锁定纹理资源与上传语义的唯一所有者。
class GraphicsRhiTextureResourceContractTests(unittest.TestCase):
    # 纹理描述必须封闭字段并统一拥有格式布局与目标能力。
    def test_texture_desc_owns_common_resource_facts(self) -> None:
        # 读取共享纹理 Component。
        texture = TEXTURE.read_text(encoding="utf-8")
        # 描述必须通过封闭构造绑定尺寸与格式。
        self.assertIn("pub(crate) const fn new(extent: RhiExtent, format: TextureFormat)", texture)
        # 调用方不得直接写入公开尺寸字段。
        self.assertNotIn("pub(crate) extent: RhiExtent", texture)
        # 调用方不得直接写入公开格式字段。
        self.assertNotIn("pub(crate) format: TextureFormat", texture)
        # 每像素字节宽度必须由共享格式闭集拥有。
        self.assertIn("pub(crate) const fn bytes_per_pixel(self) -> usize", texture)
        # Render target 能力也必须由同一格式闭集拥有。
        self.assertIn("pub(crate) const fn supports_render_target(self) -> bool", texture)
        # 两个 Adapter 共用有符号原生尺寸门禁。
        self.assertIn("pub(crate) fn validate(self) -> Result<RhiTextureNativeDesc>", texture)

    # Device 入口必须只接收把资源、区域与载荷绑定起来的上传命令。
    def test_device_accepts_one_typed_texture_upload(self) -> None:
        # 读取薄 RHI trait。
        rhi = RHI.read_text(encoding="utf-8")
        # 读取共享上传值对象。
        transfer = TRANSFER.read_text(encoding="utf-8")
        # trait 只能保留一个类型化纹理上传入口。
        self.assertIn("fn update_texture(&mut self, _upload: RhiTextureUpload<'_>)", rhi)
        # 旧的子区域旁路不得继续存在。
        self.assertNotIn("fn update_texture_region", rhi)
        # 上传命令必须私有绑定目标纹理。
        self.assertIn("texture: TextureHandle", transfer)
        # 上传命令必须私有绑定完整区域。
        self.assertIn("region: RhiTextureRegion", transfer)
        # 上传命令必须私有绑定载荷借用。
        self.assertIn("data: &'a [u8]", transfer)
        # 共享验证必须统一计算格式字节宽度与紧密布局。
        self.assertIn("tight_payload_layout(desc.format().bytes_per_pixel())", transfer)
        # 精确载荷长度拒绝只能出现在共享 Component。
        self.assertIn("if self.data.len() != required", transfer)

    # 两个 Adapter 必须保存同一描述并机械消费共享验证结果。
    def test_adapters_only_map_validated_texture_commands(self) -> None:
        # 读取 OpenGL 资源表。
        opengl_device = OPENGL_DEVICE.read_text(encoding="utf-8")
        # 读取 OpenGL 上传实现。
        opengl_upload = OPENGL_UPLOAD.read_text(encoding="utf-8")
        # 读取 D3D11 资源表与上传实现。
        d3d11 = D3D11_DEVICE.read_text(encoding="utf-8")
        # 两个资源表都必须只保存完整共享描述。
        self.assertIn("struct OpenGlRhiTexture", opengl_device)
        # OpenGL 纹理资源必须保存统一描述。
        self.assertIn("desc: TextureDesc", opengl_device)
        # D3D11 纹理资源必须保存同一种描述。
        self.assertIn("struct D3d11RhiTexture", d3d11)
        # D3D11 纹理资源必须保存统一描述。
        self.assertIn("desc: TextureDesc", d3d11)
        # OpenGL 上传必须消费共享验证结果。
        self.assertIn("upload.validate(texture.desc)?", opengl_upload)
        # D3D11 上传必须消费同一个共享验证结果。
        self.assertIn("upload.validate(resource.desc)?", d3d11)
        # OpenGL Adapter 不得私自计算期望载荷长度。
        self.assertNotIn("data.len() != expected", opengl_upload)
        # D3D11 Adapter 不得私自维护像素字节宽度。
        self.assertNotIn("bytes_per_pixel", d3d11)


# 支持直接运行这一个精确契约测试。
if __name__ == "__main__":
    # 执行当前文件中的契约测试。
    unittest.main()
