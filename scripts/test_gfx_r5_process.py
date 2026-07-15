import subprocess
import unittest
from unittest.mock import Mock, patch

from scripts.gfx_r5_process import (
    PROCESS_TREE_STOP_TIMEOUT_SECONDS,
    terminate_process_tree,
)


class GfxR5ProcessTests(unittest.TestCase):
    def test_already_exited_process_needs_no_cleanup(self) -> None:
        process = Mock()
        process.poll.return_value = 0

        with patch("scripts.gfx_r5_process.subprocess.run") as run:
            self.assertTrue(terminate_process_tree(process))

        run.assert_not_called()
        process.terminate.assert_not_called()
        process.kill.assert_not_called()

    def test_windows_cleanup_terminates_the_entire_process_tree(self) -> None:
        process = Mock(pid=4242)
        process.poll.return_value = None
        process.wait.return_value = 1
        completed = Mock(returncode=0)

        with (
            patch("scripts.gfx_r5_process.os.name", "nt"),
            patch("scripts.gfx_r5_process.subprocess.run", return_value=completed) as run,
        ):
            self.assertTrue(terminate_process_tree(process))

        run.assert_called_once_with(
            ("taskkill", "/PID", "4242", "/T", "/F"),
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            timeout=PROCESS_TREE_STOP_TIMEOUT_SECONDS,
        )
        process.wait.assert_called_once_with(timeout=PROCESS_TREE_STOP_TIMEOUT_SECONDS)
        process.kill.assert_not_called()

    def test_cleanup_falls_back_to_kill_without_claiming_tree_success(self) -> None:
        process = Mock(pid=4242)
        process.poll.return_value = None
        process.wait.side_effect = [
            subprocess.TimeoutExpired("cargo", PROCESS_TREE_STOP_TIMEOUT_SECONDS),
            1,
        ]

        with (
            patch("scripts.gfx_r5_process.os.name", "nt"),
            patch(
                "scripts.gfx_r5_process.subprocess.run",
                side_effect=OSError("taskkill unavailable"),
            ),
        ):
            self.assertFalse(terminate_process_tree(process))

        process.kill.assert_called_once_with()
        self.assertEqual(process.wait.call_count, 2)


if __name__ == "__main__":
    unittest.main()
