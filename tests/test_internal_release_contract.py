# 引入 SHA-256 以生成测试清单。
import hashlib
# 引入 PowerShell 子进程调用。
import shutil
# 引入外部校验器进程执行。
import subprocess
# 引入隔离测试目录。
import tempfile
# 引入标准 TOML 解析器以读取 Cargo package 事实。
import tomllib
# 引入标准单元测试框架。
import unittest
# 引入稳定路径拼接。
from pathlib import Path
# 引入 ZIP fixture 创建能力。
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位被测 PowerShell 校验器。
VERIFIER = ROOT / "scripts" / "verify_internal_release.ps1"
# 定位当前 PowerShell 运行时，并兼容 PowerShell Core 的标准命令名。
POWERSHELL = shutil.which("powershell") or shutil.which("pwsh")
# 固化清单必须覆盖的七个 payload 顺序。
PAYLOAD_NAMES = (
    # uix-lang release Demo。
    "uix-lang-demo.exe",
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
# 固化与生产构建器相同的 ZIP 规范时间。
FIXED_ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)


# 验证内部 crate 身份与发布组合根门禁。
class PackageMetadataContractTests(unittest.TestCase):
    # 固化根 package 的内部交付元数据。
    def test_root_package_has_stable_internal_release_identity(self) -> None:
        # 读取仓库权威 Cargo manifest。
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        # 取得根 package 表。
        package = manifest["package"]
        # crate 名称必须保持为 uix。
        self.assertEqual(package["name"], "uix")
        # 首发候选版本必须保持为 0.0.1。
        self.assertEqual(package["version"], "0.0.1")
        # 内部 crate 不得误开放 crates.io 发布。
        self.assertFalse(package["publish"])
        # 专有许可文件必须继续进入 Cargo package。
        self.assertEqual(package["license-file"], "LICENSE")
        # 描述必须准确表达产品形态与授权边界。
        self.assertEqual(
            # 读取实际 package 描述。
            package["description"],
            # 对照冻结的内部交付文案。
            "UIX cross-platform native app framework for authorized internal use",
        )

    # 固化发布组合根必须拒绝空描述。
    def test_release_builder_requires_non_empty_package_description(self) -> None:
        # 读取发布构建器源码。
        builder = (ROOT / "scripts" / "build_internal_release.ps1").read_text(encoding="utf-8")
        # 构建器必须从 cargo metadata 取得描述。
        self.assertIn("$packageDescription = [string]$package[0].description", builder)
        # 构建器必须把空白描述作为失败条件。
        self.assertIn("[string]::IsNullOrWhiteSpace($packageDescription)", builder)
        # 缺失描述必须产生稳定诊断。
        self.assertIn("declare a non-empty description", builder)

    # 固化发布组合根的规范 ZIP 时间写入边界。
    def test_release_builder_freezes_archive_entry_timestamp(self) -> None:
        # 读取发布构建器源码。
        builder = (ROOT / "scripts" / "build_internal_release.ps1").read_text(encoding="utf-8")
        # 构建器必须声明 ZIP 规范允许的最早时间。
        self.assertIn("[DateTimeOffset]::new(1980, 1, 1, 0, 0, 0", builder)
        # 每个新条目必须在 archive 关闭前写入统一时间。
        self.assertIn("$archiveEntry.LastWriteTime = $archiveTimestamp", builder)
        # Create 模式只允许在条目 stream 首次打开前修改时间。
        self.assertLess(
            # 定位统一时间写入语句。
            builder.index("$archiveEntry.LastWriteTime = $archiveTimestamp"),
            # 定位条目 stream 打开语句。
            builder.index("$entryStream = $archiveEntry.Open()"),
        )

    # 固化 0.0.1 的签名、分发、升级与回滚边界。
    def test_delivery_document_freezes_release_operations_policy(self) -> None:
        # 读取稳定产品交付契约，不从动态 Issue 推断操作策略。
        delivery = (ROOT / "docs" / "产品" / "交付与许可.md").read_text(encoding="utf-8")
        # 0.0.1 不得把普通 SHA-256 误报成发布者签名。
        self.assertIn("不提供 detached signature", delivery)
        self.assertIn("只证明解包载荷完整性", delivery)
        # 正式制品只允许进入项目既定私有分发面。
        self.assertIn("私有 Gitea Release", delivery)
        self.assertIn("不复制到公开网盘", delivery)
        # 当前没有自动更新器，升级与回滚必须保留可恢复边界。
        self.assertIn("不提供安装器或自动更新器", delivery)
        self.assertIn("新的并行版本目录", delivery)
        self.assertIn("切回上一目录", delivery)


