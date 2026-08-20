# 声明测试文件使用 UTF-8 编码。
# -*- coding: utf-8 -*-
# 说明本测试守护测试实现只能位于根 tests 目录的边界。
"""Guard the repository convention that test implementations live in tests."""

# 启用延迟解析类型标注。
from __future__ import annotations

# 引入正则表达式以识别 Rust 测试属性。
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
# 定位不得承载测试实现的产品源码目录。
SOURCE_ROOT = ROOT / "src"
# 排除版本库元数据与构建产物。
IGNORED_PARTS = {".git", "target"}
# 固化 Python 独立测试文件命名。
PYTHON_TEST_PATTERNS = ("test_*.py", "*_test.py", "*_tests.py")
# 固化 Rust 测试源码文件命名。
RUST_TEST_PATTERNS = ("test_*.rs", "*_test.rs", "*_tests.rs")
# 固化不得位于产品源码树的测试目录名。
RUST_TEST_DIRECTORY_NAMES = {"test", "tests", "test_harness"}
# 识别 Rust 内建及属性宏测试入口。
RUST_TEST_ATTRIBUTE = re.compile(
    # 允许普通 test 与命名空间限定的测试属性。
    r"#\s*\[\s*(?:[A-Za-z_][A-Za-z0-9_]*::)?test\s*\]"
)
# 固化当前独立 Rust 集成入口只承载公开 API 契约的命名后缀。
PUBLIC_API_TEST_SUFFIX = "_public_api.rs"


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


# 收集产品 src 目录中的 Rust 测试命名文件。
def rust_test_sources_in_source(source_root: Path, repository_root: Path) -> list[str]:
    # 保存违反统一 tests 目录边界的 Rust 测试路径。
    outside: set[Path] = set()
    # 检查产品源码树中的全部 Rust 测试命名文件。
    for path in repository_files(source_root, RUST_TEST_PATTERNS):
        # 保存违反文件命名边界的规范路径。
        outside.add(path)
    # 检查产品源码树中的全部 Rust 文件。
    for path in source_root.rglob("*.rs"):
        # 计算相对产品源码根的目录层级。
        directories = set(path.relative_to(source_root).parts[:-1])
        # 测试目录中的所有实现都必须迁到根 tests 目录。
        if RUST_TEST_DIRECTORY_NAMES.intersection(directories):
            # 保存违反目录命名边界的规范路径。
            outside.add(path.resolve())
    # 返回稳定排序结果。
    return sorted(
        # 将规范路径转换为稳定仓库相对路径。
        path.relative_to(repository_root.resolve()).as_posix()
        # 逐项转换全部违规路径。
        for path in outside
    )


# 收集产品 src 目录中仍声明测试入口的 Rust 文件。
def rust_sources_with_test_attributes(source_root: Path, repository_root: Path) -> list[str]:
    # 保存带测试属性的源码相对路径。
    violations: list[str] = []
    # 遍历产品源码树中的全部 Rust 文件。
    for path in sorted(source_root.rglob("*.rs")):
        # 读取源码以识别真实测试属性。
        source = path.read_text(encoding="utf-8")
        # 只登记仍包含测试入口属性的文件。
        if RUST_TEST_ATTRIBUTE.search(source):
            # 保存相对仓库根目录的稳定诊断路径。
            violations.append(path.relative_to(repository_root.resolve()).as_posix())
    # 返回稳定排序结果。
    return violations


