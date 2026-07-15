import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import Mock, patch

from scripts.run_windows_gfx_r5 import (
    DEVICE_FAULT_ENV,
    DEVICE_LOST_TIMEOUT_ENV,
    EVIDENCE_VALIDATION_EXIT_CODE,
    ROOT,
    SOAK_SECONDS_ENV,
    VENDOR_ENV,
    VENDOR_IDS,
    EvidenceSession,
    build_plan,
    capture,
    file_sha256,
    filter_allowed_untracked,
    load_resumable_evidence,
    main,
    normalize_log_ending,
    parse_test_inventory,
    require_case_success,
    require_planned_tests,
    run_case,
    run_plan,
    verify_evidence_dir,
    write_json_atomic,
)


def passing_log(case) -> str:
    vendor = dict(case.environment)[VENDOR_ENV]
    adapter = (
        f'adapter="Test GPU"; type=discrete_gpu; '
        f"vendor=0x{VENDOR_IDS[vendor]:04X}; device=0x1234; "
        "api=1.3.280; driver=0x12345678; queue_family=0"
    )
    return (
        "\n".join(case.required_evidence_markers())
        + "\n"
        + adapter
        + "\n"
        "running 1 test\n"
        f"test {case.test_name} ... ok\n"
        "test result: ok. 1 passed; 0 failed;\n"
    )