# 只在任一受支持的 PowerShell 运行时可用时执行发布包行为测试。
@unittest.skipIf(POWERSHELL is None, "PowerShell is required")
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
                # 为当前条目建立带规范时间的显式元数据。
                entry = ZipInfo(entry_name, date_time=FIXED_ZIP_TIMESTAMP)
                # 使用与 fixture 既有契约相同的 deflate 压缩。
                entry.compress_type = ZIP_DEFLATED
                # 写入单个规范 archive 条目。
                archive.writestr(entry, content)
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

    # 验证相同载荷与规范时间生成字节一致的 ZIP。
    def test_same_payload_produces_identical_zip_bytes(self) -> None:
        # 只构造一次稳定条目集合。
        entries = self.valid_entries()
        # 分别写入两个独立候选包。
        first = self.write_zip("first.zip", entries)
        # 第二个路径不得复用第一个文件。
        second = self.write_zip("second.zip", entries)
        # ZIP 的完整容器字节必须一致。
        self.assertEqual(first.read_bytes(), second.read_bytes())

    # 验证任一条目的文件时间漂移都会失败。
    def test_rejects_non_canonical_entry_timestamp(self) -> None:
        # 构造完整合法条目集合。
        entries = self.valid_entries()
        # 计算当前非法 fixture 路径。
        path = self.temp_dir / "timestamp-drift.zip"
        # 创建仅一个条目时间漂移的 ZIP。
        with ZipFile(path, "w", compression=ZIP_DEFLATED) as archive:
            # 按规范顺序写入全部条目。
            for entry_name, content in entries:
                # README 使用次日时间，其余条目保持规范时间。
                timestamp = (1980, 1, 2, 0, 0, 0) if entry_name == "README.md" else FIXED_ZIP_TIMESTAMP
                # 为当前条目建立显式 ZIP 元数据。
                entry = ZipInfo(entry_name, date_time=timestamp)
                # 保持与合法 fixture 相同的 deflate 压缩。
                entry.compress_type = ZIP_DEFLATED
                # 写入当前测试条目。
                archive.writestr(entry, content)
        # 执行真实 PowerShell 校验器。
        result = self.run_verifier(path)
        # 时间戳漂移必须返回失败。
        self.assertNotEqual(result.returncode, 0)
        # 失败必须定位到规范时间契约。
        self.assertIn("timestamp is not canonical", result.stdout + result.stderr)

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
            # 加入带盘符的绝对路径条目。
            "absolute": (entries + [("C:/escape.txt", b"escape")], False),
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
                self.assertRegex(
                    # 合并 PowerShell 的标准输出与错误输出。
                    result.stdout + result.stderr,
                    # 接受三种明确路径拒绝分类。
                    "unsafe path segment|not canonical|entry is rooted",
                )

    # 验证仅大小写不同的重复条目也会失败。
    def test_rejects_case_insensitive_duplicate_entry(self) -> None:
        # 在合法包后加入 Windows 语义下重复的 Demo 名称。
        entries = self.valid_entries() + [("UIX-LANG-DEMO.EXE", b"duplicate")]
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

    # 验证清单行数与语法必须同时完整。
    def test_rejects_incomplete_and_malformed_hash_manifest(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 保存合法清单字节。
        valid_manifest = entries[-1][1]
        # 按保留换行的方式拆分七行清单。
        manifest_lines = valid_manifest.splitlines(keepends=True)
        # 分别构造缺少末项与分隔符错误的清单。
        cases = {
            # 删除最后一个 payload 摘要。
            "incomplete": b"".join(manifest_lines[:-1]),
            # 把首行两个空格分隔改成一个空格。
            "malformed": valid_manifest.replace(b"  ", b" ", 1),
        }
        # 逐项验证两类清单漂移。
        for case, manifest in cases.items():
            # 让失败名称进入子测试诊断。
            with self.subTest(case=case):
                # 复制基线条目避免跨子测试共享修改。
                case_entries = list(entries)
                # 只替换清单内容，保留八项包结构。
                case_entries[-1] = ("SHA256SUMS.txt", manifest)
                # 写入当前非法 fixture。
                path = self.write_zip(f"{case}.zip", case_entries)
                # 执行真实校验器。
                result = self.run_verifier(path)
                # 不完整或格式错误清单必须失败。
                self.assertNotEqual(result.returncode, 0)
                # 失败必须来自清单行数或语法门禁。
                self.assertRegex(
                    # 合并 PowerShell 的标准输出与错误输出。
                    result.stdout + result.stderr,
                    # 接受两种明确清单拒绝分类。
                    "hash line count mismatch|hash line is invalid",
                )


# 允许直接运行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
