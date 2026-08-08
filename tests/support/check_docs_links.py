# -*- coding: utf-8 -*-
"""Validate the repository-owned UIX documentation tree, links, and anchors."""
from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlparse

# 测试辅助文件位于 tests/support，仓库根目录向上两级取得。
ROOT = Path(__file__).resolve().parents[2]

# Unicode escapes avoid source-encoding issues on Windows consoles.
PROGRESS = "\u8fdb\u5ea6.md"
PRODUCT = "\u4ea7\u54c1.md"
USAGE = "\u4f7f\u7528.md"
ARCHITECTURE = "\u67b6\u6784.md"
DOMAIN = "\u9886\u57df"
DEMAND = "\u6309\u9700\u9a71\u52a8.md"
PUBLIC_API = "\u516c\u5f00API.md"
UI = "\u754c\u9762.md"
RUNTIME = "\u8fd0\u884c\u65f6.md"
RENDER = "\u6e32\u67d3.md"
DEFECTS_OLD = "\u7f3a\u9677.md"

LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
WIKI_LINK_RE = re.compile(r"\[\[[^\]\n]+\]\]")
USAGE_EXAMPLE_ID_RE = re.compile(
    r"^```rust[^\n]*\buix-compile=([A-Za-z0-9_-]+)", re.MULTILINE
)
TARGET_EXAMPLE_RE = re.compile(r"^```text\s*$", re.MULTILINE)
HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$", re.M)
HTML_ID_RE = re.compile(r'<a\s+id="([^"]+)"\s*>', re.I)
LEGACY_PROJECT_PATH_RE = re.compile(
    r"\u6211\u7684\u9879\u76ee[/\\]+\u8f6f\u4ef6[/\\]+UIX(?:%20| )App",
    re.IGNORECASE,
)
EXTERNAL_SCHEMES = frozenset({"data", "file", "http", "https", "mailto"})
EXTERNAL_DOCUMENT_SCHEMES = frozenset({"file"})

REQUIRED_FILES = (
    ROOT / "README.md",
    ROOT / "docs" / "README.md",
    ROOT / "docs" / PRODUCT,
    ROOT / "docs" / USAGE,
    ROOT / "docs" / ARCHITECTURE,
    ROOT / "docs" / PROGRESS,
)
ROLE_DIRECTORIES = (
    ROOT / "docs" / PRODUCT.removesuffix(".md"),
    ROOT / "docs" / USAGE.removesuffix(".md"),
    ROOT / "docs" / ARCHITECTURE.removesuffix(".md"),
    ROOT / "docs" / PROGRESS.removesuffix(".md"),
)
FORBIDDEN_GENERATED_INDEXES = frozenset(
    {"\u76ee\u5f55\u7d22\u5f15.md", "\u9879\u76ee\u4e0a\u4e0b\u6587\u7d22\u5f15.md"}
)

SCAN = [
    *sorted(ROOT.glob("*.md")),
    ROOT / "demo" / "README.md",
    *sorted((ROOT / "assets").rglob("*.md")),
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
    angle_target = target.startswith("<") and target.endswith(">")
    if angle_target:
        target = target[1:-1]
    if not angle_target and " " in target and not target.startswith("#"):
        target = target.split(" ", 1)[0]
    target = unquote(target)
    if target.startswith("#"):
        return "", target[1:]
    if "#" in target:
        path, frag = target.split("#", 1)
        return path, frag
    return target, None


def target_scheme(raw: str) -> str:
    """Return the URI scheme of a Markdown link target, if it has one."""

    path_part, _ = split_target(raw)
    return urlparse(path_part).scheme.casefold()


def is_external_target(raw: str) -> bool:
    return target_scheme(raw) in EXTERNAL_SCHEMES


def is_external_document_target(raw: str) -> bool:
    return target_scheme(raw) in EXTERNAL_DOCUMENT_SCHEMES


def validate_local_target(
    md: Path, raw: str, anchor_cache: dict[Path, set[str]]
) -> str | None:
    if is_external_target(raw):
        return None
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
    usage_example_owners: dict[str, Path] = {}
    usage_dir = ROOT / "docs" / USAGE.removesuffix(".md")

    for p in REQUIRED_FILES:
        if not p.is_file():
            errors.append(f"missing required file: {p.relative_to(ROOT).as_posix()}")

    for p in ROLE_DIRECTORIES:
        if not p.is_dir():
            errors.append(f"missing role directory: {p.relative_to(ROOT).as_posix()}")

    for p in (ROOT / "docs").rglob("*.md"):
        if p.name in FORBIDDEN_GENERATED_INDEXES:
            errors.append(
                "knowledge-base generated index must not be kept in repository: "
                f"{p.relative_to(ROOT).as_posix()}"
            )

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

    scanned = 0
    external_document_links = 0
    for md in SCAN:
        if not md.is_file():
            continue
        scanned += 1
        for anchor in duplicate_explicit_anchors(md):
            errors.append(
                f"{md.relative_to(ROOT).as_posix()}: duplicate explicit anchor #{anchor}"
            )
        text = md.read_text(encoding="utf-8")
        if text.startswith("---") and "schema: ai-note/" in text.split("---", 2)[1]:
            errors.append(
                f"{md.relative_to(ROOT).as_posix()}: Obsidian governance frontmatter"
            )
        for match in WIKI_LINK_RE.finditer(text):
            errors.append(
                f"{md.relative_to(ROOT).as_posix()}: Obsidian Wiki link -> "
                f"{match.group(0)}"
            )
        if LEGACY_PROJECT_PATH_RE.search(text):
            errors.append(
                f"{md.relative_to(ROOT).as_posix()}: legacy external UIX project path"
            )
        if md.is_relative_to(usage_dir):
            if TARGET_EXAMPLE_RE.search(text):
                errors.append(
                    f"{md.relative_to(ROOT).as_posix()}: unimplemented target example "
                    "must be tracked by progress, not usage"
                )
            for example_id in USAGE_EXAMPLE_ID_RE.findall(text):
                if owner := usage_example_owners.get(example_id):
                    errors.append(
                        f"{md.relative_to(ROOT).as_posix()}: duplicate uix-compile id "
                        f"{example_id} (first in {owner.relative_to(ROOT).as_posix()})"
                    )
                else:
                    usage_example_owners[example_id] = md
        for m in LINK_RE.finditer(text):
            raw = m.group(1).strip()
            if is_external_target(raw):
                if is_external_document_target(raw):
                    external_document_links += 1
                continue
            if error := validate_local_target(md, raw, anchor_cache):
                errors.append(error)

    print(f"scanned {scanned} files")
    print(
        "skipped "
        f"{external_document_links} external document link(s) outside repository boundary"
    )
    if errors:
        print(f"FAIL: {len(errors)} issue(s)")
        for e in errors:
            print(" ", e)
        return 1
    print("OK: repository-local tree + file links valid")
    return 0


if __name__ == "__main__":
    sys.exit(main())
