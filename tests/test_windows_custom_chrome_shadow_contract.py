# -*- coding: utf-8 -*-
"""冻结 Windows 自绘标题栏仍保留 DWM 阴影的共享窗口契约。"""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CONSTS = ROOT / "src" / "native" / "backends" / "windows" / "consts.rs"
CHROME = ROOT / "src" / "native" / "backends" / "windows" / "custom_chrome.rs"
WND_PROC = ROOT / "src" / "native" / "backends" / "windows" / "wnd_proc.rs"


class WindowsCustomChromeShadowContractTests(unittest.TestCase):
    # 首次激活会重置部分 DWM 帧状态，必须在同一原生消息边界重新提交。
    def test_activation_reapplies_dwm_frame_effects(self) -> None:
        consts = CONSTS.read_text(encoding="utf-8")
        wnd_proc = WND_PROC.read_text(encoding="utf-8")

        self.assertIn("WM_ACTIVATE: u32 = 0x0006", consts)
        activation = wnd_proc[wnd_proc.index("WM_ACTIVATE =>"):]
        activation = activation[: activation.index("WM_NCCALCSIZE =>")]
        self.assertIn("apply_dwm_frame_effects", activation)
        self.assertIn("is_effectively_maximized", activation)
        self.assertIn("if !state_minimized", activation)
        self.assertIn("def_window_proc", activation)

    # 去掉 WS_CAPTION 后不能再让 DWM 只依赖窗口样式判断是否绘制非客户区。
    def test_extended_client_forces_non_client_rendering_policy(self) -> None:
        chrome = CHROME.read_text(encoding="utf-8")

        self.assertIn("DWMWA_NCRENDERING_POLICY", chrome)
        self.assertIn("set_nc_rendering_policy(handle, DWMNCRP_ENABLED)", chrome)
        self.assertIn("set_nc_rendering_policy(handle, DWMNCRP_USEWINDOWSTYLE)", chrome)

    # 最小化的 0x0 只是过渡值，不得覆盖最后有效客户区或重配 DWM 帧。
    def test_minimize_preserves_last_visible_extent_and_dwm_state(self) -> None:
        wnd_proc = WND_PROC.read_text(encoding="utf-8")

        size = wnd_proc[wnd_proc.index("WM_SIZE =>") :]
        size = size[: size.index("WM_DPICHANGED =>")]
        self.assertIn("let chrome_style = if wparam == SIZE_MINIMIZED", size)
        minimized = size[size.index("SIZE_MINIMIZED =>") : size.index("SIZE_MAXIMIZED =>")]
        self.assertIn("state.minimized = true", minimized)
        self.assertNotIn("state.width = w", minimized)
        self.assertNotIn("state.height = h", minimized)
        self.assertNotIn("apply_dwm_frame_effects", minimized)


if __name__ == "__main__":
    unittest.main()
