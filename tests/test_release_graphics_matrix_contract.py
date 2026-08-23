# 引入正则表达式以读取平台 registry 的邻接字段。
import re
# 引入标准 TOML 解析器以读取 Cargo feature。
import tomllib
# 引入标准单元测试框架。
import unittest
# 引入稳定路径拼接。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 固化 0.0.1 首发图形 feature 顺序。
BACKEND_FEATURES = ["vulkan", "d3d11", "opengles"]


# 冻结首发平台、feature 与自动选择优先级的一致性。
class ReleaseGraphicsMatrixContractTests(unittest.TestCase):
    # 读取 TOML 文件。
    def load_toml(self, path: Path) -> dict:
        # 使用 UTF-8 与标准解析器，避免以字符串猜测 manifest 结构。
        return tomllib.loads(path.read_text(encoding="utf-8"))

    # 根包与主演示必须编译同一首发后端集合。
    def test_manifests_enable_the_release_backend_matrix(self) -> None:
        # 读取根包默认 feature。
        root_default = self.load_toml(ROOT / "Cargo.toml")["features"]["default"]
        # 后端必须位于默认集合开头，保持首选与兼容回退的可读顺序。
        self.assertEqual(root_default[:3], BACKEND_FEATURES)
        # 读取主演示对根包显式启用的 feature。
        demo_features = self.load_toml(ROOT / "demo" / "uix-lang-demo" / "Cargo.toml")[
            "dependencies"
        ]["uix"]["features"]
        # 主演示必须使用与正式候选包相同的首发后端顺序。
        self.assertEqual(demo_features[:3], BACKEND_FEATURES)

    # 使用 --locked 的两个发布工作区必须随仓库携带锁文件。
    def test_release_workspaces_ship_lockfiles(self) -> None:
        # 根工作区与独立 Demo 工作区都参与候选包构建。
        lockfiles = (ROOT / "Cargo.lock", ROOT / "demo" / "Cargo.lock")
        # 每个锁文件必须存在、使用当前格式并包含 UIX package。
        for path in lockfiles:
            # 解析锁文件，避免只验证一个空占位文件。
            lock = self.load_toml(path)
            # Cargo 1.78 及后续生成的锁文件使用版本 4。
            self.assertEqual(lock["version"], 4, path)
            # 锁定图必须包含正式 UIX package 身份。
            self.assertTrue(
                any(package.get("name") == "uix" for package in lock["package"]),
                path,
            )

    # 从一个平台 registry 读取 API 到优先级的映射。
    def registry_priorities(self, relative_path: str) -> dict[str, int]:
        # 读取平台私有 registry 源码。
        source = (ROOT / relative_path).read_text(encoding="utf-8")
        # 每个条目必须在同一结构体内先声明 API、再声明优先级。
        pairs = re.findall(
            r"id:\s*GraphicsApi::(\w+),\s*priority:\s*(\d+),",
            source,
            flags=re.MULTILINE,
        )
        # 转换为便于精确比较的稳定映射。
        return {api: int(priority) for api, priority in pairs}

    # 自动选择必须在两个首发平台统一 Vulkan-first。
    def test_platform_registries_freeze_vulkan_first_priorities(self) -> None:
        # Windows 先选 Vulkan，再按 D3D11、OpenGL ES 顺序兼容回退。
        self.assertEqual(
            self.registry_priorities("src/native/factory/registry_windows.rs"),
            {"D3d11": 30, "Vulkan": 100, "OpenGlEs": 10},
        )
        # Linux 先选 Vulkan，再兼容回退到 OpenGL ES。
        self.assertEqual(
            self.registry_priorities("src/native/factory/registry_linux.rs"),
            {"Vulkan": 100, "OpenGlEs": 10},
        )

    # 使用方入口与交付契约必须陈述同一首发矩阵。
    def test_release_documents_state_the_same_matrix(self) -> None:
        # 固化必须同步的平台职责关键词。
        required_fragments = ("Vulkan", "Windows", "D3D11", "Linux", "OpenGL ES", "兼容回退")
        # 这些文件分别承担入口、产品与主演示契约。
        paths = (
            ROOT / "README.md",
            ROOT / "docs" / "产品" / "交付与许可.md",
            ROOT / "demo" / "uix-lang-demo" / "README.md",
        )
        # 每个权威入口都必须完整覆盖首发矩阵。
        for path in paths:
            # 读取当前文档文本。
            text = path.read_text(encoding="utf-8")
            # 逐项报告缺失关键词并保留文件定位。
            for fragment in required_fragments:
                with self.subTest(path=path.relative_to(ROOT), fragment=fragment):
                    self.assertIn(fragment, text)


# 允许直接执行当前契约测试。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
