import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.run_windows_gfx_r5 import (
    DEVICE_FAULT_ENV,
    DEVICE_LOST_TIMEOUT_ENV,
    SOAK_SECONDS_ENV,
    VENDOR_ENV,
    EvidenceSession,
    build_plan,
    normalize_log_ending,
    write_json_atomic,
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

    def test_evidence_manifest_survives_each_case_transition(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)

            log_path = session.start_case(0)
            self.assertEqual(log_path.name, "01-vulkan-mixed-dpi.log")
            session.finish_case(0, 0, 1.23456)
            session.finish("passed")

            manifest = session.manifest
            self.assertEqual(manifest["schema_version"], 1)
            self.assertEqual(manifest["status"], "passed")
            self.assertEqual(manifest["cases"][0]["status"], "passed")
            self.assertEqual(manifest["cases"][0]["duration_seconds"], 1.235)
            self.assertTrue(session.manifest_path.is_file())

    def test_atomic_manifest_write_leaves_no_temporary_file(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            write_json_atomic(path, {"status": "running"})

            self.assertIn('"status": "running"', path.read_text(encoding="utf-8"))
            self.assertFalse(path.with_suffix(".json.tmp").exists())

    def test_log_normalization_removes_eof_blank_lines(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "case.log"
            path.write_text("result: ok\n\n", encoding="utf-8")

            normalize_log_ending(path)

            self.assertEqual(path.read_bytes(), b"result: ok\n")


if __name__ == "__main__":
    unittest.main()
