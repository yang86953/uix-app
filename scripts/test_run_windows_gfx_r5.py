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
    parse_test_inventory,
    require_case_success,
    require_planned_tests,
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
            self.assertEqual(manifest["schema_version"], 2)
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

    def test_inventory_parser_ignores_cargo_noise(self) -> None:
        inventory = parse_test_inventory(
            "Finished test profile\n"
            "tests::one: test\n"
            "tests::benchmark: benchmark\n"
            "tests::two: test\n"
        )

        self.assertEqual(inventory, {"tests::one", "tests::two"})

    def test_plan_preflight_rejects_renamed_or_cfg_missing_tests(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)

        with self.assertRaisesRegex(ValueError, plan[0].test_name):
            require_planned_tests(plan, {"tests::some_other_test"})

        require_planned_tests(plan, {plan[0].test_name})

    def test_exact_case_success_requires_one_named_passing_test(self) -> None:
        case = build_plan("mixed-dpi", "amd", None, 900, 60)[0]
        valid = (
            "running 1 test\n"
            f"test {case.test_name} ... evidence\n"
            "ok\n\n"
            "test result: ok. 1 passed; 0 failed; 0 ignored\n"
        )
        with TemporaryDirectory() as directory:
            path = Path(directory) / "case.log"
            path.write_text(valid, encoding="utf-8")
            require_case_success(case, path)

            path.write_text(
                "running 0 tests\n"
                "test result: ok. 0 passed; 0 failed; 0 ignored\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "exact one-test success"):
                require_case_success(case, path)

    def test_manifest_distinguishes_cargo_success_from_evidence_failure(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            session = EvidenceSession(
                Path(directory) / "evidence",
                "mixed-dpi",
                "amd",
                None,
                900,
                60,
                plan,
            )

            session.start_case(0)
            session.finish_case(0, 0, 0.25, "missing exact test evidence")
            session.finish("failed")

            case = session.manifest["cases"][0]
            self.assertEqual(case["status"], "failed")
            self.assertEqual(case["exit_code"], 0)
            self.assertEqual(case["evidence_error"], "missing exact test evidence")


if __name__ == "__main__":
    unittest.main()
