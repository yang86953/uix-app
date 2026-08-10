# 导入正则表达式以识别 Rust 文档注释。
import re
# 导入临时目录以构造隔离的守卫夹具。
from tempfile import TemporaryDirectory
# 导入单元测试框架以接入现有 Python 测试套件。
import unittest
# 导入路径类型以遍历 Rust 源码目录。
from pathlib import Path


# 指向当前仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 限定需要承担公开文档契约的 Rust 源码根目录。
SOURCE_DIRECTORIES = (Path("src"), Path("uix-derive/src"))
# 只识别模块级和条目级 Rust 文档注释。
RUST_DOC_COMMENT = re.compile(r"^\s*(?://!|///)")
# 固定旧 UIX App 私有知识库的规范化路径前缀。
LEGACY_UIX_VAULT_PREFIX = "c:/data/note/我的项目/软件/uix app"


# 收集指定仓库根目录下仍指向私有 UIX App 知识库的 Rustdoc 行。
def find_legacy_rustdoc_paths(root: Path) -> list[str]:
    # 保存可直接定位到文件和行号的违规项。
    violations: list[str] = []
    # 逐个扫描由仓库维护的 Rust 源码根目录。
    for relative_directory in SOURCE_DIRECTORIES:
        # 解析当前源码根目录的绝对位置。
        source_directory = root / relative_directory
        # 允许精简夹具只创建需要测试的源码根目录。
        if not source_directory.is_dir():
            # 跳过夹具中不存在的可选源码根目录。
            continue
        # 按稳定顺序遍历所有 Rust 源文件。
        for source_path in sorted(source_directory.rglob("*.rs")):
            # 以 UTF-8 读取源码并保留可报告的行号。
            source_lines = source_path.read_text(encoding="utf-8").splitlines()
            # 逐行检查 Rust 文档注释。
            for line_number, source_line in enumerate(source_lines, start=1):
                # 普通源码、字符串和非文档注释不属于本守卫边界。
                if RUST_DOC_COMMENT.match(source_line) is None:
                    # 继续检查下一行。
                    continue
                # 将 Windows 分隔符和大小写归一化后再判断路径所有权。
                normalized_line = source_line.replace("\\", "/").lower()
                # 只报告旧 UIX App 私有知识库路径。
                if LEGACY_UIX_VAULT_PREFIX in normalized_line:
                    # 使用仓库相对路径生成稳定的失败信息。
                    relative_path = source_path.relative_to(root).as_posix()
                    # 记录违规文件与准确行号。
                    violations.append(f"{relative_path}:{line_number}")
    # 返回所有违规项供测试断言和失败报告使用。
    return violations


# 写入隔离的 Rust 源码夹具。
def write_rust_fixture(root: Path, source: str) -> None:
    # 使用标准 src 目录模拟真实仓库扫描边界。
    source_path = root / "src" / "fixture.rs"
    # 创建夹具文件所需的父目录。
    source_path.parent.mkdir(parents=True)
    # 以 UTF-8 写入待扫描的 Rust 源码。
    source_path.write_text(source, encoding="utf-8")


# 验证私有知识库路径守卫的边界和当前仓库状态。
class SourceDocumentationPathTests(unittest.TestCase):
    # Windows 风格路径必须被识别。
    def test_windows_vault_path_in_rustdoc_is_rejected(self) -> None:
        # 创建不受当前仓库内容影响的临时根目录。
        with TemporaryDirectory() as temporary_directory:
            # 将临时目录转换为路径对象。
            root = Path(temporary_directory)
            # 写入包含 Windows 私有知识库路径的文档注释。
            write_rust_fixture(root, "/// 知识库：`C:\\data\\note\\我的项目\\软件\\UIX App\\使用.md`\n")
            # 断言守卫定位到夹具的首行。
            self.assertEqual(find_legacy_rustdoc_paths(root), ["src/fixture.rs:1"])

    # file URI 风格路径也必须被识别。
    def test_file_uri_vault_path_in_rustdoc_is_rejected(self) -> None:
        # 创建不受当前仓库内容影响的临时根目录。
        with TemporaryDirectory() as temporary_directory:
            # 将临时目录转换为路径对象。
            root = Path(temporary_directory)
            # 写入包含 file URI 私有知识库路径的模块文档注释。
            write_rust_fixture(root, "//! <file:///C:/data/note/我的项目/软件/UIX App/架构/系统列表.md>\n")
            # 断言守卫定位到夹具的首行。
            self.assertEqual(find_legacy_rustdoc_paths(root), ["src/fixture.rs:1"])

    # 普通源码字符串不属于公开 Rustdoc 契约。
    def test_ordinary_source_string_is_ignored(self) -> None:
        # 创建不受当前仓库内容影响的临时根目录。
        with TemporaryDirectory() as temporary_directory:
            # 将临时目录转换为路径对象。
            root = Path(temporary_directory)
            # 写入仅存在于普通 Rust 字符串中的私有路径。
            write_rust_fixture(root, 'const PATH: &str = r"C:\\data\\note\\我的项目\\软件\\UIX App";\n')
            # 断言普通源码字符串不会被守卫误报。
            self.assertEqual(find_legacy_rustdoc_paths(root), [])

    # 当前仓库的 Rustdoc 必须全部使用仓库自有文档目标。
    def test_current_repository_has_no_legacy_rustdoc_paths(self) -> None:
        # 断言实际源码目录不存在旧私有知识库路径。
        self.assertEqual(find_legacy_rustdoc_paths(ROOT), [])


# 支持直接运行本测试文件进行快速验证。
if __name__ == "__main__":
    # 执行本文件定义的全部单元测试。
    unittest.main()
