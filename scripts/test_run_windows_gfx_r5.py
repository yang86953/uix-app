import unittest

from scripts.run_windows_gfx_r5 import (
    DEVICE_FAULT_ENV,
    DEVICE_LOST_TIMEOUT_ENV,
    SOAK_SECONDS_ENV,
    VENDOR_ENV,
    build_plan,
)


class RunWindowsGfxR5Tests(unittest.TestCase):
    def test_vendor_profile_is_exact_and_serial(self) -> None:
        plan = build_plan("vendor", "amd", None, 900, 60)

        self.assertEqual(len(plan), 4)
        for case in plan:
            self.assertEqual(dict(case.environment), {VENDOR_ENV: "amd"})
            self.assertIn("--exact", case.command())
            self.assertIn("--ignored", case.command())
            self.assertIn("--nocapture", case.command())
            self.assertIn("--test-threads=1", case.command())

    def test_soak_profile_applies_full_gate_duration(self) -> None:
        plan = build_plan("soak", "intel", None, 1_200, 60)

        self.assertEqual(
            [case.name for case in plan],
            ["single-window-soak", "shared-device-soak"],
        )
        for case in plan:
            self.assertEqual(
                dict(case.environment),
                {VENDOR_ENV: "intel", SOAK_SECONDS_ENV: "1200"},
            )

    def test_device_lost_profile_requires_explicit_fault_capability(self) -> None:
        with self.assertRaisesRegex(ValueError, "device-fault"):
            build_plan("device-lost", "nvidia", None, 900, 60)

        plan = build_plan("device-lost", "nvidia", True, 900, 90)
        self.assertEqual(
            dict(plan[0].environment),
            {
                VENDOR_ENV: "nvidia",
                DEVICE_FAULT_ENV: "true",
                DEVICE_LOST_TIMEOUT_ENV: "90",
            },
        )

    def test_all_profile_cannot_shorten_gate_bounds(self) -> None:
        with self.assertRaisesRegex(ValueError, "soak seconds"):
            build_plan("all", "amd", False, 899, 60)
        with self.assertRaisesRegex(ValueError, "device-lost timeout"):
            build_plan("all", "amd", False, 900, 601)


if __name__ == "__main__":
    unittest.main()
