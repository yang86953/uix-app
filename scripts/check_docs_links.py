# -*- coding: utf-8 -*-
"""Validate local Markdown links in core docs, reports, and project READMEs."""
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
    *sorted((ROOT / "assets").rglob("*.md")),
    *sorted((ROOT / "docs").rglob("*.md")),
    *sorted((ROOT / "test-reports").rglob("*.md")),
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


def duplicate_explicit_anchors(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    seen: set[str] = set()
    duplicates: set[str] = set()
    for anchor in HTML_ID_RE.findall(text):
        key = anchor.casefold()
        if key in seen:
            duplicates.add(anchor)
        else:
            seen.add(key)
    return sorted(duplicates, key=str.casefold)


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


def validate_local_target(
    md: Path, raw: str, anchor_cache: dict[Path, set[str]]
) -> str | None:
    path_part, frag = split_target(raw)
    target = md if path_part == "" else (md.parent / path_part).resolve()
    if not target.is_file():
        return f"{md.relative_to(ROOT).as_posix()}: broken link -> {raw}"
    if not frag:
        return None
    if target not in anchor_cache:
        anchor_cache[target] = collect_anchors(target)
    anchors = anchor_cache[target]
    frag_l = frag.lower()
    if frag not in anchors and frag_l not in {a.lower() for a in anchors}:
        return (
            f"{md.relative_to(ROOT).as_posix()}: missing anchor #{frag} "
            f"in {target.relative_to(ROOT).as_posix()}"
        )
    return None


def main() -> int:
    errors: list[str] = []
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
        for anchor in duplicate_explicit_anchors(md):
            errors.append(
                f"{md.relative_to(ROOT).as_posix()}: duplicate explicit anchor #{anchor}"
            )
        text = md.read_text(encoding="utf-8")
        for m in LINK_RE.finditer(text):
            raw = m.group(1).strip()
            if raw.startswith(("http://", "https://", "mailto:", "data:")):
                continue
            if error := validate_local_target(md, raw, anchor_cache):
                errors.append(error)

    print(f"scanned {len(SCAN)} files")
    if errors:
        print(f"FAIL: {len(errors)} issue(s)")
        for e in errors:
            print(" ", e)
        return 1
    print("OK: tree + file links valid")
    return 0


if __name__ == "__main__":
    sys.exit(main())
