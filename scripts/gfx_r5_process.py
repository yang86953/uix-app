"""Bounded child-process cleanup for the Windows GFX-R5 runner."""
from __future__ import annotations

import os
import subprocess

PROCESS_TREE_STOP_TIMEOUT_SECONDS = 5.0


def terminate_process_tree(process: subprocess.Popen[str]) -> bool:
    """Stop the spawned cargo process tree without blocking the runner forever."""
    if process.poll() is not None:
        return True

    tree_stop_confirmed = True
    if os.name == "nt":
        try:
            result = subprocess.run(
                ("taskkill", "/PID", str(process.pid), "/T", "/F"),
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=PROCESS_TREE_STOP_TIMEOUT_SECONDS,
            )
            tree_stop_confirmed = result.returncode == 0
        except (OSError, subprocess.TimeoutExpired):
            tree_stop_confirmed = False
    else:
        process.terminate()

    try:
        process.wait(timeout=PROCESS_TREE_STOP_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        tree_stop_confirmed = False
        process.kill()
        try:
            process.wait(timeout=PROCESS_TREE_STOP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            return False
    return tree_stop_confirmed
