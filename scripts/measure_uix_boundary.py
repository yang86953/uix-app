#!/usr/bin/env python3
"""度量 UIX Lang 边界成本：统计 .uix 文件中的事件处理器分类与 Rust 回调密度。

方法论（启发式，非语法解析）：
- 处理器：@event="expr" 内出现的调用名。
  - 属于 {setState,setStyle,setTheme} 或当前文档声明的 action 名 => 纯界面处理。
  - 其他 camelCase 调用名（含 external 登记）=> Rust 回调转发。
  - PascalCase 调用名视为数据构造器，不计边界。
- 回调 prop：调用已知自定义 Widget 时传入 {ident} 的属性，
  其类型在该 Widget 声明中为回调形状；未登记标签按 onXxx 命名启发式计数。
- external：external="..." 中登记的每个符号各计一次。

用法：python3 scripts/measure_uix_boundary.py [根目录] [--markdown]
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter
from pathlib import Path

# 语言内置操作与关键字：这些名称不构成 Rust 边界。
BUILTIN_CALLS = {"setState", "setStyle", "setTheme"}

# 预编译正则。
EVENT_HANDLER = re.compile(r'@[\w]+="([^"]*)"')
EXTERNAL_LIST = re.compile(r'external="([^"]*)"')
WIDGET_DECL = re.compile(
    r'<Widget\s+name="(\w+)"((?:\s+(?:props|state|computed|actions|external)="[^"]*")*)\s*/?>',
    re.S,
)
ATTR_BLOCK = re.compile(r'\b(props|state|computed|actions|external)="([^"]*)"')
WIDGET_MEMBER_BLOCK = re.compile(r'@props\s*\{(.*?)\}', re.S)
CALL_SITE = re.compile(r'<([A-Z]\w*)((?:\s+[^<>]*?)?)/?>', re.S)
BRACED_IDENT = re.compile(r'=\{([a-z_]\w*)\}')
CALL_NAME = re.compile(r'\b([A-Za-z_]\w*)\s*\(')


def member_block_sources(source: str, kind: str) -> list[str]:
    """提取 @kind { ... } 块体；容忍嵌套花括号。"""
    results: list[str] = []
    marker = f"@{kind}"
    index = source.find(marker)
    while index != -1:
        open_index = source.find("{", index + len(marker))
        close_index = _matching_brace(source, open_index) if open_index != -1 else -1
        if close_index == -1:
            break
        results.append(source[open_index + 1:close_index])
        index = source.find(marker, close_index)
    return results


def _matching_brace(source: str, open_index: int) -> int:
    """返回与 open_index 配对的右花括号下标；找不到返回 -1。"""
    depth = 0
    quoted = False
    escaped = False
    for position in range(open_index, len(source)):
        character = source[position]
        if escaped:
            escaped = False
            continue
        if quoted:
            if character == "\\":
                escaped = True
            elif character == "'":
                quoted = False
            continue
        if character == "'":
            quoted = True
        elif character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return position
    return -1


def attribute_sources(element_open: str, full_source: str) -> dict[str, str]:
    """合并字符串属性形式与块级形式的成员源码。"""
    sources: dict[str, str] = {}
    for name, value in ATTR_BLOCK.findall(element_open):
        sources[name] = value
    for kind in ("props", "state", "computed", "actions"):
        blocks = member_block_sources(full_source, kind)
        if blocks:
            sources[kind] = ",".join(blocks)
    return sources


def parse_widgets(text: str) -> dict[str, dict]:
    """解析全部 <Widget> 声明，返回 name -> 元信息。"""
    widgets: dict[str, dict] = {}
    for match in WIDGET_DECL.finditer(text):
        attributes = attribute_sources(match.group(0), text)
        props_raw = attributes.get("props", "")
        # 逐项提取 name: Type；类型以第一个顶层冒号切分的近似即可满足度量需求。
        prop_types: dict[str, str] = {}
        for entry in [part.strip() for part in props_raw.split(",") if part.strip()]:
            if ":" not in entry:
                continue
            name, prop_type = entry.split(":", 1)
            # 默认值形式 name: Type = expr 只取类型段第一列。
            prop_type = prop_type.split("=")[0].strip()
            prop_types[name.strip()] = prop_type.replace(" ", "")
        actions_raw = attributes.get("actions", "")
        action_names = {
            part.split(":", 1)[0].strip()
            for part in actions_raw.split(",")
            if ":" in part
        }
        external = [
            symbol.strip()
            for symbol in attributes.get("external", "").split(",")
            if symbol.strip()
        ]
        widgets[match.group(1)] = {
            "prop_types": prop_types,
            "action_names": action_names,
            "external": external,
        }
    return widgets


def classify_handler(expr: str, action_names: set[str]) -> tuple[str, list[str]]:
    """把处理器表达式分类为 pure / forward / mixed，并返回驼峰调用名。"""
    names = CALL_NAME.findall(expr)
    camel = [
        name for name in names if name[0].islower() and name not in BUILTIN_CALLS
    ]
    unknown_camel = [name for name in camel if name not in action_names]
    # setState(x: y) 参数表达式里的外层名已被捕获；此处不做二次区分。
    if not unknown_camel:
        return ("pure", names)
    if all(name in BUILTIN_CALLS or name in action_names or name[0].isupper() for name in names):
        return ("pure", names)
    if any(name in BUILTIN_CALLS or name in action_names for name in names) and unknown_camel:
        return ("mixed", names)
    return ("forward", names)


def measure_file(path: Path) -> dict:
    """收集单个 .uix 文件的度量值。"""
    text = path.read_text(encoding="utf-8")
    widgets = parse_widgets(text)
    action_names: set[str] = set()
    for meta in widgets.values():
        action_names |= meta["action_names"]
    handler_counter = Counter()
    handler_details = {"pure": 0, "forward": 0, "mixed": 0}
    forwarded_symbols: set[str] = set()
    externals: set[str] = set()
    callback_props = 0
    for match in EVENT_HANDLER.finditer(text):
        kind, _names = classify_handler(match.group(1), action_names)
        handler_details[kind] += 1
        handler_counter[kind] += 1
    for match in EXTERNAL_LIST.finditer(text):
        externals.update(symbol.strip() for symbol in match.group(1).split(",") if symbol.strip())
    # 回调 prop 计数：汇总各 Widget 声明中回调形状的 prop 名，再全文本匹配传值。
    callback_names: set[str] = set()
    for meta in widgets.values():
        callback_names |= {
            prop_name
            for prop_name, prop_type in meta["prop_types"].items()
            if "(" in prop_type
        }
    for prop_name in sorted(callback_names):
        pattern = re.compile(rf'\b{re.escape(prop_name)}=\{{\s*[a-z_]\w*\s*\}}')
        callback_props += len(pattern.findall(text))
    # 启发式兜底：onXxx={ident}。
    callback_props += sum(
        1 for name in BRACED_IDENT.findall(text) if re.fullmatch(r'on[A-Z]\w*', name)
    )
    # 启发式兜底：onXxx={ident}。
    callback_props += sum(
        1 for name in BRACED_IDENT.findall(text) if re.fullmatch(r'on[A-Z]\w*', name)
    )
    lines = text.count("\n") + (0 if text.endswith("\n") else 1)
    return {
        "path": str(path),
        "lines": lines,
        "handlers": handler_details,
        "externals": sorted(externals),
        "callback_props": callback_props,
        "widgets": len(widgets),
    }


def main() -> int:
    args = [argument for argument in sys.argv[1:] if not argument.startswith("--")]
    markdown = "--markdown" in sys.argv
    root = Path(args[0]) if args else Path(__file__).resolve().parent.parent
    files = [
        path
        for base in (root / "src" / "ui" / "widgets", root / "demo" / "uix-lang-demo" / "src")
        if base.exists()
        for path in sorted(base.rglob("*.uix"))
    ]
    records = [measure_file(path) for path in files]
    totals = {
        "files": len(records),
        "lines": sum(record["lines"] for record in records),
        "handlers_pure": sum(record["handlers"]["pure"] for record in records),
        "handlers_forward": sum(record["handlers"]["forward"] for record in records),
        "handlers_mixed": sum(record["handlers"]["mixed"] for record in records),
        "callback_props": sum(record["callback_props"] for record in records),
        "external_symbols": len({s for r in records for s in r["externals"]}),
    }
    payload = {"root": str(root), "totals": totals, "files": records}
    if markdown:
        print(f"| 文件 | 行数 | 纯界面处理器 | 混合 | 回调转发 | external 数 |")
        print("|---|---|---|---|---|---|")
        for record in records:
            print(
                f"| `{record['path'].replace(str(root) + '/', '')}` "
                f"| {record['lines']} | {record['handlers']['pure']} "
                f"| {record['handlers']['mixed']} | {record['handlers']['forward']} "
                f"| {len(record['externals'])} |"
            )
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    summary = (
        f"\n汇总: 文件 {totals['files']} 行 {totals['lines']} | "
        f"处理器 纯 {totals['handlers_pure']} / 混合 {totals['handlers_mixed']} / "
        f"转发 {totals['handlers_forward']} | 回调prop {totals['callback_props']} | "
        f"external 符号 {totals['external_symbols']}"
    )
    print(summary, file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
