# 引入 SHA-256 以生成测试清单。
import hashlib
# 引入 PowerShell 子进程调用。
import shutil
# 引入外部校验器进程执行。
import subprocess
# 引入隔离测试目录。
import tempfile
# 引入标准单元测试框架。
import unittest
# 引入稳定路径拼接。
from pathlib import Path
# 引入 ZIP fixture 创建能力。
from zipfile import ZIP_DEFLATED, ZipFile

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位被测 PowerShell 校验器。
VERIFIER = ROOT / "scripts" / "verify_internal_release.ps1"
# 定位当前 Windows PowerShell 运行时。
POWERSHELL = shutil.which("powershell")
# 固化清单必须覆盖的七个 payload 顺序。
PAYLOAD_NAMES = (
    # Windows release Demo。
    "uix-demo.exe",
    # 当前版本内部 crate。
    "uix-0.0.1.crate",
    # 专有许可。
    "LICENSE",
    # 第三方声明。
    "THIRD_PARTY_NOTICES.md",
    # 版本变更记录。
    "CHANGELOG.md",
    # 使用入口。
    "README.md",
    # Demo 运行时图片。
    "assets/images/demo.png",
)


# 只在 Windows PowerShell 可用时执行发布包行为测试。
@unittest.skipIf(POWERSHELL is None, "Windows PowerShell is required")
# 验证内部 ZIP 的允许与拒绝边界。
class InternalReleaseContractTests(unittest.TestCase):
    # 为每个用例创建独立目录。
    def setUp(self) -> None:
        # 持有临时目录生命周期。
        self._temp_dir = tempfile.TemporaryDirectory()
        # 保存便于构造 fixture 的路径。
        self.temp_dir = Path(self._temp_dir.name)

    # 用例结束后删除隔离目录。
    def tearDown(self) -> None:
        # 释放临时目录及其中 ZIP。
        self._temp_dir.cleanup()

    # 构造稳定且彼此不同的测试 payload。
    def valid_payload(self) -> dict[str, bytes]:
        # 每个名称映射为包含名称的可复现字节。
        return {name: f"payload:{name}".encode("ascii") for name in PAYLOAD_NAMES}

    # 为给定 payload 生成严格顺序的 SHA-256 清单。
    def manifest_for(self, payload: dict[str, bytes]) -> bytes:
        # 按冻结顺序生成小写摘要与两个空格分隔。
        lines = [f"{hashlib.sha256(payload[name]).hexdigest()}  {name}" for name in payload]
        # 使用 CRLF 与最终换行模拟正式构建器输出。
        return ("\r\n".join(lines) + "\r\n").encode("ascii")

    # 写入一个可定制条目的 ZIP fixture。
    def write_zip(self, name: str, entries: list[tuple[str, bytes]]) -> Path:
        # 计算当前 fixture 路径。
        path = self.temp_dir / name
        # 创建使用常规压缩的 ZIP。
        with ZipFile(path, "w", compression=ZIP_DEFLATED) as archive:
            # 保留调用方顺序并允许构造重复条目。
            for entry_name, content in entries:
                # 写入单个原始 archive 条目。
                archive.writestr(entry_name, content)
        # 返回已关闭的 ZIP 路径。
        return path

    # 把规范图片条目改写为等长反斜杠名称以模拟 Compress-Archive 输出。
    def rewrite_image_entry_with_backslashes(self, path: Path) -> None:
        # 读取仅位于测试临时目录的 ZIP 原始字节。
        archive_bytes = path.read_bytes()
        # 保存本地文件头与中央目录中的规范名称。
        canonical_name = b"assets/images/demo.png"
        # 保存与规范名称等长的反斜杠形式。
        backslash_name = b"assets\\images\\demo.png"
        # 同一条目名称必须恰好出现在本地文件头与中央目录各一次。
        self.assertEqual(archive_bytes.count(canonical_name), 2)
        # 只改写名称字节，不改变 ZIP offset 或长度。
        path.write_bytes(archive_bytes.replace(canonical_name, backslash_name))

    # 调用真实 PowerShell 校验器。
    def run_verifier(self, path: Path) -> subprocess.CompletedProcess[str]:
        # 以非交互方式运行被测脚本并捕获完整诊断。
        return subprocess.run(
            # 传入固定版本与当前 fixture。
            [POWERSHELL, "-NoProfile", "-File", str(VERIFIER), "-ZipPath", str(path)],
            # 从仓库根运行以保持错误路径稳定。
            cwd=ROOT,
            # 捕获标准输出与错误输出。
            capture_output=True,
            # 以文本形式解码诊断。
            text=True,
            # 由测试显式断言退出码。
            check=False,
        )

    # 创建包含精确 payload 与清单的合法条目列表。
    def valid_entries(self) -> list[tuple[str, bytes]]:
        # 构造稳定 payload。
        payload = self.valid_payload()
        # 先按冻结顺序添加七个载荷。
        entries = list(payload.items())
        # 最后添加不自哈希的清单。
        entries.append(("SHA256SUMS.txt", self.manifest_for(payload)))
        # 返回完整八项包结构。
        return entries

    # 验证规范 ZIP 可以通过全部门禁。
    def test_accepts_exact_payload_and_hash_manifest(self) -> None:
        # 写入合法候选包。
        path = self.write_zip("valid.zip", self.valid_entries())
        # 执行真实校验器。
        result = self.run_verifier(path)
        # 合法包必须返回成功。
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        # 成功输出必须给出稳定确认。
        self.assertIn("Verified internal release", result.stdout)

    # 验证缺失与额外载荷都会失败。
    def test_rejects_missing_and_extra_entries(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 分别描述缺失与额外两种漂移。
        cases = {
            # 删除 README 形成缺失载荷。
            "missing": [entry for entry in entries if entry[0] != "README.md"],
            # 加入未声明文件形成额外载荷。
            "extra": entries + [("unexpected.txt", b"unexpected")],
        }
        # 逐项验证两类集合漂移。
        for case, case_entries in cases.items():
            # 让失败名称进入子测试诊断。
            with self.subTest(case=case):
                # 写入当前非法 fixture。
                path = self.write_zip(f"{case}.zip", case_entries)
                # 执行真实校验器。
                result = self.run_verifier(path)
                # 非精确条目集合必须失败。
                self.assertNotEqual(result.returncode, 0)
                # 失败必须定位条目数量契约。
                self.assertIn("entry count mismatch", result.stdout + result.stderr)

    # 验证危险与非规范路径都会失败。
    def test_rejects_unsafe_and_backslash_paths(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 分别描述路径穿越与 Windows 私有分隔。
        cases = {
            # 加入父目录穿越条目。
            "traversal": (entries + [("../escape.txt", b"escape")], False),
            # 反斜杠用例先创建规范 ZIP，再等长改写原始名称。
            "backslash": (entries, True),
        }
        # 逐项验证两类路径风险。
        for case, (case_entries, rewrite_backslashes) in cases.items():
            # 让失败名称进入子测试诊断。
            with self.subTest(case=case):
                # 写入当前非法 fixture。
                path = self.write_zip(f"{case}.zip", case_entries)
                # 仅为反斜杠用例改写 ZIP 名称字节。
                if rewrite_backslashes:
                    # 模拟 Windows Compress-Archive 的非规范嵌套路径。
                    self.rewrite_image_entry_with_backslashes(path)
                # 执行真实校验器。
                result = self.run_verifier(path)
                # 非规范路径必须失败。
                self.assertNotEqual(result.returncode, 0)
                # 失败必须来自路径门禁。
                self.assertRegex(result.stdout + result.stderr, "unsafe path segment|not canonical")

    # 验证仅大小写不同的重复条目也会失败。
    def test_rejects_case_insensitive_duplicate_entry(self) -> None:
        # 在合法包后加入 Windows 语义下重复的 Demo 名称。
        entries = self.valid_entries() + [("UIX-DEMO.EXE", b"duplicate")]
        # 写入重复条目 fixture。
        path = self.write_zip("duplicate.zip", entries)
        # 执行真实校验器。
        result = self.run_verifier(path)
        # 重复路径必须失败。
        self.assertNotEqual(result.returncode, 0)
        # 失败必须明确指出重复条目。
        self.assertIn("duplicate entry", result.stdout + result.stderr)

    # 验证清单哈希必须匹配解包后的真实字节。
    def test_rejects_incorrect_payload_hash(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 用语法正确但内容错误的首项 SHA-256 替换清单。
        manifest = entries[-1][1].replace(entries[-1][1][:64], b"0" * 64, 1)
        # 保存错误清单到最后一个条目。
        entries[-1] = ("SHA256SUMS.txt", manifest)
        # 写入错误摘要 fixture。
        path = self.write_zip("wrong-hash.zip", entries)
        # 执行真实校验器。
        result = self.run_verifier(path)
        # 错误哈希必须失败。
        self.assertNotEqual(result.returncode, 0)
        # 失败必须定位到内容摘要。
        self.assertIn("hash mismatch", result.stdout + result.stderr)


# 允许直接运行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
