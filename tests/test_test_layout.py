# -*- coding: utf-8 -*-
"""Guard the repository convention that standalone tests live under tests/."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TESTS_ROOT = ROOT / "tests"
IGNORED_PARTS = {".git", "target"}
PYTHON_TEST_PATTERNS = ("test_*.py", "*_test.py", "*_tests.py")
RUST_TEST_PATTERNS = ("test_*.rs", "*_test.rs", "*_tests.rs")


def repository_files(pattern: str) -> list[Path]:
    return [
        path
        for path in ROOT.rglob(pattern)
        if not IGNORED_PARTS.intersection(path.parts)
    ]


def outside_tests(patterns: tuple[str, ...]) -> list[str]:
    return sorted(
        path.relative_to(ROOT).as_posix()
        for pattern in patterns
        for path in repository_files(pattern)
        if TESTS_ROOT not in path.parents
    )


class TestLayoutConvention(unittest.TestCase):
    def test_standalone_python_tests_are_under_tests(self) -> None:
        outside = outside_tests(PYTHON_TEST_PATTERNS)
        self.assertEqual(outside, [])

    def test_standalone_rust_test_sources_are_under_tests(self) -> None:
        allowed_support = (ROOT / "src/draw/renderer/test_harness.rs").resolve()
        outside = [
            relative
            for relative in outside_tests(RUST_TEST_PATTERNS)
            if (ROOT / relative).resolve() != allowed_support
        ]
        self.assertEqual(outside, [])

    def test_gfx_r5_integration_entrypoint_is_in_tests(self) -> None:
        self.assertTrue((TESTS_ROOT / "tests.rs").is_file())


if __name__ == "__main__":
    unittest.main()
