import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from scripts.run_release_g5 import (
    EVIDENCE_SCHEMA,
    FRAME_SCHEMA,
    G5Error,
    MACRO_CYCLES,
    MACRO_CYCLE_SECONDS,
    MACRO_IDLE_SECONDS,
    P95_LIMIT_US,
    ScenarioAccumulator,
    WARMUP_US,
    assess_frozen_environment,
    build_release,
    determine_g5_gate_status,
    ensure_clean_status,
    file_sha256,
    main,
    nearest_rank,
    parse_g5_frame,
    summarize_memory_growth,
    summarize_idle_cpu,
    summarize_performance,
    validate_process_outcome,
    verify_evidence_manifest,
    write_evidence_manifest,
)


COMMIT = "a" * 40


def write_blocked_evidence(evidence: Path) -> None:
    (evidence / "build.log").write_text("build\n", encoding="utf-8")
    (evidence / "candidate-uix-demo.exe").write_bytes(b"candidate")
    (evidence / "candidate-Cargo.lock").write_text("lock\n", encoding="utf-8")
    (evidence / "runner.py").write_text("# runner\n", encoding="utf-8")
    (evidence / "raw.log").write_text("trusted\n", encoding="utf-8")
    (evidence / "report.md").write_text("blocked\n", encoding="utf-8")
    (evidence / "frames.ndjson").write_text(
        json.dumps(
            {"evidence_schema": EVIDENCE_SCHEMA, "commit": COMMIT, "schema": FRAME_SCHEMA}
        )
        + "\n",
        encoding="utf-8",
    )
    (evidence / "memory.ndjson").write_text(
        json.dumps({"evidence_schema": EVIDENCE_SCHEMA, "commit": COMMIT}) + "\n",
        encoding="utf-8",
    )
    (evidence / "scenarios.ndjson").write_text(
        json.dumps(
            {
                "evidence_schema": EVIDENCE_SCHEMA,
                "commit": COMMIT,
                "schema": str(FRAME_SCHEMA),
            }
        )
        + "\n",
        encoding="utf-8",
    )
    executable_hash = file_sha256(evidence / "candidate-uix-demo.exe")
    lock_hash = file_sha256(evidence / "candidate-Cargo.lock")
    runner_hash = file_sha256(evidence / "runner.py")
    (evidence / "environment.json").write_text(
        json.dumps(
            {
                "evidence_schema": EVIDENCE_SCHEMA,
                "runtime_frame_schema": FRAME_SCHEMA,
                "commit": COMMIT,
                "frozen_environment": {"status": "pass"},
            }
        ),
        encoding="utf-8",
    )
    (evidence / "summary.json").write_text(
        json.dumps(
            {
                "evidence_schema": EVIDENCE_SCHEMA,
                "runtime_frame_schema": FRAME_SCHEMA,
                "commit": COMMIT,
                "mode": "performance",
                "status": "pass",
                "g5_gate_status": "blocked",
                "executable_sha256": executable_hash,
                "cargo_lock_sha256": lock_hash,
                "result": {},
            }
        ),
        encoding="utf-8",
    )
    write_evidence_manifest(
        evidence,
        COMMIT,
        executable_hash,
        {
            "mode": "performance",
            "cargo_lock_sha256": lock_hash,
            "runner_sha256": runner_hash,
        },
    )