class RunWindowsGfxR5Tests(unittest.TestCase):
    def test_capture_preserves_failed_command_diagnostics(self) -> None:
        completed = Mock(returncode=101, stdout="error[E0433]: missing API\n")
        with patch("scripts.run_windows_gfx_r5.subprocess.run", return_value=completed):
            with self.assertRaisesRegex(
                ValueError,
                r"exit code 101: cargo test --lib\nerror\[E0433\]: missing API",
            ):
                capture(("cargo", "test", "--lib"))

    def test_preflight_checks_clean_source_and_inventory_without_running(self) -> None:
        stdout = StringIO()
        with (
            patch("scripts.run_windows_gfx_r5.os.name", "nt"),
            patch("scripts.run_windows_gfx_r5.require_clean_source") as clean,
            patch("scripts.run_windows_gfx_r5.preflight_test_inventory") as inventory,
            patch("scripts.run_windows_gfx_r5.run_plan") as run_plan_mock,
            redirect_stdout(stdout),
        ):
            result = main(
                ["--preflight", "--profile", "vendor", "--vendor", "amd"]
            )

        self.assertEqual(result, 0)
        clean.assert_called_once_with()
        inventory.assert_called_once_with(build_plan("vendor", "amd", None, 900, 60))
        run_plan_mock.assert_not_called()
        self.assertIn("GFX-R5 preflight passed", stdout.getvalue())

    def test_preflight_rejects_list_mode(self) -> None:
        stderr = StringIO()
        with redirect_stderr(stderr):
            result = main(
                [
                    "--preflight",
                    "--list",
                    "--profile",
                    "vendor",
                    "--vendor",
                    "amd",
                ]
            )

        self.assertEqual(result, 2)
        self.assertIn("mutually exclusive", stderr.getvalue())

    def test_run_case_cleans_process_tree_when_output_stream_fails(self) -> None:
        class BrokenOutput:
            def __iter__(self):
                raise OSError("output pipe failed")

        case = build_plan("mixed-dpi", "amd", None, 900, 60)[0]
        process = Mock(stdout=BrokenOutput())
        with TemporaryDirectory() as directory:
            log_path = Path(directory) / "case.log"
            with (
                patch("scripts.run_windows_gfx_r5.subprocess.Popen", return_value=process),
                patch(
                    "scripts.run_windows_gfx_r5.terminate_process_tree",
                    return_value=True,
                ) as terminate,
                self.assertRaisesRegex(OSError, "output pipe failed"),
            ):
                run_case(case, log_path)

        terminate.assert_called_once_with(process)
        process.wait.assert_not_called()

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

    def test_all_profile_requires_hardware_specific_evidence_markers(self) -> None:
        plan = build_plan("all", "intel", False, 1_200, 60)
        markers = {
            case.name: case.required_evidence_markers() for case in plan
        }

        for case in plan:
            self.assertIn("expected=intel;", case.required_evidence_markers()[0])
        self.assertIn("dpi=", markers["vulkan-mixed-dpi"])
        self.assertIn("duration=1200.0s", markers["single-window-soak"])
        self.assertIn("ERROR_DEVICE_LOST", markers["external-device-reset"])
        self.assertIn("device_fault=false;", markers["external-device-reset"])

    def test_evidence_manifest_survives_each_case_transition(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)

            log_path = session.start_case(0)
            self.assertEqual(log_path.name, "01-vulkan-mixed-dpi-attempt-01.log")
            log_path.write_text("matrix evidence\n", encoding="utf-8")
            session.finish_case(0, 0, 1.23456)
            session.finish("passed")

            manifest = session.manifest
            self.assertEqual(manifest["schema_version"], 4)
            self.assertEqual(manifest["status"], "passed")
            case = manifest["cases"][0]
            self.assertEqual(case["status"], "passed")
            attempt = case["attempts"][0]
            self.assertEqual(attempt["duration_seconds"], 1.235)
            self.assertEqual(attempt["log_bytes"], 16)
            self.assertEqual(attempt["log_sha256"], file_sha256(log_path))
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
        valid = passing_log(case)
        with TemporaryDirectory() as directory:
            path = Path(directory) / "case.log"
            path.write_text(valid, encoding="utf-8")
            require_case_success(case, path)

            path.write_text(
                "running 1 test\n"
                f"test {case.test_name} ... ok\n"
                "test result: ok. 1 passed; 0 failed; 0 ignored\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "semantic evidence markers"):
                require_case_success(case, path)

            path.write_text(
                "running 0 tests\n"
                "test result: ok. 0 passed; 0 failed; 0 ignored\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "exact one-test success"):
                require_case_success(case, path)

    def test_case_success_rejects_invalid_adapter_diagnostics(self) -> None:
        case = build_plan("vendor", "amd", None, 900, 60)[0]
        with TemporaryDirectory() as directory:
            path = Path(directory) / "case.log"

            path.write_text(
                passing_log(case).replace("device=0x1234", "device=0x0000"),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "device ID must be non-zero"):
                require_case_success(case, path)

            path.write_text(
                passing_log(case).replace("vendor=0x1002", "vendor=0x10DE"),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "adapter vendor mismatch"):
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

            log_path = session.start_case(0)
            log_path.write_text("cargo returned without a test\n", encoding="utf-8")
            session.finish_case(0, 0, 0.25, "missing exact test evidence")
            session.finish("failed")

            case = session.manifest["cases"][0]
            self.assertEqual(case["status"], "failed")
            attempt = case["attempts"][0]
            self.assertEqual(attempt["exit_code"], 0)
            self.assertEqual(attempt["evidence_error"], "missing exact test evidence")

    def test_evidence_verifier_rejects_log_tampering(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text(passing_log(plan[0]), encoding="utf-8")
            session.finish_case(0, 0, 0.5)
            session.finish("passed")

            verified = verify_evidence_dir(output)
            self.assertEqual(verified["status"], "passed")

            log_path.write_text("tampered evidence\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "size changed|SHA-256 changed"):
                verify_evidence_dir(output)

    def test_evidence_verifier_rejects_exact_success_without_semantic_markers(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text(
                "running 1 test\n"
                f"test {plan[0].test_name} ... ok\n"
                "test result: ok. 1 passed; 0 failed;\n",
                encoding="utf-8",
            )
            session.finish_case(0, 0, 0.5)
            session.finish("passed")

            with self.assertRaisesRegex(ValueError, "semantic evidence markers"):
                verify_evidence_dir(output)

    def test_evidence_verifier_rejects_plan_or_provenance_tampering(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text(passing_log(plan[0]), encoding="utf-8")
            session.finish_case(0, 0, 0.5)
            session.finish("passed")

            original_command = list(session.manifest["cases"][0]["command"])
            session.manifest["cases"][0]["command"].append("--tampered")
            session._write()
            with self.assertRaisesRegex(ValueError, "plan"):
                verify_evidence_dir(output)

            session.manifest["cases"][0]["command"] = original_command
            session.manifest["source"]["git_head"] = "not-a-commit"
            session._write()
            with self.assertRaisesRegex(ValueError, "git_head"):
                verify_evidence_dir(output)

            session.manifest["source"]["git_head"] = "0" * 40
            session.manifest["host"]["rustc"] = ""
            session._write()
            with self.assertRaisesRegex(ValueError, "host rustc"):
                verify_evidence_dir(output)

    def test_evidence_verifier_rejects_inconsistent_attempt_metadata(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text(passing_log(plan[0]), encoding="utf-8")
            session.finish_case(0, 0, 0.5)
            session.finish("passed")
            attempt = session.manifest["cases"][0]["attempts"][0]

            attempt["exit_code"] = 9
            session._write()
            with self.assertRaisesRegex(ValueError, "exit_code=0"):
                verify_evidence_dir(output)

            attempt["exit_code"] = 0
            attempt["duration_seconds"] = -0.5
            session._write()
            with self.assertRaisesRegex(ValueError, "duration"):
                verify_evidence_dir(output)

            attempt["duration_seconds"] = 0.5
            attempt["completed_at"] = None
            session._write()
            with self.assertRaisesRegex(ValueError, "completion time"):
                verify_evidence_dir(output)

    def test_verify_command_succeeds_only_for_a_passed_manifest(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text("target topology unavailable\n", encoding="utf-8")
            session.finish_case(0, 1, 0.5)
            session.finish("failed")

            stderr = StringIO()
            with redirect_stderr(stderr):
                result = main(["--verify-evidence", str(output)])

            self.assertEqual(result, EVIDENCE_VALIDATION_EXIT_CODE)
            self.assertIn("requires a passed manifest", stderr.getvalue())

            retry_log = session.start_case(0)
            retry_log.write_text(passing_log(plan[0]), encoding="utf-8")
            session.finish_case(0, 0, 0.5)
            session.finish("passed")

            self.assertEqual(main(["--verify-evidence", str(output)]), 0)

    def test_attempt_history_retains_failure_before_success(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)

            first_log = session.start_case(0)
            first_log.write_text("first attempt failed\n", encoding="utf-8")
            session.finish_case(0, 1, 0.2)
            second_log = session.start_case(0)
            second_log.write_text(passing_log(plan[0]), encoding="utf-8")
            session.finish_case(0, 0, 0.3)
            session.finish("passed")

            case = session.manifest["cases"][0]
            self.assertEqual(case["status"], "passed")
            self.assertEqual(
                [attempt["status"] for attempt in case["attempts"]],
                ["failed", "passed"],
            )
            self.assertNotEqual(first_log, second_log)
            verify_evidence_dir(output)

    def test_resume_loader_reconstructs_exact_failed_plan(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text("topology unavailable\n", encoding="utf-8")
            session.finish_case(0, 1, 0.2)
            session.finish("failed")

            resumed, reconstructed = load_resumable_evidence(output)

            self.assertEqual(resumed.output_dir, output.resolve())
            self.assertEqual(reconstructed, plan)

    def test_resume_loader_rejects_source_or_plan_drift(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text("topology unavailable\n", encoding="utf-8")
            session.finish_case(0, 1, 0.2)
            session.finish("failed")

            original_source = dict(session.manifest["source"])
            session.manifest["source"]["git_head"] = "0" * 40
            session._write()
            with self.assertRaisesRegex(ValueError, "source"):
                load_resumable_evidence(output)

            session.manifest["source"] = original_source
            session.manifest["cases"][0]["test_name"] = "tests::renamed"
            session._write()
            with self.assertRaisesRegex(ValueError, "plan"):
                load_resumable_evidence(output)

    def test_resume_allows_relocated_clone_and_equivalent_branch(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            log_path = session.start_case(0)
            log_path.write_text("topology unavailable\n", encoding="utf-8")
            session.finish_case(0, 1, 0.2)
            session.finish("failed")

            session.manifest["source"]["repository"] = "D:\\relocated\\uix-app"
            session.manifest["source"]["git_branch"] = "gfx-r5-retry"
            session._write()

            resumed, reconstructed = load_resumable_evidence(output)
            self.assertEqual(resumed.output_dir, output.resolve())
            self.assertEqual(reconstructed, plan)

    def test_resume_skips_passed_cases_and_appends_attempts(self) -> None:
        plan = build_plan("vendor", "nvidia", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "vendor", "nvidia", None, 900, 60, plan)
            first_log = session.start_case(0)
            first_log.write_text(passing_log(plan[0]), encoding="utf-8")
            session.finish_case(0, 0, 0.1)
            failed_log = session.start_case(1)
            failed_log.write_text("first attempt failed\n", encoding="utf-8")
            session.finish_case(1, 1, 0.1)
            session.finish("failed")
            session.begin_resume()

            executed: list[str] = []

            def fake_run(case, log_path):
                executed.append(case.name)
                log_path.write_text(passing_log(case), encoding="utf-8")
                return 0, 0.1

            with patch("scripts.run_windows_gfx_r5.run_case", side_effect=fake_run):
                self.assertEqual(run_plan(plan, session), 0)

            self.assertNotIn(plan[0].name, executed)
            self.assertEqual(len(session.manifest["cases"][0]["attempts"]), 1)
            self.assertEqual(len(session.manifest["cases"][1]["attempts"]), 2)
            self.assertEqual(session.manifest["status"], "passed")
            self.assertEqual(session.manifest["resume_count"], 1)
            verify_evidence_dir(output)

    def test_resume_recovers_stale_running_attempt(self) -> None:
        plan = build_plan("mixed-dpi", "amd", None, 900, 60)
        with TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            session = EvidenceSession(output, "mixed-dpi", "amd", None, 900, 60, plan)
            stale_log = session.start_case(0)
            stale_log.write_text("partial output\n", encoding="utf-8")

            session.begin_resume()

            case = session.manifest["cases"][0]
            attempt = case["attempts"][0]
            self.assertEqual(case["status"], "interrupted")
            self.assertEqual(attempt["status"], "interrupted")
            self.assertIsNotNone(attempt["log_sha256"])
            self.assertEqual(session.manifest["status"], "running")
            verify_evidence_dir(output)

    def test_clean_filter_allows_only_the_resume_directory(self) -> None:
        allowed = ROOT / "test-reports" / "partial"
        entries = [
            "?? test-reports/partial/",
            " M scripts/run_windows_gfx_r5.py",
        ]

        self.assertEqual(
            filter_allowed_untracked(entries[:1], allowed),
            [],
        )
        self.assertEqual(
            filter_allowed_untracked(entries, allowed),
            [" M scripts/run_windows_gfx_r5.py"],
        )


if __name__ == "__main__":
    unittest.main()
