# 引入确定性 gzip fixture 创建能力。
import gzip
# 引入 SHA-256 以生成测试清单。
import hashlib
# 引入内存条目流。
import io
# 引入外部校验器进程执行。
import subprocess
# 引入当前 Python 解释器路径。
import sys
# 引入 ustar fixture 创建能力。
import tarfile
# 引入隔离测试目录。
import tempfile
# 引入标准单元测试框架。
import unittest
# 引入稳定路径拼接。
from pathlib import Path

# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 Linux 候选包校验器。
VERIFIER = ROOT / "scripts" / "verify_internal_release.py"
# 固定候选版本。
VERSION = "0.0.1"
# 固定正式候选包文件名。
ARCHIVE_NAME = f"uix-{VERSION}-internal-linux-x64.tar.gz"
# 固化清单必须覆盖的七个 payload 顺序。
PAYLOAD_NAMES = (
    # Linux 主演示可执行文件。
    "bin/uix-lang-demo",
    # 当前版本内部 crate。
    f"uix-{VERSION}.crate",
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


# 验证 Linux 内部候选包的允许与拒绝边界。
class LinuxInternalReleaseContractTests(unittest.TestCase):
    # 为每个用例创建独立目录。
    def setUp(self) -> None:
        # 持有临时目录生命周期。
        self._temp_dir = tempfile.TemporaryDirectory()
        # 保存便于构造 fixture 的路径。
        self.temp_dir = Path(self._temp_dir.name)

    # 用例结束后删除隔离目录。
    def tearDown(self) -> None:
        # 释放临时目录及其中归档。
        self._temp_dir.cleanup()

    # 构造稳定且彼此不同的测试 payload。
    def valid_payload(self) -> dict[str, bytes]:
        # 每个名称映射为包含名称的可复现字节。
        return {name: f"payload:{name}".encode("ascii") for name in PAYLOAD_NAMES}

    # 为给定 payload 生成严格顺序的 SHA-256 清单。
    def manifest_for(self, payload: dict[str, bytes]) -> bytes:
        # 按冻结顺序生成小写摘要与两个空格分隔。
        lines = [f"{hashlib.sha256(payload[name]).hexdigest()}  {name}" for name in PAYLOAD_NAMES]
        # 使用 LF 与最终换行模拟正式构建器输出。
        return ("\n".join(lines) + "\n").encode("ascii")

    # 创建包含精确 payload 与清单的合法条目列表。
    def valid_entries(self) -> list[tuple[str, bytes]]:
        # 构造稳定 payload。
        payload = self.valid_payload()
        # 先按冻结顺序添加七个载荷。
        entries = [(name, payload[name]) for name in PAYLOAD_NAMES]
        # 最后添加不自哈希的清单。
        entries.append(("SHA256SUMS.txt", self.manifest_for(payload)))
        # 返回完整八项包结构。
        return entries

    # 写入一个可定制元数据和条目的确定性 gzip ustar fixture。
    def write_archive(
        # 接收当前测试实例。
        self,
        # 使用独立子目录保留规范归档文件名。
        case: str,
        # 保留调用方给定条目顺序并允许重复名称。
        entries: list[tuple[str, bytes]],
        # 允许单个条目覆盖规范时间。
        timestamp_overrides: dict[str, int] | None = None,
        # 允许单个条目覆盖规范模式。
        mode_overrides: dict[str, int] | None = None,
        # 允许全部条目使用非规范数字所有者。
        owner: int = 0,
        # 允许外层 gzip 时间漂移。
        gzip_mtime: int = 0,
    # 返回已关闭归档路径。
    ) -> Path:
        # 创建当前用例独立目录。
        case_dir = self.temp_dir / case
        # 确保目录存在。
        case_dir.mkdir()
        # 固定规范候选包名称。
        path = case_dir / ARCHIVE_NAME
        # 缺省时间覆盖为空映射。
        timestamps = timestamp_overrides or {}
        # 缺省模式覆盖为空映射。
        modes = mode_overrides or {}
        # 打开原始 gzip 输出文件。
        with path.open("wb") as raw_stream:
            # 创建不携带文件名且时间可控的 gzip 流。
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw_stream, mtime=gzip_mtime) as gzip_stream:
                # 在 gzip 内写入不含 PAX 扩展的 ustar。
                with tarfile.open(fileobj=gzip_stream, mode="w", format=tarfile.USTAR_FORMAT) as archive:
                    # 保留调用方顺序写入全部条目。
                    for entry_name, content in entries:
                        # 创建当前条目的 ustar 元数据。
                        entry = tarfile.TarInfo(entry_name)
                        # 固定真实内容长度。
                        entry.size = len(content)
                        # 固定或覆盖条目时间。
                        entry.mtime = timestamps.get(entry_name, 0)
                        # 固定数字用户所有者。
                        entry.uid = owner
                        # 固定数字组所有者。
                        entry.gid = owner
                        # 禁止用户名进入 header。
                        entry.uname = ""
                        # 禁止组名进入 header。
                        entry.gname = ""
                        # 主演示默认 0755，其余文件默认 0644。
                        default_mode = 0o755 if entry_name == "bin/uix-lang-demo" else 0o644
                        # 固定或覆盖当前条目模式。
                        entry.mode = modes.get(entry_name, default_mode)
                        # 写入当前普通文件内容。
                        archive.addfile(entry, io.BytesIO(content))
        # 返回已关闭的候选包路径。
        return path

    # 调用真实 Python 校验器。
    def run_verifier(self, path: Path) -> subprocess.CompletedProcess[str]:
        # 以当前解释器运行被测脚本并捕获完整诊断。
        return subprocess.run(
            # 传入固定版本与当前 fixture。
            [sys.executable, str(VERIFIER), "--archive", str(path), "--version", VERSION],
            # 从仓库根运行以保持错误路径稳定。
            cwd=ROOT,
            # 捕获标准输出与错误输出。
            capture_output=True,
            # 以文本形式解码诊断。
            text=True,
            # 由测试显式断言退出码。
            check=False,
        )

    # 验证精确 payload 可通过且相同输入生成相同容器字节。
    def test_accepts_exact_payload_and_is_deterministic(self) -> None:
        # 只构造一次稳定条目集合。
        entries = self.valid_entries()
        # 写入第一份合法候选包。
        first = self.write_archive("first", entries)
        # 写入第二份独立合法候选包。
        second = self.write_archive("second", entries)
        # 执行真实校验器。
        result = self.run_verifier(first)
        # 合法包必须返回成功。
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        # 成功输出必须给出稳定确认。
        self.assertIn("Verified Linux internal release", result.stdout)
        # 两份容器的完整字节必须一致。
        self.assertEqual(first.read_bytes(), second.read_bytes())

    # 验证缺失、额外、危险和重复条目都会失败。
    def test_rejects_invalid_entry_sets_and_names(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 定义四类归档结构漂移。
        cases = {
            # 删除 README 形成缺失载荷。
            "missing": [entry for entry in entries if entry[0] != "README.md"],
            # 加入未声明文件形成额外载荷。
            "extra": entries + [("unexpected.txt", b"unexpected")],
            # 加入父目录穿越条目。
            "traversal": entries + [("../escape.txt", b"escape")],
            # 加入完全相同的重复条目。
            "duplicate": entries + [("LICENSE", b"duplicate")],
        }
        # 逐项验证四类集合漂移。
        for case, case_entries in cases.items():
            # 让失败名称进入子测试诊断。
            with self.subTest(case=case):
                # 写入当前非法 fixture。
                path = self.write_archive(case, case_entries)
                # 执行真实校验器。
                result = self.run_verifier(path)
                # 非规范结构必须失败。
                self.assertNotEqual(result.returncode, 0)
                # 失败必须来自路径、重复或顺序门禁。
                self.assertRegex(result.stderr, "unsafe path segment|duplicate entry|entry order mismatch")

    # 验证时间、所有者、模式与 gzip 头漂移都会失败。
    def test_rejects_non_canonical_metadata(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 定义四类元数据漂移参数。
        cases = {
            # README 使用非 epoch 时间。
            "timestamp": {"timestamp_overrides": {"README.md": 1}},
            # 全部条目携带构建用户身份。
            "owner": {"owner": 1000},
            # 主演示丢失可执行权限。
            "mode": {"mode_overrides": {"bin/uix-lang-demo": 0o644}},
            # 外层 gzip 写入构建时间。
            "gzip-time": {"gzip_mtime": 1},
        }
        # 逐项验证四类元数据漂移。
        for case, options in cases.items():
            # 让失败名称进入子测试诊断。
            with self.subTest(case=case):
                # 写入当前非法 fixture。
                path = self.write_archive(case, entries, **options)
                # 执行真实校验器。
                result = self.run_verifier(path)
                # 非规范元数据必须失败。
                self.assertNotEqual(result.returncode, 0)
                # 失败必须定位到对应元数据边界。
                self.assertRegex(result.stderr, "timestamp is not canonical|owner is not canonical|mode is not canonical")

    # 验证清单哈希必须匹配解包后的真实字节。
    def test_rejects_incorrect_payload_hash(self) -> None:
        # 构造合法基线条目。
        entries = self.valid_entries()
        # 用语法正确但内容错误的首项 SHA-256 替换清单。
        manifest = entries[-1][1].replace(entries[-1][1][:64], b"0" * 64, 1)
        # 保存错误清单到最后一个条目。
        entries[-1] = ("SHA256SUMS.txt", manifest)
        # 写入错误摘要 fixture。
        path = self.write_archive("wrong-hash", entries)
        # 执行真实校验器。
        result = self.run_verifier(path)
        # 错误哈希必须失败。
        self.assertNotEqual(result.returncode, 0)
        # 失败必须定位到内容摘要。
        self.assertIn("hash mismatch", result.stderr)


# 允许直接运行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
