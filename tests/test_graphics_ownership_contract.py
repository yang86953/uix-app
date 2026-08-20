"""锁定 graphics renderer 与 platform presentation 的物理所有权边界。"""

# 引入标准库单元测试框架。
import unittest
# 引入跨平台路径值。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 枚举必须由 graphics backend 独占的 Canvas2D GPU 原语。
PRIMITIVE_TYPES = ("GpuSolidRect", "GpuStrokeRect", "GpuGlyphBlit", "GpuLinearGradientRect", "GpuRadialGradient", "GpuSector", "GpuSolidMesh", "GpuBoxShadow", "GpuImageBlit")

# 校验 graphics 与 platform 的静态所有权边界。
class GraphicsOwnershipContractTests(unittest.TestCase):
    # 验证 renderer 原语不会重新进入 platform presentation。
    def test_canvas_gpu_primitives_belong_to_graphics_backend(self) -> None:
        # 读取 platform presentation 公共契约。
        present = (ROOT / "src/native/presentation/contracts/mod.rs").read_text(encoding="utf-8")
        # 读取 graphics backend 私有原语定义。
        primitives = (ROOT / "src/draw/backend/gpu/primitives.rs").read_text(encoding="utf-8")
        # 逐项锁定原语的唯一物理定义位置。
        for primitive in PRIMITIVE_TYPES:
            # platform presentation 不得声明 renderer 原语。
            self.assertNotIn(f"struct {primitive}", present)
            # graphics backend 必须继续持有 renderer 原语。
            self.assertIn(f"struct {primitive}", primitives)


# 允许直接执行该契约测试。
if __name__ == "__main__":
    # 运行标准库测试入口。
    unittest.main()
