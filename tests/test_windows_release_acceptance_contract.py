# 引入标准单元测试框架。
import unittest
# 引入稳定路径拼接。
from pathlib import Path

# 定位仓库根目录与 Windows 验收入口。
ROOT = Path(__file__).resolve().parents[1]
ACCEPTANCE = ROOT / "scripts" / "accept_windows_release.ps1"


# 冻结真实 Windows 候选包验收入口的必要门禁。
class WindowsReleaseAcceptanceContractTests(unittest.TestCase):
    # 每个用例读取同一入库脚本。
    def setUp(self) -> None:
        # 使用 UTF-8 保持中文注释与稳定命令文本。
        self.script = ACCEPTANCE.read_text(encoding="utf-8")

    # 验收必须拒绝错误主机、错误分支或未同步提交。
    def test_requires_windows_x64_clean_synced_main(self) -> None:
        # 必须使用运行时平台事实，而不是调用方传入的字符串。
        self.assertIn("OSPlatform]::Windows", self.script)
        # 首发 Windows 架构固定为 x64。
        self.assertIn("Architecture]::X64", self.script)
        # 未提交与未跟踪文件都必须进入工作树门禁。
        self.assertIn("status --porcelain=v1 --untracked-files=all", self.script)
        # 候选包只能从远端主分支的精确提交生成。
        self.assertIn("requires branch main", self.script)
        self.assertIn("requires HEAD to equal origin/main", self.script)

    # Vulkan 主路径与 D3D11 回退必须在真实 Windows 环境分别验收。
    def test_runs_both_native_graphics_parity_suites(self) -> None:
        # 首选 Vulkan 使用共享真实 GPU harness。
        self.assertIn("'--features', 'vulkan-parity-test'", self.script)
        self.assertIn("'--test', 'vulkan_gpu_parity'", self.script)
        # 兼容 D3D11 使用同一共享规范的真实 harness。
        self.assertIn("'--features', 'd3d11-parity-test'", self.script)
        self.assertIn("'--test', 'd3d11_gpu_parity'", self.script)

    # 主演示必须经默认自动策略完成最终 surface 像素验收。
    def test_runs_automatic_demo_surface_readback(self) -> None:
        # 运行 test-harness 不得显式固定另一个图形 API。
        self.assertIn("--features test-harness", self.script)
        # 专用入口会在成功或失败后自行退出，适合无人值守 runner。
        self.assertIn("--test-graphics-readback", self.script)
        # 退出码之外还必须检查稳定成功标记。
        self.assertIn("UIX_GRAPHICS_READBACK_OK", self.script)

    # 候选 ZIP 必须两次生成、再次校验并比较完整容器摘要。
    def test_proves_archive_reproducibility(self) -> None:
        # 同一低层构建器必须被执行两次。
        self.assertEqual(self.script.count("'build_internal_release.ps1'"), 2)
        # 最终保留的第二份 ZIP必须再次经过独立校验。
        self.assertIn("'verify_internal_release.ps1'", self.script)
        # 两次 SHA-256 不一致必须终止验收。
        self.assertIn("if ($firstHash -ne $secondHash)", self.script)

    # 成功证据必须包含提交、环境、图形与候选包身份。
    def test_writes_non_secret_machine_evidence(self) -> None:
        # 证据使用 JSON，便于 Gitea 或后续 runner 直接读取。
        self.assertIn("ConvertTo-Json -Depth 6", self.script)
        # 关键事实必须全部存在。
        for field in (
            "commit = $commit",
            "operating_system =",
            "toolchain =",
            "graphics =",
            "artifact =",
            "sha256 = $secondHash",
            "reproducible = $true",
        ):
            with self.subTest(field=field):
                self.assertIn(field, self.script)
        # 成功结尾必须提供机器可识别标记。
        self.assertIn("UIX_WINDOWS_ACCEPTANCE_OK", self.script)


# 允许直接运行当前契约测试。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
