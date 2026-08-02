#!/usr/bin/env python3
"""Run the frozen UIX 0.0.1 release G5 measurement protocol.

The runner intentionally accepts only a clean committed tree and the default
release build. It never converts missing hardware/automation into a pass: the
evidence summary distinguishes failed measurements from blocked G5 classes.
"""
from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import math
import os
import platform
import queue
import re
import secrets
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Iterator, Sequence


ROOT = Path(__file__).resolve().parent.parent
EVIDENCE_SCHEMA = 1
FRAME_SCHEMA = 1
WARMUP_US = 30_000_000
MIN_PRESENTED_FRAMES = 10_000
P95_LIMIT_US = 16_700
SOAK_SECONDS = 8 * 60 * 60
MACRO_CYCLE_SECONDS = 10 * 60
MACRO_CYCLES = 48
MACRO_IDLE_SECONDS = 60
REQUIRED_MACRO_PHASES = frozenset(("interaction", "idle"))
MEMORY_MEDIAN_WINDOW_SECONDS = 5 * 60
MEMORY_GROWTH_LIMIT_BYTES = 64 * 1024 * 1024
MIN_MEMORY_WINDOW_SAMPLES = 285
REQUIRED_CLASS_LOOPS = 1_000
LOOP_FRAME_SCENARIOS = {
    "window": ("window_open", "window_closed"),
    "modal": ("modal_feedback", "modal_closed"),
    "drawer": ("drawer_feedback", "drawer_closed"),
    "theme": ("page_general_dark", "page_general_light"),
    "dpi": ("dpi_changed", "dpi_restored"),
}
INITIAL_FRAME_WATCHDOG_SECONDS = 90
FRAME_WATCHDOG_SECONDS = 30
STOP_WATCHDOG_SECONDS = 10
MONOTONIC_DRIFT_LIMIT_US = 5_000_000
MACRO_PHASE_TOLERANCE_US = 5_000_000
IDLE_PHASE_WATCHDOG_SECONDS = MACRO_IDLE_SECONDS + 15
IDLE_SETTLE_SECONDS = 5
IDLE_CPU_CORE_LIMIT = 0.25
IDLE_CPU_CONSECUTIVE_LIMIT = 5
MIN_IDLE_CPU_SAMPLES_PER_CYCLE = 45
EXPECTED_DRAWABLE_WIDTH = 1_200
EXPECTED_DRAWABLE_HEIGHT = 800
EXPECTED_WINDOWS_BUILD = "26200"
EXPECTED_CPU_FRAGMENT = "AMD Ryzen 7 7700"
EXPECTED_GPU_FRAGMENT = "NVIDIA GeForce RTX 4070 Ti SUPER"
EXPECTED_GPU_DRIVER = "32.0.16.1062"
EXPECTED_RAM_MIN_BYTES = 31 * 1024**3
EXPECTED_RAM_MAX_BYTES = 33 * 1024**3

REQUIRED_EVIDENCE_FILES = frozenset(
    {
        "build.log",
        "candidate-uix-demo.exe",
        "candidate-Cargo.lock",
        "environment.json",
        "frames.ndjson",
        "memory.ndjson",
        "raw.log",
        "report.md",
        "runner.py",
        "scenarios.ndjson",
        "summary.json",
    }
)

PASS_EXIT = 0
FAIL_EXIT = 1
BLOCKED_EXIT = 2
INVALID_EVIDENCE_EXIT = 3

FIELD_PATTERN = re.compile(r"(?<![A-Za-z0-9_])([A-Za-z][A-Za-z0-9_]*)=([^\s]+)")
COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")
EXPECTED_ENGINE_MARKER = (
    "Graphics bootstrap: selected recipe backend=vulkan; "
    "raster=gpu_native; present=swapchain"
)
EXPECTED_WGPU_MARKER = "WgpuContext: backend=vulkan"
FORBIDDEN_BACKEND_MARKERS = (
    "GPU engine initialized after probe fallback",
    "fallback=software_cpu",
    "CPU software engine initialized",
)
ABNORMAL_PATTERNS = tuple(
    re.compile(pattern, re.IGNORECASE)
    for pattern in (
        r"\[ERROR\]",
        r"\[FATAL\]",
        r"validation error",
        r"thread .* panicked",
        r"\bpanic(?:ked)?\b",
        r"\bdeadlock\b",
        r"\bout of memory\b",
        r"\bdevice lost\b",
    )
)


