# -*- coding: utf-8 -*-
"""Run the Windows Vulkan GFX-R5 target-machine acceptance matrix."""
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass
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


def run_plan(plan: Sequence[GfxR5Case]) -> int:
    if os.name != "nt":
        print("error: GFX-R5 target-machine acceptance requires Windows", file=sys.stderr)
        return 2
    inherited = os.environ.copy()
    for index, case in enumerate(plan, start=1):
        print(f"[{index}/{len(plan)}] {format_case(case)}", flush=True)
        environment = inherited.copy()
        environment.update(case.environment)
        result = subprocess.run(case.command(), cwd=ROOT, env=environment, check=False)
        if result.returncode != 0:
            print(
                f"error: GFX-R5 case {case.name!r} failed with exit code "
                f"{result.returncode}",
                file=sys.stderr,
            )
            return result.returncode
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
    return run_plan(plan)


if __name__ == "__main__":
    sys.exit(main())
