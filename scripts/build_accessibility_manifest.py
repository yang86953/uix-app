"""S4-03 accessibility manifest assembler.

Reads the per-component semantic evidence captured by the real-window test
``component_visual``-style collector (``tests/support/agent_gui_windows/
accessibility.rs``) plus the function ledger rows in ``docs/进度/功能账本.md``
and produces the 110-row accessibility manifest:

- ``test-reports/<candidate>/accessibility/manifest.json``
- ``test-reports/<candidate>/accessibility/items/<item-id>.json`` (one per
  ledger row that maps to a captured component case)

Schema is defined in ``docs/进度/无障碍证据.md``. Rows that have no component
case in the QA demo (advanced charts) are marked ``pending-scene``; the six
historical ledger rows ``8.7``..``8.12`` that were not preserved in the
migration are marked ``ledger-gap`` and never counted as captured.

Usage:
    python scripts/build_accessibility_manifest.py <candidate>
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "docs" / "进度" / "功能账本.md"
COMPONENTS_DIR_NAME = "components"

# Component names that the QA demo renders inside dedicated component cases
# but which the function ledger lists as their own rows. Component name in the
# ledger is camel-case; case ids are kebab-case.
COMPONENT_TO_CASE: dict[str, str] = {
    "Alert": "alert",
    "Anchor": "anchor",
    "BarChart": "bar-chart",
    "Breadcrumb": "breadcrumb",
    "Calendar": "calendar",
    "Carousel": "carousel",
    "Collapse": "collapse",
    "Dropdown": "dropdown",
    "FloatButton": "float-button",
    "FloatButtonGroup": "float-button-group",
    "Image": "image",
    "ImageGroup": "image-group",
    "LineChart": "line-chart",
    "Menu": "menu",
    "Message": "message",
    "Modal": "modal",
    "Navigation": "navigation",
    "Notification": "notification",
    "Pagination": "pagination",
    "PieChart": "pie-chart",
    "Popover": "popover",
    "ProgressBar": "progress-bar",
    "Skeleton": "skeleton",
    "Spin": "spin",
    "Steps": "steps",
    "Table": "table",
    "Tabs": "tabs",
    "Tooltip": "tooltip",
    "Tree": "tree",
    "Typography": "typography",
}

# Advanced chart rows have no dedicated case in the component QA demo; their
# scene is the charts demo page (not part of S4-03 evidence scope).
PENDING_SCENE_COMPONENTS = {
    "AreaChart",
    "Chart通用",
    "ComboChart",
    "FunnelChart",
    "Gauge",
    "Heatmap",
    "RadarChart",
    "ScatterChart",
    "Treemap",
    "WaterfallChart",
}

LEDGER_GAP_ITEMS = {f"8.{n}" for n in range(7, 13)}


def ledger_rows() -> list[dict[str, str]]:
    rows: list[dict[str, str]] = []
    for line in LEDGER.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^(\d+\.\d+)\|(\d+)\|([^|]+)\|", line)
        if not match:
            continue
        rows.append(
            {
                "item_id": match.group(1),
                "batch": match.group(2),
                "component": match.group(3).strip(),
            }
        )
    return rows


def component_evidence(candidate_dir: Path) -> dict[str, dict]:
    components_dir = candidate_dir / COMPONENTS_DIR_NAME
    if not components_dir.is_dir():
        return {}
    evidence: dict[str, dict] = {}
    for path in sorted(components_dir.glob("*.json")):
        data = json.loads(path.read_text(encoding="utf-8"))
        evidence[data["case_id"]] = data
    return evidence


def build(candidate: str) -> None:
    root = ROOT / "test-reports" / candidate / "accessibility"
    items_dir = root / "items"
    items_dir.mkdir(parents=True, exist_ok=True)

    rows = ledger_rows()
    if len(rows) != 104:
        print(f"WARNING: ledger rows = {len(rows)} (expected 104)", file=sys.stderr)

    evidence = component_evidence(root)
    captured = set(COMPONENT_TO_CASE.values())

    entries: list[dict] = []
    captured_items = 0
    for row in rows:
        component = row["component"]
        if component in PENDING_SCENE_COMPONENTS:
            status = "pending-scene"
            case_id = None
        elif component in COMPONENT_TO_CASE:
            case_id = COMPONENT_TO_CASE[component]
            if case_id not in evidence:
                print(
                    f"ERROR: no evidence for component {component} (case {case_id})",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            status = "captured"
            captured_items += 1
        else:
            print(f"ERROR: unmapped ledger component {component}", file=sys.stderr)
            raise SystemExit(1)
        entries.append(
            {
                "item_id": row["item_id"],
                "batch": int(row["batch"]),
                "component": component,
                "case_id": case_id,
                "evidence_file": (
                    f"items/{row['item_id']}.json" if status == "captured" else None
                ),
                "status": status,
            }
        )
        if status == "captured":
            write_item(items_dir / f"{row['item_id']}.json", row, evidence[case_id])

    for item_id in sorted(LEDGER_GAP_ITEMS):
        entries.append(
            {
                "item_id": item_id,
                "batch": 8,
                "component": "ledger-gap",
                "case_id": None,
                "evidence_file": None,
                "status": "ledger-gap",
            }
        )

    entries.sort(key=lambda entry: tuple(float(part) for part in entry["item_id"].split(".")))
    by_status = {status: 0 for status in ("captured", "pending-scene", "ledger-gap")}
    for entry in entries:
        by_status[entry["status"]] += 1
    manifest = {
        "schema": "uix.accessibility.manifest.v1",
        "candidate": candidate,
        "declared": 110,
        "captured": by_status["captured"],
        "pending_scene": by_status["pending-scene"],
        "ledger_gap": by_status["ledger-gap"],
        "entries": entries,
    }
    (root / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(
        f"manifest: {root / 'manifest.json'} "
        f"captured={by_status['captured']} "
        f"pending_scene={by_status['pending-scene']} "
        f"ledger_gap={by_status['ledger-gap']}"
    )
    assert sum(by_status.values()) == 110
    # Every captured row must have an item file.
    for entry in entries:
        if entry["status"] == "captured":
            assert (root / entry["evidence_file"]).is_file(), entry


def write_item(path: Path, row: dict[str, str], evidence: dict) -> None:
    target = evidence.get("target") or {}
    focus = evidence.get("focus") or {}
    payload = {
        "schema": "uix.accessibility.item.v1",
        "item_id": row["item_id"],
        "batch": int(row["batch"]),
        "component": row["component"],
        "case_id": evidence.get("case_id"),
        "component_capture": f"components/{evidence['case_index']:02d}-{evidence['case_id']}.json",
        "role": target.get("role"),
        "name": target.get("name"),
        "state": target.get("state"),
        "value": target.get("value"),
        "actions": target.get("actions"),
        "focus": focus.get("focus"),
        "keyboard": target.get("keyboard"),
        "declared_states": evidence.get("declared_states"),
    }
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    build(sys.argv[1])