class ReleaseG5LogicTests(unittest.TestCase):
    def test_clean_candidate_rejects_any_porcelain_entry(self) -> None:
        ensure_clean_status("\n")
        with self.assertRaisesRegex(G5Error, "clean worktree"):
            ensure_clean_status(" M src/lib.rs\n")
        with self.assertRaisesRegex(G5Error, "clean worktree"):
            ensure_clean_status("?? untracked.txt\n")

    def test_release_build_uses_a_fresh_isolated_target_directory(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            evidence = Path(temporary) / "evidence"
            root.mkdir()
            evidence.mkdir()

            def fake_capture(command, _root):
                target_dir = Path(command[command.index("--target-dir") + 1])
                self.assertEqual(list(target_dir.iterdir()), [])
                executable = target_dir / "release" / "uix-demo.exe"
                executable.parent.mkdir(parents=True)
                executable.write_bytes(b"isolated")
                return "built\n"

            with patch("scripts.run_release_g5.capture", side_effect=fake_capture):
                candidate = build_release(root, evidence)
            self.assertEqual(candidate.read_bytes(), b"isolated")
            self.assertFalse(
                any(
                    path.name.startswith("uix-g5-isolated-target-")
                    for path in evidence.parent.iterdir()
                )
            )

    def test_nearest_rank_and_threshold_are_exact(self) -> None:
        values = list(range(1, 101))
        self.assertEqual(nearest_rank(values, 0.50), 50)
        self.assertEqual(nearest_rank(values, 0.95), 95)
        self.assertEqual(nearest_rank(values, 0.99), 99)

        passing = summarize_performance(
            [P95_LIMIT_US] * 20,
            minimum_samples=20,
        )
        self.assertEqual(passing["status"], "pass")
        failing = summarize_performance(
            [P95_LIMIT_US + 1] * 20,
            minimum_samples=20,
        )
        self.assertEqual(failing["status"], "fail")

    def test_insufficient_presented_samples_never_pass(self) -> None:
        with self.assertRaisesRegex(G5Error, "insufficient successful presented frames"):
            summarize_performance([1] * 9, minimum_samples=10)

    def test_frame_parser_requires_real_wgpu_presentation_fields(self) -> None:
        line = (
            "12:00:00 [INFO] G5_FRAME schema=1 frame_seq=7 scenario=modal_feedback "
            "window=0 logical_width=1200 logical_height=800 dpi=96 "
            "monotonic_us=31000000 presented=1 present_skipped=0 "
            "active_work=2 due_active_work=1 frame_us=14000 present_us=300 "
            "drawable_width=1200 drawable_height=800 drawable_pixels=960000 "
            "wgpu_surface_present_cpu_us=12 "
            "reconcile_us=10 reconcile_ran=1 (support.rs:1)"
        )
        parsed = parse_g5_frame(line)
        self.assertIsNotNone(parsed)
        assert parsed is not None
        self.assertEqual(parsed["schema"], FRAME_SCHEMA)
        self.assertEqual(parsed["drawable_pixels"], 960000)
        self.assertEqual(parsed["drawable_width"], 1200)
        self.assertEqual(parsed["wgpu_surface_present_cpu_us"], 12)

        skipped = line.replace("presented=1 present_skipped=0", "presented=0 present_skipped=1")
        with self.assertRaisesRegex(G5Error, "not a successful real presentation"):
            parse_g5_frame(skipped)
        duplicate = line.replace("frame_us=14000", "frame_us=14000 frame_us=1")
        with self.assertRaisesRegex(G5Error, "duplicate G5_FRAME field"):
            parse_g5_frame(duplicate)
        wrong_size = line.replace("drawable_width=1200", "drawable_width=1199")
        with self.assertRaisesRegex(G5Error, "frozen 1200x800"):
            parse_g5_frame(wrong_size)
        wrong_dpi = line.replace("dpi=96", "dpi=144")
        with self.assertRaisesRegex(G5Error, "96-DPI"):
            parse_g5_frame(wrong_dpi)

    def test_abnormal_exit_and_logs_fail_even_with_completion(self) -> None:
        validate_process_outcome(0, True, [])
        with self.assertRaisesRegex(G5Error, "exited abnormally"):
            validate_process_outcome(101, True, [])
        with self.assertRaisesRegex(G5Error, "completion marker"):
            validate_process_outcome(0, False, [])
        with self.assertRaisesRegex(G5Error, "abnormal logs"):
            validate_process_outcome(0, True, ["[ERROR] validation error"])
        with self.assertRaisesRegex(G5Error, "watchdog failure"):
            validate_process_outcome(0, True, [], "frame timeout")

    def test_manifest_detects_raw_log_tampering(self) -> None:
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            write_blocked_evidence(evidence)
            verified = verify_evidence_manifest(evidence)
            self.assertEqual(verified["commit"], COMMIT)
            self.assertEqual(
                verified["verified_summary"]["g5_gate_status"], "blocked"
            )

            (evidence / "raw.log").write_text("tampered\n", encoding="utf-8")
            with self.assertRaisesRegex(G5Error, "hash mismatch: raw.log"):
                verify_evidence_manifest(evidence)

    def test_manifest_semantics_reject_rehashed_commit_mismatch(self) -> None:
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            write_blocked_evidence(evidence)
            summary = json.loads((evidence / "summary.json").read_text(encoding="utf-8"))
            summary["commit"] = "b" * 40
            (evidence / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
            write_evidence_manifest(
                evidence,
                COMMIT,
                file_sha256(evidence / "candidate-uix-demo.exe"),
                {
                    "mode": "performance",
                    "cargo_lock_sha256": file_sha256(evidence / "candidate-Cargo.lock"),
                    "runner_sha256": file_sha256(evidence / "runner.py"),
                },
            )
            with self.assertRaisesRegex(G5Error, "summary.json commit mismatch"):
                verify_evidence_manifest(evidence)

    def test_manifest_cannot_promote_blocked_files_to_passing_g5(self) -> None:
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            write_blocked_evidence(evidence)
            summary = json.loads((evidence / "summary.json").read_text(encoding="utf-8"))
            summary.update({"mode": "soak", "status": "pass", "g5_gate_status": "pass"})
            summary["result"] = {
                "environment": {"status": "pass"},
                "soak": {"status": "pass"},
                "loop_matrix": {
                    category: {"status": "pass", "completed_iterations": 1000}
                    for category in ("window", "modal", "drawer", "theme", "dpi")
                },
            }
            (evidence / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
            write_evidence_manifest(
                evidence,
                COMMIT,
                file_sha256(evidence / "candidate-uix-demo.exe"),
                {
                    "mode": "soak",
                    "cargo_lock_sha256": file_sha256(evidence / "candidate-Cargo.lock"),
                    "runner_sha256": file_sha256(evidence / "runner.py"),
                },
            )
            with self.assertRaisesRegex(G5Error, "captured environment"):
                verify_evidence_manifest(evidence)

    def test_memory_growth_uses_first_and_last_medians(self) -> None:
        end_us = 30_000_000 + 8 * 60 * 60 * 1_000_000
        samples = [
            {
                "runtime_monotonic_us": 31_000_000,
                "working_set_bytes": 100,
                "private_bytes": 200,
            },
            {
                "runtime_monotonic_us": end_us - 1_000_000,
                "working_set_bytes": 100 + 64 * 1024 * 1024,
                "private_bytes": 200 + 64 * 1024 * 1024,
            },
        ]
        at_limit = summarize_memory_growth(samples, end_us, minimum_window_samples=1)
        self.assertEqual(at_limit["status"], "pass")
        samples[-1]["private_bytes"] += 1
        over_limit = summarize_memory_growth(samples, end_us, minimum_window_samples=1)
        self.assertEqual(over_limit["status"], "fail")

    def test_idle_cpu_oracle_rejects_a_busy_loop(self) -> None:
        samples = []
        for cycle in range(MACRO_CYCLES):
            for second in range(56):
                samples.append(
                    {
                        "phase": "idle",
                        "scenario_macro_cycle": cycle,
                        "runner_monotonic_us": (
                            cycle * MACRO_CYCLE_SECONDS + second
                        )
                        * 1_000_000,
                        "cpu_core_equivalents": 0.01,
                    }
                )
        self.assertEqual(summarize_idle_cpu(samples)["status"], "pass")
        for sample in samples:
            sample["cpu_core_equivalents"] = 1.0
        self.assertEqual(summarize_idle_cpu(samples)["status"], "fail")

    def test_unavailable_loop_classes_are_structured_blocked(self) -> None:
        scenarios = ScenarioAccumulator(COMMIT)
        for category in ("window", "modal", "drawer", "theme", "dpi"):
            scenarios.observe(
                f"G5_CAPABILITY schema=1 category={category} "
                "status=blocked reason=no_closed_loop_observer"
            )
        matrix = scenarios.loop_matrix()
        self.assertEqual(matrix["window"]["status"], "blocked")
        self.assertEqual(matrix["dpi"]["status"], "blocked")
        self.assertEqual(matrix["modal"]["status"], "blocked")
        gate_status, reason = determine_g5_gate_status(
            "performance",
            "pass",
            {"loop_matrix": matrix, "environment": {"status": "pass"}},
        )
        self.assertEqual(gate_status, "blocked")
        self.assertEqual(reason, "eight_hour_soak_not_run")

    def test_loop_pass_requires_unique_closed_baseline_proofs(self) -> None:
        scenarios = ScenarioAccumulator(COMMIT)
        scenarios.observe(
            "G5_SCENARIO schema=1 name=modal_feedback category=modal "
            "iteration=1000 page=7 theme=dark overlay=modal"
        )
        marker_only = scenarios.loop_matrix()
        self.assertEqual(marker_only["modal"]["status"], "blocked")

        ready = ScenarioAccumulator(COMMIT)
        ready.observe(
            "G5_CAPABILITY schema=1 category=modal status=ready reason=closed_observer"
        )
        ready.observe(
            "G5_SCENARIO schema=1 name=modal_feedback category=modal "
            "iteration=1000 page=7 theme=dark overlay=modal"
        )
        frames = []
        for iteration in range(1, 1001):
            ready.observe(
                "G5_LOOP schema=1 category=modal "
                f"iteration={iteration} status=pass final_state=stable baseline=pass "
                "open_presented=1 closed_presented=1 final_window_count=1 "
                "final_overlay_count=0 final_active_work=0 resource_delta=0 "
                f"open_window=1 open_frame_seq={iteration * 2 - 1} "
                f"closed_window=1 closed_frame_seq={iteration * 2}"
            )
            frames.extend(
                (
                    {
                        "window": 1,
                        "frame_seq": iteration * 2 - 1,
                        "scenario": f"modal_feedback.{iteration}",
                        "monotonic_us": WARMUP_US + iteration * 2 - 1,
                        "presented": 1,
                        "present_skipped": 0,
                    },
                    {
                        "window": 1,
                        "frame_seq": iteration * 2,
                        "scenario": f"modal_closed.{iteration}",
                        "monotonic_us": WARMUP_US + iteration * 2,
                        "presented": 1,
                        "present_skipped": 0,
                    },
                )
            )
        self_reported_only = ready.loop_matrix(
            {"modal_feedback": set(range(1, 1001))}
        )
        self.assertEqual(self_reported_only["modal"]["status"], "fail")
        ready.bind_loop_frames(frames)
        completed = ready.loop_matrix({"modal_feedback": set(range(1, 1001))})
        self.assertEqual(completed["modal"]["status"], "pass")
        self.assertEqual(completed["modal"]["completed_iterations"], 1000)

    def test_loop_frame_binding_rejects_reuse_wrong_state_and_reverse_order(self) -> None:
        def proof(
            scenarios: ScenarioAccumulator,
            iteration: int,
            open_sequence: int,
            closed_sequence: int,
        ) -> None:
            scenarios.observe(
                "G5_LOOP schema=1 category=modal "
                f"iteration={iteration} status=pass final_state=stable baseline=pass "
                "open_presented=1 closed_presented=1 final_window_count=1 "
                "final_overlay_count=0 final_active_work=0 resource_delta=0 "
                f"open_window=1 open_frame_seq={open_sequence} "
                f"closed_window=1 closed_frame_seq={closed_sequence}"
            )

        reused = ScenarioAccumulator(COMMIT)
        proof(reused, 1, 1, 2)
        proof(reused, 2, 1, 2)
        with self.assertRaisesRegex(G5Error, "is reused"):
            reused.bind_loop_frames([])

        wrong_state = ScenarioAccumulator(COMMIT)
        proof(wrong_state, 1, 1, 2)
        with self.assertRaisesRegex(G5Error, "expected scenario"):
            wrong_state.bind_loop_frames(
                [
                    {
                        "window": 1,
                        "frame_seq": 1,
                        "scenario": "page_home.1",
                        "monotonic_us": WARMUP_US + 1,
                        "presented": 1,
                        "present_skipped": 0,
                    },
                    {
                        "window": 1,
                        "frame_seq": 2,
                        "scenario": "modal_closed.1",
                        "monotonic_us": WARMUP_US + 2,
                        "presented": 1,
                        "present_skipped": 0,
                    },
                ]
            )

        reversed_order = ScenarioAccumulator(COMMIT)
        proof(reversed_order, 1, 1, 2)
        with self.assertRaisesRegex(G5Error, "ordered post-warmup pair"):
            reversed_order.bind_loop_frames(
                [
                    {
                        "window": 1,
                        "frame_seq": 1,
                        "scenario": "modal_feedback.1",
                        "monotonic_us": WARMUP_US + 2,
                        "presented": 1,
                        "present_skipped": 0,
                    },
                    {
                        "window": 1,
                        "frame_seq": 2,
                        "scenario": "modal_closed.1",
                        "monotonic_us": WARMUP_US + 1,
                        "presented": 1,
                        "present_skipped": 0,
                    },
                ]
            )

    def test_capability_pass_or_fail_cannot_bypass_closed_loops(self) -> None:
        for invalid in ("pass", "fail", "unknown"):
            scenarios = ScenarioAccumulator(COMMIT)
            scenarios.observe(
                f"G5_CAPABILITY schema=1 category=window status={invalid} reason=invalid"
            )
            matrix = scenarios.loop_matrix()
            self.assertEqual(matrix["window"]["status"], "fail")
            gate_status, _ = determine_g5_gate_status(
                "soak",
                "pass",
                {"loop_matrix": matrix, "environment": {"status": "pass"}},
            )
            self.assertEqual(gate_status, "fail")

    def test_frozen_environment_is_exact_and_capture_failures_block(self) -> None:
        environment = {
            "system": "Windows",
            "machine": "AMD64",
            "rustc": "rustc 1.0",
            "cargo": "cargo 1.0",
            "windows_cim": {
                "os": {"BuildNumber": "26200"},
                "cpu": {"Name": "AMD Ryzen 7 7700 8-Core Processor"},
                "computer": {"TotalPhysicalMemory": 32 * 1024**3},
                "gpu": [
                    {
                        "Name": "NVIDIA GeForce RTX 4070 Ti SUPER",
                        "DriverVersion": "32.0.16.1062",
                    }
                ],
            },
        }
        self.assertEqual(assess_frozen_environment(environment)["status"], "pass")
        environment["windows_cim"] = {"status": "blocked", "reason": "denied"}
        blocked = assess_frozen_environment(environment)
        self.assertEqual(blocked["status"], "blocked")
        self.assertIn("windows_cim_capture_blocked", blocked["reasons"])

    def test_soak_requires_all_48_interaction_idle_phase_pairs(self) -> None:
        scenarios = ScenarioAccumulator(COMMIT)
        for cycle in range(MACRO_CYCLES):
            interaction_us = cycle * MACRO_CYCLE_SECONDS * 1_000_000
            idle_us = interaction_us + (
                MACRO_CYCLE_SECONDS - MACRO_IDLE_SECONDS
            ) * 1_000_000
            scenarios.observe(
                "G5_PHASE schema=1 "
                f"macro_cycle={cycle} phase=interaction elapsed_us={interaction_us}"
            )
            scenarios.observe(
                "G5_PHASE schema=1 "
                f"macro_cycle={cycle} phase=idle elapsed_us={idle_us}"
            )
        self.assertEqual(scenarios.validate_macro_phases()["status"], "pass")

        missing = ScenarioAccumulator(COMMIT)
        missing.observe(
            "G5_PHASE schema=1 macro_cycle=0 phase=interaction elapsed_us=0"
        )
        with self.assertRaisesRegex(G5Error, "macro phases missing"):
            missing.validate_macro_phases()

        forged = ScenarioAccumulator(COMMIT)
        for cycle in range(MACRO_CYCLES):
            interaction_us = cycle * MACRO_CYCLE_SECONDS * 1_000_000
            idle_us = interaction_us + (
                MACRO_CYCLE_SECONDS - MACRO_IDLE_SECONDS
            ) * 1_000_000
            forged.observe(
                "G5_PHASE schema=1 "
                f"macro_cycle={cycle} phase=interaction elapsed_us={interaction_us}",
                observed_runner_us=0,
            )
            forged.observe(
                "G5_PHASE schema=1 "
                f"macro_cycle={cycle} phase=idle elapsed_us={idle_us}",
                observed_runner_us=0,
            )
        with self.assertRaisesRegex(G5Error, "timing drift"):
            forged.validate_macro_phases()

    def test_verify_returns_nonzero_for_integral_but_blocked_evidence(self) -> None:
        with TemporaryDirectory() as temporary:
            evidence = Path(temporary)
            write_blocked_evidence(evidence)
            self.assertEqual(main(["--verify", str(evidence)]), 2)


if __name__ == "__main__":
    unittest.main()
