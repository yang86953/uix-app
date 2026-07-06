#!/usr/bin/env python3
"""Wire src/tests/*.rs into matching source modules via #[path]."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
TESTS = SRC / "tests"


def test_path_attr(source: Path) -> str:
    rel = source.relative_to(SRC).as_posix()
    ups = len(Path(rel).parts) - 1
    return "../" * ups + f"tests/{rel}"


def strip_test_wiring(text: str) -> str:
    return re.sub(
        r"\n#\[cfg\(test\)\]\n(?:#\[path = \"[^\"]+\"\]\n)?mod tests;\n",
        "\n",
        text,
    )


def wire_source(source: Path) -> None:
    rel = source.relative_to(SRC)
    test_file = TESTS / rel.with_suffix(".rs")
    if not test_file.exists():
        return
    body = test_file.read_text(encoding="utf-8").strip()
    if len(body.splitlines()) < 3:
        return

    # style lives at style/mod.rs but test file is foundation/style.rs
    if source == SRC / "ui" / "foundation" / "style" / "mod.rs":
        test_file = TESTS / "ui" / "foundation" / "style.rs"
        if not test_file.exists():
            return
        attr = "../../../tests/ui/foundation/style.rs"
    else:
        attr = test_path_attr(source)

    text = strip_test_wiring(source.read_text(encoding="utf-8")).rstrip()
    text += f'\n\n#[cfg(test)]\n#[path = "{attr}"]\nmod tests;\n'
    source.write_text(text + "\n", encoding="utf-8", newline="\n")
    print(f"wired {source.relative_to(ROOT)}")


def main() -> None:
    # Special case: style test file maps to style/mod.rs
    style_mod = SRC / "ui" / "foundation" / "style" / "mod.rs"
    style_test = TESTS / "ui" / "foundation" / "style.rs"
    if style_test.exists():
        wire_source(style_mod)

    for test_file in sorted(TESTS.rglob("*.rs")):
        rel = test_file.relative_to(TESTS)
        if rel.as_posix() == "ui/foundation/style.rs":
            continue
        source = SRC / rel
        if source.name == "mod.rs":
            source = source  # design_tokens/mod.rs etc.
        if source.exists():
            wire_source(source)

    lib = SRC / "lib.rs"
    lib.write_text(strip_test_wiring(lib.read_text(encoding="utf-8")).rstrip() + "\n", encoding="utf-8")

    print("done")


if __name__ == "__main__":
    main()