# 定义测试布局约定的回归测试。
class TestLayoutConvention(unittest.TestCase):
    # 确认 Python 独立测试全部位于 tests 目录。
    def test_standalone_python_tests_are_under_tests(self) -> None:
        # 收集当前仓库违规路径。
        outside = outside_python_tests(ROOT, TESTS_ROOT)
        # 仓库不得存在 tests 外 Python 测试入口。
        self.assertEqual(outside, [])

    # 确认产品 src 不再持有 Rust 测试命名文件。
    def test_rust_test_sources_are_under_tests(self) -> None:
        # 收集产品源码树中的 Rust 测试命名文件。
        outside = rust_test_sources_in_source(SOURCE_ROOT, ROOT)
        # 测试源码只能由根 tests 目录持有。
        self.assertEqual(outside, [])

    # 确认产品 src 不再直接声明 Rust 测试入口。
    def test_rust_test_attributes_are_under_tests(self) -> None:
        # 收集仍含测试属性的产品源码文件。
        outside = rust_sources_with_test_attributes(SOURCE_ROOT, ROOT)
        # 测试函数只能由根 tests 目录持有。
        self.assertEqual(outside, [])

    # 验证模块接线不能豁免 tests 目录归属规则。
    def test_module_wired_rust_test_source_is_rejected(self) -> None:
        # 创建完全隔离的临时仓库。
        with tempfile.TemporaryDirectory() as temp_dir:
            # 定位隔离仓库根目录。
            root = Path(temp_dir)
            # 创建 Rust 源码目录。
            source_root = root / "src"
            # 建立源码根。
            source_root.mkdir()
            # 创建测试支撑目录以验证目录名同样受约束。
            harness_root = source_root / "test_harness"
            # 建立测试支撑 fixture 目录。
            harness_root.mkdir()
            # 写入普通命名的测试支撑实现。
            (harness_root / "fake.rs").write_text(
                # fixture 只需一个合法 Rust 项。
                "pub fn fake() {}\n",
                # 使用稳定 UTF-8 编码写入 fixture。
                encoding="utf-8",
            )
            # 写入被模块树接线的测试命名文件。
            (source_root / "direct_tests.rs").write_text(
                # fixture 只需一个合法 Rust 项。
                "pub fn marker() {}\n",
                # 使用稳定 UTF-8 编码写入 fixture。
                encoding="utf-8",
            )
            # 写入普通模块接线。
            (source_root / "lib.rs").write_text(
                # 直接模块声明绑定同名文件。
                "#[cfg(test)]\nmod direct_tests;\n",
                # 使用稳定 UTF-8 编码写入 fixture。
                encoding="utf-8",
            )
            # 显式接线不能让测试文件继续留在 src。
            self.assertEqual(
                # 检查隔离产品源码树。
                rust_test_sources_in_source(source_root, root),
                # 返回精确的违规相对路径。
                ["src/direct_tests.rs", "src/test_harness/fake.rs"],
            )

    # 验证普通命名源码中的测试属性同样被拒绝。
    def test_rust_test_attribute_in_source_is_rejected(self) -> None:
        # 创建完全隔离的临时仓库。
        with tempfile.TemporaryDirectory() as temp_dir:
            # 定位隔离仓库根目录。
            root = Path(temp_dir)
            # 创建 Rust 源码目录。
            source_root = root / "src"
            # 建立源码根。
            source_root.mkdir()
            # 在普通命名源码中写入测试入口。
            (source_root / "component.rs").write_text(
                # fixture 直接声明一个测试函数。
                "#[test]\nfn misplaced() {}\n",
                # 使用稳定 UTF-8 编码写入 fixture。
                encoding="utf-8",
            )
            # 普通文件名不能规避测试属性扫描。
            self.assertEqual(
                # 检查隔离产品源码树。
                rust_sources_with_test_attributes(source_root, root),
                # 返回精确的违规相对路径。
                ["src/component.rs"],
            )

    # 确认独立 Rust 集成入口只保留公开 API 契约测试。
    def test_rust_integration_entrypoints_are_public_api_contracts(self) -> None:
        # 收集 tests 根目录下由 Cargo 自动发现的 Rust 集成入口。
        entrypoints = sorted(path for path in TESTS_ROOT.glob("*.rs") if path.is_file())
        # 当前仓库必须至少保留一个公开 API 集成入口。
        self.assertNotEqual(entrypoints, [])
        # 收集仍使用旧聚合入口或非公开契约命名的文件。
        violations = [
            # 使用文件名生成跨机器稳定诊断。
            path.name
            # 逐个检查 Cargo 自动发现的集成入口。
            for path in entrypoints
            # 只接受当前仓库决策明确保留的公开 API 契约后缀。
            if not path.name.endswith(PUBLIC_API_TEST_SUFFIX)
        ]
        # 不锁死入口数量，只拒绝重新引入非公开集成测试入口。
        self.assertEqual(violations, [])


# 允许直接执行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
