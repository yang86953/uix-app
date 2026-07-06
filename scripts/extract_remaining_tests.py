#!/usr/bin/env python3
"""Extract remaining inline test modules (json_parser_tests, demo dashboard)."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def extract_mod_block(content: str, mod_name: str) -> tuple[str, str] | None:
    marker = f"#[cfg(test)]\nmod {mod_name} {{"
    idx = content.rfind(marker) if mod_name == "tests" else content.find(marker)
    if idx == -1:
        return None
    open_brace = content.find("{", idx)
    depth = 0
    end = None
    for i in range(open_brace, len(content)):
        ch = content[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                end = i + 1
                break
    if end is None:
        return None
    block = content[idx:end]
    pat = rf"mod {mod_name}\s*\{{(.*)\}}\s*$"
    m = re.search(pat, block, re.DOTALL)
    if not m:
        return None
    body = m.group(1).strip() + "\n"
    prefix = content[:idx].rstrip() + "\n"
    return body, prefix


def main() -> None:
    # settings: json_parser_tests
    src = ROOT / "src/data/settings/settings.rs"
    content = src.read_text(encoding="utf-8")
    result = extract_mod_block(content, "json_parser_tests")
    if result is None:
        raise SystemExit("settings: no json_parser_tests block")
    body, prefix = result
    dest = ROOT / "src/tests/data/settings/settings.rs"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(body, encoding="utf-8", newline="\n")
    src.write_text(
        prefix
        + "\n#[cfg(test)]\n#[path = \"../../tests/data/settings/settings.rs\"]\nmod json_parser_tests;\n",
        encoding="utf-8",
        newline="\n",
    )
    print(f"extracted settings -> {dest.relative_to(ROOT)}")

    # demo dashboard: tests
    src = ROOT / "demo/src/demos/dashboard/mod.rs"
    content = src.read_text(encoding="utf-8")
    result = extract_mod_block(content, "tests")
    if result is None:
        raise SystemExit("demo: no tests block")
    body, prefix = result
    dest = ROOT / "demo/src/tests/demos/dashboard.rs"
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(body, encoding="utf-8", newline="\n")
    src.write_text(
        prefix + "\n#[cfg(test)]\n#[path = \"../../tests/demos/dashboard.rs\"]\nmod tests;\n",
        encoding="utf-8",
        newline="\n",
    )
    print(f"extracted demo dashboard -> {dest.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
