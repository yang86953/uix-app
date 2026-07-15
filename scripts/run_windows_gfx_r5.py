# -*- coding: utf-8 -*-
"""Run the Windows Vulkan GFX-R5 target-machine acceptance matrix."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Sequence

ROOT = Path(__file__).resolve().parent.parent

VENDOR_ENV = "UIX_GFX_R5_EXPECT_VENDOR"
DEVICE_FAULT_ENV = "UIX_VULKAN_EXPECT_DEVICE_FAULT"
DEVICE_LOST_TIMEOUT_ENV = "UIX_VULKAN_DEVICE_LOST_TIMEOUT_SECONDS"
SOAK_SECONDS_ENV = "UIX_VULKAN_SOAK_SECONDS"

VENDORS = ("nvidia", "amd", "intel")
PROFILES = ("vendor", "mixed-dpi", "soak", "device-lost", "all")
MIN_SOAK_SECONDS = 900
MAX_SOAK_SECONDS = 3_600
MIN_DEVICE_LOST_TIMEOUT = 5
MAX_DEVICE_LOST_TIMEOUT = 600
EVIDENCE_VALIDATION_EXIT_CODE = 3
EVIDENCE_SCHEMA_VERSION = 4

VENDOR_CASES = (
    (
        "vendor-resize-present-readback",
        "tests::native::graphics::vulkan::platform::context::"
        "windows_vulkan_gfx_r5_expected_vendor_resize_present_readback",
    ),
    (
        "native-out-of-date-recovery",
        "tests::native::graphics::vulkan::platform::context::"
        "windows_vulkan_gfx_r5_native_out_of_date_is_typed_and_recovers",
    ),
    (
        "fatal-native-surface",
        "tests::native::graphics::vulkan::platform::context::"
        "windows_vulkan_gfx_r5_destroyed_hwnd_returns_native_surface_lost",
    ),
    (
        "engine-recovery-boundary",
        "tests::native::backends::windows::vulkan_fault_recovery::"
        "native_vulkan_surface_fault_reaches_engine_recovery_boundary",
    ),
)

MIXED_DPI_CASES = (
    (
        "vulkan-mixed-dpi",
        "tests::native::backends::windows::hardware_matrix::"
        "windows_vulkan_gfx_r5_crosses_real_mixed_dpi_monitors",
    ),
)

SOAK_CASES = (
    (
        "single-window-soak",
        "tests::native::graphics::vulkan::platform::context::"
        "windows_vulkan_hardware_resize_present_soak_is_bounded",
    ),
    (
        "shared-device-soak",
        "tests::native::graphics::vulkan::platform::context::"
        "windows_vulkan_shared_device_multiwindow_soak_is_bounded",
    ),
)

DEVICE_LOST_CASES = (
    (
        "external-device-reset",
        "tests::native::graphics::vulkan::platform::fault::"
        "windows_vulkan_gfx_r5_external_reset_returns_device_lost_with_diagnostics",
    ),
)


@dataclass(frozen=True)
class GfxR5Case:
    name: str
    test_name: str
    environment: tuple[tuple[str, str], ...]

    def command(self) -> tuple[str, ...]:
        return (
            "cargo",
            "test",
            "--features",
            "vulkan",
            "--lib",
            self.test_name,
            "--",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        )


def test_inventory_command() -> tuple[str, ...]:
    return ("cargo", "test", "--features", "vulkan", "--lib", "--", "--list")


def parse_test_inventory(output: str) -> set[str]:
    suffix = ": test"
    return {
        line[: -len(suffix)]
        for raw_line in output.splitlines()
        if (line := raw_line.strip()).endswith(suffix)
    }


def require_planned_tests(plan: Sequence[GfxR5Case], inventory: set[str]) -> None:
    missing = [case.test_name for case in plan if case.test_name not in inventory]
    if missing:
        details = "\n".join(f"  - {test_name}" for test_name in missing)
        raise ValueError(
            "GFX-R5 plan references tests absent from the current Windows Vulkan "
            f"inventory:\n{details}"
        )


def preflight_test_inventory(plan: Sequence[GfxR5Case]) -> None:
    inventory = parse_test_inventory(capture(test_inventory_command()))
    require_planned_tests(plan, inventory)


class EvidenceSession:
    """Persist progress after every state transition so interrupted runs are visible."""

    def __init__(
        self,
        output_dir: Path,
        profile: str,
        vendor: str,
        device_fault: bool | None,
        soak_seconds: int,
        device_lost_timeout: int,
        plan: Sequence[GfxR5Case],
    ) -> None:
        self.output_dir = output_dir
        self.logs_dir = output_dir / "logs"
        self.manifest_path = output_dir / "manifest.json"
        if output_dir.exists():
            raise ValueError(f"evidence output already exists: {output_dir}")
        self.logs_dir.mkdir(parents=True)
        self.manifest = {
            "schema_version": EVIDENCE_SCHEMA_VERSION,
            "status": "running",
            "started_at": utc_now(),
            "completed_at": None,
            "resume_count": 0,
            "resumed_at": [],
            "configuration": {
                "profile": profile,
                "vendor": vendor,
                "device_fault_expected": device_fault,
                "soak_seconds": soak_seconds,
                "device_lost_timeout_seconds": device_lost_timeout,
            },
            "source": source_metadata(),
            "host": host_metadata(),
            "cases": [self._pending_case(index, case) for index, case in enumerate(plan, 1)],
        }
        self._write()

    @classmethod
    def from_existing(
        cls,
        output_dir: Path,
        manifest: dict[str, object],
    ) -> "EvidenceSession":
        session = cls.__new__(cls)
        session.output_dir = output_dir.resolve()
        session.logs_dir = session.output_dir / "logs"
        session.manifest_path = session.output_dir / "manifest.json"
        session.manifest = manifest
        return session

    def begin_resume(self) -> None:
        for case in self.manifest["cases"]:
            if case["status"] != "running":
                continue
            attempt = case["attempts"][-1]
            attempt["status"] = "interrupted"
            attempt["completed_at"] = utc_now()
            attempt["evidence_error"] = "runner stopped before completing this attempt"
            case["status"] = "interrupted"
            log_path = self.output_dir / attempt["log"]
            if not log_path.is_file():
                log_path.write_text(
                    "GFX-R5 runner recovered a stale attempt without a case log.\n",
                    encoding="utf-8",
                    newline="\n",
                )
            self._record_log_integrity(attempt)
        self.manifest["status"] = "running"
        self.manifest["completed_at"] = None
        self.manifest["resume_count"] = self.manifest.get("resume_count", 0) + 1
        self.manifest.setdefault("resumed_at", []).append(utc_now())
        self._write()

    def _pending_case(self, index: int, case: GfxR5Case) -> dict[str, object]:
        return {
            "index": index,
            "name": case.name,
            "test_name": case.test_name,
            "status": "pending",
            "environment": dict(case.environment),
            "command": list(case.command()),
            "attempts": [],
        }

    def start_case(self, index: int) -> Path:
        case = self.manifest["cases"][index]
        attempt_number = len(case["attempts"]) + 1
        attempt = {
            "attempt": attempt_number,
            "status": "running",
            "started_at": utc_now(),
            "completed_at": None,
            "duration_seconds": None,
            "exit_code": None,
            "evidence_error": None,
            "log": (
                f"logs/{case['index']:02d}-{case['name']}-"
                f"attempt-{attempt_number:02d}.log"
            ),
            "log_bytes": None,
            "log_sha256": None,
        }
        case["attempts"].append(attempt)
        case["status"] = "running"
        self._write()
        return self.output_dir / attempt["log"]

    def finish_case(
        self,
        index: int,
        exit_code: int,
        duration: float,
        evidence_error: str | None = None,
    ) -> None:
        case = self.manifest["cases"][index]
        attempt = case["attempts"][-1]
        attempt["status"] = (
            "passed" if exit_code == 0 and evidence_error is None else "failed"
        )
        attempt["completed_at"] = utc_now()
        attempt["duration_seconds"] = round(duration, 3)
        attempt["exit_code"] = exit_code
        attempt["evidence_error"] = evidence_error
        case["status"] = attempt["status"]
        self._record_log_integrity(attempt)
        self._write()

    def finish(self, status: str) -> None:
        self.manifest["status"] = status
        self.manifest["completed_at"] = utc_now()
        if status == "interrupted":
            for case in self.manifest["cases"]:
                if case["status"] == "running":
                    attempt = case["attempts"][-1]
                    attempt["status"] = "interrupted"
                    attempt["completed_at"] = self.manifest["completed_at"]
                    case["status"] = "interrupted"
                    log_path = self.output_dir / attempt["log"]
                    if log_path.is_file():
                        self._record_log_integrity(attempt)
        self._write()

    def _record_log_integrity(self, attempt: dict[str, object]) -> None:
        log_path = self.output_dir / str(attempt["log"])
        normalize_log_ending(log_path)
        attempt["log_bytes"] = log_path.stat().st_size
        attempt["log_sha256"] = file_sha256(log_path)

    def _write(self) -> None:
        write_json_atomic(self.manifest_path, self.manifest)


def _profile_cases(profile: str) -> tuple[tuple[str, str], ...]:
    if profile == "vendor":
        return VENDOR_CASES
    if profile == "mixed-dpi":
        return MIXED_DPI_CASES
    if profile == "soak":
        return SOAK_CASES
    if profile == "device-lost":
        return DEVICE_LOST_CASES
    if profile == "all":
        return VENDOR_CASES + MIXED_DPI_CASES + SOAK_CASES + DEVICE_LOST_CASES
    raise ValueError(f"unknown GFX-R5 profile: {profile}")


def build_plan(
    profile: str,
    vendor: str,
    device_fault: bool | None,
    soak_seconds: int,
    device_lost_timeout: int,
) -> list[GfxR5Case]:
    if vendor not in VENDORS:
        raise ValueError(f"vendor must be one of: {', '.join(VENDORS)}")
    if not MIN_SOAK_SECONDS <= soak_seconds <= MAX_SOAK_SECONDS:
        raise ValueError(
            f"soak seconds must be {MIN_SOAK_SECONDS}..={MAX_SOAK_SECONDS}"
        )
    if not MIN_DEVICE_LOST_TIMEOUT <= device_lost_timeout <= MAX_DEVICE_LOST_TIMEOUT:
        raise ValueError(
            "device-lost timeout must be "
            f"{MIN_DEVICE_LOST_TIMEOUT}..={MAX_DEVICE_LOST_TIMEOUT} seconds"
        )
    includes_device_lost = profile in ("device-lost", "all")
    if includes_device_lost and device_fault is None:
        raise ValueError(
            "--device-fault true|false is required for device-lost evidence"
        )

    plan: list[GfxR5Case] = []
    for name, test_name in _profile_cases(profile):
        environment = [(VENDOR_ENV, vendor)]
        if (name, test_name) in SOAK_CASES:
            environment.append((SOAK_SECONDS_ENV, str(soak_seconds)))
        if (name, test_name) in DEVICE_LOST_CASES:
            environment.extend(
                (
                    (DEVICE_FAULT_ENV, str(device_fault).lower()),
                    (DEVICE_LOST_TIMEOUT_ENV, str(device_lost_timeout)),
                )
            )
        plan.append(GfxR5Case(name, test_name, tuple(environment)))
    return plan


def load_resumable_evidence(
    output_dir: Path,
) -> tuple[EvidenceSession, list[GfxR5Case]]:
    output_dir = output_dir.resolve()
    manifest = verify_evidence_dir(output_dir)
    if manifest.get("status") == "passed":
        raise ValueError("passed GFX-R5 evidence has no remaining cases to resume")
    plan = plan_from_manifest_configuration(manifest)
    require_resume_environment(manifest)
    return EvidenceSession.from_existing(output_dir, manifest), plan


def plan_from_manifest_configuration(manifest: dict[str, object]) -> list[GfxR5Case]:
    configuration = manifest.get("configuration")
    if not isinstance(configuration, dict):
        raise ValueError("evidence manifest has no configuration object")
    profile = configuration.get("profile")
    vendor = configuration.get("vendor")
    device_fault = configuration.get("device_fault_expected")
    soak_seconds = configuration.get("soak_seconds")
    device_lost_timeout = configuration.get("device_lost_timeout_seconds")
    if not isinstance(profile, str) or not isinstance(vendor, str):
        raise ValueError("evidence profile and vendor must be strings")
    if device_fault is not None and not isinstance(device_fault, bool):
        raise ValueError("evidence device-fault expectation must be boolean or null")
    if type(soak_seconds) is not int or type(device_lost_timeout) is not int:
        raise ValueError("evidence soak and device-lost durations must be integers")
    return build_plan(
        profile,
        vendor,
        device_fault,
        soak_seconds,
        device_lost_timeout,
    )


def require_manifest_matches_plan(
    manifest: dict[str, object],
    plan: Sequence[GfxR5Case],
) -> None:
    cases = manifest.get("cases")
    if not isinstance(cases, list) or len(cases) != len(plan):
        raise ValueError("evidence case count does not match the reconstructed plan")
    for index, (stored, planned) in enumerate(zip(cases, plan), 1):
        if not isinstance(stored, dict):
            raise ValueError(f"evidence case {index} must be an object")
        expected = {
            "index": index,
            "name": planned.name,
            "test_name": planned.test_name,
            "environment": dict(planned.environment),
            "command": list(planned.command()),
        }
        actual = {name: stored.get(name) for name in expected}
        if actual != expected:
            raise ValueError(
                f"evidence case {index} no longer matches the current GFX-R5 plan"
            )


def require_resume_environment(manifest: dict[str, object]) -> None:
    source = manifest.get("source")
    if not isinstance(source, dict):
        raise ValueError("evidence manifest has no source object")
    if source.get("git_head") != source_metadata()["git_head"]:
        raise ValueError("evidence source does not match the current repository HEAD")
    if source.get("worktree_clean") is not True:
        raise ValueError("evidence source was not captured from a clean worktree")
    if manifest.get("host") != host_metadata():
        raise ValueError("evidence host or toolchain does not match the current target machine")


def require_recorded_provenance(manifest: dict[str, object]) -> None:
    source = manifest.get("source")
    if not isinstance(source, dict):
        raise ValueError("evidence manifest has no source object")
    repository = source.get("repository")
    git_head = source.get("git_head")
    git_branch = source.get("git_branch")
    if not isinstance(repository, str) or not repository:
        raise ValueError("evidence source repository must be a non-empty string")
    if (
        not isinstance(git_head, str)
        or len(git_head) != 40
        or any(character not in "0123456789abcdefABCDEF" for character in git_head)
    ):
        raise ValueError("evidence source git_head must be a 40-digit hexadecimal commit")
    if not isinstance(git_branch, str):
        raise ValueError("evidence source git_branch must be a string")
    if source.get("worktree_clean") is not True:
        raise ValueError("evidence source was not captured from a clean worktree")

    host = manifest.get("host")
    if not isinstance(host, dict):
        raise ValueError("evidence manifest has no host object")
    for name in ("node", "platform", "python", "rustc", "cargo"):
        if not isinstance(host.get(name), str) or not host[name]:
            raise ValueError(f"evidence host {name} must be a non-empty string")


def format_case(case: GfxR5Case) -> str:
    environment = " ".join(f"{name}={value}" for name, value in case.environment)
    command = subprocess.list2cmdline(case.command())
    return f"{case.name}: {environment} {command}"


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def default_evidence_dir(profile: str, vendor: str) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    return ROOT / "test-reports" / f"{stamp}-gfx-r5-{vendor}-{profile}"


def write_json_atomic(path: Path, value: object) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(64 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_evidence_dir(output_dir: Path) -> dict[str, object]:
    output_dir = output_dir.resolve()
    manifest_path = output_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema_version") != EVIDENCE_SCHEMA_VERSION:
        raise ValueError(
            "unsupported evidence schema "
            f"{manifest.get('schema_version')!r}; expected {EVIDENCE_SCHEMA_VERSION}"
        )
    status = manifest.get("status")
    cases = manifest.get("cases")
    if status not in ("running", "passed", "failed", "interrupted"):
        raise ValueError(f"invalid evidence status: {status!r}")
    if not isinstance(cases, list) or not cases:
        raise ValueError("evidence manifest must contain at least one case")
    resume_count = manifest.get("resume_count")
    resumed_at = manifest.get("resumed_at")
    if type(resume_count) is not int or resume_count < 0:
        raise ValueError("evidence resume_count must be a non-negative integer")
    if not isinstance(resumed_at, list) or len(resumed_at) != resume_count:
        raise ValueError("evidence resumed_at history does not match resume_count")

    case_statuses: list[object] = []
    seen_logs: set[str] = set()
    for case in cases:
        if not isinstance(case, dict):
            raise ValueError("evidence case must be an object")
        case_name = case.get("name")
        test_name = case.get("test_name")
        if not isinstance(test_name, str):
            raise ValueError(f"evidence case {case_name!r} has no test name")
        case_status = case.get("status")
        case_statuses.append(case_status)
        if case_status not in ("pending", "running", "passed", "failed", "interrupted"):
            raise ValueError(f"invalid status for evidence case {case_name!r}")
        attempts = case.get("attempts")
        if not isinstance(attempts, list):
            raise ValueError(f"evidence case {case_name!r} attempts must be a list")
        if not attempts:
            if case_status != "pending":
                raise ValueError(f"evidence case {case_name!r} has status without attempts")
            continue
        for attempt_index, attempt in enumerate(attempts, 1):
            if not isinstance(attempt, dict):
                raise ValueError(f"evidence case {case_name!r} attempt must be an object")
            if attempt.get("attempt") != attempt_index:
                raise ValueError(f"evidence case {case_name!r} attempt order is invalid")
            attempt_status = attempt.get("status")
            if attempt_status not in ("running", "passed", "failed", "interrupted"):
                raise ValueError(
                    f"invalid attempt status for evidence case {case_name!r}"
                )
            if attempt_index < len(attempts) and attempt_status == "running":
                raise ValueError(f"evidence case {case_name!r} has a stale running attempt")
            if attempt_status != "running":
                verify_attempt_log(
                    output_dir,
                    case_name,
                    test_name,
                    attempt,
                    seen_logs,
                )
        latest_status = attempts[-1].get("status")
        if latest_status != case_status:
            raise ValueError(f"evidence case {case_name!r} status disagrees with latest attempt")

    if status == "passed" and any(case_status != "passed" for case_status in case_statuses):
        raise ValueError("passed manifest contains a non-passed case")
    if status == "failed" and "failed" not in case_statuses:
        raise ValueError("failed manifest contains no failed case")
    if status == "interrupted" and "interrupted" not in case_statuses:
        raise ValueError("interrupted manifest contains no interrupted case")
    plan = plan_from_manifest_configuration(manifest)
    require_manifest_matches_plan(manifest, plan)
    require_recorded_provenance(manifest)
    return manifest


def verify_attempt_log(
    output_dir: Path,
    case_name: object,
    test_name: str,
    attempt: dict[str, object],
    seen_logs: set[str],
) -> None:
    relative_log = attempt.get("log")
    if not isinstance(relative_log, str):
        raise ValueError(f"evidence case {case_name!r} attempt has no log path")
    if relative_log in seen_logs:
        raise ValueError(f"evidence log is reused by multiple attempts: {relative_log}")
    seen_logs.add(relative_log)
    log_path = (output_dir / relative_log).resolve()
    if not log_path.is_relative_to(output_dir):
        raise ValueError(f"evidence case {case_name!r} log escapes output directory")
    if not log_path.is_file():
        raise ValueError(f"evidence case {case_name!r} log is missing")
    actual_bytes = log_path.stat().st_size
    expected_bytes = attempt.get("log_bytes")
    if actual_bytes != expected_bytes:
        raise ValueError(
            f"evidence case {case_name!r} log size changed: "
            f"expected={expected_bytes!r}, actual={actual_bytes}"
        )
    actual_hash = file_sha256(log_path)
    expected_hash = attempt.get("log_sha256")
    if actual_hash != expected_hash:
        raise ValueError(f"evidence case {case_name!r} log SHA-256 changed")
    if attempt.get("status") == "passed":
        require_exact_test_success(test_name, case_name, log_path)


def capture(command: Sequence[str]) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return result.stdout.strip()


def require_clean_source(allowed_untracked_dir: Path | None = None) -> None:
    raw = capture(
        ("git", "status", "--porcelain=v1", "-z", "--untracked-files=normal")
    )
    entries = [entry for entry in raw.split("\0") if entry]
    dirty = filter_allowed_untracked(entries, allowed_untracked_dir)
    if dirty:
        raise ValueError(
            "GFX-R5 evidence requires a clean worktree; commit or remove:\n"
            + "\n".join(dirty)
        )


def filter_allowed_untracked(
    entries: Sequence[str],
    allowed_untracked_dir: Path | None,
) -> list[str]:
    allowed = allowed_untracked_dir.resolve() if allowed_untracked_dir is not None else None
    dirty: list[str] = []
    for entry in entries:
        status = entry[:2]
        path = entry[3:].rstrip("/") if len(entry) >= 4 else ""
        candidate = (ROOT / path).resolve()
        if status == "??" and allowed is not None and candidate == allowed:
            continue
        dirty.append(entry)
    return dirty


def source_metadata() -> dict[str, object]:
    return {
        "repository": str(ROOT),
        "git_head": capture(("git", "rev-parse", "HEAD")),
        "git_branch": capture(("git", "branch", "--show-current")),
        "worktree_clean": True,
    }


def host_metadata() -> dict[str, str]:
    return {
        "node": platform.node(),
        "platform": platform.platform(),
        "python": platform.python_version(),
        "rustc": capture(("rustc", "--version")),
        "cargo": capture(("cargo", "--version")),
    }


def normalize_log_ending(path: Path) -> None:
    content = path.read_text(encoding="utf-8")
    path.write_text(content.rstrip() + "\n", encoding="utf-8", newline="\n")


def require_case_success(case: GfxR5Case, log_path: Path) -> None:
    require_exact_test_success(case.test_name, case.name, log_path)


def require_exact_test_success(
    test_name: str,
    case_name: object,
    log_path: Path,
) -> None:
    content = log_path.read_text(encoding="utf-8")
    required = (
        "running 1 test",
        f"\ntest {test_name} ...",
        "test result: ok. 1 passed; 0 failed;",
    )
    missing = [marker for marker in required if marker not in content]
    if missing:
        details = ", ".join(repr(marker) for marker in missing)
        raise ValueError(
            f"cargo exited successfully but {case_name!r} lacks exact one-test "
            f"success evidence: {details}"
        )


def run_case(case: GfxR5Case, log_path: Path) -> tuple[int, float]:
    inherited = os.environ.copy()
    inherited.update(case.environment)
    started = time.monotonic()
    with log_path.open("w", encoding="utf-8", newline="\n") as log:
        heading = format_case(case)
        log.write(heading + "\n")
        log.flush()
        process = subprocess.Popen(
            case.command(),
            cwd=ROOT,
            env=inherited,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            bufsize=1,
        )
        assert process.stdout is not None
        try:
            for line in process.stdout:
                print(line, end="", flush=True)
                log.write(line)
        except KeyboardInterrupt:
            process.terminate()
            process.wait()
            raise
        exit_code = process.wait()
    normalize_log_ending(log_path)
    return exit_code, time.monotonic() - started


def run_plan(
    plan: Sequence[GfxR5Case],
    evidence: EvidenceSession,
) -> int:
    if os.name != "nt":
        print("error: GFX-R5 target-machine acceptance requires Windows", file=sys.stderr)
        return 2
    runnable = [
        (index, case)
        for index, case in enumerate(plan)
        if evidence.manifest["cases"][index]["status"] != "passed"
    ]
    for progress, (index, case) in enumerate(runnable, start=1):
        print(f"[{progress}/{len(runnable)}] {format_case(case)}", flush=True)
        log_path = evidence.start_case(index)
        exit_code, duration = run_case(case, log_path)
        evidence_error = None
        if exit_code == 0:
            try:
                require_case_success(case, log_path)
            except ValueError as error:
                evidence_error = str(error)
        evidence.finish_case(index, exit_code, duration, evidence_error)
        if exit_code != 0 or evidence_error is not None:
            runner_exit_code = (
                exit_code if exit_code != 0 else EVIDENCE_VALIDATION_EXIT_CODE
            )
            print(
                f"error: GFX-R5 case {case.name!r} failed; "
                f"cargo_exit_code={exit_code}; evidence_error={evidence_error!r}; "
                f"evidence={evidence.output_dir}",
                file=sys.stderr,
            )
            evidence.finish("failed")
            return runner_exit_code
    if any(case["status"] != "passed" for case in evidence.manifest["cases"]):
        evidence.finish("failed")
        print(
            f"error: GFX-R5 run ended with unresolved cases; evidence={evidence.output_dir}",
            file=sys.stderr,
        )
        return EVIDENCE_VALIDATION_EXIT_CODE
    evidence.finish("passed")
    return 0


def _bool_argument(value: str) -> bool:
    normalized = value.strip().lower()
    if normalized in ("true", "1"):
        return True
    if normalized in ("false", "0"):
        return False
    raise argparse.ArgumentTypeError("expected true, false, 1, or 0")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=PROFILES)
    parser.add_argument("--vendor", choices=VENDORS)
    parser.add_argument(
        "--device-fault",
        type=_bool_argument,
        help="whether VK_EXT_device_fault is expected on the target",
    )
    parser.add_argument("--soak-seconds", type=int)
    parser.add_argument("--device-lost-timeout", type=int)
    parser.add_argument(
        "--list",
        action="store_true",
        help="print the exact plan without requiring Windows or running tests",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="new evidence directory (default: test-reports/<timestamp>-gfx-r5-...)",
    )
    parser.add_argument(
        "--verify-evidence",
        type=Path,
        help="verify an existing schema-4 evidence directory without running tests",
    )
    parser.add_argument(
        "--resume-evidence",
        type=Path,
        help="resume non-passed cases in an existing schema-4 evidence directory",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.verify_evidence is not None and args.resume_evidence is not None:
        print(
            "error: --verify-evidence and --resume-evidence are mutually exclusive",
            file=sys.stderr,
        )
        return 2
    if args.verify_evidence is not None:
        if (
            args.profile is not None
            or args.vendor is not None
            or args.device_fault is not None
            or args.soak_seconds is not None
            or args.device_lost_timeout is not None
            or args.list
            or args.output
        ):
            print(
                "error: --verify-evidence cannot be combined with run/list options",
                file=sys.stderr,
            )
            return 2
        try:
            manifest = verify_evidence_dir(args.verify_evidence)
        except (OSError, ValueError) as error:
            print(f"error: evidence verification failed: {error}", file=sys.stderr)
            return 2
        if manifest["status"] != "passed":
            print(
                "error: evidence verification requires a passed manifest; "
                f"status={manifest['status']}; path={args.verify_evidence.resolve()}",
                file=sys.stderr,
            )
            return EVIDENCE_VALIDATION_EXIT_CODE
        print(
            f"GFX-R5 evidence verified: status={manifest['status']}; "
            f"cases={len(manifest['cases'])}; path={args.verify_evidence.resolve()}"
        )
        return 0
    if args.resume_evidence is not None:
        if (
            args.profile is not None
            or args.vendor is not None
            or args.device_fault is not None
            or args.soak_seconds is not None
            or args.device_lost_timeout is not None
            or args.list
            or args.output
        ):
            print(
                "error: --resume-evidence cannot be combined with run/list options",
                file=sys.stderr,
            )
            return 2
        if os.name != "nt":
            print(
                "error: GFX-R5 target-machine acceptance requires Windows",
                file=sys.stderr,
            )
            return 2
        try:
            evidence, plan = load_resumable_evidence(args.resume_evidence)
            require_clean_source(evidence.output_dir)
            preflight_test_inventory(plan)
            evidence.begin_resume()
        except (OSError, subprocess.SubprocessError, ValueError) as error:
            print(f"error: evidence resume preflight failed: {error}", file=sys.stderr)
            return 2
        try:
            result = run_plan(plan, evidence)
        except KeyboardInterrupt:
            evidence.finish("interrupted")
            print(f"interrupted; partial evidence={evidence.output_dir}", file=sys.stderr)
            return 130
        print(f"GFX-R5 evidence resumed: {evidence.output_dir}")
        return result
    if args.profile is None or args.vendor is None:
        print("error: --profile and --vendor are required for run/list mode", file=sys.stderr)
        return 2
    try:
        plan = build_plan(
            args.profile,
            args.vendor,
            args.device_fault,
            args.soak_seconds if args.soak_seconds is not None else MIN_SOAK_SECONDS,
            args.device_lost_timeout
            if args.device_lost_timeout is not None
            else 60,
        )
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if args.list:
        for case in plan:
            print(format_case(case))
        return 0
    if os.name != "nt":
        print("error: GFX-R5 target-machine acceptance requires Windows", file=sys.stderr)
        return 2
    try:
        require_clean_source()
        preflight_test_inventory(plan)
        output_dir = (
            args.output.resolve()
            if args.output is not None
            else default_evidence_dir(args.profile, args.vendor)
        )
        evidence = EvidenceSession(
            output_dir,
            args.profile,
            args.vendor,
            args.device_fault,
            args.soak_seconds if args.soak_seconds is not None else MIN_SOAK_SECONDS,
            args.device_lost_timeout
            if args.device_lost_timeout is not None
            else 60,
            plan,
        )
    except (OSError, subprocess.SubprocessError, ValueError) as error:
        print(f"error: evidence preflight failed: {error}", file=sys.stderr)
        return 2
    try:
        result = run_plan(plan, evidence)
    except KeyboardInterrupt:
        evidence.finish("interrupted")
        print(f"interrupted; partial evidence={evidence.output_dir}", file=sys.stderr)
        return 130
    print(f"GFX-R5 evidence: {evidence.output_dir}")
    return result


if __name__ == "__main__":
    sys.exit(main())
