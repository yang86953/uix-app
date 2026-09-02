#!/usr/bin/env python3
"""诊断处置契约静态审计：防止框架代码绕过诊断系统报告错误。

依据 docs/架构/platform/diagnostics.md 的「错误处置决策矩阵」，扫描 src/
生产代码并执行三条规则；新增违规时以非零退出码失败，供验证流程门禁使用。

规则：
  R1 错误事实的裸日志 —— tracing::error!/warn! 记录错误值（短述/错误绑定）
     时，同一分支必须已调用诊断通道（report_with_origin / observe_transient /
     observe_boundary_error / report / enqueue / attempt_recovery /
     record_failure / report_window_operation_error），或携带显式豁免标记。
  R2 契约 panic —— 生产路径的 panic!/unreachable!/expect 必须带邻近契约
     注释（文档声明的开发者契约或不变量理由）；todo!/unimplemented! 一律违规。
  R3 静默吞没 —— 空体的 `Err(_) =>` 分支必须带理由注释或豁免标记。

豁免标记：`diagnostics-exempt: <理由>`（同一行或上方紧邻注释）。

用法：python3 scripts/diagnostics_audit.py [--root <仓库根>] [--verbose]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# 诊断系统自身实现（emit/crash 的 fallback 日志是通道的一部分）。
SELF_OWNED_DIRS = ("src/diagnostics/",)

# 测试/验收组合根：不参与生产处置契约。
EXCLUDED_PATH_PARTS = (
    "parity.rs",
    "consistency.rs",
    "graphics_parity.rs",
    "test_harness",
    "tests/support",
)

# 视为「已走诊断通道」的调用形态（R1 的同分支豁免）。
CHANNEL_PATTERNS = re.compile(
    r"report_with_origin|observe_transient|observe_boundary_error"
    r"|\.report\(|\.enqueue\(|attempt_recovery|record_failure"
    r"|report_window_operation_error|uix_contract_violation"
    r"|external_present_failed|frame_failure|frame_failed|protocol_failure",
)

# 错误值的典型引用形态：短述方法、格式化插值、结构化字段。
ERROR_VALUE_PATTERNS = re.compile(
    r"short_what\(\)|\.what\(\)|\{error\}|\{err\b|%error\b|%err\b|error = |err = ",
)

TRACING_CALL = re.compile(r"tracing::(error|warn)!")
PANIC_FAMILY = re.compile(r"\bpanic!|\bunreachable!|\btodo!|\bunimplemented!|\.expect\(")
ERR_WILDCARD = re.compile(r"Err\(_\)\s*=>")
EXEMPT_MARKER = "diagnostics-exempt:"

# 上下文窗口（行数）：宏参数延续与同分支判定。
CALL_WINDOW = 10
COMMENT_WINDOW = 6


GATED_CFG = re.compile(r"#\[cfg\(([^)]*)\)\]")
GATED_FEATURE_HINTS = ("test", "parity", "consistency", "harness", "agent-control")


def is_gated_cfg(line: str) -> bool:
    """cfg 属性命中测试/验收 feature 时视为非生产代码门控。"""
    match = GATED_CFG.search(line)
    if match is None:
        return False
    predicate = match.group(1)
    return any(hint in predicate for hint in GATED_FEATURE_HINTS)


def strip_gated_blocks(lines: list[str]) -> list[str]:
    """把门控 cfg 块（test/parity/验收 feature 的 fn/mod/impl）置空，保持行号。"""
    result = list(lines)
    index = 0
    while index < len(result):
        line = result[index]
        if "#[cfg(test)]" not in line and not is_gated_cfg(line):
            index += 1
            continue
        # 从属性向下找到第一个块开括号（跨声明与其余属性行）。
        cursor = index
        open_line = None
        while cursor < len(result):
            if "{" in result[cursor]:
                open_line = cursor
                break
            if ";" in result[cursor]:
                break  # 无块体的属性（如 cfg 属性修饰的声明）直接跳过。
            cursor += 1
        if open_line is None:
            index += 1
            continue
        depth = 0
        closer = open_line
        while closer < len(result):
            depth += result[closer].count("{") - result[closer].count("}")
            if depth <= 0:
                break
            closer += 1
        for blank in range(index, closer + 1):
            result[blank] = ""
        index = closer + 1
    return result


def window(lines: list[str], start: int, span: int) -> str:
    return "\n".join(lines[start : min(start + span, len(lines))])


def nearby_comment(lines: list[str], line_index: int, span: int) -> bool:
    """调用点上方 span 行内是否存在注释行（契约声明惯例）。"""
    for offset in range(max(0, line_index - span), line_index + 1):
        if "//" in lines[offset]:
            return True
    return False


def audit_file(path: Path, root: Path) -> tuple[list[str], dict[str, int]]:
    relative = path.relative_to(root).as_posix()
    if relative.startswith(SELF_OWNED_DIRS):
        return [], {}
    if any(part in relative for part in EXCLUDED_PATH_PARTS):
        return [], {}

    text = path.read_text(encoding="utf-8", errors="replace")
    lines = strip_gated_blocks(text.splitlines())
    violations: list[str] = []
    channels = {
        "report_with_origin": 0,
        "observe_transient": 0,
        "observe_boundary_error": 0,
        "panic_family": 0,
    }

    for index, line in enumerate(lines):
        if EXEMPT_MARKER in window(lines, max(0, index - COMMENT_WINDOW), index + 1):
            exempt = True
        else:
            exempt = False

        # 通道正向统计。
        for name in ("report_with_origin", "observe_transient", "observe_boundary_error"):
            if name in line:
                channels[name] += 1

        # R1：错误事实的裸日志。
        if TRACING_CALL.search(line) and not exempt:
            context = window(lines, index, CALL_WINDOW)
            if ERROR_VALUE_PATTERNS.search(context):
                preceding = window(lines, max(0, index - COMMENT_WINDOW), CALL_WINDOW)
                if not CHANNEL_PATTERNS.search(context) and not CHANNEL_PATTERNS.search(
                    preceding
                ):
                    violations.append(
                        f"R1 {relative}:{index + 1}: 错误值经裸 tracing 记录而未走诊断通道"
                        f"（补通道调用或加 diagnostics-exempt 标记）: {line.strip()[:90]}"
                    )

        # R2：契约 panic。
        if PANIC_FAMILY.search(line):
            channels["panic_family"] += 1
            if not exempt:
                if re.search(r"\btodo!|\bunimplemented!", line):
                    violations.append(
                        f"R2 {relative}:{index + 1}: 生产路径禁止 todo!/unimplemented!: {line.strip()[:90]}"
                    )
                elif not nearby_comment(lines, index, COMMENT_WINDOW):
                    violations.append(
                        f"R2 {relative}:{index + 1}: panic/unreachable/expect 缺少契约注释声明: {line.strip()[:90]}"
                    )

        # R3：空体 Err(_) => 吞没。
        if ERR_WILDCARD.search(line) and not exempt:
            body = window(lines, index, 3)
            stripped_body = re.sub(r"//.*", "", body)
            if re.search(r"Err\(_\)\s*=>\s*(\{\s*\}|None|false|true|Skipped|continue|break|Vec::new\(\))\s*,?", stripped_body.strip()):
                if not nearby_comment(lines, index, COMMENT_WINDOW):
                    violations.append(
                        f"R3 {relative}:{index + 1}: 空体 Err(_) 吞没错误细节且无理由注释: {line.strip()[:90]}"
                    )

    return violations, channels


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="仓库根目录")
    parser.add_argument("--verbose", action="store_true", help="输出通道统计")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    source_root = root / "src"
    if not source_root.is_dir():
        print(f"error: {source_root} 不存在", file=sys.stderr)
        return 2

    all_violations: list[str] = []
    totals = {"report_with_origin": 0, "observe_transient": 0, "observe_boundary_error": 0, "panic_family": 0}
    file_count = 0
    for path in sorted(source_root.rglob("*.rs")):
        violations, channels = audit_file(path, root)
        all_violations.extend(violations)
        for name, count in channels.items():
            totals[name] += count
        file_count += 1

    if args.verbose:
        print(f"scanned {file_count} files under src/")
        print(f"  report_with_origin 调用点:      {totals['report_with_origin']}")
        print(f"  observe_transient 调用点:       {totals['observe_transient']}")
        print(f"  observe_boundary_error 调用点:  {totals['observe_boundary_error']}")
        print(f"  生产 panic 家族点位:            {totals['panic_family']}")

    if all_violations:
        print(f"\n诊断处置契约违规 {len(all_violations)} 处：")
        for violation in all_violations:
            print(f"  {violation}")
        print(
            "\n处置依据见 docs/架构/platform/diagnostics.md#错误处置决策矩阵；"
            "确属事实日志或设计豁免时，在调用点上方加 `// diagnostics-exempt: <理由>`。"
        )
        return 1

    print("诊断处置契约审计通过：src/ 无绕过诊断系统的错误报告点。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
