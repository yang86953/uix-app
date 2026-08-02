# -*- coding: utf-8 -*-
"""Contract tests for the release G5 evidence parser."""
from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER_PATH = ROOT / "scripts" / "run_release_g5.py"
SPEC = importlib.util.spec_from_file_location("run_release_g5", RUNNER_PATH)
if SPEC is None or SPEC.loader is None:  # pragma: no cover - import guard
    raise RuntimeError(f"cannot load {RUNNER_PATH}")
G5 = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = G5
SPEC.loader.exec_module(G5)


def frame_line(
    *, frame_seq: int, scenario: str, monotonic_us: int
) -> str:
    return (
        "G5_FRAME schema=1 "
        f"frame_seq={frame_seq} window=1 logical_width=1200 logical_height=800 "
        f"dpi=96 monotonic_us={monotonic_us} presented=1 present_skipped=0 "
        "active_work=1 due_active_work=1 frame_us=100 present_us=50 "
        "drawable_width=1200 drawable_height=800 drawable_pixels=960000 "
        f"scenario={scenario} wgpu_surface_present_cpu_us=5"
    )


class ReleaseG5ContractTests(unittest.TestCase):
    def test_backend_evidence_accepts_production_vulkan_markers(self) -> None:
        evidence = G5.BackendEvidence()
        evidence.observe(
            "Graphics bootstrap: selected recipe backend=vulkan; "
            "raster=gpu_native; present=swapchain"
        )
        evidence.observe(
            'WgpuContext: backend=vulkan; adapter="NVIDIA GeForce RTX 4070 Ti SUPER"; '
            "type=DiscreteGpu; driver=\"NVIDIA\""
        )

        result = evidence.validate()

        self.assertEqual(result["status"], "pass")
        self.assertFalse(result["fallback_observed"])

    def test_backend_evidence_rejects_software_fallback(self) -> None:
        evidence = G5.BackendEvidence()
        evidence.observe(
            "Graphics bootstrap: selected recipe backend=vulkan; "
            "raster=gpu_native; present=swapchain"
        )
        evidence.observe(
            'WgpuContext: backend=vulkan; adapter="NVIDIA GeForce RTX 4070 Ti SUPER"; '
            "type=DiscreteGpu"
        )
        evidence.observe("fallback=software_cpu")

        with self.assertRaisesRegex(G5.G5Error, "graphics fallback"):
            evidence.validate()

    def test_frame_parser_enforces_frozen_release_geometry(self) -> None:
        parsed = G5.parse_g5_frame(
            frame_line(
                frame_seq=1,
                scenario="page_home.0",
                monotonic_us=G5.WARMUP_US + 1,
            )
        )

        self.assertIsNotNone(parsed)
        assert parsed is not None
        self.assertEqual(parsed["drawable_pixels"], 960000)
        self.assertEqual(parsed["scenario"], "page_home.0")

        with self.assertRaisesRegex(G5.G5Error, "frozen 1200x800"):
            G5.parse_g5_frame(
                frame_line(
                    frame_seq=2,
                    scenario="page_home.0",
                    monotonic_us=G5.WARMUP_US + 2,
                ).replace("drawable_width=1200", "drawable_width=1199")
            )

    def test_loop_proof_binds_two_unique_presented_frames(self) -> None:
        accumulator = G5.ScenarioAccumulator("a" * 40)
        accumulator.observe(
            "G5_LOOP schema=1 category=theme iteration=1 status=pass "
            "final_state=stable baseline=pass open_presented=1 closed_presented=1 "
            "final_window_count=1 final_overlay_count=0 final_active_work=0 "
            "resource_delta=0 open_window=1 open_frame_seq=1 "
            "closed_window=1 closed_frame_seq=2"
        )
        frames = [
            G5.parse_g5_frame(
                frame_line(
                    frame_seq=1,
                    scenario="page_general_dark.1",
                    monotonic_us=G5.WARMUP_US + 1,
                )
            ),
            G5.parse_g5_frame(
                frame_line(
                    frame_seq=2,
                    scenario="page_general_light.1",
                    monotonic_us=G5.WARMUP_US + 2,
                )
            ),
        ]

        accumulator.bind_loop_frames(frame for frame in frames if frame is not None)

        self.assertEqual(accumulator.validated_iterations["theme"], {1})

    def test_post_present_resource_observer_records_closed_state(self) -> None:
        accumulator = G5.ScenarioAccumulator("a" * 40)

        opened = accumulator.observe(
            "G5_RESOURCE schema=1 phase=post_present category=modal iteration=1 "
            "state=open window=1 frame_seq=10 live_nodes=20 tree_slots=24 "
            "overlay_count=1 active_work=1"
        )
        closed = accumulator.observe(
            "G5_RESOURCE schema=1 phase=post_present category=modal iteration=1 "
            "state=closed window=1 frame_seq=11 live_nodes=20 tree_slots=24 "
            "overlay_count=0 active_work=1"
        )

        self.assertEqual(opened["kind"], "resource")
        self.assertEqual(closed["state"], "closed")
        self.assertEqual(closed["live_nodes"], 20)
        self.assertEqual(closed["overlay_count"], 0)

    def test_missing_loop_capability_remains_blocked(self) -> None:
        accumulator = G5.ScenarioAccumulator("a" * 40)

        result = accumulator.loop_matrix()

        self.assertEqual(result["theme"]["status"], "blocked")
        self.assertEqual(
            result["theme"]["reason"],
            "missing_closed_loop_capability_evidence",
        )

    def test_ready_loop_capability_still_fails_without_bound_proofs(self) -> None:
        accumulator = G5.ScenarioAccumulator("a" * 40)
        accumulator.observe(
            "G5_CAPABILITY schema=1 category=theme status=ready reason=observer_ready"
        )
        accumulator.observe(
            "G5_SCENARIO schema=1 name=page_general_dark.1 category=theme "
            "iteration=1 page=1 theme=dark overlay=none"
        )
        accumulator.bind_loop_frames([])

        result = accumulator.loop_matrix()

        self.assertEqual(result["theme"]["status"], "fail")
        self.assertEqual(
            result["theme"]["reason"],
            "missing_frame_bound_closed_loop_proofs",
        )

    def test_loop_proof_rejects_nonzero_resource_delta(self) -> None:
        accumulator = G5.ScenarioAccumulator("a" * 40)

        with self.assertRaisesRegex(
            G5.G5Error, "successful closed-loop baseline proof"
        ):
            accumulator.observe(
                "G5_LOOP schema=1 category=theme iteration=1 status=pass "
                "final_state=stable baseline=pass open_presented=1 closed_presented=1 "
                "final_window_count=1 final_overlay_count=0 final_active_work=0 "
                "resource_delta=1 open_window=1 open_frame_seq=1 "
                "closed_window=1 closed_frame_seq=2"
            )


if __name__ == "__main__":
    unittest.main()
