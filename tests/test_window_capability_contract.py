import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CAPABILITY = ROOT / "src/platform/windowing/capability.rs"
SHARED = ROOT / "src/native/windowing/shared/window.rs"
BACKENDS = (
    ROOT / "src/native/backends/linux/windowing/wayland/window_ops.rs",
    ROOT / "src/native/backends/windows/window_ops.rs",
    ROOT / "src/native/backends/macos/windowing/window_ops.rs",
)
WAYLAND = BACKENDS[0]
WAYLAND_DIR = WAYLAND.parent
WAYLAND_RESIZE_CONSTRAINTS = WAYLAND_DIR / "resize_constraints.rs"


def braced_block(source: str, marker: str) -> str:
    start = source.index(marker)
    opening = source.index("{", start)
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
    raise AssertionError(f"unclosed block: {marker}")


class WindowCapabilityContractTests(unittest.TestCase):
    def test_capability_value_and_set_have_one_authoritative_definition(self) -> None:
        definitions = []
        for path in (ROOT / "src").rglob("*.rs"):
            source = path.read_text(encoding="utf-8")
            if "pub enum WindowCapability" in source or "pub struct WindowCapabilities" in source:
                definitions.append(path.relative_to(ROOT).as_posix())
        self.assertEqual(definitions, ["src/platform/windowing/capability.rs"])

        source = CAPABILITY.read_text(encoding="utf-8")
        enum_body = braced_block(source, "pub enum WindowCapability")
        values = re.findall(r"^\s{4}([A-Z][A-Za-z0-9]+),$", enum_body, re.MULTILINE)
        self.assertEqual(len(values), 33)
        self.assertEqual(len(values), len(set(values)))

    def test_three_backends_explicitly_declare_capabilities_and_all_window_ops(self) -> None:
        shared = SHARED.read_text(encoding="utf-8")
        trait_body = braced_block(shared, "trait WindowOps")
        trait_methods = set(re.findall(r"\bfn\s+(\w+)\s*\(", trait_body))
        self.assertNotIn("{", trait_body, "WindowOps must not retain default method bodies")
        self.assertGreaterEqual(len(trait_methods), 35)

        for path in BACKENDS:
            source = path.read_text(encoding="utf-8")
            self.assertIn("fn capabilities(&self) -> WindowCapabilities", source, path)
            impl_body = braced_block(source, "impl WindowOps for")
            impl_methods = set(re.findall(r"\bfn\s+(\w+)\s*\(", impl_body))
            self.assertEqual(impl_methods, trait_methods, path)

    def test_shared_core_gates_every_optional_action_before_adapter_dispatch(self) -> None:
        source = SHARED.read_text(encoding="utf-8")
        gated_values = {
            "RequestClose",
            "BeginMoveDrag",
            "BeginResizeDrag",
            "ShowSystemMenu",
            "CenterOnScreen",
            "Raise",
            "Lower",
            "SetWindowIcon",
            "FlashWindow",
            "ResizeNotify",
            "SetMinimumSize",
            "SetMaximumSize",
            "SetPosition",
            "SetResizable",
            "Maximize",
            "Minimize",
            "Restore",
            "ShowSystemTitleBar",
            "HideSystemTitleBar",
            "SetBorderless",
            "SetFullscreen",
            "SetAlwaysOnTop",
            "SetWindowOpacity",
            "StartTextInput",
            "StopTextInput",
            "EnableFileDrop",
            "DisableFileDrop",
            "RequestNativeFrame",
            "NativeFramePresented",
            "CancelNativeFrame",
        }
        for value in gated_values:
            self.assertIn(f"WindowCapability::{value}", source)
        self.assertGreaterEqual(source.count("require_capability("), 27)

    def test_wayland_set_resizable_capability_and_constraint_adapter_are_unique(self) -> None:
        source = WAYLAND.read_text(encoding="utf-8")
        start = source.index("const WAYLAND_WINDOW_CAPABILITIES")
        end = source.index("]);", start)
        capability_values = re.findall(
            r"WindowCapability::([A-Z][A-Za-z0-9]+)", source[start:end]
        )
        self.assertEqual(len(capability_values), 16)
        self.assertEqual(capability_values.count("SetResizable"), 1)
        self.assertNotIn(
            "WindowCapability::SetResizable.unsupported_error()", source
        )

        protocol_call_owners = set()
        for path in WAYLAND_DIR.rglob("*.rs"):
            candidate = path.read_text(encoding="utf-8")
            if ".set_min_size(" in candidate or ".set_max_size(" in candidate:
                protocol_call_owners.add(path.relative_to(ROOT).as_posix())
        self.assertEqual(
            protocol_call_owners,
            {WAYLAND_RESIZE_CONSTRAINTS.relative_to(ROOT).as_posix()},
        )

    def test_upper_layers_do_not_branch_on_operating_system(self) -> None:
        forbidden = re.compile(r"target_os|cfg!\(\s*(?:windows|unix)|std::os::|winapi|wayland|AppKit|Win32")
        offenders = []
        for area in ("app", "ui", "draw"):
            for path in (ROOT / "src" / area).rglob("*.rs"):
                if forbidden.search(path.read_text(encoding="utf-8")):
                    offenders.append(path.relative_to(ROOT).as_posix())
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
