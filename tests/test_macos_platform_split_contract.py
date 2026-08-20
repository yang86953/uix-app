# 验证 macOS platform 拆分不会再次遗落类型属性或复制导入。
"""Guard the macOS platform split boundary."""

# 引入标准单元测试框架。
import unittest
# 引入稳定的仓库路径解析能力。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位拆分后的 macOS presenter 实现。
PRESENTER = ROOT / "src/native/backends/macos/presentation/presenter.rs"
# 定位拆分后的 macOS 应用事件实现。
APP_EVENT = ROOT / "src/native/backends/macos/windowing/app_event.rs"


# 聚合 macOS platform 拆分边界的源码契约。
class MacosPlatformSplitContractTests(unittest.TestCase):
    # 锁定 presenter 不再保留孤立属性或重复父模块导入。
    def test_presenter_has_one_parent_import_and_no_orphan_derive(self) -> None:
        # 读取 presenter 私有实现。
        presenter = PRESENTER.read_text(encoding="utf-8")
        # 拆分文件只允许借用一次父模块私有项。
        self.assertEqual(presenter.count("use super::*;"), 1)
        # 文件尾部不得再次留下无法附着到类型的 derive 属性。
        self.assertFalse(presenter.rstrip().endswith("#[derive(Debug, Clone)]"))

    # 锁定拆分前的 Debug 与 Clone 契约仍由 MacosAppEvent 持有。
    def test_app_event_owns_original_derive_and_one_parent_import(self) -> None:
        # 读取应用事件私有实现。
        app_event = APP_EVENT.read_text(encoding="utf-8")
        # 拆分文件只允许借用一次父模块私有项。
        self.assertEqual(app_event.count("use super::*;"), 1)
        # 原属性必须直接附着到父 platform Module 可见的事件类型。
        self.assertIn("#[derive(Debug, Clone)]\npub(super) struct MacosAppEvent", app_event)


# 支持直接运行该源码契约文件。
if __name__ == "__main__":
    # 执行本文件中的 unittest 契约。
    unittest.main()
