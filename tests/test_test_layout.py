# -*- coding: utf-8 -*-
"""Guard the repository convention that standalone tests live under tests/."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TESTS_ROOT = ROOT / "tests"
IGNORED_PARTS = {".git", "target"}


def repository_files(pattern: str) -> list[Path]:
    return [
        path
        for path in ROOT.rglob(pattern)
        if not IGNORED_PARTS.intersection(path.parts)
    ]


class TestLayoutConvention(unittest.TestCase):
    def test_standalone_python_tests_are_under_tests(self) -> None:
        outside = sorted(
            path.relative_to(ROOT).as_posix()
            for path in repository_files("test_*.py")
            if TESTS_ROOT not in path.parents
        )
        self.assertEqual(outside, [])

    def test_standalone_rust_test_sources_are_under_tests(self) -> None:
        outside = sorted(
            path.relative_to(ROOT).as_posix()
            for path in repository_files("test_*.rs")
            if TESTS_ROOT not in path.parents
            and path.as_posix() != (ROOT / "src/draw/renderer/test_harness.rs").as_posix()
        )
        self.assertEqual(outside, [])

    def test_gfx_r5_integration_entrypoint_is_in_tests(self) -> None:
        self.assertTrue((TESTS_ROOT / "tests.rs").is_file())


if __name__ == "__main__":
    unittest.main()
