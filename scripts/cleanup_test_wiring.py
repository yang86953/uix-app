#!/usr/bin/env python3
"""Remove broken test path wiring; keep only existing test bodies."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"


def resolve_test_path(source: Path, attr: str) -> Path:
    return (source.parent / attr).resolve()


def cleanup_source(path: Path) -> bool:
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(
        r"\n#\[cfg\(test\)\]\n#\[path = \"([^\"]+)\"\]\nmod tests;\n",
        re.MULTILINE,
    )
    changed = False

    def repl(match: re.Match[str]) -> str:
        nonlocal changed
        target = resolve_test_path(path, match.group(1))
        if not target.exists():
            changed = True
            return "\n"
        body = target.read_text(encoding="utf-8").strip()
        if len(body.splitlines()) < 3:
            changed = True
            return "\n"
        return match.group(0)

    new_text = pattern.sub(repl, text)
    if changed:
        path.write_text(new_text.rstrip() + "\n", encoding="utf-8", newline="\n")
        print(f"cleaned {path.relative_to(ROOT)}")
    return changed


def wire(source: Path, attr: str) -> None:
    text = source.read_text(encoding="utf-8")
    if "#[path =" in text and "mod tests;" in text:
        return
    text = text.rstrip() + f'\n\n#[cfg(test)]\n#[path = "{attr}"]\nmod tests;\n'
    source.write_text(text, encoding="utf-8", newline="\n")
    print(f"wired {source.relative_to(ROOT)}")


def main() -> None:
    for path in sorted(SRC.rglob("*.rs")):
        if path.is_relative_to(SRC / "tests"):
            continue
        cleanup_source(path)

    style_mod = SRC / "ui" / "foundation" / "style" / "mod.rs"
    wire(style_mod, "../../../tests/ui/foundation/style.rs")

    tree_core = SRC / "ui" / "core" / "widget" / "tree_core.rs"
    if not re.search(r'tests/ui/core/widget/tree_core\.rs', tree_core.read_text()):
        wire(tree_core, "../../../../tests/ui/core/widget/tree_core.rs")

    lib = SRC / "lib.rs"
    lib_text = lib.read_text(encoding="utf-8")
    lib_text = re.sub(r"\n#\[cfg\(test\)\]\nmod tests;\n?", "\n", lib_text)
    lib.write_text(lib_text.rstrip() + "\n", encoding="utf-8", newline="\n")

    print("done")


if __name__ == "__main__":
    main()
