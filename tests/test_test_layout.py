# 声明测试文件使用 UTF-8 编码。
# -*- coding: utf-8 -*-
# 说明本测试守护独立测试入口与源码内测试模块的目录边界。
"""Guard the repository convention for standalone and module-wired tests."""

# 启用延迟解析类型标注。
from __future__ import annotations

# 引入正则表达式以识别 Rust 模块接线。
import re
# 引入隔离测试目录。
import tempfile
# 引入标准库单元测试框架。
import unittest
# 引入跨平台路径类型。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位独立测试入口目录。
TESTS_ROOT = ROOT / "tests"
# 排除版本库元数据与构建产物。
IGNORED_PARTS = {".git", "target"}
# 固化 Python 独立测试文件命名。
PYTHON_TEST_PATTERNS = ("test_*.py", "*_test.py", "*_tests.py")
# 固化 Rust 测试源码文件命名。
RUST_TEST_PATTERNS = ("test_*.rs", "*_test.rs", "*_tests.rs")


# 收集匹配一组测试命名模式的仓库文件。
def repository_files(root: Path, patterns: tuple[str, ...]) -> list[Path]:
    # 使用集合避免一个文件同时匹配多个模式时重复报告。
    matches: set[Path] = set()
    # 逐模式扫描当前隔离根目录。
    for pattern in patterns:
        # 遍历当前模式匹配的全部路径。
        for path in root.rglob(pattern):
            # 跳过版本库元数据与构建输出。
            if IGNORED_PARTS.intersection(path.parts):
                # 当前路径不属于仓库源文件事实。
                continue
            # 只登记普通文件。
            if path.is_file():
                # 保存规范绝对路径供后续归属判断。
                matches.add(path.resolve())
    # 返回稳定排序后的路径集合。
    return sorted(matches)


# 判断文件是否位于独立 tests 目录下。
def belongs_to_tests_root(path: Path, tests_root: Path) -> bool:
    # resolve 后通过父目录关系判断，不依赖字符串前缀。
    return tests_root.resolve() in path.resolve().parents


# 收集可能声明或 include 当前 Rust 测试源码的父模块文件。
def rust_declaration_sources(path: Path) -> list[Path]:
    # 同目录任一 Rust 文件都可能通过 mod 或 include 接线测试源码。
    candidates = {candidate.resolve() for candidate in path.parent.glob("*.rs")}
    # 目录模块也可能由上一级同名文件持有。
    parent_module = path.parent.parent / f"{path.parent.name}.rs"
    # 只在真实存在时加入上一级模块文件。
    if parent_module.is_file():
        # 保存父级文件模块候选。
        candidates.add(parent_module.resolve())
    # 被测文件不能把自身误判为接线声明。
    candidates.discard(path.resolve())
    # 返回稳定候选顺序。
    return sorted(candidates)


# 判断 tests 外 Rust 测试源码是否被模块树显式接线。
def rust_test_is_module_wired(path: Path) -> bool:
    # 转义文件 stem 以构造直接 mod 声明模式。
    module_name = re.escape(path.stem)
    # 转义完整文件名以构造 path/include 模式。
    file_name = re.escape(path.name)
    # 识别同名的普通 Rust 文件模块声明。
    direct_module = re.compile(
        # 注释行不会以 mod 开头，因此不形成虚假接线。
        rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+{module_name}\s*;"
    )
    # 识别模块名不同但通过 path 属性显式绑定当前文件的声明。
    path_module = re.compile(
        # path 可以包含相对目录，但必须以当前完整文件名结束。
        rf'#\[\s*path\s*=\s*"[^"]*{file_name}"\s*\]\s*'
        # path 后允许继续声明其它属性，再进入具名模块。
        rf'(?:#\[[^\]]+\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;'
    )
    # 识别直接把当前文件包含进已有测试模块的接线。
    include_module = re.compile(
        # include 路径可以包含相对目录，但必须以当前完整文件名结束。
        rf'(?m)^\s*include!\s*\(\s*"[^"]*{file_name}"\s*\)\s*;'
    )
    # 逐个真实父模块候选检查接线声明。
    for source_path in rust_declaration_sources(path):
        # 读取 Rust 模块源码并移除独占整行的注释。
        source = "\n".join(
            # 保留代码、属性和行尾注释，避免改变声明 token。
            line
            # 逐行检查当前候选源码。
            for line in source_path.read_text(encoding="utf-8").splitlines()
            # 中文逐行说明不能切断 path 属性与 mod 声明的识别。
            if not line.lstrip().startswith("//")
        )
        # 任一种显式接线都证明该文件不是独立测试入口。
        if direct_module.search(source) or path_module.search(source) or include_module.search(source):
            # 当前测试源码已由 Rust 模块树拥有。
            return True
    # 未找到接线时保持为 tests 外独立/孤儿测试。
    return False


