# -*- coding: utf-8 -*-
"""Validate markdown links under docs/, AGENTS.md, README.md, demo/README.md."""
from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent

# Unicode escapes avoid source-encoding issues on Windows consoles.
PRODUCT = "\u4ea7\u54c1.md"
DECISIONS = "\u51b3\u7b56.md"
PROGRESS = "\u8fdb\u5ea6.md"
ISSUES = "\u95ee\u9898.md"
ARCH = "\u67b6\u6784.md"
DOMAIN = "\u9886\u57df"
DEMAND = "\u6309\u9700\u9a71\u52a8.md"
PUBLIC_API = "\u516c\u5f00API.md"
UI = "\u754c\u9762.md"
RUNTIME = "\u8fd0\u884c\u65f6.md"
RENDER = "\u6e32\u67d3.md"
DEFECTS_OLD = "\u7f3a\u9677.md"

LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$", re.M)
HTML_ID_RE = re.compile(r'<a\s+id="([^"]+)"\s*>', re.I)

SCAN = [
    ROOT / "AGENTS.md",
    ROOT / "README.md",
    ROOT / "demo" / "README.md",
    *sorted((ROOT / "docs").rglob("*.md")),
]


def slugify(heading: str) -> str:
    s = heading.strip().lower()
    s = re.sub(r"[`*_~]", "", s)
    s = re.sub(r"[^\w\u4e00-\u9fff\s-]", "", s, flags=re.UNICODE)
    s = re.sub(r"\s+", "-", s.strip())
    return s


def collect_anchors(path: Path) -> set[str]:
    text = path.read_text(encoding="utf-8")
    anchors = set(HTML_ID_RE.findall(text))
    for m in HEADING_RE.finditer(text):
        anchors.add(slugify(m.group(2)))
    return anchors


def split_target(target: str) -> tuple[str, str | None]:
    target = target.strip()
    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1]
    if " " in target and not target.startswith("#"):
        target = target.split(" ", 1)[0]
    target = unquote(target)
    if target.startswith("#"):
        return "", target[1:]
    if "#" in target:
        path, frag = target.split("#", 1)
        return path, frag
    return target, None


def main() -> int:
    errors: list[str] = []
    warnings: list[str] = []
    anchor_cache: dict[Path, set[str]] = {}

    required = [
        ROOT / "AGENTS.md",
        ROOT / "docs" / PRODUCT,
        ROOT / "docs" / DECISIONS,
        ROOT / "docs" / PROGRESS,
        ROOT / "docs" / ISSUES,
        ROOT / "docs" / ARCH,
    ]
    for p in required:
        if not p.is_file():
            errors.append(f"missing required file: {p.relative_to(ROOT).as_posix()}")

    domain_dir = ROOT / "docs" / DOMAIN
    if domain_dir.exists():
        errors.append(
            f"docs/{DOMAIN}/ must not exist (generic skeleton only): "
            f"{domain_dir.relative_to(ROOT).as_posix()}"
        )

    forbidden_root_domain = [
        ROOT / "docs" / DEMAND,
        ROOT / "docs" / PUBLIC_API,
        ROOT / "docs" / "ui.md",
        ROOT / "docs" / UI,
        ROOT / "docs" / RUNTIME,
        ROOT / "docs" / RENDER,
        ROOT / "docs" / DEFECTS_OLD,
    ]
    for p in forbidden_root_domain:
        if p.exists():
            errors.append(
                f"removed encyclopedia must not sit at docs root: {p.relative_to(ROOT).as_posix()}"
            )

    for md in SCAN:
        if not md.is_file():
            continue
        text = md.read_text(encoding="utf-8")
        for m in LINK_RE.finditer(text):
            raw = m.group(1).strip()
            if raw.startswith(("http://", "https://", "mailto:", "data:")):
                continue
            path_part, frag = split_target(raw)
            if path_part == "":
                target = md
            else:
                target = (md.parent / path_part).resolve()
            if not target.is_file():
                errors.append(f"{md.relative_to(ROOT).as_posix()}: broken link -> {raw}")
                continue
            if frag:
                if target not in anchor_cache:
                    anchor_cache[target] = collect_anchors(target)
                anchors = anchor_cache[target]
                frag_l = frag.lower()
                if frag not in anchors and frag_l not in {a.lower() for a in anchors}:
                    warnings.append(
                        f"{md.relative_to(ROOT).as_posix()}: missing anchor #{frag} in {target.relative_to(ROOT).as_posix()}"
                    )

    print(f"scanned {len(SCAN)} files")
    if warnings:
        print(f"WARN: {len(warnings)} missing-anchor(s) (non-fatal)")
        for w in warnings[:30]:
            print(" ", w)
        if len(warnings) > 30:
            print(f"  ... and {len(warnings) - 30} more")
    if errors:
        print(f"FAIL: {len(errors)} issue(s)")
        for e in errors:
            print(" ", e)
        return 1
    print("OK: tree + file links valid")
    return 0


if __name__ == "__main__":
    sys.exit(main())
