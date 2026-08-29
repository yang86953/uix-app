#!/usr/bin/env python3
"""校验 Linux x64 内部候选包的结构、元数据与内容摘要。"""

# 引入命令行参数解析。
import argparse
# 引入流式 SHA-256 计算。
import hashlib
# 引入摘要行语法验证。
import re
# 引入标准错误输出。
import sys
# 引入 gzip tar 读取能力。
import tarfile
# 引入稳定路径处理。
from pathlib import Path, PurePosixPath

# 固化摘要文件名称。
MANIFEST_NAME = "SHA256SUMS.txt"
# 固化摘要文件最大字节数。
MAX_MANIFEST_BYTES = 65_536
# 固化小写 SHA-256 清单语法。
MANIFEST_LINE = re.compile(r"^([0-9a-f]{64})  (.+)$")
# 固化可接受的语义版本语法。
VERSION_PATTERN = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")


# 返回指定版本的七项 payload 冻结顺序。
def expected_payload(version: str) -> tuple[str, ...]:
    # 组合 Linux 可执行文件、crate 与五项资料载荷。
    return (
        # Linux 主演示使用规范 bin 路径。
        "bin/uix-lang-demo",
        # 内部 crate 使用精确版本命名。
        f"uix-{version}.crate",
        # 专有许可必须随包交付。
        "LICENSE",
        # 第三方声明必须随包交付。
        "THIRD_PARTY_NOTICES.md",
        # 当前版本变更记录必须随包交付。
        "CHANGELOG.md",
        # 使用入口必须随包交付。
        "README.md",
        # Demo 图片使用规范相对路径。
        "assets/images/demo.png",
    )


# 验证归档条目名称是安全且规范的 POSIX 相对路径。
def validate_entry_name(name: str) -> None:
    # 空名称不能描述交付文件。
    if not name:
        # 报告稳定空名称错误。
        raise ValueError("internal release contains an empty archive entry name")
    # 反斜杠会让不同解包器产生不同路径。
    if "\\" in name:
        # 拒绝非规范分隔符。
        raise ValueError(f"internal release entry is not canonical: {name}")
    # 根路径与盘符形式都可能越过解包根。
    if name.startswith("/") or ":" in name:
        # 拒绝绝对或带卷标的路径。
        raise ValueError(f"internal release entry is rooted: {name}")
    # 按 POSIX 分隔符保留每个原始路径段。
    segments = name.split("/")
    # 空段、当前目录与父目录都不是规范交付路径。
    if any(segment in {"", ".", ".."} for segment in segments):
        # 拒绝路径穿越与重复分隔符。
        raise ValueError(f"internal release entry contains an unsafe path segment: {name}")
    # PurePosixPath 必须保持原始名称不变。
    if str(PurePosixPath(name)) != name:
        # 拒绝会被路径库归一化的名称。
        raise ValueError(f"internal release entry is not canonical: {name}")


# 验证 gzip 头没有文件名或构建时间漂移。
def validate_gzip_header(path: Path) -> None:
    # 只读取固定十字节基础头。
    with path.open("rb") as stream:
        # 保存基础 gzip header。
        header = stream.read(10)
    # gzip 基础头必须完整并使用 deflate。
    if len(header) != 10 or header[:3] != b"\x1f\x8b\x08":
        # 拒绝错误容器或截断头。
        raise ValueError("internal release is not a canonical gzip stream")
    # FNAME 标志会把临时文件名写入容器。
    if header[3] & 0x08:
        # 拒绝携带文件名的 gzip 头。
        raise ValueError("internal release gzip header contains a file name")
    # MTIME 四字节必须全部为零。
    if header[4:8] != b"\0\0\0\0":
        # 拒绝构建时间进入容器摘要。
        raise ValueError("internal release gzip timestamp is not canonical")


# 从 tar 中读取一个已验证存在的普通文件。
def read_member(archive: tarfile.TarFile, member: tarfile.TarInfo) -> bytes:
    # 打开当前普通文件的解压流。
    stream = archive.extractfile(member)
    # 普通文件必须可以取得内容流。
    if stream is None:
        # 报告无法读取的具体条目。
        raise ValueError(f"internal release entry cannot be read: {member.name}")
    # 由上下文管理器确保流及时关闭。
    with stream:
        # 读取由上层大小门禁约束的内容。
        return stream.read()


