# -*- coding: utf-8 -*-
"""Run the Windows Vulkan GFX-R5 target-machine acceptance matrix."""
from __future__ import annotations

import argparse
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
            "schema_version": 2,
            "status": "running",
            "started_at": utc_now(),
            "completed_at": None,
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

    def _pending_case(self, index: int, case: GfxR5Case) -> dict[str, object]:
        return {
            "index": index,
            "name": case.name,
            "test_name": case.test_name,
            "status": "pending",
            "started_at": None,
            "completed_at": None,
            "duration_seconds": None,
            "exit_code": None,
            "evidence_error": None,
            "environment": dict(case.environment),
            "command": list(case.command()),
            "log": f"logs/{index:02d}-{case.name}.log",
        }

    def start_case(self, index: int) -> Path:
        case = self.manifest["cases"][index]
        case["status"] = "running"
        case["started_at"] = utc_now()
        self._write()
        return self.output_dir / case["log"]

    def finish_case(
        self,
        index: int,
        exit_code: int,
        duration: float,
        evidence_error: str | None = None,
    ) -> None:
        case = self.manifest["cases"][index]
        case["status"] = (
            "passed" if exit_code == 0 and evidence_error is None else "failed"
        )
        case["completed_at"] = utc_now()
        case["duration_seconds"] = round(duration, 3)
        case["exit_code"] = exit_code
        case["evidence_error"] = evidence_error
        self._write()

    def finish(self, status: str) -> None:
        self.manifest["status"] = status
        self.manifest["completed_at"] = utc_now()
        if status == "interrupted":
            for case in self.manifest["cases"]:
                if case["status"] == "running":
                    case["status"] = "interrupted"
                    case["completed_at"] = self.manifest["completed_at"]
        self._write()

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


def require_clean_source() -> None:
    dirty = capture(("git", "status", "--porcelain", "--untracked-files=normal"))
    if dirty:
        raise ValueError(
            "GFX-R5 evidence requires a clean worktree; commit or remove:\n" + dirty
        )


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
    content = log_path.read_text(encoding="utf-8")
    required = (
        "running 1 test",
        f"\ntest {case.test_name} ...",
        "test result: ok. 1 passed; 0 failed;",
    )
    missing = [marker for marker in required if marker not in content]
    if missing:
        details = ", ".join(repr(marker) for marker in missing)
        raise ValueError(
            f"cargo exited successfully but {case.name!r} lacks exact one-test "
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
    for index, case in enumerate(plan, start=1):
        print(f"[{index}/{len(plan)}] {format_case(case)}", flush=True)
        log_path = evidence.start_case(index - 1)
        exit_code, duration = run_case(case, log_path)
        evidence_error = None
        if exit_code == 0:
            try:
                require_case_success(case, log_path)
            except ValueError as error:
                evidence_error = str(error)
        evidence.finish_case(index - 1, exit_code, duration, evidence_error)
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
    parser.add_argument("--profile", choices=PROFILES, required=True)
    parser.add_argument("--vendor", choices=VENDORS, required=True)
    parser.add_argument(
        "--device-fault",
        type=_bool_argument,
        help="whether VK_EXT_device_fault is expected on the target",
    )
    parser.add_argument("--soak-seconds", type=int, default=MIN_SOAK_SECONDS)
    parser.add_argument("--device-lost-timeout", type=int, default=60)
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
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        plan = build_plan(
            args.profile,
            args.vendor,
            args.device_fault,
            args.soak_seconds,
            args.device_lost_timeout,
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
            args.soak_seconds,
            args.device_lost_timeout,
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