class G5Error(RuntimeError):
    """A measurement or evidence contract failed."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json_atomic(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def iter_ndjson_objects(path: Path) -> Iterator[dict[str, Any]]:
    try:
        stream = path.open("r", encoding="utf-8")
    except OSError as error:
        raise G5Error(f"cannot read {path.name}: {error}") from error
    with stream:
        for line_number, line in enumerate(stream, start=1):
            if not line.strip():
                continue
            try:
                payload = json.loads(line)
            except json.JSONDecodeError as error:
                raise G5Error(
                    f"invalid {path.name}:{line_number}: {error}"
                ) from error
            if not isinstance(payload, dict):
                raise G5Error(f"{path.name}:{line_number} must be a JSON object")
            yield payload


def capture(command: Sequence[str], cwd: Path = ROOT) -> str:
    completed = subprocess.run(
        list(command),
        cwd=cwd,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    if completed.returncode != 0:
        rendered = " ".join(command)
        raise G5Error(f"command failed ({completed.returncode}): {rendered}\n{completed.stdout}")
    return completed.stdout


def ensure_clean_status(status: str) -> None:
    if status.strip():
        raise G5Error("G5 release evidence requires a clean worktree")


def require_clean_commit(root: Path = ROOT) -> str:
    ensure_clean_status(capture(["git", "status", "--porcelain=v1", "--untracked-files=all"], root))
    commit = capture(["git", "rev-parse", "HEAD"], root).strip().lower()
    if not COMMIT_PATTERN.fullmatch(commit):
        raise G5Error(f"invalid candidate commit: {commit!r}")
    return commit


def nearest_rank(values: Sequence[int], percentile: float) -> int:
    if not values:
        raise G5Error("nearest-rank percentile requires at least one sample")
    if not 0 < percentile <= 1:
        raise ValueError("percentile must be in (0, 1]")
    ordered = sorted(values)
    rank = max(1, math.ceil(percentile * len(ordered)))
    return ordered[rank - 1]


def summarize_performance(
    frame_us: Sequence[int],
    minimum_samples: int = MIN_PRESENTED_FRAMES,
    p95_limit_us: int = P95_LIMIT_US,
) -> dict[str, Any]:
    if len(frame_us) < minimum_samples:
        raise G5Error(
            f"insufficient successful presented frames: {len(frame_us)} < {minimum_samples}"
        )
    p95 = nearest_rank(frame_us, 0.95)
    return {
        "sample_count": len(frame_us),
        "percentile_method": "nearest-rank",
        "p50_us": nearest_rank(frame_us, 0.50),
        "p95_us": p95,
        "p99_us": nearest_rank(frame_us, 0.99),
        "max_us": max(frame_us),
        "p95_limit_us": p95_limit_us,
        "status": "pass" if p95 <= p95_limit_us else "fail",
    }


def summarize_distribution(values: Sequence[int]) -> dict[str, int]:
    if not values:
        raise G5Error("scenario percentile requires at least one presented frame")
    return {
        "sample_count": len(values),
        "p50_us": nearest_rank(values, 0.50),
        "p95_us": nearest_rank(values, 0.95),
        "p99_us": nearest_rank(values, 0.99),
        "max_us": max(values),
    }


def parse_tagged_fields(line: str, marker: str) -> dict[str, str] | None:
    position = line.find(marker)
    if position < 0:
        return None
    fragment = line[position + len(marker) :]
    fields: dict[str, str] = {}
    for match in FIELD_PATTERN.finditer(fragment):
        key, value = match.groups()
        if key in fields:
            raise G5Error(f"duplicate {marker} field {key!r}")
        fields[key] = value
    return fields


def _required_int(fields: dict[str, str], key: str) -> int:
    try:
        value = int(fields[key], 10)
    except KeyError as error:
        raise G5Error(f"G5_FRAME missing field {key!r}") from error
    except ValueError as error:
        raise G5Error(f"G5_FRAME field {key!r} is not an integer") from error
    if value < 0:
        raise G5Error(f"G5_FRAME field {key!r} must be nonnegative")
    return value


def parse_g5_frame(line: str) -> dict[str, Any] | None:
    fields = parse_tagged_fields(line, "G5_FRAME")
    if fields is None:
        return None
    integer_fields = (
        "schema",
        "frame_seq",
        "window",
        "logical_width",
        "logical_height",
        "dpi",
        "monotonic_us",
        "presented",
        "present_skipped",
        "active_work",
        "due_active_work",
        "frame_us",
        "present_us",
        "drawable_width",
        "drawable_height",
        "drawable_pixels",
        "wgpu_surface_present_cpu_us",
    )
    parsed: dict[str, Any] = {key: _required_int(fields, key) for key in integer_fields}
    try:
        parsed["scenario"] = fields["scenario"]
    except KeyError as error:
        raise G5Error("G5_FRAME missing field 'scenario'") from error
    if parsed["schema"] != FRAME_SCHEMA:
        raise G5Error(
            f"G5_FRAME schema mismatch: {parsed['schema']} != {FRAME_SCHEMA}"
        )
    if not parsed["scenario"]:
        raise G5Error("G5_FRAME scenario must not be empty")
    if parsed["presented"] != 1 or parsed["present_skipped"] != 0:
        raise G5Error("G5_FRAME is not a successful real presentation")
    if parsed["drawable_pixels"] <= 0:
        raise G5Error("G5_FRAME lacks a positive production drawable pixel count")
    if (
        parsed["drawable_width"] != EXPECTED_DRAWABLE_WIDTH
        or parsed["drawable_height"] != EXPECTED_DRAWABLE_HEIGHT
    ):
        raise G5Error(
            "G5_FRAME drawable does not match the frozen 1200x800 physical target"
        )
    if (
        parsed["logical_width"] != EXPECTED_DRAWABLE_WIDTH
        or parsed["logical_height"] != EXPECTED_DRAWABLE_HEIGHT
        or parsed["dpi"] != 96
    ):
        raise G5Error(
            "G5_FRAME window does not match the frozen 1200x800 logical/96-DPI target"
        )
    if parsed["drawable_pixels"] != (
        parsed["drawable_width"] * parsed["drawable_height"]
    ):
        raise G5Error("G5_FRAME drawable dimensions/pixel count are inconsistent")
    for key, value in fields.items():
        if key not in parsed:
            try:
                parsed[key] = int(value, 10)
            except ValueError:
                parsed[key] = value
    return parsed


def bind_loop_frame_proofs(
    loop_records: Iterable[dict[str, Any]],
    frames: Iterable[dict[str, Any]],
) -> dict[str, set[int]]:
    """Bind every successful loop proof to two exact, one-use presented frames."""
    validated: dict[str, set[int]] = {
        category: set() for category in LOOP_FRAME_SCENARIOS
    }
    requested: set[tuple[int, int]] = set()
    used_by: dict[tuple[int, int], str] = {}
    proofs: list[
        tuple[str, int, tuple[int, int], tuple[int, int], str, str]
    ] = []

    for record in loop_records:
        category = record.get("category")
        iteration = record.get("iteration")
        if category not in LOOP_FRAME_SCENARIOS:
            raise G5Error("G5 loop frame proof has an unknown category")
        if type(iteration) is not int or iteration <= 0:
            raise G5Error("G5 loop frame proof has an invalid iteration")
        if iteration in validated[category]:
            raise G5Error(f"duplicate G5 loop frame proof {category}.{iteration}")

        ref_names = (
            "open_window",
            "open_frame_seq",
            "closed_window",
            "closed_frame_seq",
        )
        refs = tuple(record.get(name) for name in ref_names)
        if any(type(value) is not int or value <= 0 for value in refs):
            raise G5Error("G5 loop frame proof has an invalid frame reference")
        open_key = (refs[0], refs[1])
        closed_key = (refs[2], refs[3])
        if open_key == closed_key:
            raise G5Error(
                f"G5 loop frame proof {category}.{iteration} reuses its open frame"
            )
        proof_name = f"{category}.{iteration}"
        for state, key in (("open", open_key), ("closed", closed_key)):
            previous = used_by.get(key)
            if previous is not None:
                raise G5Error(
                    f"G5 loop frame reference {key} is reused by "
                    f"{previous} and {proof_name}.{state}"
                )
            used_by[key] = f"{proof_name}.{state}"
            requested.add(key)

        open_base, closed_base = LOOP_FRAME_SCENARIOS[category]
        proofs.append(
            (
                category,
                iteration,
                open_key,
                closed_key,
                f"{open_base}.{iteration}",
                f"{closed_base}.{iteration}",
            )
        )
        # Reserve the category/iteration immediately so duplicates are rejected
        # before any frame data is trusted.
        validated[category].add(iteration)

    bound_frames: dict[tuple[int, int], dict[str, Any]] = {}
    for frame in frames:
        window = frame.get("window")
        sequence = frame.get("frame_seq")
        if type(window) is not int or type(sequence) is not int:
            continue
        key = (window, sequence)
        if key not in requested:
            continue
        if key in bound_frames:
            raise G5Error(f"duplicate presented G5 frame identity {key}")
        bound_frames[key] = frame

    for category, iteration, open_key, closed_key, open_scenario, closed_scenario in proofs:
        proof_name = f"{category}.{iteration}"
        open_frame = bound_frames.get(open_key)
        closed_frame = bound_frames.get(closed_key)
        if open_frame is None or closed_frame is None:
            raise G5Error(
                f"G5 loop frame proof {proof_name} references a missing presented frame"
            )
        for state, frame, expected_scenario in (
            ("open", open_frame, open_scenario),
            ("closed", closed_frame, closed_scenario),
        ):
            if frame.get("presented") != 1 or frame.get("present_skipped") != 0:
                raise G5Error(
                    f"G5 loop frame proof {proof_name}.{state} is not a real presentation"
                )
            if frame.get("scenario") != expected_scenario:
                raise G5Error(
                    f"G5 loop frame proof {proof_name}.{state} expected scenario "
                    f"{expected_scenario!r}, got {frame.get('scenario')!r}"
                )
        open_monotonic = open_frame.get("monotonic_us")
        closed_monotonic = closed_frame.get("monotonic_us")
        if (
            type(open_monotonic) is not int
            or type(closed_monotonic) is not int
            or open_monotonic < WARMUP_US
            or closed_monotonic <= open_monotonic
        ):
            raise G5Error(
                f"G5 loop frame proof {proof_name} is not an ordered post-warmup pair"
            )

    return validated


def abnormal_log_reason(line: str) -> str | None:
    for pattern in ABNORMAL_PATTERNS:
        if pattern.search(line):
            return pattern.pattern
    return None


def validate_process_outcome(
    returncode: int,
    completion_seen: bool,
    abnormal_lines: Sequence[str],
    watchdog_reason: str | None = None,
) -> None:
    if watchdog_reason:
        raise G5Error(f"watchdog failure: {watchdog_reason}")
    if returncode != 0:
        raise G5Error(f"G5 demo exited abnormally with code {returncode}")
    if not completion_seen:
        raise G5Error("G5 demo exited without the authenticated completion marker")
    if abnormal_lines:
        raise G5Error(f"G5 demo emitted abnormal logs: {abnormal_lines[0]}")


@dataclass
class BackendEvidence:
    engine_seen: bool = False
    wgpu_seen: bool = False
    reference_adapter_seen: bool = False
    forbidden: list[str] = field(default_factory=list)

    def observe(self, line: str) -> None:
        lowered = line.lower()
        if EXPECTED_ENGINE_MARKER.lower() in lowered:
            self.engine_seen = True
        if EXPECTED_WGPU_MARKER.lower() in lowered:
            self.wgpu_seen = True
            if (
                EXPECTED_GPU_FRAGMENT.lower() in lowered
                and "type=discretegpu" in lowered
            ):
                self.reference_adapter_seen = True
        for marker in FORBIDDEN_BACKEND_MARKERS:
            if marker.lower() in lowered:
                self.forbidden.append(line.strip())

    def validate(self) -> dict[str, Any]:
        if not self.engine_seen:
            raise G5Error("default Vulkan gpu_native/swapchain engine marker was not observed")
        if not self.wgpu_seen:
            raise G5Error("production WgpuContext Vulkan marker was not observed")
        if not self.reference_adapter_seen:
            raise G5Error("frozen NVIDIA discrete wgpu adapter marker was not observed")
        if self.forbidden:
            raise G5Error(f"graphics fallback was observed: {self.forbidden[0]}")
        return {
            "default_backend": "vulkan",
            "raster": "gpu_native",
            "present": "swapchain",
            "fallback_observed": False,
            "reference_adapter": EXPECTED_GPU_FRAGMENT,
            "status": "pass",
        }


@dataclass
class FrameAccumulator:
    commit: str
    measured_frame_us: list[int] = field(default_factory=list)
    measured_frame_us_by_scenario: dict[str, list[int]] = field(default_factory=dict)
    measured_present_interval_us: list[int] = field(default_factory=list)
    successful_total: int = 0
    latest_monotonic_us: int = 0
    last_sequence_by_window: dict[int, int] = field(default_factory=dict)
    last_monotonic_by_window: dict[int, int] = field(default_factory=dict)
    last_scenario_by_window: dict[int, str] = field(default_factory=dict)
    scenario_names: set[str] = field(default_factory=set)
    presented_scenario_iterations: dict[str, set[int]] = field(default_factory=dict)
    macro_cycles: set[int] = field(default_factory=set)

    def observe(self, frame: dict[str, Any]) -> dict[str, Any]:
        window = frame["window"]
        sequence = frame["frame_seq"]
        previous = self.last_sequence_by_window.get(window)
        if previous is not None and sequence != previous + 1:
            raise G5Error(
                f"non-contiguous G5 frame sequence for window {window}: {previous} -> {sequence}"
            )
        if frame["monotonic_us"] < self.latest_monotonic_us:
            raise G5Error("G5 monotonic frame timestamp moved backwards")
        previous_monotonic = self.last_monotonic_by_window.get(window)
        self.last_sequence_by_window[window] = sequence
        self.last_monotonic_by_window[window] = frame["monotonic_us"]
        self.latest_monotonic_us = frame["monotonic_us"]
        self.successful_total += 1
        self.scenario_names.add(frame["scenario"])
        scenario_name, separator, iteration_text = frame["scenario"].rpartition(".")
        base_scenario = scenario_name if separator else frame["scenario"]
        previous_base_scenario = self.last_scenario_by_window.get(window)
        self.last_scenario_by_window[window] = base_scenario
        if separator and iteration_text.isdigit():
            self.presented_scenario_iterations.setdefault(scenario_name, set()).add(
                int(iteration_text, 10)
            )
        if frame["monotonic_us"] >= WARMUP_US and base_scenario != "idle":
            self.measured_frame_us.append(frame["frame_us"])
            self.measured_frame_us_by_scenario.setdefault(base_scenario, []).append(
                frame["frame_us"]
            )
            if (
                previous_monotonic is not None
                and previous_monotonic >= WARMUP_US
                and previous_base_scenario != "idle"
            ):
                interval_us = frame["monotonic_us"] - previous_monotonic
                if interval_us <= 0:
                    raise G5Error("G5 inter-present interval must be positive")
                self.measured_present_interval_us.append(interval_us)
            cycle = min(
                MACRO_CYCLES - 1,
                (frame["monotonic_us"] - WARMUP_US)
                // (MACRO_CYCLE_SECONDS * 1_000_000),
            )
            frame["macro_cycle"] = cycle
            self.macro_cycles.add(cycle)
        else:
            frame["macro_cycle"] = None
        frame["evidence_schema"] = EVIDENCE_SCHEMA
        frame["commit"] = self.commit
        return frame

@dataclass
class ScenarioAccumulator:
    commit: str
    maximum_iterations: dict[str, int] = field(default_factory=dict)
    capability_status: dict[str, dict[str, str]] = field(default_factory=dict)
    completed_iterations: dict[str, set[int]] = field(default_factory=dict)
    loop_records: list[dict[str, Any]] = field(default_factory=list)
    validated_iterations: dict[str, set[int]] = field(default_factory=dict)
    macro_phases: dict[int, set[str]] = field(default_factory=dict)
    macro_phase_elapsed_us: dict[tuple[int, str], int] = field(default_factory=dict)
    macro_phase_runner_us: dict[tuple[int, str], int] = field(default_factory=dict)
    names: set[str] = field(default_factory=set)

    def observe(
        self, line: str, observed_runner_us: int | None = None
    ) -> dict[str, Any] | None:
        fields = parse_tagged_fields(line, "G5_SCENARIO")
        kind = "scenario"
        if fields is None:
            fields = parse_tagged_fields(line, "G5_CAPABILITY")
            kind = "capability"
        if fields is None:
            fields = parse_tagged_fields(line, "G5_LOOP")
            kind = "loop"
        if fields is None:
            fields = parse_tagged_fields(line, "G5_PHASE")
            kind = "phase"
        if fields is None:
            fields = parse_tagged_fields(line, "G5_RESOURCE")
            kind = "resource"
        if fields is None:
            return None
        if fields.get("schema") != str(FRAME_SCHEMA):
            raise G5Error(f"{kind} schema mismatch")
        if kind == "scenario":
            required = ("name", "category", "iteration", "page", "theme", "overlay")
            missing = [key for key in required if key not in fields]
            if missing:
                raise G5Error(f"G5_SCENARIO missing fields: {', '.join(missing)}")
            try:
                iteration = int(fields["iteration"], 10)
                page = int(fields["page"], 10)
            except ValueError as error:
                raise G5Error("G5_SCENARIO iteration/page must be integers") from error
            if iteration < 0 or page < 0:
                raise G5Error("G5_SCENARIO iteration/page must be nonnegative")
            category = fields["category"]
            self.maximum_iterations[category] = max(
                iteration, self.maximum_iterations.get(category, 0)
            )
            self.names.add(fields["name"])
            payload: dict[str, Any] = dict(fields)
            payload["iteration"] = iteration
            payload["page"] = page
        elif kind == "capability":
            required = ("category", "status", "reason")
            missing = [key for key in required if key not in fields]
            if missing:
                raise G5Error(f"G5_CAPABILITY missing fields: {', '.join(missing)}")
            if fields["category"] in self.capability_status:
                raise G5Error(
                    f"duplicate G5_CAPABILITY category {fields['category']!r}"
                )
            self.capability_status[fields["category"]] = {
                "status": fields["status"],
                "reason": fields["reason"],
            }
            payload = dict(fields)
        elif kind == "loop":
            required = (
                "category",
                "iteration",
                "status",
                "final_state",
                "baseline",
                "open_presented",
                "closed_presented",
                "final_window_count",
                "final_overlay_count",
                "final_active_work",
                "resource_delta",
                "open_window",
                "open_frame_seq",
                "closed_window",
                "closed_frame_seq",
            )
            missing = [key for key in required if key not in fields]
            if missing:
                raise G5Error(f"G5_LOOP missing fields: {', '.join(missing)}")
            try:
                iteration = int(fields["iteration"], 10)
            except ValueError as error:
                raise G5Error("G5_LOOP iteration must be an integer") from error
            if iteration <= 0:
                raise G5Error("G5_LOOP iteration must be positive")
            if fields["category"] not in {"window", "modal", "drawer", "theme", "dpi"}:
                raise G5Error("G5_LOOP category is not a frozen loop class")
            try:
                proof_values = {
                    key: int(fields[key], 10)
                    for key in (
                        "open_presented",
                        "closed_presented",
                        "final_window_count",
                        "final_overlay_count",
                        "final_active_work",
                        "resource_delta",
                    )
                }
            except ValueError as error:
                raise G5Error("G5_LOOP proof fields must be integers") from error
            try:
                frame_refs = tuple(
                    int(fields[key], 10)
                    for key in (
                        "open_window",
                        "open_frame_seq",
                        "closed_window",
                        "closed_frame_seq",
                    )
                )
            except ValueError as error:
                raise G5Error("G5_LOOP frame references must be integers") from error
            if any(value <= 0 for value in frame_refs):
                raise G5Error("G5_LOOP frame references must be positive")
            if (
                fields["status"] != "pass"
                or fields["final_state"] != "stable"
                or fields["baseline"] != "pass"
                or proof_values
                != {
                    "open_presented": 1,
                    "closed_presented": 1,
                    "final_window_count": 1,
                    "final_overlay_count": 0,
                    "final_active_work": 0,
                    "resource_delta": 0,
                }
            ):
                raise G5Error("G5_LOOP is not a successful closed-loop baseline proof")
            completed = self.completed_iterations.setdefault(fields["category"], set())
            if iteration in completed:
                raise G5Error(
                    f"duplicate G5_LOOP iteration {fields['category']}.{iteration}"
                )
            completed.add(iteration)
            payload = dict(fields)
            payload["iteration"] = iteration
            payload.update(proof_values)
            for key, value in zip(
                ("open_window", "open_frame_seq", "closed_window", "closed_frame_seq"),
                frame_refs,
            ):
                payload[key] = value
            self.loop_records.append(dict(payload))
        elif kind == "resource":
            required = (
                "phase",
                "category",
                "iteration",
                "state",
                "window",
                "frame_seq",
                "live_nodes",
                "tree_slots",
                "overlay_count",
                "active_work",
            )
            missing = [key for key in required if key not in fields]
            if missing:
                raise G5Error(f"G5_RESOURCE missing fields: {', '.join(missing)}")
            if fields["phase"] != "post_present":
                raise G5Error("G5_RESOURCE phase must be post_present")
            if fields["category"] not in {"theme", "modal", "drawer"}:
                raise G5Error("G5_RESOURCE category is not a frozen loop class")
            if fields["state"] not in {"open", "closed"}:
                raise G5Error("G5_RESOURCE state must be open or closed")
            try:
                integer_fields = {
                    key: int(fields[key], 10)
                    for key in (
                        "iteration",
                        "window",
                        "frame_seq",
                        "live_nodes",
                        "tree_slots",
                        "overlay_count",
                        "active_work",
                    )
                }
            except ValueError as error:
                raise G5Error("G5_RESOURCE numeric fields must be integers") from error
            if integer_fields["iteration"] <= 0:
                raise G5Error("G5_RESOURCE iteration must be positive")
            if integer_fields["window"] <= 0 or integer_fields["frame_seq"] <= 0:
                raise G5Error("G5_RESOURCE frame identity must be positive")
            if any(value < 0 for value in integer_fields.values()):
                raise G5Error("G5_RESOURCE counts must be nonnegative")
            payload = dict(fields)
            payload.update(integer_fields)
        else:
            required = ("macro_cycle", "phase", "elapsed_us")
            missing = [key for key in required if key not in fields]
            if missing:
                raise G5Error(f"G5_PHASE missing fields: {', '.join(missing)}")
            try:
                macro_cycle = int(fields["macro_cycle"], 10)
                elapsed_us = int(fields["elapsed_us"], 10)
            except ValueError as error:
                raise G5Error("G5_PHASE macro_cycle/elapsed_us must be integers") from error
            if macro_cycle < 0 or elapsed_us < 0:
                raise G5Error("G5_PHASE values must be nonnegative")
            phase = fields["phase"]
            if phase not in REQUIRED_MACRO_PHASES:
                raise G5Error(f"unknown G5_PHASE phase: {phase}")
            phases = self.macro_phases.setdefault(macro_cycle, set())
            if phase in phases:
                raise G5Error(f"duplicate G5_PHASE {macro_cycle}.{phase}")
            phases.add(phase)
            self.macro_phase_elapsed_us[(macro_cycle, phase)] = elapsed_us
            self.macro_phase_runner_us[(macro_cycle, phase)] = (
                elapsed_us if observed_runner_us is None else observed_runner_us
            )
            payload = dict(fields)
            payload["macro_cycle"] = macro_cycle
            payload["elapsed_us"] = elapsed_us
        payload.update(
            {
                "kind": kind,
                "evidence_schema": EVIDENCE_SCHEMA,
                "commit": self.commit,
            }
        )
        return payload

    def bind_loop_frames(self, frames: Iterable[dict[str, Any]]) -> None:
        self.validated_iterations = bind_loop_frame_proofs(self.loop_records, frames)

    def loop_matrix(
        self,
        presented_iterations: dict[str, set[int]] | None = None,
    ) -> dict[str, Any]:
        presented_iterations = presented_iterations or {}
        result: dict[str, Any] = {}
        scenario_by_category = {
            "theme": "page_general_dark",
            "modal": "modal_feedback",
            "drawer": "drawer_feedback",
        }
        for category in ("window", "modal", "drawer", "theme", "dpi"):
            scenario_name = scenario_by_category.get(category)
            iterations = self.maximum_iterations.get(category, 0)
            presented = len(
                {
                    iteration
                    for iteration in presented_iterations.get(scenario_name or "", set())
                    if iteration > 0
                }
            )
            frame_bound_completed = self.validated_iterations.get(category, set())
            evidence = {
                "iterations": iterations,
                "presented_iterations": presented,
                "completed_iterations": len(frame_bound_completed),
                "required": REQUIRED_CLASS_LOOPS,
            }
            capability = self.capability_status.get(category)
            if capability is None:
                result[category] = {
                    **evidence,
                    "status": "blocked",
                    "reason": "missing_closed_loop_capability_evidence",
                }
            elif capability["status"] == "blocked":
                result[category] = {
                    **evidence,
                    **capability,
                }
            elif capability["status"] != "ready":
                result[category] = {
                    **evidence,
                    "status": "fail",
                    "reason": f"invalid_capability_status={capability['status']}",
                }
            else:
                first_required = set(range(1, REQUIRED_CLASS_LOOPS + 1))
                scenario_proof = (
                    category not in scenario_by_category
                    or (
                        iterations >= REQUIRED_CLASS_LOOPS
                        and first_required.issubset(
                            presented_iterations.get(scenario_name or "", set())
                        )
                    )
                )
                has_proof = first_required.issubset(frame_bound_completed) and scenario_proof
                result[category] = {
                    **evidence,
                    "status": "pass" if has_proof else "fail",
                    "reason": "" if has_proof else "missing_frame_bound_closed_loop_proofs",
                }
        return result

    def validate_macro_phases(self) -> dict[str, Any]:
        missing: list[str] = []
        timing_errors: list[str] = []
        idle_offset_us = (MACRO_CYCLE_SECONDS - MACRO_IDLE_SECONDS) * 1_000_000
        runner_origin = self.macro_phase_runner_us.get((0, "interaction"))
        if runner_origin is None:
            raise G5Error("soak macro phases missing: ['0.interaction']")
        for cycle in range(MACRO_CYCLES):
            phases = self.macro_phases.get(cycle, set())
            for phase in REQUIRED_MACRO_PHASES:
                if phase not in phases:
                    missing.append(f"{cycle}.{phase}")
                    continue
                expected = cycle * MACRO_CYCLE_SECONDS * 1_000_000
                if phase == "idle":
                    expected += idle_offset_us
                actual = self.macro_phase_elapsed_us[(cycle, phase)]
                if abs(actual - expected) > MACRO_PHASE_TOLERANCE_US:
                    timing_errors.append(f"{cycle}.{phase}")
                runner_actual = self.macro_phase_runner_us[(cycle, phase)] - runner_origin
                if abs(runner_actual - expected) > MACRO_PHASE_TOLERANCE_US:
                    timing_errors.append(f"{cycle}.{phase}.runner")
        if missing:
            raise G5Error(f"soak macro phases missing: {missing}")
        if timing_errors:
            raise G5Error(f"soak macro phase timing drift: {timing_errors}")
        return {
            "macro_cycles": MACRO_CYCLES,
            "macro_cycle_seconds": MACRO_CYCLE_SECONDS,
            "interaction_seconds": MACRO_CYCLE_SECONDS - MACRO_IDLE_SECONDS,
            "idle_seconds": MACRO_IDLE_SECONDS,
            "required_phases": sorted(REQUIRED_MACRO_PHASES),
            "status": "pass",
        }


def sample_process_memory(pid: int) -> tuple[int, int, int, int]:
    if os.name != "nt":
        raise G5Error("working set/private bytes sampling requires Windows")
    from ctypes import wintypes

    class ProcessMemoryCountersEx(ctypes.Structure):
        _fields_ = [
            ("cb", wintypes.DWORD),
            ("PageFaultCount", wintypes.DWORD),
            ("PeakWorkingSetSize", ctypes.c_size_t),
            ("WorkingSetSize", ctypes.c_size_t),
            ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPagedPoolUsage", ctypes.c_size_t),
            ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
            ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
            ("PagefileUsage", ctypes.c_size_t),
            ("PeakPagefileUsage", ctypes.c_size_t),
            ("PrivateUsage", ctypes.c_size_t),
        ]

    class FileTime(ctypes.Structure):
        _fields_ = [("low", wintypes.DWORD), ("high", wintypes.DWORD)]

        def as_100ns(self) -> int:
            return (int(self.high) << 32) | int(self.low)

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    open_process = kernel32.OpenProcess
    open_process.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    open_process.restype = wintypes.HANDLE
    close_handle = kernel32.CloseHandle
    close_handle.argtypes = [wintypes.HANDLE]
    get_memory = psapi.GetProcessMemoryInfo
    get_memory.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(ProcessMemoryCountersEx),
        wintypes.DWORD,
    ]
    get_memory.restype = wintypes.BOOL
    get_process_times = kernel32.GetProcessTimes
    get_process_times.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(FileTime),
        ctypes.POINTER(FileTime),
        ctypes.POINTER(FileTime),
        ctypes.POINTER(FileTime),
    ]
    get_process_times.restype = wintypes.BOOL

    process_query_information = 0x0400
    process_vm_read = 0x0010
    handle = open_process(process_query_information | process_vm_read, False, pid)
    if not handle:
        raise OSError(ctypes.get_last_error(), f"OpenProcess({pid}) failed")
    try:
        counters = ProcessMemoryCountersEx()
        counters.cb = ctypes.sizeof(counters)
        if not get_memory(handle, ctypes.byref(counters), counters.cb):
            raise OSError(ctypes.get_last_error(), "GetProcessMemoryInfo failed")
        creation = FileTime()
        exit_time = FileTime()
        kernel = FileTime()
        user = FileTime()
        if not get_process_times(
            handle,
            ctypes.byref(creation),
            ctypes.byref(exit_time),
            ctypes.byref(kernel),
            ctypes.byref(user),
        ):
            raise OSError(ctypes.get_last_error(), "GetProcessTimes failed")
        return (
            int(counters.WorkingSetSize),
            int(counters.PrivateUsage),
            kernel.as_100ns(),
            user.as_100ns(),
        )
    finally:
        close_handle(handle)


def summarize_memory_growth(
    samples: Sequence[dict[str, Any]],
    measurement_end_us: int,
    minimum_window_samples: int = MIN_MEMORY_WINDOW_SAMPLES,
    limit_bytes: int = MEMORY_GROWTH_LIMIT_BYTES,
) -> dict[str, Any]:
    start_min = WARMUP_US
    start_max = WARMUP_US + MEMORY_MEDIAN_WINDOW_SECONDS * 1_000_000
    end_min = measurement_end_us - MEMORY_MEDIAN_WINDOW_SECONDS * 1_000_000
    first = [
        sample
        for sample in samples
        if sample.get("runtime_monotonic_us") is not None
        and start_min <= sample["runtime_monotonic_us"] < start_max
    ]
    last = [
        sample
        for sample in samples
        if sample.get("runtime_monotonic_us") is not None
        and end_min <= sample["runtime_monotonic_us"] <= measurement_end_us
    ]
    if len(first) < minimum_window_samples or len(last) < minimum_window_samples:
        raise G5Error(
            "insufficient one-second process samples for the first/last five-minute medians"
        )
    first_ws = int(statistics.median(sample["working_set_bytes"] for sample in first))
    last_ws = int(statistics.median(sample["working_set_bytes"] for sample in last))
    first_private = int(statistics.median(sample["private_bytes"] for sample in first))
    last_private = int(statistics.median(sample["private_bytes"] for sample in last))
    ws_growth = last_ws - first_ws
    private_growth = last_private - first_private
    return {
        "sampling_period_seconds": 1,
        "median_window_seconds": MEMORY_MEDIAN_WINDOW_SECONDS,
        "first_window_samples": len(first),
        "last_window_samples": len(last),
        "working_set": {
            "first_median_bytes": first_ws,
            "last_median_bytes": last_ws,
            "growth_bytes": ws_growth,
        },
        "private_bytes": {
            "first_median_bytes": first_private,
            "last_median_bytes": last_private,
            "growth_bytes": private_growth,
        },
        "growth_limit_bytes": limit_bytes,
        "status": "pass"
        if ws_growth <= limit_bytes and private_growth <= limit_bytes
        else "fail",
    }


def summarize_idle_cpu(samples: Sequence[dict[str, Any]]) -> dict[str, Any]:
    values_by_cycle: dict[int, list[float]] = {}
    idle_samples_by_cycle: dict[int, list[dict[str, Any]]] = {}
    for sample in samples:
        cycle = sample.get("scenario_macro_cycle")
        if sample.get("phase") == "idle" and isinstance(cycle, int):
            idle_samples_by_cycle.setdefault(cycle, []).append(sample)
    for cycle, cycle_samples in idle_samples_by_cycle.items():
        cycle_samples.sort(key=lambda sample: int(sample["runner_monotonic_us"]))
        phase_start_us = int(cycle_samples[0]["runner_monotonic_us"])
        for sample in cycle_samples:
            if (
                int(sample["runner_monotonic_us"]) - phase_start_us
                < IDLE_SETTLE_SECONDS * 1_000_000
            ):
                continue
            value = sample.get("cpu_core_equivalents")
            if isinstance(value, (int, float)):
                values_by_cycle.setdefault(cycle, []).append(float(value))
    missing = [
        cycle
        for cycle in range(MACRO_CYCLES)
        if len(values_by_cycle.get(cycle, [])) < MIN_IDLE_CPU_SAMPLES_PER_CYCLE
    ]
    if missing:
        raise G5Error(f"insufficient idle CPU samples for macro cycles: {missing}")
    values = [value for cycle in range(MACRO_CYCLES) for value in values_by_cycle[cycle]]
    ordered = sorted(values)
    p95 = ordered[max(0, math.ceil(0.95 * len(ordered)) - 1)]
    return {
        "sample_count": len(values),
        "minimum_samples_per_cycle": MIN_IDLE_CPU_SAMPLES_PER_CYCLE,
        "p95_core_equivalents": p95,
        "max_core_equivalents": max(values),
        "limit_core_equivalents": IDLE_CPU_CORE_LIMIT,
        "status": "pass" if p95 <= IDLE_CPU_CORE_LIMIT else "fail",
    }


def capture_environment() -> dict[str, Any]:
    environment: dict[str, Any] = {
        "captured_at": utc_now(),
        "platform": platform.platform(),
        "system": platform.system(),
        "release": platform.release(),
        "version": platform.version(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "python": sys.version,
    }
    for name, command in (
        ("rustc", ["rustc", "--version", "--verbose"]),
        ("cargo", ["cargo", "--version"]),
    ):
        try:
            environment[name] = capture(command).strip()
        except G5Error as error:
            environment[name] = {"status": "blocked", "reason": str(error)}
    powershell = shutil.which("powershell.exe") or shutil.which("powershell")
    if powershell:
        script = (
            "$ErrorActionPreference='Stop';"
            "[ordered]@{"
            "os=Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber,OSArchitecture;"
            "cpu=Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors;"
            "computer=Get-CimInstance Win32_ComputerSystem | Select-Object TotalPhysicalMemory;"
            "gpu=@(Get-CimInstance Win32_VideoController | Select-Object Name,DriverVersion,AdapterRAM)"
            "}|ConvertTo-Json -Depth 5 -Compress"
        )
        try:
            environment["windows_cim"] = json.loads(capture([powershell, "-NoProfile", "-NonInteractive", "-Command", script]))
        except (G5Error, json.JSONDecodeError) as error:
            environment["windows_cim"] = {"status": "blocked", "reason": str(error)}
    else:
        environment["windows_cim"] = {
            "status": "blocked",
            "reason": "PowerShell is unavailable",
        }
    return environment


def assess_frozen_environment(environment: dict[str, Any]) -> dict[str, Any]:
    reasons: list[str] = []
    if environment.get("system") != "Windows":
        reasons.append("system_not_windows")
    if str(environment.get("machine", "")).lower() not in {"amd64", "x86_64"}:
        reasons.append("machine_not_x64")
    for tool in ("rustc", "cargo"):
        if not isinstance(environment.get(tool), str):
            reasons.append(f"{tool}_capture_blocked")

    cim = environment.get("windows_cim")
    if not isinstance(cim, dict) or cim.get("status") == "blocked":
        reasons.append("windows_cim_capture_blocked")
        cim = {}

    def first_object(value: Any) -> dict[str, Any]:
        if isinstance(value, dict):
            return value
        if isinstance(value, list) and value and isinstance(value[0], dict):
            return value[0]
        return {}

    os_info = first_object(cim.get("os"))
    cpu_info = first_object(cim.get("cpu"))
    computer_info = first_object(cim.get("computer"))
    gpu_value = cim.get("gpu")
    gpu_infos = (
        [item for item in gpu_value if isinstance(item, dict)]
        if isinstance(gpu_value, list)
        else [gpu_value]
        if isinstance(gpu_value, dict)
        else []
    )

    if str(os_info.get("BuildNumber", "")) != EXPECTED_WINDOWS_BUILD:
        reasons.append("windows_build_mismatch")
    if EXPECTED_CPU_FRAGMENT.lower() not in str(cpu_info.get("Name", "")).lower():
        reasons.append("cpu_mismatch")
    try:
        total_ram = int(computer_info.get("TotalPhysicalMemory", 0))
    except (TypeError, ValueError):
        total_ram = 0
    if not EXPECTED_RAM_MIN_BYTES <= total_ram <= EXPECTED_RAM_MAX_BYTES:
        reasons.append("physical_memory_mismatch")
    matching_gpu = next(
        (
            gpu
            for gpu in gpu_infos
            if EXPECTED_GPU_FRAGMENT.lower() in str(gpu.get("Name", "")).lower()
        ),
        None,
    )
    if matching_gpu is None:
        reasons.append("gpu_mismatch")
    elif str(matching_gpu.get("DriverVersion", "")) != EXPECTED_GPU_DRIVER:
        reasons.append("gpu_driver_mismatch")

    return {
        "status": "pass" if not reasons else "blocked",
        "reasons": reasons,
        "expected": {
            "windows_build": EXPECTED_WINDOWS_BUILD,
            "machine": "x64",
            "cpu_contains": EXPECTED_CPU_FRAGMENT,
            "physical_memory_bytes_range": [
                EXPECTED_RAM_MIN_BYTES,
                EXPECTED_RAM_MAX_BYTES,
            ],
            "gpu_contains": EXPECTED_GPU_FRAGMENT,
            "gpu_driver": EXPECTED_GPU_DRIVER,
            "drawable_physical": [
                EXPECTED_DRAWABLE_WIDTH,
                EXPECTED_DRAWABLE_HEIGHT,
            ],
            "window_logical": [EXPECTED_DRAWABLE_WIDTH, EXPECTED_DRAWABLE_HEIGHT],
            "window_dpi": 96,
        },
        "observed": {
            "windows_build": os_info.get("BuildNumber"),
            "machine": environment.get("machine"),
            "cpu": cpu_info.get("Name"),
            "physical_memory_bytes": total_ram,
            "gpus": gpu_infos,
        },
    }


def create_evidence_dir(base: Path, commit: str, mode: str) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    candidate = base / f"{commit[:12]}-{stamp}-{mode}"
    counter = 1
    while candidate.exists():
        candidate = base / f"{commit[:12]}-{stamp}-{mode}-{counter}"
        counter += 1
    candidate.mkdir(parents=True)
    return candidate


def terminate_process(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _write_stop_request(path: Path, token: str) -> None:
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(token + "\n", encoding="utf-8")
    temporary.replace(path)


def run_demo(
    executable: Path,
    session_dir: Path,
    commit: str,
    mode: str,
) -> dict[str, Any]:
    if os.name != "nt":
        raise G5Error("release G5 execution is supported only on Windows")
    stop_file = session_dir / "control.stop"
    stop_token = secrets.token_hex(32)
    environment = os.environ.copy()
    environment.update(
        {
            "RUST_LOG": "info",
            "UIX_PERF_PROBE": "1",
            "UIX_G5_RELEASE_PROBE": "1",
            "UIX_G5_STOP_FILE": str(stop_file),
            "UIX_G5_STOP_TOKEN": stop_token,
        }
    )
    for key in (
        "UIX_GRAPHICS_BACKEND",
        "UIX_PERF_SKIP_PRESENT",
        "UIX_PERF_SKIP_PICTURE_RASTER",
    ):
        environment.pop(key, None)

    process = subprocess.Popen(
        [str(executable), "--g5-release-scenario"],
        cwd=ROOT,
        env=environment,
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=1,
    )
    assert process.stdout is not None
    output_queue: queue.Queue[str | None] = queue.Queue()

    def read_output() -> None:
        try:
            for line in process.stdout:
                output_queue.put(line.rstrip("\r\n"))
        finally:
            output_queue.put(None)

    reader = threading.Thread(target=read_output, name="uix-g5-log-reader", daemon=True)
    reader.start()

    frames = FrameAccumulator(commit)
    scenarios = ScenarioAccumulator(commit)
    backend = BackendEvidence()
    memory_samples: list[dict[str, Any]] = []
    abnormal_lines: list[str] = []
    completion_seen = False
    watchdog_reason: str | None = None
    parsing_failure: str | None = None
    reader_finished = False
    stop_requested_at: float | None = None
    started = time.monotonic()
    driver_started_at: float | None = None
    last_frame_at = started
    next_memory_sample_at = started + 1.0
    current_phase = "interaction"
    current_macro_cycle = 0
    phase_started_at = started
    previous_cpu_total_100ns: int | None = None
    previous_cpu_sample_at: float | None = None
    consecutive_idle_cpu_violations = 0

    with (
        (session_dir / "raw.log").open("w", encoding="utf-8", newline="\n") as raw_log,
        (session_dir / "frames.ndjson").open("w", encoding="utf-8", newline="\n") as frame_log,
        (session_dir / "memory.ndjson").open("w", encoding="utf-8", newline="\n") as memory_log,
        (session_dir / "scenarios.ndjson").open("w", encoding="utf-8", newline="\n") as scenario_log,
    ):
        try:
            while True:
                now = time.monotonic()
                if process.poll() is None and now >= next_memory_sample_at:
                    while next_memory_sample_at <= now:
                        next_memory_sample_at += 1.0
                    try:
                        (
                            working_set,
                            private_bytes,
                            kernel_100ns,
                            user_100ns,
                        ) = sample_process_memory(process.pid)
                        cpu_total_100ns = kernel_100ns + user_100ns
                        cpu_core_equivalents: float | None = None
                        if (
                            previous_cpu_total_100ns is not None
                            and previous_cpu_sample_at is not None
                            and now > previous_cpu_sample_at
                        ):
                            cpu_core_equivalents = max(
                                0.0,
                                (cpu_total_100ns - previous_cpu_total_100ns)
                                * 1e-7
                                / (now - previous_cpu_sample_at),
                            )
                        previous_cpu_total_100ns = cpu_total_100ns
                        previous_cpu_sample_at = now
                        runtime_us = (
                            None
                            if driver_started_at is None
                            else max(0, int((now - driver_started_at) * 1_000_000))
                        )
                        sample = {
                            "evidence_schema": EVIDENCE_SCHEMA,
                            "commit": commit,
                            "runner_monotonic_us": int((now - started) * 1_000_000),
                            "runtime_monotonic_us": runtime_us,
                            "working_set_bytes": working_set,
                            "private_bytes": private_bytes,
                            "kernel_time_100ns": kernel_100ns,
                            "user_time_100ns": user_100ns,
                            "cpu_core_equivalents": cpu_core_equivalents,
                            "phase": current_phase,
                            "scenario_macro_cycle": current_macro_cycle,
                            "macro_cycle": None
                            if runtime_us is None or runtime_us < WARMUP_US
                            else min(
                                MACRO_CYCLES - 1,
                                (runtime_us - WARMUP_US)
                                // (MACRO_CYCLE_SECONDS * 1_000_000),
                            ),
                        }
                        memory_samples.append(sample)
                        memory_log.write(json.dumps(sample, sort_keys=True) + "\n")
                        memory_log.flush()
                        if (
                            current_phase == "idle"
                            and now - phase_started_at >= IDLE_SETTLE_SECONDS
                            and cpu_core_equivalents is not None
                        ):
                            if cpu_core_equivalents > IDLE_CPU_CORE_LIMIT:
                                consecutive_idle_cpu_violations += 1
                            else:
                                consecutive_idle_cpu_violations = 0
                            if (
                                consecutive_idle_cpu_violations
                                >= IDLE_CPU_CONSECUTIVE_LIMIT
                            ):
                                parsing_failure = (
                                    "idle CPU busy-loop threshold exceeded for "
                                    f"{consecutive_idle_cpu_violations} samples"
                                )
                        elif current_phase != "idle":
                            consecutive_idle_cpu_violations = 0
                    except OSError as error:
                        if process.poll() is None:
                            parsing_failure = f"process memory sampling failed: {error}"

                try:
                    line = output_queue.get(timeout=0.2)
                except queue.Empty:
                    line = ""
                if line is None:
                    reader_finished = True
                elif line:
                    raw_log.write(
                        f"runner_monotonic_us={int((time.monotonic() - started) * 1_000_000)} {line}\n"
                    )
                    raw_log.flush()
                    backend.observe(line)
                    reason = abnormal_log_reason(line)
                    if reason:
                        abnormal_lines.append(f"{reason}: {line}")
                    if "G5_CONTROL schema=1 outcome=complete" in line:
                        completion_seen = True
                    try:
                        frame = parse_g5_frame(line)
                        if frame is not None:
                            observed_at = time.monotonic()
                            if (
                                current_phase == "idle"
                                and observed_at - phase_started_at >= IDLE_SETTLE_SECONDS
                            ):
                                raise G5Error(
                                    "successful presentation continued after idle settling"
                                )
                            if driver_started_at is None:
                                driver_started_at = observed_at - frame["monotonic_us"] / 1_000_000
                            frame = frames.observe(frame)
                            frame_log.write(json.dumps(frame, sort_keys=True) + "\n")
                            if frames.successful_total % 100 == 0:
                                frame_log.flush()
                            last_frame_at = observed_at
                        scenario_observed_us = int(
                            (time.monotonic() - started) * 1_000_000
                        )
                        scenario = scenarios.observe(line, scenario_observed_us)
                        if scenario is not None:
                            if scenario["kind"] == "phase":
                                current_phase = scenario["phase"]
                                current_macro_cycle = scenario["macro_cycle"]
                                phase_started_at = time.monotonic()
                                consecutive_idle_cpu_violations = 0
                                last_frame_at = phase_started_at
                            scenario["runner_monotonic_us"] = scenario_observed_us
                            scenario_log.write(json.dumps(scenario, sort_keys=True) + "\n")
                            scenario_log.flush()
                    except G5Error as error:
                        parsing_failure = str(error)

                if stop_requested_at is None and parsing_failure is None and not abnormal_lines:
                    performance_done = (
                        mode == "performance"
                        and len(frames.measured_frame_us) >= MIN_PRESENTED_FRAMES
                    )
                    soak_done = (
                        mode == "soak"
                        and frames.latest_monotonic_us >= WARMUP_US + SOAK_SECONDS * 1_000_000
                        and driver_started_at is not None
                        and now - driver_started_at >= WARMUP_US / 1_000_000 + SOAK_SECONDS
                    )
                    if performance_done or soak_done:
                        _write_stop_request(stop_file, stop_token)
                        stop_requested_at = time.monotonic()

                if parsing_failure is not None or abnormal_lines:
                    if stop_requested_at is None:
                        _write_stop_request(stop_file, stop_token)
                        stop_requested_at = time.monotonic()

                if frames.successful_total == 0 and now - started > INITIAL_FRAME_WATCHDOG_SECONDS:
                    watchdog_reason = "no G5_FRAME before startup deadline"
                elif (
                    frames.successful_total > 0
                    and current_phase != "idle"
                    and now - last_frame_at > FRAME_WATCHDOG_SECONDS
                ):
                    watchdog_reason = "no successful presented frame before frame deadline"
                elif (
                    current_phase == "idle"
                    and now - phase_started_at > IDLE_PHASE_WATCHDOG_SECONDS
                ):
                    watchdog_reason = "idle phase exceeded its fixed deadline"
                if stop_requested_at is not None and now - stop_requested_at > STOP_WATCHDOG_SECONDS:
                    watchdog_reason = "demo did not stop after authenticated request"
                if watchdog_reason is not None:
                    terminate_process(process)

                if process.poll() is not None and reader_finished and output_queue.empty():
                    break
        finally:
            if process.poll() is None:
                terminate_process(process)
            reader.join(timeout=2)

    if parsing_failure:
        raise G5Error(parsing_failure)
    validate_process_outcome(
        process.returncode if process.returncode is not None else -1,
        completion_seen,
        abnormal_lines,
        watchdog_reason,
    )
    backend_summary = backend.validate()
    performance = summarize_performance(frames.measured_frame_us)
    performance["inter_present"] = summarize_performance(
        frames.measured_present_interval_us,
        minimum_samples=MIN_PRESENTED_FRAMES - 1,
    )
    required_pages = {
        "page_home",
        "page_general_dark",
        "modal_feedback",
        "drawer_feedback",
        "page_charts",
    }
    missing_pages = sorted(required_pages - scenarios.names)
    if missing_pages:
        raise G5Error(f"G5 fixed scenario coverage missing: {', '.join(missing_pages)}")
    missing_frame_scenarios = sorted(
        required_pages - frames.measured_frame_us_by_scenario.keys()
    )
    if missing_frame_scenarios:
        raise G5Error(
            "G5 fixed scenario presented-frame coverage missing: "
            + ", ".join(missing_frame_scenarios)
        )
    performance["scenarios"] = {
        name: summarize_distribution(frames.measured_frame_us_by_scenario[name])
        for name in sorted(required_pages)
    }
    scenarios.bind_loop_frames(iter_ndjson_objects(session_dir / "frames.ndjson"))

    finished = time.monotonic()
    if driver_started_at is None:
        raise G5Error("G5 driver monotonic origin was not observed")
    runner_runtime_us = int((finished - driver_started_at) * 1_000_000)
    monotonic_drift_us = abs(runner_runtime_us - frames.latest_monotonic_us)
    if mode == "soak" and monotonic_drift_us > MONOTONIC_DRIFT_LIMIT_US:
        raise G5Error(
            f"runner/runtime monotonic drift exceeds limit: {monotonic_drift_us} us"
        )

    result: dict[str, Any] = {
        "backend": backend_summary,
        "performance": performance,
        "successful_presented_frames_total": frames.successful_total,
        "warmup_us": WARMUP_US,
        "latest_runtime_monotonic_us": frames.latest_monotonic_us,
        "runner_runtime_monotonic_us": runner_runtime_us,
        "runner_runtime_drift_us": monotonic_drift_us,
        "scenario_coverage": sorted(scenarios.names),
        "loop_matrix": scenarios.loop_matrix(frames.presented_scenario_iterations),
        "abnormal_log_count": len(abnormal_lines),
        "completion_seen": completion_seen,
    }
    if mode == "soak":
        missing_cycles = sorted(set(range(MACRO_CYCLES)) - frames.macro_cycles)
        if missing_cycles:
            raise G5Error(f"soak macro cycles missing: {missing_cycles}")
        memory_summary = summarize_memory_growth(
            memory_samples, WARMUP_US + SOAK_SECONDS * 1_000_000
        )
        idle_cpu_summary = summarize_idle_cpu(memory_samples)
        result["soak"] = {
            "status": "pass"
            if memory_summary["status"] == "pass"
            and idle_cpu_summary["status"] == "pass"
            else "fail",
            "duration_seconds": SOAK_SECONDS,
            "actual_runner_duration_seconds": runner_runtime_us / 1_000_000,
            "macro_cycle_seconds": MACRO_CYCLE_SECONDS,
            "macro_cycles": MACRO_CYCLES,
            "covered_macro_cycles": sorted(frames.macro_cycles),
            "phase_protocol": scenarios.validate_macro_phases(),
            "memory": memory_summary,
            "idle_cpu": idle_cpu_summary,
        }
    else:
        result["soak"] = {
            "status": "not_run",
            "required_duration_seconds": SOAK_SECONDS,
            "required_macro_cycles": MACRO_CYCLES,
        }
    return result


def determine_status(mode: str, result: dict[str, Any]) -> tuple[str, str]:
    failures: list[str] = []
    if result["performance"]["status"] != "pass":
        failures.append("p95_threshold")
    if result["performance"]["inter_present"]["status"] != "pass":
        failures.append("inter_present_p95_threshold")
    if mode == "soak" and result["soak"]["memory"]["status"] != "pass":
        failures.append("memory_growth")
    if mode == "soak" and result["soak"]["idle_cpu"]["status"] != "pass":
        failures.append("idle_cpu_busy_loop")
    for category in ("window", "modal", "drawer", "theme", "dpi"):
        if mode == "soak" and result["loop_matrix"][category]["status"] != "pass":
            if result["loop_matrix"][category]["status"] == "fail":
                failures.append(f"{category}_loops")
    if failures:
        return "fail", ",".join(failures)
    environment = result.get("environment", {})
    environment_blocked = environment.get("status") != "pass"
    blocked = [
        category
        for category in ("window", "modal", "drawer", "theme", "dpi")
        if result["loop_matrix"][category]["status"] == "blocked"
    ]
    if environment_blocked:
        blocked.append("frozen_environment")
    if (mode == "soak" and blocked) or environment_blocked:
        return "blocked", ",".join(blocked)
    return "pass", ""


def determine_g5_gate_status(
    mode: str, measurement_status: str, result: dict[str, Any]
) -> tuple[str, str]:
    if measurement_status == "fail":
        return "fail", "measurement_failed"
    loop_failures = sorted(
        category
        for category, item in result.get("loop_matrix", {}).items()
        if item.get("status") == "fail"
    )
    if loop_failures:
        return "fail", "failed_classes=" + ",".join(loop_failures)
    environment = result.get("environment", {})
    if environment.get("status") != "pass":
        return "blocked", "frozen_environment_not_verified"
    if mode != "soak":
        return "blocked", "eight_hour_soak_not_run"
    blocked = [
        category
        for category, item in result.get("loop_matrix", {}).items()
        if item.get("status") == "blocked"
    ]
    if blocked:
        return "blocked", "blocked_classes=" + ",".join(sorted(blocked))
    if measurement_status == "blocked":
        return "blocked", "measurement_blocked"
    return "pass", ""


def render_report(summary: dict[str, Any]) -> str:
    performance = summary.get("result", {}).get("performance", {})
    lines = [
        "# UIX 0.0.1 G5 release evidence",
        "",
        f"- Evidence schema: `{summary['evidence_schema']}`",
        f"- Runtime frame schema: `{summary['runtime_frame_schema']}`",
        f"- Commit: `{summary['commit']}`",
        f"- Mode: `{summary['mode']}`",
        f"- Measurement status: **{summary['status']}**",
        f"- Complete G5 gate status: **{summary['g5_gate_status']}**",
    ]
    if summary.get("reason"):
        lines.append(f"- Reason: `{summary['reason']}`")
    if summary.get("g5_gate_reason"):
        lines.append(f"- G5 gate reason: `{summary['g5_gate_reason']}`")
    if performance:
        lines.extend(
            [
                f"- Successful measured frames: `{performance.get('sample_count')}`",
                f"- P95 (nearest-rank): `{performance.get('p95_us')} us` / `{performance.get('p95_limit_us')} us`",
                f"- Inter-present P95: `{performance.get('inter_present', {}).get('p95_us')} us` / `{performance.get('inter_present', {}).get('p95_limit_us')} us`",
            ]
        )
        for name, item in sorted(performance.get("scenarios", {}).items()):
            lines.append(
                f"- {name}: n={item.get('sample_count')}, "
                f"P50/P95/P99={item.get('p50_us')}/{item.get('p95_us')}/{item.get('p99_us')} us"
            )
    environment = summary.get("result", {}).get("environment", {})
    if environment:
        lines.append(f"- Frozen environment: **{environment.get('status', 'missing')}**")
    soak = summary.get("result", {}).get("soak", {})
    if soak.get("actual_runner_duration_seconds") is not None:
        lines.append(
            f"- Actual runner duration: `{soak['actual_runner_duration_seconds']:.3f} s`"
        )
    loop_matrix = summary.get("result", {}).get("loop_matrix", {})
    if loop_matrix:
        lines.extend(["", "## 1,000-loop class matrix", ""])
        for category in ("window", "modal", "drawer", "theme", "dpi"):
            item = loop_matrix.get(category, {})
            reason = f" ({item['reason']})" if item.get("reason") else ""
            presented = (
                f", presented={item['presented_iterations']}"
                if "presented_iterations" in item
                else ""
            )
            completed = (
                f", closed={item['completed_iterations']}"
                if "completed_iterations" in item
                else ""
            )
            lines.append(
                f"- {category}: {item.get('status', 'missing')}, "
                f"{item.get('iterations', 0)}/{item.get('required', REQUIRED_CLASS_LOOPS)}"
                f"{presented}{completed}{reason}"
            )
    lines.extend(
        [
            "",
            "Raw logs, frame NDJSON, one-second process memory samples, environment,",
            "summary and hashes are bound by `manifest.json`. A blocked class is not a pass.",
            "",
        ]
    )
    return "\n".join(lines)


def write_evidence_manifest(
    session_dir: Path,
    commit: str,
    executable_sha256: str,
    additional: dict[str, Any] | None = None,
) -> dict[str, Any]:
    files = {}
    for path in sorted(session_dir.iterdir()):
        if path.is_file() and path.name not in {"manifest.json", "control.stop"}:
            files[path.name] = file_sha256(path)
    manifest: dict[str, Any] = {
        "evidence_schema": EVIDENCE_SCHEMA,
        "runtime_frame_schema": FRAME_SCHEMA,
        "commit": commit,
        "executable_sha256": executable_sha256,
        "generated_at": utc_now(),
        "files": files,
    }
    if additional:
        manifest.update(additional)
    write_json_atomic(session_dir / "manifest.json", manifest)
    return manifest


def verify_evidence_manifest(session_dir: Path) -> dict[str, Any]:
    manifest_path = session_dir / "manifest.json"
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise G5Error(f"cannot read evidence manifest: {error}") from error
    if manifest.get("evidence_schema") != EVIDENCE_SCHEMA:
        raise G5Error("evidence manifest schema mismatch")
    if manifest.get("runtime_frame_schema") != FRAME_SCHEMA:
        raise G5Error("runtime frame schema mismatch")
    commit = manifest.get("commit", "")
    if not COMMIT_PATTERN.fullmatch(commit):
        raise G5Error("evidence manifest commit is invalid")
    files = manifest.get("files")
    if not isinstance(files, dict) or not files:
        raise G5Error("evidence manifest has no file hashes")
    missing_required = sorted(REQUIRED_EVIDENCE_FILES - files.keys())
    if missing_required:
        raise G5Error(f"evidence manifest missing required files: {missing_required}")
    actual_files = {
        path.name
        for path in session_dir.iterdir()
        if path.is_file() and path.name not in {"manifest.json", "control.stop"}
    }
    if actual_files != set(files):
        raise G5Error("evidence manifest file set does not match the evidence directory")
    for relative, expected in files.items():
        if (
            not isinstance(relative, str)
            or not isinstance(expected, str)
            or Path(relative).name != relative
            or not re.fullmatch(r"[0-9a-f]{64}", expected)
        ):
            raise G5Error(f"invalid evidence manifest file entry: {relative!r}")
        path = session_dir / relative
        if not path.is_file():
            raise G5Error(f"evidence file is missing: {relative}")
        actual = file_sha256(path)
        if actual != expected:
            raise G5Error(f"evidence file hash mismatch: {relative}")

    if manifest.get("executable_sha256") != files["candidate-uix-demo.exe"]:
        raise G5Error("manifest executable hash is not bound to the saved candidate")
    if manifest.get("cargo_lock_sha256") != files["candidate-Cargo.lock"]:
        raise G5Error("manifest Cargo.lock hash is not bound to the saved lockfile")
    if manifest.get("runner_sha256") != files["runner.py"]:
        raise G5Error("manifest runner hash is not bound to the saved runner")

    def read_json(name: str) -> dict[str, Any]:
        try:
            payload = json.loads((session_dir / name).read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise G5Error(f"cannot parse evidence file {name}: {error}") from error
        if not isinstance(payload, dict):
            raise G5Error(f"evidence file {name} must contain a JSON object")
        return payload

    summary = read_json("summary.json")
    environment = read_json("environment.json")
    for name, payload in (("summary.json", summary), ("environment.json", environment)):
        if payload.get("evidence_schema") != EVIDENCE_SCHEMA:
            raise G5Error(f"{name} evidence schema mismatch")
        if payload.get("runtime_frame_schema") != FRAME_SCHEMA:
            raise G5Error(f"{name} runtime frame schema mismatch")
        if payload.get("commit") != commit:
            raise G5Error(f"{name} commit mismatch")
    if summary.get("mode") != manifest.get("mode"):
        raise G5Error("summary/manifest mode mismatch")
    if summary.get("executable_sha256") != manifest.get("executable_sha256"):
        raise G5Error("summary/manifest executable hash mismatch")
    if summary.get("cargo_lock_sha256") != manifest.get("cargo_lock_sha256"):
        raise G5Error("summary/manifest Cargo.lock hash mismatch")
    if summary.get("status") not in {"pass", "blocked", "fail"}:
        raise G5Error("summary measurement status is invalid")
    if summary.get("g5_gate_status") not in {"pass", "blocked", "fail"}:
        raise G5Error("summary G5 gate status is invalid")

    ndjson_records: dict[str, list[dict[str, Any]]] = {
        "frames.ndjson": [],
        "memory.ndjson": [],
        "scenarios.ndjson": [],
    }
    for name in ndjson_records:
        for line_number, line in enumerate(
            (session_dir / name).read_text(encoding="utf-8").splitlines(), start=1
        ):
            if not line.strip():
                continue
            try:
                payload = json.loads(line)
            except json.JSONDecodeError as error:
                raise G5Error(f"invalid {name}:{line_number}: {error}") from error
            if not isinstance(payload, dict):
                raise G5Error(f"{name}:{line_number} must be a JSON object")
            if payload.get("evidence_schema") != EVIDENCE_SCHEMA:
                raise G5Error(f"{name}:{line_number} evidence schema mismatch")
            if payload.get("commit") != commit:
                raise G5Error(f"{name}:{line_number} commit mismatch")
            if name == "frames.ndjson" and payload.get("schema") != FRAME_SCHEMA:
                raise G5Error(f"{name}:{line_number} runtime frame schema mismatch")
            if name == "scenarios.ndjson" and str(payload.get("schema")) != str(
                FRAME_SCHEMA
            ):
                raise G5Error(f"{name}:{line_number} runtime event schema mismatch")
            ndjson_records[name].append(payload)

    if summary["g5_gate_status"] == "pass":
        result = summary.get("result")
        if summary.get("status") != "pass" or summary.get("mode") != "soak":
            raise G5Error("a passing G5 gate must be a passing soak measurement")
        if not isinstance(result, dict):
            raise G5Error("passing G5 evidence has no result object")
        capture_payload = environment.get("capture")
        if not isinstance(capture_payload, dict):
            raise G5Error("passing G5 evidence lacks the captured environment")
        reassessed_environment = assess_frozen_environment(capture_payload)
        if (
            reassessed_environment.get("status") != "pass"
            or reassessed_environment != environment.get("frozen_environment")
        ):
            raise G5Error("passing G5 frozen environment cannot be reproduced")
        if result.get("environment", {}).get("status") != "pass":
            raise G5Error("passing G5 evidence lacks the frozen environment proof")
        if environment.get("frozen_environment") != result.get("environment"):
            raise G5Error("passing G5 summary/environment evidence mismatch")
        if result.get("soak", {}).get("status") != "pass":
            raise G5Error("passing G5 evidence lacks a passing soak result")
        frames = ndjson_records["frames.ndjson"]
        measured_frames = [
            frame
            for frame in frames
            if frame.get("monotonic_us", -1) >= WARMUP_US
            and str(frame.get("scenario", "")).split(".", 1)[0] != "idle"
        ]
        if len(measured_frames) < MIN_PRESENTED_FRAMES:
            raise G5Error("passing G5 evidence has insufficient bound frame records")
        for frame in measured_frames:
            if (
                frame.get("presented") != 1
                or frame.get("present_skipped") != 0
                or frame.get("drawable_width") != EXPECTED_DRAWABLE_WIDTH
                or frame.get("drawable_height") != EXPECTED_DRAWABLE_HEIGHT
                or frame.get("logical_width") != EXPECTED_DRAWABLE_WIDTH
                or frame.get("logical_height") != EXPECTED_DRAWABLE_HEIGHT
                or frame.get("dpi") != 96
            ):
                raise G5Error("passing G5 evidence contains a non-production frame")
        recomputed_performance = summarize_performance(
            [int(frame["frame_us"]) for frame in measured_frames]
        )
        reported_performance = result.get("performance", {})
        for key in ("sample_count", "p50_us", "p95_us", "p99_us", "max_us", "status"):
            if reported_performance.get(key) != recomputed_performance.get(key):
                raise G5Error(f"passing G5 performance summary mismatch: {key}")
        intervals: list[int] = []
        previous_by_window: dict[int, tuple[int, str]] = {}
        previous_sequence_by_window: dict[int, int] = {}
        presented_iterations: dict[str, set[int]] = {}
        for frame in frames:
            window = int(frame["window"])
            sequence = int(frame["frame_seq"])
            previous_sequence = previous_sequence_by_window.get(window)
            if previous_sequence is not None and sequence != previous_sequence + 1:
                raise G5Error("passing G5 evidence has a non-contiguous frame sequence")
            previous_sequence_by_window[window] = sequence
            monotonic_us = int(frame["monotonic_us"])
            scenario = str(frame.get("scenario", ""))
            name, separator, suffix = scenario.rpartition(".")
            base = name if separator and suffix.isdigit() else scenario
            if separator and suffix.isdigit():
                presented_iterations.setdefault(name, set()).add(int(suffix, 10))
            previous = previous_by_window.get(window)
            if (
                previous is not None
                and monotonic_us >= WARMUP_US
                and previous[0] >= WARMUP_US
                and base != "idle"
                and previous[1] != "idle"
            ):
                intervals.append(monotonic_us - previous[0])
            previous_by_window[window] = (monotonic_us, base)
        recomputed_inter_present = summarize_performance(
            intervals, minimum_samples=MIN_PRESENTED_FRAMES - 1
        )
        if reported_performance.get("inter_present") != recomputed_inter_present:
            raise G5Error("passing G5 inter-present summary mismatch")
        required_scenarios = {
            "page_home",
            "page_general_dark",
            "modal_feedback",
            "drawer_feedback",
            "page_charts",
        }
        frames_by_scenario: dict[str, list[int]] = {}
        for frame in measured_frames:
            scenario = str(frame.get("scenario", ""))
            name, separator, suffix = scenario.rpartition(".")
            base = name if separator and suffix.isdigit() else scenario
            frames_by_scenario.setdefault(base, []).append(int(frame["frame_us"]))
        if not required_scenarios.issubset(frames_by_scenario):
            raise G5Error("passing G5 evidence lacks fixed scenario frame coverage")
        for scenario in required_scenarios:
            if reported_performance.get("scenarios", {}).get(
                scenario
            ) != summarize_distribution(frames_by_scenario[scenario]):
                raise G5Error(
                    f"passing G5 scenario performance summary mismatch: {scenario}"
                )

        scenario_records = ndjson_records["scenarios.ndjson"]
        maximum_scenario_iterations: dict[str, int] = {}
        capability_records: dict[str, str] = {}
        for record in scenario_records:
            if record.get("kind") == "scenario":
                category = str(record.get("category", ""))
                iteration = int(record.get("iteration", 0))
                maximum_scenario_iterations[category] = max(
                    iteration, maximum_scenario_iterations.get(category, 0)
                )
            elif record.get("kind") == "capability":
                category = str(record.get("category", ""))
                if category in capability_records:
                    raise G5Error("passing G5 evidence has duplicate capability records")
                capability_records[category] = str(record.get("status", ""))
        loop_records: list[dict[str, Any]] = []
        for record in scenario_records:
            if record.get("kind") != "loop":
                continue
            category = record.get("category")
            if category not in LOOP_FRAME_SCENARIOS:
                raise G5Error("passing G5 evidence contains an unknown loop category")
            iteration = record.get("iteration")
            if type(iteration) is not int or iteration <= 0:
                raise G5Error("passing G5 evidence contains an invalid loop iteration")
            expected_proof = {
                "status": "pass",
                "final_state": "stable",
                "baseline": "pass",
                "open_presented": 1,
                "closed_presented": 1,
                "final_window_count": 1,
                "final_overlay_count": 0,
                "final_active_work": 0,
                "resource_delta": 0,
            }
            if any(record.get(key) != value for key, value in expected_proof.items()):
                raise G5Error("passing G5 evidence contains an incomplete loop proof")
            loop_records.append(record)
        closed_by_category = bind_loop_frame_proofs(loop_records, frames)
        phase_record_list = [
            record for record in scenario_records if record.get("kind") == "phase"
        ]
        phase_records = {
            (record.get("macro_cycle"), record.get("phase")): record
            for record in phase_record_list
        }
        if len(phase_records) != len(phase_record_list):
            raise G5Error("passing G5 evidence contains duplicate macro phases")
        phase_origin = phase_records.get((0, "interaction"), {}).get(
            "runner_monotonic_us"
        )
        if not isinstance(phase_origin, int):
            raise G5Error("passing G5 evidence lacks the first interaction phase")
        for cycle in range(MACRO_CYCLES):
            for phase in REQUIRED_MACRO_PHASES:
                record = phase_records.get((cycle, phase))
                if record is None:
                    raise G5Error("passing G5 evidence lacks a macro phase")
                expected = cycle * MACRO_CYCLE_SECONDS * 1_000_000
                if phase == "idle":
                    expected += (MACRO_CYCLE_SECONDS - MACRO_IDLE_SECONDS) * 1_000_000
                elapsed_us = record.get("elapsed_us")
                runner_us = record.get("runner_monotonic_us")
                if (
                    not isinstance(elapsed_us, int)
                    or not isinstance(runner_us, int)
                    or abs(elapsed_us - expected) > MACRO_PHASE_TOLERANCE_US
                    or abs((runner_us - phase_origin) - expected)
                    > MACRO_PHASE_TOLERANCE_US
                ):
                    raise G5Error("passing G5 evidence has invalid macro phase timing")
        for frame in frames:
            scenario = str(frame.get("scenario", ""))
            name, separator, suffix = scenario.rpartition(".")
            if name == "idle" and separator and suffix.isdigit():
                cycle = int(suffix, 10)
                settled_at = (
                    WARMUP_US
                    + cycle * MACRO_CYCLE_SECONDS * 1_000_000
                    + (MACRO_CYCLE_SECONDS - MACRO_IDLE_SECONDS) * 1_000_000
                    + IDLE_SETTLE_SECONDS * 1_000_000
                )
                if int(frame["monotonic_us"]) >= settled_at:
                    raise G5Error("passing G5 evidence presents during fixed idle")
        minimum_runtime_us = WARMUP_US + SOAK_SECONDS * 1_000_000
        latest_bound_frame_us = max(int(frame["monotonic_us"]) for frame in frames)
        bound_macro_cycles = {
            min(
                MACRO_CYCLES - 1,
                (int(frame["monotonic_us"]) - WARMUP_US)
                // (MACRO_CYCLE_SECONDS * 1_000_000),
            )
            for frame in frames
            if int(frame.get("monotonic_us", -1)) >= WARMUP_US
            and not str(frame.get("scenario", "")).startswith("idle.")
        }
        if (
            latest_bound_frame_us < minimum_runtime_us
            or result.get("latest_runtime_monotonic_us") != latest_bound_frame_us
            or result.get("runner_runtime_monotonic_us", 0) < minimum_runtime_us
            or bound_macro_cycles != set(range(MACRO_CYCLES))
        ):
            raise G5Error("passing G5 evidence did not run for the frozen duration")

        recomputed_memory = summarize_memory_growth(
            ndjson_records["memory.ndjson"], minimum_runtime_us
        )
        recomputed_idle_cpu = summarize_idle_cpu(ndjson_records["memory.ndjson"])
        reported_memory = result.get("soak", {}).get("memory", {})
        if recomputed_memory != reported_memory or recomputed_memory["status"] != "pass":
            raise G5Error("passing G5 memory summary does not match bound samples")
        if (
            recomputed_idle_cpu != result.get("soak", {}).get("idle_cpu", {})
            or recomputed_idle_cpu["status"] != "pass"
        ):
            raise G5Error("passing G5 idle CPU summary does not match bound samples")
        loop_matrix = result.get("loop_matrix", {})
        for category in ("window", "modal", "drawer", "theme", "dpi"):
            item = loop_matrix.get(category, {})
            first_required = set(range(1, REQUIRED_CLASS_LOOPS + 1))
            scenario_name = {
                "modal": "modal_feedback",
                "drawer": "drawer_feedback",
                "theme": "page_general_dark",
            }.get(category)
            scenario_bound = (
                scenario_name is None
                or (
                    maximum_scenario_iterations.get(category, 0)
                    >= REQUIRED_CLASS_LOOPS
                    and first_required.issubset(
                        presented_iterations.get(scenario_name, set())
                    )
                )
            )
            if (
                item.get("status") != "pass"
                or item.get("completed_iterations", 0) < REQUIRED_CLASS_LOOPS
                or not first_required.issubset(closed_by_category[category])
                or item.get("completed_iterations") != len(closed_by_category[category])
                or not scenario_bound
                or capability_records.get(category) != "ready"
            ):
                raise G5Error(
                    f"passing G5 evidence lacks {REQUIRED_CLASS_LOOPS} closed {category} loops"
                )
        raw_log = (session_dir / "raw.log").read_text(encoding="utf-8")
        if (
            EXPECTED_ENGINE_MARKER not in raw_log
            or EXPECTED_WGPU_MARKER not in raw_log
            or EXPECTED_GPU_FRAGMENT not in raw_log
            or "type=DiscreteGpu" not in raw_log
        ):
            raise G5Error("passing G5 evidence lacks the bound production backend log")
        if "G5_CONTROL schema=1 outcome=complete" not in raw_log:
            raise G5Error("passing G5 evidence lacks the bound completion log")
        if any(pattern.search(raw_log) for pattern in ABNORMAL_PATTERNS):
            raise G5Error("passing G5 evidence contains abnormal bound logs")
    manifest["verified_summary"] = {
        "status": summary["status"],
        "g5_gate_status": summary["g5_gate_status"],
    }
    return manifest


def build_release(root: Path, session_dir: Path) -> Path:
    evidence_executable = session_dir / "candidate-uix-demo.exe"
    with tempfile.TemporaryDirectory(
        prefix="uix-g5-isolated-target-", dir=session_dir.parent
    ) as temporary_target:
        target_dir = Path(temporary_target)
        command = [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--bin",
            "uix-demo",
            "--target-dir",
            str(target_dir),
        ]
        output = capture(command, root)
        (session_dir / "build.log").write_text(output, encoding="utf-8", newline="\n")
        executable = target_dir / "release" / "uix-demo.exe"
        if not executable.is_file():
            raise G5Error(f"release executable was not produced: {executable}")
        shutil.copy2(executable, evidence_executable)
    return evidence_executable


def run_candidate(root: Path, output_root: Path, mode: str) -> tuple[Path, dict[str, Any]]:
    if os.name != "nt":
        raise G5Error("the frozen 0.0.1 G5 target is Windows x64")
    commit = require_clean_commit(root)
    session_dir = create_evidence_dir(output_root, commit, mode)
    environment = capture_environment()
    environment_assessment = assess_frozen_environment(environment)
    environment_record = {
        "evidence_schema": EVIDENCE_SCHEMA,
        "runtime_frame_schema": FRAME_SCHEMA,
        "commit": commit,
        "capture": environment,
        "frozen_environment": environment_assessment,
    }
    write_json_atomic(session_dir / "environment.json", environment_record)
    if environment_assessment["status"] != "pass":
        raise G5Error(
            "frozen release environment was not verified: "
            + ",".join(environment_assessment["reasons"])
        )
    executable = build_release(root, session_dir)
    if require_clean_commit(root) != commit:
        raise G5Error("candidate commit changed while building the release executable")
    executable_hash = file_sha256(executable)
    lock_hash = file_sha256(root / "Cargo.lock")
    shutil.copy2(root / "Cargo.lock", session_dir / "candidate-Cargo.lock")
    shutil.copy2(Path(__file__).resolve(), session_dir / "runner.py")
    started_at = utc_now()
    try:
        try:
            result = run_demo(executable, session_dir, commit, mode)
            if require_clean_commit(root) != commit:
                raise G5Error("candidate worktree changed during G5 execution")
            result["environment"] = environment_assessment
            status, reason = determine_status(mode, result)
        except G5Error as error:
            result = {}
            status, reason = "fail", str(error)
    finally:
        (session_dir / "control.stop").unlink(missing_ok=True)
    g5_gate_status, g5_gate_reason = determine_g5_gate_status(mode, status, result)
    summary = {
        "evidence_schema": EVIDENCE_SCHEMA,
        "runtime_frame_schema": FRAME_SCHEMA,
        "commit": commit,
        "mode": mode,
        "started_at": started_at,
        "finished_at": utc_now(),
        "status": status,
        "reason": reason,
        "g5_gate_status": g5_gate_status,
        "g5_gate_reason": g5_gate_reason,
        "build_command": (
            "cargo build --release --locked --bin uix-demo "
            "--target-dir <isolated-empty-dir>"
        ),
        "launch_arguments": ["--g5-release-scenario"],
        "default_backend_environment_override": None,
        "executable_sha256": executable_hash,
        "cargo_lock_sha256": lock_hash,
        "result": result,
    }
    write_json_atomic(session_dir / "summary.json", summary)
    (session_dir / "report.md").write_text(
        render_report(summary), encoding="utf-8", newline="\n"
    )
    write_evidence_manifest(
        session_dir,
        commit,
        executable_hash,
        {
            "mode": mode,
            "cargo_lock_sha256": lock_hash,
            "runner_sha256": file_sha256(Path(__file__).resolve()),
        },
    )
    verify_evidence_manifest(session_dir)
    return session_dir, summary


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("performance", "soak"), default="performance")
    parser.add_argument(
        "--output-root",
        type=Path,
        default=ROOT / "test-reports" / "g5-release",
    )
    parser.add_argument(
        "--verify",
        type=Path,
        help="verify an existing evidence directory without building or running",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.verify is not None:
        try:
            manifest = verify_evidence_manifest(args.verify.resolve())
        except G5Error as error:
            print(f"G5 evidence invalid: {error}", file=sys.stderr)
            return INVALID_EVIDENCE_EXIT
        print(
            f"G5 evidence verified: commit={manifest['commit']} schema={manifest['evidence_schema']}"
        )
        gate_status = manifest["verified_summary"]["g5_gate_status"]
        if gate_status == "pass":
            return PASS_EXIT
        if gate_status == "blocked":
            return BLOCKED_EXIT
        return FAIL_EXIT
    try:
        session_dir, summary = run_candidate(ROOT, args.output_root.resolve(), args.mode)
    except G5Error as error:
        print(f"G5 runner refused: {error}", file=sys.stderr)
        return FAIL_EXIT
    print(f"G5 evidence: {session_dir}")
    print(f"G5 status: {summary['status']}")
    print(f"Complete G5 gate: {summary['g5_gate_status']}")
    if summary["reason"]:
        print(f"G5 reason: {summary['reason']}")
    if summary["g5_gate_status"] == "pass":
        return PASS_EXIT
    if summary["g5_gate_status"] == "blocked":
        return BLOCKED_EXIT
    return FAIL_EXIT


if __name__ == "__main__":
    raise SystemExit(main())