# 流式计算一个 tar payload 的 SHA-256。
def hash_member(archive: tarfile.TarFile, member: tarfile.TarInfo) -> str:
    # 创建单项 SHA-256 owner。
    digest = hashlib.sha256()
    # 打开当前普通文件的解压流。
    stream = archive.extractfile(member)
    # 普通文件必须可以取得内容流。
    if stream is None:
        # 报告无法读取的具体条目。
        raise ValueError(f"internal release entry cannot be read: {member.name}")
    # 由上下文管理器确保流及时关闭。
    with stream:
        # 分块读取以避免把二进制整体载入内存。
        while chunk := stream.read(1024 * 1024):
            # 把当前块加入摘要。
            digest.update(chunk)
    # 返回小写十六进制摘要。
    return digest.hexdigest()


# 流式计算整个归档的 SHA-256。
def hash_file(path: Path) -> str:
    # 创建容器 SHA-256 owner。
    digest = hashlib.sha256()
    # 打开候选包原始字节流。
    with path.open("rb") as stream:
        # 分块读取整个归档。
        while chunk := stream.read(1024 * 1024):
            # 把当前块加入摘要。
            digest.update(chunk)
    # 返回小写十六进制摘要。
    return digest.hexdigest()


# 验证一个 Linux 内部候选包。
def verify_archive(path: Path, version: str) -> str:
    # 版本必须满足冻结数字形式。
    if VERSION_PATTERN.fullmatch(version) is None:
        # 拒绝无法安全拼接载荷名称的版本。
        raise ValueError(f"internal release version is invalid: {version}")
    # 输入必须是现有普通文件。
    if not path.is_file():
        # 拒绝目录或消失路径。
        raise ValueError(f"internal release archive is not a file: {path}")
    # 候选包文件名必须与平台契约完全一致。
    expected_archive_name = f"uix-{version}-internal-linux-x64.tar.gz"
    # 临时或误命名容器不能作为正式候选包通过。
    if path.name != expected_archive_name:
        # 报告实际与预期文件名。
        raise ValueError(f"internal release archive name mismatch: expected {expected_archive_name}, got {path.name}")
    # 先验证外层 gzip 确定性元数据。
    validate_gzip_header(path)
    # 固化七项载荷顺序。
    payload_names = expected_payload(version)
    # 清单必须是最后且唯一不自哈希的条目。
    expected_names = payload_names + (MANIFEST_NAME,)
    # 以 gzip tar 模式打开候选包。
    with tarfile.open(path, mode="r:gz") as archive:
        # 固化条目快照，避免多次枚举产生状态差异。
        members = archive.getmembers()
        # 保存大小写折叠后的已见名称。
        seen_names: set[str] = set()
        # 在集合比较前逐项验证路径与元数据。
        for member in members:
            # 验证原始条目名称。
            validate_entry_name(member.name)
            # 使用大小写折叠键拒绝跨平台重复。
            folded_name = member.name.casefold()
            # 重复路径会让解包结果不确定。
            if folded_name in seen_names:
                # 报告重复条目。
                raise ValueError(f"internal release contains a duplicate entry: {member.name}")
            # 发布当前条目名称所有权。
            seen_names.add(folded_name)
            # 候选包只允许普通文件。
            if not member.isfile():
                # 拒绝目录、链接与设备节点。
                raise ValueError(f"internal release entry is not a regular file: {member.name}")
            # ustar 构建不得携带扩展 PAX 元数据。
            if member.pax_headers:
                # 拒绝可能包含漂移字段的 PAX header。
                raise ValueError(f"internal release entry contains PAX metadata: {member.name}")
            # 所有条目时间统一冻结为 Unix epoch。
            if member.mtime != 0:
                # 拒绝文件时间漂移。
                raise ValueError(f"internal release entry timestamp is not canonical: {member.name}")
            # 数字所有者必须固定为 root/root 身份值。
            if member.uid != 0 or member.gid != 0:
                # 拒绝构建机器用户进入归档。
                raise ValueError(f"internal release entry owner is not canonical: {member.name}")
            # 只有主演示保留可执行权限。
            expected_mode = 0o755 if member.name == "bin/uix-lang-demo" else 0o644
            # 文件模式必须与平台契约完全一致。
            if member.mode != expected_mode:
                # 报告实际八进制模式。
                raise ValueError(f"internal release entry mode is not canonical: {member.name} has {member.mode:o}")
        # 条目名称和顺序必须与冻结契约完全一致。
        actual_names = tuple(member.name for member in members)
        # 同时拒绝缺失、额外、重排与大小写漂移。
        if actual_names != expected_names:
            # 报告精确集合与顺序错误。
            raise ValueError(f"internal release entry order mismatch: expected {expected_names}, got {actual_names}")
        # 取得唯一清单条目。
        manifest_member = members[-1]
        # 小型清单必须保持有界。
        if manifest_member.size > MAX_MANIFEST_BYTES:
            # 拒绝异常大清单。
            raise ValueError(f"internal release hash manifest is too large: {manifest_member.size} bytes")
        # 读取已通过大小门禁的清单。
        manifest_bytes = read_member(archive, manifest_member)
        # 清单不能为空。
        if not manifest_bytes:
            # 拒绝无法证明 payload 的空清单。
            raise ValueError("internal release hash manifest is empty")
        # 清单必须以 LF 结束且不使用 CRLF。
        if not manifest_bytes.endswith(b"\n") or b"\r" in manifest_bytes:
            # 拒绝不稳定的换行格式。
            raise ValueError("internal release hash manifest newline format is invalid")
        # 清单只允许纯 ASCII。
        try:
            # 解码已经完成边界检查的清单。
            manifest_text = manifest_bytes.decode("ascii")
        # 非 ASCII 字节必须形成稳定失败。
        except UnicodeDecodeError as error:
            # 包装为公开校验错误而不泄漏二进制。
            raise ValueError("internal release hash manifest is not ASCII") from error
        # 按最终 LF 拆分出七行摘要。
        manifest_lines = manifest_text[:-1].split("\n")
        # 摘要行数必须与 payload 一致。
        if len(manifest_lines) != len(payload_names):
            # 拒绝缺失、重复或额外摘要。
            raise ValueError(f"internal release hash line count mismatch: expected {len(payload_names)}, got {len(manifest_lines)}")
        # 建立名称到唯一成员的映射。
        members_by_name = {member.name: member for member in members}
        # 按冻结顺序验证每条摘要。
        for expected_name, line in zip(payload_names, manifest_lines, strict=True):
            # 解析小写摘要和规范名称。
            match = MANIFEST_LINE.fullmatch(line)
            # 行格式必须完全匹配。
            if match is None:
                # 拒绝长度、大小写或分隔漂移。
                raise ValueError(f"internal release hash line is invalid: {line}")
            # 读取声明摘要。
            declared_hash = match.group(1)
            # 读取声明名称。
            declared_name = match.group(2)
            # 名称必须与冻结顺序完全一致。
            if declared_name != expected_name:
                # 拒绝别名、重排或未覆盖载荷。
                raise ValueError(f"internal release hash entry mismatch: expected {expected_name}, got {declared_name}")
            # 对归档内真实解包内容计算摘要。
            actual_hash = hash_member(archive, members_by_name[expected_name])
            # 声明摘要必须逐字节匹配。
            if declared_hash != actual_hash:
                # 报告具体损坏载荷。
                raise ValueError(f"internal release hash mismatch for {expected_name}: expected {declared_hash}, got {actual_hash}")
    # 返回整个候选包的最终摘要。
    return hash_file(path)