# 收集 tests 目录之外的 Python 独立测试文件。
def outside_python_tests(root: Path, tests_root: Path) -> list[str]:
    # 保存仓库相对路径以产生稳定诊断。
    outside: list[str] = []
    # 检查全部 Python 测试命名文件。
    for path in repository_files(root, PYTHON_TEST_PATTERNS):
        # tests 根下文件符合独立入口约定。
        if belongs_to_tests_root(path, tests_root):
            # 当前路径无需报告。
            continue
        # 保存不符合目录约定的路径。
        outside.append(path.relative_to(root.resolve()).as_posix())
    # 返回稳定排序结果。
    return sorted(outside)


# 收集 tests 目录之外且未被模块树接线的 Rust 测试源码。
def outside_unwired_rust_tests(root: Path, tests_root: Path) -> list[str]:
    # 保存真正独立或孤儿的 Rust 测试路径。
    outside: list[str] = []
    # 检查全部 Rust 测试命名文件。
    for path in repository_files(root, RUST_TEST_PATTERNS):
        # tests 根下文件符合独立入口约定。
        if belongs_to_tests_root(path, tests_root):
            # 当前路径无需模块接线证明。
            continue
        # 被模块树显式接线的源码内单元测试不是独立入口。
        if rust_test_is_module_wired(path):
            # 当前模块测试符合 Rust 源码布局。
            continue
        # 保存没有任何所有者的 tests 外测试源码。
        outside.append(path.relative_to(root.resolve()).as_posix())
    # 返回稳定排序结果。
    return sorted(outside)


# 定义测试布局约定的回归测试。
class TestLayoutConvention(unittest.TestCase):
    # 确认 Python 独立测试全部位于 tests 目录。
    def test_standalone_python_tests_are_under_tests(self) -> None:
        # 收集当前仓库违规路径。
        outside = outside_python_tests(ROOT, TESTS_ROOT)
        # 仓库不得存在 tests 外 Python 测试入口。
        self.assertEqual(outside, [])

    # 确认 Rust 测试源码位于 tests 或被模块树显式接线。
    def test_standalone_rust_test_sources_are_under_tests(self) -> None:
        # 收集 tests 外且无模块所有者的 Rust 测试源码。
        outside = outside_unwired_rust_tests(ROOT, TESTS_ROOT)
        # 当前仓库不得存在孤儿 Rust 测试源码。
        self.assertEqual(outside, [])

    # 验证三种合法 Rust 模块接线都被识别。
    def test_module_wired_rust_test_sources_are_allowed(self) -> None:
        # 创建完全隔离的临时仓库。
        with tempfile.TemporaryDirectory() as temp_dir:
            # 定位隔离仓库根目录。
            root = Path(temp_dir)
            # 创建独立 tests 目录。
            tests_root = root / "tests"
            # 建立 tests 根以参与归属判断。
            tests_root.mkdir()
            # 创建 Rust 源码目录。
            source_root = root / "src"
            # 建立源码根。
            source_root.mkdir()
            # 写入三个带测试命名的模块源文件。
            for name in ("direct_tests.rs", "renamed_tests.rs", "included_tests.rs"):
                # 每个 fixture 只需一个合法 Rust 项。
                (source_root / name).write_text("pub fn marker() {}\n", encoding="utf-8")
            # 写入普通 mod、path 重命名与 include 三种接线。
            (source_root / "lib.rs").write_text(
                # 直接模块声明绑定同名文件。
                "#[cfg(test)]\nmod direct_tests;\n"
                # path 属性绑定不同模块名。
                "#[cfg(test)]\n#[path = \"renamed_tests.rs\"]\nmod renamed;\n"
                # include 在已有测试模块内绑定源文件。
                "#[cfg(test)]\nmod included {\n    include!(\"included_tests.rs\");\n}\n",
                # 使用稳定 UTF-8 编码写入 fixture。
                encoding="utf-8",
            )
            # 三种显式接线都不得被判为独立测试入口。
            self.assertEqual(outside_unwired_rust_tests(root, tests_root), [])

    # 验证未接线 Rust 测试源码仍被拒绝。
    def test_unwired_rust_test_source_is_rejected(self) -> None:
        # 创建完全隔离的临时仓库。
        with tempfile.TemporaryDirectory() as temp_dir:
            # 定位隔离仓库根目录。
            root = Path(temp_dir)
            # 创建独立 tests 目录。
            tests_root = root / "tests"
            # 建立 tests 根以参与归属判断。
            tests_root.mkdir()
            # 创建 Rust 源码目录。
            source_root = root / "src"
            # 建立源码根。
            source_root.mkdir()
            # 写入没有任何 mod/path/include 所有者的测试源码。
            (source_root / "orphan_tests.rs").write_text("pub fn orphan() {}\n", encoding="utf-8")
            # 未接线文件必须作为精确相对路径报告。
            self.assertEqual(outside_unwired_rust_tests(root, tests_root), ["src/orphan_tests.rs"])

    # 确认 GFX-R5 集成入口继续位于 tests 目录。
    def test_gfx_r5_integration_entrypoint_is_in_tests(self) -> None:
        # 集成测试入口必须保持为 tests/tests.rs。
        self.assertTrue((TESTS_ROOT / "tests.rs").is_file())


# 允许直接执行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