# 解析命令行并执行校验。
def main() -> int:
    # 创建稳定命令行解析器。
    parser = argparse.ArgumentParser(description=__doc__)
    # 候选包路径必须由调用方显式提供。
    parser.add_argument("--archive", required=True, type=Path)
    # 版本默认与当前首发候选一致。
    parser.add_argument("--version", default="0.0.2")
    # 解析当前进程参数。
    arguments = parser.parse_args()
    # 执行完整校验并取得容器摘要。
    archive_hash = verify_archive(arguments.archive.resolve(), arguments.version)
    # 输出稳定成功证据。
    print(f"Verified Linux internal release: {arguments.archive.resolve()}")
    # 输出整个归档的小写 SHA-256。
    print(f"Verified SHA256: {archive_hash}")
    # 返回成功进程状态。
    return 0


# 只在脚本入口执行命令行逻辑。
if __name__ == "__main__":
    # 把校验错误转换为稳定非零退出。
    try:
        # 执行主入口并返回其状态。
        raise SystemExit(main())
    # 只捕获预期输入、文件与归档错误。
    except (OSError, tarfile.TarError, ValueError) as error:
        # 把失败原因写入标准错误。
        print(f"Linux internal release verification failed: {error}", file=sys.stderr)
        # 使用非零状态结束进程。
        raise SystemExit(1) from error
