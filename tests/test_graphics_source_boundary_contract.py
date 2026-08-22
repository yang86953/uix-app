"""图形生产源码边界、共享规范与生命周期的递归架构门禁。"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
UPPER_ROOTS = (SRC / "ui", SRC / "draw", SRC / "app")
API_ADAPTER_ROOTS = {
    "vulkan": SRC / "native/presentation/graphics/vulkan",
    "d3d11": SRC / "native/presentation/graphics/d3d11",
    "opengl": SRC / "native/presentation/graphics/opengl",
}

TARGET_CFG = re.compile(
    r"(?:#\s*\[\s*cfg|cfg!)\s*\([^\n]*"
    r"(?:windows|unix|target_os|target_family|target_vendor|target_env)"
)
UPPER_API = re.compile(
    r"(?i)(?<![-\w])(?:vulkan|d3d11|dxgi|opengl|egl|wgl)(?![-\w])"
    r"|\bash::|\bvk::|\bglow::|\bkhronos_egl::|\bwindows(?:_sys)?::"
)
UPPER_CONCRETE_MODULE = re.compile(
    r"(?i)(?<![A-Za-z0-9_])(?:linux|windows|macos|vulkan|d3d11|dx11|opengl)::"
)
UPPER_BACKEND_SELECTION = re.compile(
    r"\b(?:GraphicsApi|GraphicsBackend)::"
    r"(?:D3d11|Direct3D11|D3d12|Direct3D12|Vulkan|Metal|OpenGlEs)\b"
)
UPPER_OS_API_PATH = re.compile(
    r"(?i)(?:^|[_-])(?:linux|windows|macos|vulkan|d3d11|dx11|opengl)(?:[_-]|$)"
)
PRIVATE_SURFACE_STATE = re.compile(
    r"\b(?:surface_generation|recreate_generation|recreate_pending|needs_recreate)\b"
)
NATIVE_REFERENCE = re.compile(r"\bcrate::native\b|(?<!crate::)\bnative::")

# Agent transport 的中立合同与 OS adapter 整体归属 platform，native 不保留兼容路径。
AGENT_TRANSPORT_ROOT = SRC / "platform/adapters/transport"
AGENT_TRANSPORT_CONTRACT = AGENT_TRANSPORT_ROOT / "mod.rs"
LEGACY_AGENT_TRANSPORT_ROOT = SRC / "native/agent_transport"
AGENT_TRANSPORT_APP = SRC / "app/agent/agent_transport.rs"
AGENT_TRANSPORT_DEFINITIONS = (
    ("trait", "AgentStreamIo"),
    ("type", "AgentStream"),
    ("trait", "AgentStreamCancelIo"),
    ("struct", "AcceptedAgentStream"),
)
AGENT_TRANSPORT_ADAPTERS = (
    AGENT_TRANSPORT_ROOT / "unsupported.rs",
    AGENT_TRANSPORT_ROOT / "unix.rs",
    AGENT_TRANSPORT_ROOT / "windows.rs",
)
# 平台启动输入合同不依赖具体实现；唯一组合根叶负责目标选择与对象所有权。
PLATFORM_COMPOSITION_CONTRACT = SRC / "platform/composition.rs"
PLATFORM_COMPOSITION_ROOT = SRC / "platform/composition_root.rs"
# platform 到 concrete native 的源码依赖只允许这一处，禁止新增局部 allowlist。
PLATFORM_NATIVE_DEPENDENCY_ALLOWLIST = frozenset({PLATFORM_COMPOSITION_ROOT})
# Platform 根合同的唯一中立物理归属；native 不保留兼容文件。
PLATFORM_ROOT = SRC / "platform/platform.rs"
LEGACY_PLATFORM_ROOT = SRC / "native/platform.rs"
# 三平台、fake 与应用测试桩必须直接实现同一个根合同。
PLATFORM_ROOT_IMPLEMENTERS = (
    SRC / "native/backends/linux/platform.rs",
    SRC / "native/backends/windows/platform.rs",
    SRC / "native/backends/macos/host/mod.rs",
    ROOT / "tests/support/native/test_harness/mod.rs",
    ROOT / "tests/unit/app/application/application/runtime/mod__tests.rs",
)
# 原有直接调用者必须消费中立根合同，禁止经 native 兼容路径取得。
PLATFORM_ROOT_CONSUMERS = (
    SRC / "app/application/application/mod.rs",
    SRC / "app/event_loop/event_loop.rs",
    SRC / "app/event_loop/pointer_cursor.rs",
    SRC / "app/window/text_input.rs",
    SRC / "app/window/window.rs",
    SRC / "app/window/window_driver/mod.rs",
    PLATFORM_COMPOSITION_ROOT,
    SRC / "platform/host/providers/linux.rs",
    SRC / "platform/host/providers/macos.rs",
    *PLATFORM_ROOT_IMPLEMENTERS,
)
# 固定无附属值类型的基础系统服务协议唯一物理归属。
SYSTEM_SERVICE_ROOT = SRC / "platform/system.rs"
SYSTEM_SERVICE_DEFINITIONS = (
    "pub(crate) trait IFileDialog",
    "pub(crate) trait INotification",
    "pub(crate) trait ITimer",
)
# Linux、Windows、macOS 与 fake 必须直接实现同一个 platform 合同。
SYSTEM_SERVICE_IMPLEMENTERS = {
    SRC / "native/backends/linux/file_dialog.rs": ("impl IFileDialog for LinuxFileDialog",),
    SRC / "native/backends/linux/notification.rs": (
        "impl INotification for LinuxNotification",
    ),
    SRC / "native/backends/linux/timer.rs": ("impl ITimer for LinuxTimer",),
    SRC / "native/backends/windows/file_dialog.rs": (
        "impl IFileDialog for WindowsFileDialog",
    ),
    SRC / "native/backends/windows/notification.rs": (
        "impl INotification for WindowsNotification",
    ),
    SRC / "native/backends/windows/timer.rs": ("impl ITimer for WindowsTimer",),
    SRC / "native/backends/macos/host/file_dialog.rs": (
        "impl IFileDialog for MacosFileDialog",
    ),
    SRC / "native/backends/macos/host/services2.rs": (
        "impl INotification for MacosNotification",
    ),
    SRC / "native/backends/macos/host/services.rs": ("impl ITimer for MacosTimer",),
    ROOT / "tests/support/native/test_harness/fake_file_dialog.rs": (
        "impl IFileDialog for FakeFileDialog",
    ),
    ROOT / "tests/support/native/test_harness/fake_notification.rs": (
        "impl INotification for FakeNotification",
    ),
    ROOT / "tests/support/native/test_harness/fake_timer.rs": ("impl ITimer for FakeTimer",),
}
# Platform 根及三平台/fake 聚合均需显式消费新权威路径。
SYSTEM_SERVICE_PLATFORM_ROOTS = (
    PLATFORM_ROOT,
    SRC / "native/backends/linux/platform.rs",
    SRC / "native/backends/windows/platform.rs",
    SRC / "native/backends/macos/host/mod.rs",
    ROOT / "tests/support/native/test_harness/mod.rs",
)
# 固定平台中立文件系统值、端口与共享核心的唯一物理归属。
FILESYSTEM_ROOT = SRC / "platform/system/filesystem.rs"
# native 不再保留文件系统合同或共享核心兼容模块。
LEGACY_FILESYSTEM_ROOTS = (
    SRC / "native/capabilities/system.rs",
    SRC / "native/capabilities/services/filesystem.rs",
)
# 文件系统定义只能由 system/filesystem 叶持有。
FILESYSTEM_DEFINITIONS = (
    ("pub enum SpecialDir", r"\benum\s+SpecialDir\b"),
    ("pub(crate) trait IFileSystem", r"\btrait\s+IFileSystem\b"),
    ("pub(crate) trait SpecialDirProvider", r"\btrait\s+SpecialDirProvider\b"),
    ("pub(crate) struct FileSystemCore", r"\bstruct\s+FileSystemCore\b"),
)
# 三平台特殊目录实现与 fake 必须直接绑定同一个 platform 叶。
FILESYSTEM_IMPLEMENTERS = {
    SRC / "native/backends/linux/filesystem.rs": (
        "impl SpecialDirProvider for LinuxSpecialDirs",
    ),
    SRC / "native/backends/windows/filesystem.rs": (
        "impl SpecialDirProvider for WindowsSpecialDirs",
    ),
    SRC / "native/backends/macos/host/services.rs": (
        "impl SpecialDirProvider for MacosSpecialDirs",
    ),
    ROOT / "tests/support/native/test_harness/fake_file_system.rs": (
        "impl IFileSystem for FakeFileSystem",
    ),
}
# Platform 根、三平台聚合、fake 与应用测试桩均直接消费唯一低层端口。
FILESYSTEM_PLATFORM_ROOTS = (
    PLATFORM_ROOT,
    SRC / "native/backends/linux/platform.rs",
    SRC / "native/backends/windows/platform.rs",
    SRC / "native/backends/macos/host/mod.rs",
    ROOT / "tests/support/native/test_harness/mod.rs",
    ROOT / "tests/unit/app/application/application/runtime/mod__tests.rs",
)
# 固定平台中立控制台值与端口的唯一物理归属。
CONSOLE_ROOT = SRC / "platform/system/console.rs"
# 控制台合同只允许由 system/console 叶定义。
CONSOLE_DEFINITIONS = (
    ("enum", "ConsoleColor"),
    ("struct", "TerminalCapabilities"),
    ("trait", "IConsole"),
)
# Linux、Windows、macOS 与 fake 必须直接实现同一个控制台合同。
CONSOLE_IMPLEMENTERS = {
    SRC / "native/backends/linux/console.rs": "impl IConsole for LinuxConsole",
    SRC / "native/backends/windows/console.rs": "impl IConsole for WindowsConsole",
    SRC / "native/backends/macos/host/services2.rs": "impl IConsole for MacosConsole",
    ROOT / "tests/support/native/test_harness/fake_console.rs": "impl IConsole for FakeConsole",
}
# Platform 根及三平台/fake 聚合均需显式消费控制台权威路径。
CONSOLE_PLATFORM_ROOTS = (
    PLATFORM_ROOT,
    SRC / "native/backends/linux/platform.rs",
    SRC / "native/backends/windows/platform.rs",
    SRC / "native/backends/macos/host/mod.rs",
    ROOT / "tests/support/native/test_harness/mod.rs",
)
# 固定 crate 私有系统信息值与端口的唯一物理归属。
SYSTEM_INFO_ROOT = SRC / "platform/system/info.rs"
# 字体发现是 Drawing 消费的高层 host 服务协议，继续由 services 叶唯一持有。
FONT_SYSTEM_INFO_ROOT = SRC / "platform/host/services.rs"
# crate 私有系统信息合同只允许由 system/info 叶定义。
SYSTEM_INFO_DEFINITIONS = (
    ("struct", "MemoryInfo"),
    ("struct", "OsInfo"),
    ("trait", "ISystemInfo"),
)
# Linux、Windows、macOS 与 fake 必须直接实现同一个 platform 合同。
SYSTEM_INFO_IMPLEMENTERS = {
    SRC / "native/backends/linux/host/system_info/mod.rs": (
        "impl ISystemInfo for LinuxSystemInfo"
    ),
    SRC / "native/backends/windows/host/system_info/info.rs": (
        "impl ISystemInfo for WindowsSystemInfo"
    ),
    SRC / "native/backends/macos/host/services2.rs": (
        "impl ISystemInfo for MacosSystemInfo"
    ),
    ROOT / "tests/support/native/test_harness/fake_system_info.rs": (
        "impl ISystemInfo for FakeSystemInfo"
    ),
}
# Platform 根、三平台聚合与 fake 均需显式消费新权威路径。
SYSTEM_INFO_PLATFORM_ROOTS = (
    PLATFORM_ROOT,
    SRC / "native/backends/linux/platform.rs",
    SRC / "native/backends/windows/platform.rs",
    SRC / "native/backends/macos/host/mod.rs",
    ROOT / "tests/support/native/test_harness/mod.rs",
)

# 固定平台无关事件协议的唯一物理归属。
EVENT_ROOT = SRC / "platform/windowing/event"
# native 不再保留事件定义或兼容模块。
LEGACY_EVENT_ROOT = SRC / "native/windowing/event"
# 固定平台中立窗口协议的唯一物理归属。
WINDOW_ROOT = SRC / "platform/windowing/window.rs"
# native 不再保留窗口协议定义或兼容模块。
LEGACY_WINDOW_ROOT = SRC / "native/windowing/window.rs"
# 每个 marker 锁定窗口协议的定义种类与名称。
WINDOW_DEFINITIONS = (
    "pub enum NativeFrameRequestPhase",
    "pub struct NativeFrameRequest",
    "pub enum WindowOcclusionState",
    "pub trait IWindowProperties",
    "pub trait INativeHandle",
    "pub trait IWindowManager",
    "pub trait PlatformWindow",
)
# 三个平台工厂、共享窗口核心与 fake 必须直接消费同一个中立合同。
WINDOW_IMPLEMENTERS = {
    SRC / "native/backends/windows/platform.rs": "impl IWindowManager for WindowsPlatform",
    SRC / "native/backends/macos/host/mod.rs": "impl IWindowManager for MacosPlatform",
    SRC / "native/backends/linux/windowing/wayland/window.rs": (
        "impl IWindowManager for WaylandBackend"
    ),
    SRC / "native/windowing/shared/window.rs": (
        "impl<O: WindowOps> PlatformWindow for PlatformWindowCore<O>"
    ),
    ROOT / "tests/support/native/test_harness/fake_window.rs": (
        "impl IWindowManager for FakeWindowManager"
    ),
}
# 固定平台中立输入服务合同的唯一物理归属。
INPUT_ROOT = SRC / "platform/windowing/input.rs"
# native 不再保留输入协议定义或兼容模块。
LEGACY_INPUT_ROOT = SRC / "native/windowing/input.rs"
# 三项输入合同只能由 platform 叶模块定义。
INPUT_DEFINITIONS = (
    "pub(crate) trait ICursor",
    "pub(crate) trait IKeyboard",
    "pub(crate) trait ITextInput",
)
# 三个平台与 fake 的真实实现必须直接消费 platform 权威合同。
INPUT_IMPLEMENTERS = {
    SRC / "native/backends/windows/cursor.rs": ("impl ICursor for WindowsCursor",),
    SRC / "native/backends/windows/keyboard.rs": ("impl IKeyboard for WindowsKeyboard",),
    SRC / "native/backends/windows/text_input.rs": (
        "impl ITextInput for WindowsTextInput",
    ),
    SRC / "native/backends/macos/host/services.rs": (
        "impl crate::platform::windowing::ICursor for MacosCursor",
        "impl crate::platform::windowing::IKeyboard for MacosKeyboard",
    ),
    SRC / "native/backends/macos/text_input_view.rs": (
        "impl crate::platform::windowing::ITextInput for MacosTextInput",
    ),
    SRC / "native/backends/linux/windowing/wayland/cursor.rs": (
        "impl ICursor for WaylandBackend",
    ),
    SRC / "native/backends/linux/windowing/wayland/keyboard.rs": (
        "impl IKeyboard for WaylandBackend",
    ),
    SRC / "native/backends/linux/windowing/wayland/text_input.rs": (
        "impl ITextInput for WaylandBackend",
    ),
    ROOT / "tests/support/native/test_harness/fake_cursor.rs": (
        "impl ICursor for FakeCursor",
    ),
    ROOT / "tests/support/native/test_harness/fake_keyboard.rs": (
        "impl IKeyboard for FakeKeyboard",
    ),
    ROOT / "tests/support/native/test_harness/fake_text_input.rs": (
        "impl ITextInput for FakeTextInput",
    ),
}
# 固定平台中立显示协议的唯一物理归属。
DISPLAY_ROOT = SRC / "platform/display.rs"
# native 不再保留显示协议定义或兼容模块。
LEGACY_DISPLAY_ROOT = SRC / "native/capabilities/display.rs"
# 显示能力值与端口只能由 platform 叶定义。
DISPLAY_DEFINITIONS = (
    "pub(crate) struct DisplayInfo",
    "pub(crate) trait IDisplay",
)
# 三个平台与 fake 必须直接消费同一个 platform 显示合同。
DISPLAY_IMPLEMENTERS = {
    SRC / "native/backends/windows/display.rs": "impl IDisplay for WindowsDisplay",
    SRC / "native/backends/macos/host/services.rs": (
        "impl crate::platform::display::IDisplay for MacosDisplay"
    ),
    SRC / "native/backends/linux/windowing/wayland/display.rs": (
        "impl IDisplay for WaylandBackend"
    ),
    ROOT / "tests/support/native/test_harness/fake_display.rs": (
        "impl IDisplay for FakeDisplay"
    ),
}
# 固定平台中立 CPU presenter 合同的唯一物理归属。
PRESENTER_ROOT = SRC / "platform/presentation/presenter.rs"
# 全部中立呈现合同的 platform 物理归属与已删除 native 旧目录。
PRESENTATION_CONTRACT_ROOT = SRC / "platform/presentation/contracts/mod.rs"
LEGACY_PRESENTATION_CONTRACT_ROOT = SRC / "native/presentation/contracts"
PRESENTER_DEFINITION = re.compile(r"\btrait\s+IPresenter\b")
LEGACY_PRESENTER_REFERENCE = re.compile(
    r"crate::native::present(?:::IPresenter|::\{[^}]*\bIPresenter\b)", re.DOTALL
)
# 每个 marker 同时约束权威文件与定义种类，防止复制协议或转换层。
EVENT_DEFINITIONS = {
    EVENT_ROOT / "mod.rs": (
        "pub struct EventLoopWaker",
        "pub(crate) trait IEventLoop",
    ),
    EVENT_ROOT / "bus.rs": (
        "pub(crate) type EventHandler",
        "struct SubscriberEntry",
        "pub(crate) struct EventBus",
    ),
    EVENT_ROOT / "types.rs": (
        "pub enum UiEventType",
        "pub struct KeyEventData",
        "pub struct PointerActivationId",
        "pub struct PointerButtonEventData",
        "pub struct PointerMoveEventData",
        "pub struct WheelData",
        "pub struct ResizeData",
        "pub struct TimerEventData",
        "pub struct TextInputData",
        "pub struct ImeCompositionData",
        "pub struct ClipboardData",
        "pub struct FileDropData",
        "pub struct ThemeChangeData",
        "pub struct LocaleChangeData",
        "pub struct FrameRequestToken",
        "pub struct FrameOpportunityData",
        "pub enum UiEventPayload",
        "pub struct UiEvent",
    ),
}

PIPELINE_KINDS = (
    "SolidMesh",
    "TexturedQuad",
    "GradientRect",
    "GlyphCoverageQuad",
    "ShapeRect",
    "ShapeRectAdditive",
    "BoxShadow",
    "TexturedQuadAdditive",
    "BlurPass",
    "MsdfGlyphQuad",
    "Sector",
)

# 图形共同机制的生产定义必须各自只有一个物理权威。
GRAPHICS_AUTHORITY_DEFINITIONS = (
    (
        "PipelineKind",
        re.compile(r"\benum\s+PipelineKind\b"),
        SRC / "platform/presentation/rhi/pipeline.rs",
    ),
    (
        "GraphicsDevice",
        re.compile(r"\btrait\s+GraphicsDevice\b"),
        SRC / "platform/presentation/rhi/mod.rs",
    ),
    (
        "PresentTestResult",
        re.compile(r"\benum\s+PresentTestResult\b"),
        PRESENTATION_CONTRACT_ROOT,
    ),
    (
        "GraphicsContextCaps",
        re.compile(r"\bstruct\s+GraphicsContextCaps\b"),
        PRESENTATION_CONTRACT_ROOT,
    ),
    (
        "NativeSurfaceHandle",
        re.compile(r"\bstruct\s+NativeSurfaceHandle\b"),
        PRESENTATION_CONTRACT_ROOT,
    ),
    (
        "GraphicsSurface",
        re.compile(r"\btrait\s+GraphicsSurface\b"),
        SRC / "platform/presentation/rhi/mod.rs",
    ),
    (
        "RhiSurfaceLifecycle",
        re.compile(r"\bstruct\s+RhiSurfaceLifecycle\b"),
        SRC / "platform/presentation/rhi/surface_lifecycle.rs",
    ),
    (
        "RhiSurfaceRecreateReason",
        re.compile(r"\benum\s+RhiSurfaceRecreateReason\b"),
        SRC / "platform/presentation/rhi/surface_lifecycle.rs",
    ),
    (
        "RhiPresentTransaction",
        re.compile(r"\bstruct\s+RhiPresentTransaction\b"),
        SRC / "platform/presentation/rhi/present_transaction.rs",
    ),
    (
        "RhiBlurPassGeometry",
        re.compile(r"\bstruct\s+RhiBlurPassGeometry\b"),
        SRC / "platform/presentation/rhi/blur.rs",
    ),
    (
        "CONSISTENCY_PIPELINES",
        re.compile(r"\bconst\s+CONSISTENCY_PIPELINES\b"),
        SRC / "draw/backend/rhi_renderer_consistency.rs",
    ),
    (
        "ConsistencySample",
        re.compile(r"\bstruct\s+ConsistencySample\b"),
        SRC / "draw/backend/rhi_renderer_consistency.rs",
    ),
    (
        "canonical_scenes",
        re.compile(r"\bfn\s+canonical_scenes\b"),
        SRC / "draw/backend/rhi_renderer_consistency.rs",
    ),
    (
        "validate_canonical_scenes",
        re.compile(r"\bfn\s+validate_canonical_scenes\b"),
        SRC / "draw/backend/rhi_renderer_consistency.rs",
    ),
    (
        "blur_subregion_scenario",
        re.compile(r"\bfn\s+blur_subregion_scenario\b"),
        SRC / "draw/backend/rhi_renderer_consistency.rs",
    ),
    (
        "RecoveryDriver",
        re.compile(r"\bstruct\s+RecoveryDriver\b"),
        SRC / "draw/renderer/recovery_driver.rs",
    ),
)


def rust_files(root: Path) -> tuple[Path, ...]:
    """返回稳定排序的生产 Rust 文件集合。"""
    return tuple(sorted(root.rglob("*.rs")))


def source(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def without_comments(text: str) -> str:
    """移除注释，供依赖路径解析使用；架构专名扫描仍检查原始源码。"""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def native_paths(text: str) -> set[str]:
    """展开简单 use group，并收集 app 中实际使用的 crate::native 路径。"""
    clean = without_comments(text)
    paths: set[str] = set()
    use_pattern = re.compile(r"\buse\s+(crate::native::[^;]+);", re.DOTALL)
    for match in use_pattern.finditer(clean):
        body = " ".join(match.group(1).split())
        if "{" not in body:
            paths.add(body)
            continue
        prefix, items = body.split("{", 1)
        if "{" in items:
            raise AssertionError(f"native use 不允许嵌套 group: {body}")
        prefix = prefix.rstrip()
        for item in items.rsplit("}", 1)[0].split(","):
            item = item.strip()
            if item:
                paths.add(f"{prefix}{item}")
    clean = use_pattern.sub("", clean)
    paths.update(re.findall(r"crate::native(?:::[A-Za-z_][A-Za-z0-9_]*)+", clean))
    return paths


class GraphicsSourceBoundaryContractTests(unittest.TestCase):
    """锁定共享上层、platform RHI 与原生 adapter 的单向依赖。"""

    def test_upper_sources_are_one_os_and_api_neutral_tree(self) -> None:
        observed_sources = tuple(
            sorted(path for root in UPPER_ROOTS for path in rust_files(root))
        )
        expected_sources = tuple(
            sorted(
                path
                for path in SRC.rglob("*.rs")
                if any(path.is_relative_to(root) for root in UPPER_ROOTS)
            )
        )
        self.assertEqual(observed_sources, expected_sources)

        for root in UPPER_ROOTS:
            for path in rust_files(root):
                text = source(path)
                clean = without_comments(text)
                path_parts = path.relative_to(root).parts
                with self.subTest(source=relative(path)):
                    self.assertIsNone(TARGET_CFG.search(clean))
                    self.assertIsNone(UPPER_API.search(clean))
                    self.assertIsNone(UPPER_CONCRETE_MODULE.search(clean))
                    self.assertIsNone(UPPER_BACKEND_SELECTION.search(clean))
                    self.assertIsNone(NATIVE_REFERENCE.search(clean))
                    self.assertFalse(
                        any(
                            UPPER_OS_API_PATH.search(Path(part).stem)
                            for part in path_parts
                        )
                    )

    def test_ui_and_app_reach_graphics_only_through_drawing(self) -> None:
        # UI 的绘制上下文只封装 Drawing 的 PaintContext，不取得 RHI 角色。
        ui_paint = source(SRC / "ui/widget_runtime/paint_context.rs")
        self.assertIn(
            "use crate::draw::painting::PaintContext as DrawPaintContext;", ui_paint
        )
        for root in (SRC / "ui", SRC / "app"):
            for path in rust_files(root):
                with self.subTest(upper_rhi_dependency=relative(path)):
                    self.assertNotIn(
                        "crate::platform::presentation::rhi", without_comments(source(path))
                    )

        # Drawing 的 FramePlan 只经 platform Device/Surface 合同完成提交与呈现。
        execution = source(SRC / "draw/backend/frame_plan_execution.rs")
        self.assertIn("FramePlanExecutor::for_surface", execution)
        self.assertIn(".present(RhiPresentTransaction::new(", execution)

        # Vulkan 只实现同一 Surface 合同，不向上层提供旁路入口。
        vulkan_surface = source(
            SRC / "native/presentation/graphics/vulkan/adapter/context/rhi_surface.rs"
        )
        self.assertIn("impl GraphicsSurface for VulkanContext", vulkan_surface)
        self.assertIn("crate::platform::presentation::rhi", vulkan_surface)

    def test_real_vulkan_acceptance_crosses_ui_drawing_and_shared_frame_plan(self) -> None:
        composition = source(SRC / "graphics_parity.rs")
        ui_entry = source(SRC / "ui/widgets/combinators.rs")
        drawing = source(SRC / "draw/backend/production_chain_parity.rs")
        shared_spec = source(SRC / "draw/backend/rhi_renderer_consistency.rs")
        parity_contract = source(SRC / "platform/presentation/rhi/parity.rs")
        vulkan_adapter = source(
            SRC / "native/presentation/graphics/vulkan/adapter/parity.rs"
        )
        vulkan_fixture = source(
            SRC
            / "native/presentation/graphics/vulkan/adapter/context/headless_parity.rs"
        )
        test_target = source(ROOT / "tests/vulkan_gpu_parity.rs")

        # 测试组合根只编排各责任方，期望与容差仍由 Drawing 共享规范持有。
        for marker in (
            "production_chain_scene",
            "execute_ui_production_chain",
            "render_shared_production_scene",
            "run_headless_ui_production_chain_test",
            "HeadlessUiParityAdapter",
            "A::create_context",
            "A::readback",
            "validate_production_chain_readback",
        ):
            with self.subTest(composition_marker=marker):
                self.assertIn(marker, composition)
        self.assertIn("ProductionChainScene", shared_spec)
        self.assertIn("ConsistencySample::exact", shared_spec)

        # 中立端口只描述调用形状；原生 fixture、诊断与故障值留在 Adapter。
        for marker in (
            "trait HeadlessUiParityAdapter",
            "trait WsiParityAdapter",
            "trait WsiParityFramePresenter",
        ):
            self.assertIn(marker, parity_contract)
        for marker in (
            "impl HeadlessUiParityAdapter for VulkanHeadlessUiParityAdapter",
            "new_headless_for_parity_test",
            "readback_texture_for_parity_test",
        ):
            self.assertIn(marker, vulkan_adapter)
            self.assertNotIn(marker, composition)

        # UI 使用真实 WidgetRender；Drawing 使用生产 PaintContext/Canvas2D 和统一 FramePlan。
        self.assertIn("WidgetRender::render(&widget", ui_entry)
        self.assertIn("context.fill_rect(rect, color, None)", ui_entry)
        self.assertNotIn("crate::platform::presentation::rhi", ui_entry)
        self.assertIn("PaintContext::new(", drawing)
        self.assertIn("NativeGpuCanvas2D::new_gpu_only", drawing)
        self.assertIn("canvas.submit_rhi_solid(", drawing)
        self.assertNotIn("crate::native", drawing)

        # Vulkan fixture 只提供真实 Device 与机械回读，不复制共享场景或判定。
        self.assertIn("VulkanContext", vulkan_fixture)
        self.assertIn("readback_production_texture", vulkan_fixture)
        self.assertNotIn("ConsistencySample", vulkan_fixture)
        self.assertNotIn("ProductionChainScene", vulkan_fixture)
        self.assertNotIn("ConsistencySample", vulkan_adapter)
        self.assertNotIn("ProductionChainScene", vulkan_adapter)
        self.assertIn(
            "ui_drawing_frame_plan_executes_and_reads_back_on_real_vulkan_device",
            test_target,
        )

    def test_platform_has_one_concrete_native_dependency_allowlist(self) -> None:
        locations = frozenset(
            path
            for path in rust_files(SRC / "platform")
            if NATIVE_REFERENCE.search(without_comments(source(path)))
        )
        self.assertEqual(locations, PLATFORM_NATIVE_DEPENDENCY_ALLOWLIST)

    def test_graphics_shared_mechanisms_have_one_source_authority(self) -> None:
        all_sources = {
            path: without_comments(source(path)) for path in rust_files(SRC)
        }
        for name, definition, owner in GRAPHICS_AUTHORITY_DEFINITIONS:
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(graphics_authority=name):
                self.assertEqual(locations, [owner])

    def test_agent_transport_has_one_platform_owned_source_tree(self) -> None:
        observed: set[str] = set()
        for path in rust_files(SRC / "app"):
            observed.update(native_paths(source(path)))
        self.assertEqual(observed, set())

        # 旧 native 子树必须消失，app 直接消费 platform 私有 adapter。
        self.assertFalse(LEGACY_AGENT_TRANSPORT_ROOT.exists())
        app_source = source(AGENT_TRANSPORT_APP)
        self.assertIn(
            "use crate::platform::adapters::transport::", app_source
        )
        self.assertIsNone(NATIVE_REFERENCE.search(app_source))

        # 中立字节流与取消合同只能由 platform adapter 根定义一次。
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))
        all_sources = {path: without_comments(source(path)) for path in rust_sources}
        for kind, name in AGENT_TRANSPORT_DEFINITIONS:
            definition = re.compile(rf"\b{kind}\s+{name}\b")
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(agent_transport_definition=name):
                self.assertEqual(locations, [AGENT_TRANSPORT_CONTRACT])

        # 每个 cfg 叶只保留一份端点、唤醒与安全随机数实现。
        for kind, name in (
            ("struct", "AgentEndpoint"),
            ("struct", "AgentEndpointWake"),
            ("fn", "fill_secure_random"),
        ):
            definition = re.compile(rf"\b{kind}\s+{name}\b")
            locations = {
                path for path, text in all_sources.items() if definition.search(text)
            }
            with self.subTest(agent_transport_adapter_definition=name):
                self.assertEqual(locations, set(AGENT_TRANSPORT_ADAPTERS))

        # platform adapter 子树不得反向依赖 native，OS/API 细节留在对应叶。
        for path in rust_files(AGENT_TRANSPORT_ROOT):
            with self.subTest(agent_transport_dependency=relative(path)):
                self.assertNotIn("crate::native", source(path))
        unix_source = source(AGENT_TRANSPORT_ROOT / "unix.rs")
        windows_source = source(AGENT_TRANSPORT_ROOT / "windows.rs")
        for marker in ("UnixListener", "SO_PEERCRED", 'File::open("/dev/urandom")'):
            self.assertIn(marker, unix_source)
        for marker in ("CreateNamedPipeW", "CancelSynchronousIo", "BCryptGenRandom"):
            self.assertIn(marker, windows_source)

        # 三个上层域都不得穿透平台入口选择 factory 或具体 OS 后端。
        for root in UPPER_ROOTS:
            for path in rust_files(root):
                text = source(path)
                with self.subTest(platform_creation_reference=relative(path)):
                    self.assertNotIn("crate::native::factory", text)
                    self.assertNotIn("crate::native::backends", text)

    def test_platform_root_is_the_only_source_definition(self) -> None:
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))

        # 旧 native 物理文件必须消失，递归 Rust 源码不得保留兼容路径。
        self.assertFalse(LEGACY_PLATFORM_ROOT.exists())
        for path in rust_sources:
            with self.subTest(legacy_platform_reference=relative(path)):
                self.assertNotIn("crate::native::platform", source(path))

        # 根 trait 只能定义一次，且中立权威叶不得反向依赖 native。
        all_sources = {path: without_comments(source(path)) for path in rust_sources}
        definition = re.compile(r"\btrait\s+Platform\b")
        locations = [path for path, text in all_sources.items() if definition.search(text)]
        self.assertEqual(locations, [PLATFORM_ROOT])
        owner_source = source(PLATFORM_ROOT)
        dependencies = set(
            re.findall(r"\buse\s+(crate::[^;]+);", without_comments(owner_source))
        )
        self.assertTrue(
            all(
                dependency.startswith("crate::core")
                or dependency.startswith("crate::platform")
                for dependency in dependencies
            )
        )
        self.assertNotIn("crate::native", owner_source)

        # 中立启动输入只定义一次且不依赖 native；按值消费锁定一次性交付语义。
        option_definition = re.compile(r"\bstruct\s+PendingNativeOptions\b")
        option_locations = [
            path for path, text in all_sources.items() if option_definition.search(text)
        ]
        self.assertEqual(option_locations, [PLATFORM_COMPOSITION_CONTRACT])
        composition_contract = source(PLATFORM_COMPOSITION_CONTRACT)
        self.assertNotIn("crate::native", composition_contract)
        self.assertIn("fn into_pending_failures(self)", composition_contract)

        # 平台工厂只允许由明确命名的组合根叶定义；native factory 只保留图形 recipe。
        factory_definition = re.compile(r"\bfn\s+create_platform_with_pending\b")
        factory_locations = [
            path for path, text in all_sources.items() if factory_definition.search(text)
        ]
        self.assertEqual(factory_locations, [PLATFORM_COMPOSITION_ROOT])
        self.assertNotIn(
            "create_platform_with_pending", source(SRC / "native/factory/mod.rs")
        )

        # 三平台与 unsupported 的 cfg 选择、启动输入消费和具体对象构造只在同一叶。
        composition_root = source(PLATFORM_COMPOSITION_ROOT)
        for marker in (
            '#[cfg(windows)]',
            '#[cfg(all(unix, not(target_os = "macos")))]',
            '#[cfg(target_os = "macos")]',
            '#[cfg(not(any(windows, unix)))]',
            "WindowsPlatform::new_with_pending",
            "LinuxPlatform::new",
            "MacosPlatform::new",
        ):
            with self.subTest(platform_composition_marker=marker):
                self.assertIn(marker, composition_root)
        self.assertEqual(composition_root.count("options.into_pending_failures()"), 4)

        # 具体平台聚合的选择不得再次扩散到 platform 的 host、runtime 或公开门面。
        backend_constructor = re.compile(
            r"(?:LinuxPlatform|WindowsPlatform|MacosPlatform)::(?:new|new_with_pending)\b"
        )
        constructor_locations = [
            path
            for path in rust_files(SRC / "platform")
            if backend_constructor.search(without_comments(source(path)))
        ]
        self.assertEqual(constructor_locations, [PLATFORM_COMPOSITION_ROOT])

        # 三平台只实现、composition root 只组装，其余调用者均直接消费中立合同。
        for path in PLATFORM_ROOT_IMPLEMENTERS:
            with self.subTest(platform_root_implementer=relative(path)):
                implementation_source = source(path)
                self.assertIn("crate::platform::platform::Platform", implementation_source)
                self.assertIn("impl Platform for", implementation_source)
        for path in PLATFORM_ROOT_CONSUMERS:
            with self.subTest(platform_root_consumer=relative(path)):
                self.assertIn("crate::platform::platform::Platform", source(path))

    def test_platform_system_is_the_only_base_service_definition(self) -> None:
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))

        # 生产与 fake 都不得继续消费三个合同的旧 native 路径。
        legacy_reference = re.compile(
            r"crate::native::capabilities::system::(?:IFileDialog|INotification|ITimer)\b"
        )
        for path in rust_sources:
            with self.subTest(legacy_system_service_reference=relative(path)):
                self.assertIsNone(legacy_reference.search(source(path)))

        # 权威叶只依赖 core Result，不得反向依赖 native 或高层 host/facade。
        owner_source = source(SYSTEM_SERVICE_ROOT)
        dependencies = set(
            re.findall(r"\buse\s+(crate::[^;]+);", without_comments(owner_source))
        )
        self.assertEqual(dependencies, {"crate::core::Result"})

        # 三个 trait 只能定义一次，禁止复制合同或让 fake 自有协议。
        all_sources = {path: without_comments(source(path)) for path in rust_sources}
        for marker in SYSTEM_SERVICE_DEFINITIONS:
            self.assertIn(marker, owner_source)
            name = marker.rsplit(" ", 1)[-1]
            definition = re.compile(rf"\btrait\s+{name}\b")
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(system_service_definition=name):
                self.assertEqual(locations, [SYSTEM_SERVICE_ROOT])

        # 三平台与 fake 的十二个实现均直接绑定同一内部 leaf。
        for path, implementations in SYSTEM_SERVICE_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(system_service_implementation=relative(path)):
                self.assertIn("crate::platform::system", implementation_source)
                for implementation in implementations:
                    self.assertIn(implementation, implementation_source)

        # Platform 根与三平台/fake 聚合不经兼容层取得合同。
        for path in SYSTEM_SERVICE_PLATFORM_ROOTS:
            with self.subTest(system_service_platform_root=relative(path)):
                self.assertIn("crate::platform::system", source(path))

    def test_platform_filesystem_is_the_only_source_definition(self) -> None:
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))

        # 旧合同与共享核心物理文件必须消失，递归 Rust 源码不得继续引用旧路径。
        for path in LEGACY_FILESYSTEM_ROOTS:
            with self.subTest(legacy_filesystem_root=relative(path)):
                self.assertFalse(path.exists())
        for path in rust_sources:
            text = source(path)
            with self.subTest(legacy_filesystem_reference=relative(path)):
                self.assertNotIn("native::capabilities::system", text)
                self.assertNotIn("native::capabilities::services::filesystem", text)

        # 权威叶只依赖 core，不得形成 platform 到 native 的合同依赖环。
        owner_source = source(FILESYSTEM_ROOT)
        self.assertNotIn("crate::native", owner_source)
        self.assertIn("use crate::core::{Errc, Error, Result};", owner_source)

        # 目录值、低层端口、provider 窄端口与共享核心都只能定义一次。
        all_sources = {path: without_comments(source(path)) for path in rust_sources}
        for marker, pattern in FILESYSTEM_DEFINITIONS:
            self.assertIn(marker, owner_source)
            definition = re.compile(pattern)
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(filesystem_definition=marker):
                self.assertEqual(locations, [FILESYSTEM_ROOT])

        # 公开 services 路径只能重导出同一类型，不得恢复第二份 SpecialDir。
        services_source = source(SRC / "platform/host/services.rs")
        self.assertIn(
            "pub use crate::platform::system::filesystem::SpecialDir;",
            services_source,
        )

        # 三平台与 fake 直接实现同一权威合同，不增加适配层或兼容路径。
        for path, implementations in FILESYSTEM_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(filesystem_implementation=relative(path)):
                self.assertIn(
                    "crate::platform::system::filesystem", implementation_source
                )
                for implementation in implementations:
                    self.assertIn(implementation, implementation_source)

        # Platform 根、三平台聚合、fake 与应用桩都直接取得 platform 端口。
        for path in FILESYSTEM_PLATFORM_ROOTS:
            with self.subTest(filesystem_platform_root=relative(path)):
                self.assertIn(
                    "crate::platform::system::filesystem::IFileSystem", source(path)
                )

    def test_platform_console_is_the_only_source_definition(self) -> None:
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))

        # 生产与 fake 都不得继续消费控制台合同的旧 native 路径。
        legacy_reference = re.compile(
            r"crate::native::capabilities::system::"
            r"(?:ConsoleColor|TerminalCapabilities|IConsole)\b"
        )
        for path in rust_sources:
            with self.subTest(legacy_console_reference=relative(path)):
                self.assertIsNone(legacy_reference.search(source(path)))

        # 中立叶只依赖 core Result，不取得 native、高层 host 或 facade。
        owner_source = source(CONSOLE_ROOT)
        dependencies = set(
            re.findall(r"\buse\s+(crate::[^;]+);", without_comments(owner_source))
        )
        self.assertEqual(dependencies, {"crate::core::Result"})

        # 三类合同各自只能定义一次，禁止复制类型或让 fake 自有协议。
        all_sources = {path: without_comments(source(path)) for path in rust_sources}
        for kind, name in CONSOLE_DEFINITIONS:
            marker = f"pub(crate) {kind} {name}"
            self.assertIn(marker, owner_source)
            definition = re.compile(rf"\b{kind}\s+{name}\b")
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(console_definition=name):
                self.assertEqual(locations, [CONSOLE_ROOT])

        # 三平台与 fake 均直接绑定同一内部 leaf，不增加适配 trait。
        for path, implementation in CONSOLE_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(console_implementation=relative(path)):
                self.assertIn("crate::platform::system::console", implementation_source)
                self.assertIn(implementation, implementation_source)

        # Platform 根与三平台/fake 聚合不经 native 兼容层取得合同。
        for path in CONSOLE_PLATFORM_ROOTS:
            with self.subTest(console_platform_root=relative(path)):
                self.assertIn("crate::platform::system::console", source(path))

    def test_platform_system_info_is_the_only_internal_contract_definition(self) -> None:
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))

        # 生产与 fake 都不得继续消费系统信息合同的旧 native 路径。
        legacy_reference = re.compile(
            r"crate::native::capabilities::system::"
            r"(?:MemoryInfo|OsInfo|ISystemInfo)\b"
        )
        for path in rust_sources:
            with self.subTest(legacy_system_info_reference=relative(path)):
                self.assertIsNone(legacy_reference.search(source(path)))

        # 中立叶只依赖 core 错误合同与同层公开 host 协议，不得反向取得 native。
        owner_source = source(SYSTEM_INFO_ROOT)
        self.assertNotIn("crate::native", owner_source)
        self.assertIn(
            "impl<T> crate::platform::services::FontSystemInfo for T", owner_source
        )

        # crate 私有合同各自只能定义一次；公开 hardware 描述值保持既有公共语义。
        all_sources = {path: without_comments(source(path)) for path in rust_sources}
        for kind, name in SYSTEM_INFO_DEFINITIONS:
            marker = f"pub(crate) {kind} {name}"
            self.assertIn(marker, owner_source)
            locations = [path for path, text in all_sources.items() if marker in text]
            with self.subTest(system_info_definition=name):
                self.assertEqual(locations, [SYSTEM_INFO_ROOT])

        # 字体发现继续由高层 services 叶唯一持有，system/info 只实现该中立依赖。
        font_definition = re.compile(r"\btrait\s+FontSystemInfo\b")
        font_locations = [
            path for path, text in all_sources.items() if font_definition.search(text)
        ]
        self.assertEqual(font_locations, [FONT_SYSTEM_INFO_ROOT])
        self.assertIsNone(font_definition.search(without_comments(owner_source)))

        # 三平台与 fake 均直接绑定同一内部 leaf，不增加适配 trait。
        for path, implementation in SYSTEM_INFO_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(system_info_implementation=relative(path)):
                self.assertIn("crate::platform::system::info", implementation_source)
                self.assertIn(implementation, implementation_source)

        # Platform 根与三平台/fake 聚合不经 native 兼容层取得合同。
        for path in SYSTEM_INFO_PLATFORM_ROOTS:
            with self.subTest(system_info_platform_root=relative(path)):
                self.assertIn("crate::platform::system::info", source(path))

    def test_platform_windowing_event_is_the_only_source_definition(self) -> None:
        # 旧 native 物理模块必须消失，内部调用方统一消费新权威路径。
        self.assertFalse(LEGACY_EVENT_ROOT.exists())
        for path in rust_files(SRC):
            with self.subTest(legacy_reference=relative(path)):
                self.assertNotIn("crate::native::windowing::event", source(path))

        # 权威文件必须完整持有冻结的事件协议定义。
        for owner, markers in EVENT_DEFINITIONS.items():
            owner_source = source(owner)
            for marker in markers:
                with self.subTest(owner=relative(owner), marker=marker):
                    self.assertIn(marker, owner_source)

        # windowing 内同名定义只能位于权威文件，native 不得再定义这些协议。
        platform_sources = {
            path: source(path) for path in rust_files(SRC / "platform/windowing")
        }
        native_sources = {path: source(path) for path in rust_files(SRC / "native")}
        for owner, markers in EVENT_DEFINITIONS.items():
            for marker in markers:
                kind_and_name = re.search(
                    r"(?:type|struct|enum|trait)\s+([A-Za-z_][A-Za-z0-9_]*)", marker
                )
                self.assertIsNotNone(kind_and_name)
                name = kind_and_name.group(1)
                definition = re.compile(rf"\b(?:type|struct|enum|trait)\s+{name}\b")
                platform_locations = [
                    path
                    for path, text in platform_sources.items()
                    if definition.search(text)
                ]
                native_locations = [
                    path for path, text in native_sources.items() if definition.search(text)
                ]
                with self.subTest(definition=name):
                    self.assertEqual(platform_locations, [owner])
                    self.assertEqual(native_locations, [])

    def test_platform_window_is_the_only_source_definition(self) -> None:
        # 旧 native 模块必须消失，生产与测试源码都不得再引用旧路径。
        self.assertFalse(LEGACY_WINDOW_ROOT.exists())
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))
        for path in rust_sources:
            with self.subTest(legacy_window_reference=relative(path)):
                self.assertNotIn("crate::native::windowing::window", source(path))

        # 权威叶只能依赖 core 与 platform 公开合同，禁止形成 platform → native 环。
        owner_source = source(WINDOW_ROOT)
        self.assertNotIn("crate::native", owner_source)
        for dependency in (
            "crate::core::WindowId",
            "crate::core::error::{Error, Result}",
            "crate::core::geometry::Point",
            "crate::platform::presentation::IPresenter",
            "crate::platform::windowing::event::{FrameRequestToken, PointerActivationId}",
        ):
            with self.subTest(window_dependency=dependency):
                self.assertIn(dependency, owner_source)
        # 公共窗口类型允许按 Rust 惯例独立导入或与同层类型分组导入。
        self.assertRegex(
            without_comments(owner_source),
            r"\buse\s+crate::platform::windowing::(?:WindowResizeEdge|\{[^;}]*\bWindowResizeEdge\b[^;}]*\});",
        )

        # 所有窗口协议定义只能出现一次，禁止 native 或测试替身复制合同。
        all_sources = {path: source(path) for path in rust_sources}
        for marker in WINDOW_DEFINITIONS:
            self.assertIn(marker, owner_source)
            name = marker.rsplit(" ", 1)[-1]
            definition = re.compile(rf"\b(?:struct|enum|trait)\s+{name}\b")
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(window_definition=name):
                self.assertEqual(locations, [WINDOW_ROOT])

        # 生产后端、共享实现与 fake 均直接实现 platform 权威合同。
        for path, implementation in WINDOW_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(window_implementation=relative(path)):
                self.assertIn(implementation, implementation_source)
                self.assertIn(
                    "crate::platform::windowing::window", implementation_source
                )

    def test_platform_input_is_the_only_source_definition(self) -> None:
        # 删除旧物理模块，生产与测试 Rust 源码不得继续消费兼容路径。
        self.assertFalse(LEGACY_INPUT_ROOT.exists())
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))
        for path in rust_sources:
            with self.subTest(legacy_input_reference=relative(path)):
                self.assertNotIn("crate::native::windowing::input", source(path))

        # 权威叶只依赖 core 值与同层输入值，不得反向取得 native 实现。
        owner_source = source(INPUT_ROOT)
        self.assertNotIn("crate::native", owner_source)
        for dependency in (
            "crate::core::{Error, Point, Rect, WindowId}",
            "super::{CursorType, KeyCode}",
        ):
            with self.subTest(input_dependency=dependency):
                self.assertIn(dependency, owner_source)

        # 三项协议定义只能出现一次，禁止后端、app 或 fake 复制合同。
        all_sources = {path: source(path) for path in rust_sources}
        for marker in INPUT_DEFINITIONS:
            self.assertIn(marker, owner_source)
            name = marker.rsplit(" ", 1)[-1]
            definition = re.compile(rf"\btrait\s+{name}\b")
            locations = [
                path for path, text in all_sources.items() if definition.search(text)
            ]
            with self.subTest(input_definition=name):
                self.assertEqual(locations, [INPUT_ROOT])

        # Windows、macOS、Wayland 与 fake 均直接绑定同一 platform 合同。
        for path, implementations in INPUT_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(input_implementation=relative(path)):
                self.assertIn("crate::platform::windowing", implementation_source)
                for implementation in implementations:
                    self.assertIn(implementation, implementation_source)

    def test_platform_display_is_the_only_capability_definition(self) -> None:
        # 删除旧物理模块，生产与测试 Rust 源码均不得继续消费旧路径。
        self.assertFalse(LEGACY_DISPLAY_ROOT.exists())
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))
        for path in rust_sources:
            with self.subTest(legacy_display_reference=relative(path)):
                self.assertNotIn(
                    "crate::native::capabilities::display", source(path)
                )

        # 权威叶只依赖 core Result/Rect，不得反向取得 native 实现。
        owner_source = source(DISPLAY_ROOT)
        self.assertNotIn("crate::native", owner_source)
        self.assertIn("use crate::core::{Rect, Result};", owner_source)

        # 显示能力合同各自只有一个定义，禁止后端或 fake 复制 trait/value。
        all_sources = {path: source(path) for path in rust_sources}
        for marker in DISPLAY_DEFINITIONS:
            locations = [
                path for path, text in all_sources.items() if marker in text
            ]
            with self.subTest(display_definition=marker):
                self.assertEqual(locations, [DISPLAY_ROOT])

        # Platform 根、三平台与 fake 都直接绑定 platform 权威路径。
        self.assertIn(
            "use crate::platform::display::IDisplay;", source(PLATFORM_ROOT)
        )
        for path, implementation in DISPLAY_IMPLEMENTERS.items():
            implementation_source = source(path)
            with self.subTest(display_implementation=relative(path)):
                self.assertIn("crate::platform::display", implementation_source)
                self.assertIn(implementation, implementation_source)

    def test_platform_presenter_is_the_only_source_definition(self) -> None:
        # trait 只能由 platform leaf 定义，native 不保留合同目录或兼容路径。
        rust_sources = (*rust_files(SRC), *rust_files(ROOT / "tests"))
        definitions = [
            path for path in rust_sources if PRESENTER_DEFINITION.search(source(path))
        ]
        self.assertEqual(definitions, [PRESENTER_ROOT])
        self.assertFalse(LEGACY_PRESENTATION_CONTRACT_ROOT.exists())

        # 上层、窗口协议及所有 native/test 实现必须直接消费 platform 权威路径。
        for path in rust_sources:
            with self.subTest(legacy_presenter_reference=relative(path)):
                self.assertIsNone(LEGACY_PRESENTER_REFERENCE.search(source(path)))

    def test_drawing_graphics_dependencies_enter_only_through_platform_rhi(self) -> None:
        allowed = (
            "crate::platform::presentation::rhi::",
            # 字体发现属于非图形 host 协议，不参与绘制调用链。
            "crate::platform::services::FontSystemInfo",
        )
        for path in rust_files(SRC / "draw"):
            text = without_comments(source(path))
            with self.subTest(source=relative(path)):
                self.assertNotIn("crate::native", text)
                for match in re.finditer(r"crate::platform::", text):
                    dependency = text[match.start() :]
                    self.assertTrue(
                        dependency.startswith(allowed),
                        f"Drawing 出现未授权 platform 依赖: {relative(path)}",
                    )

    def test_native_api_details_stay_in_corresponding_adapters(self) -> None:
        checks = (
            (re.compile(r"\bash::|\bvk::"), (API_ADAPTER_ROOTS["vulkan"],)),
            (
                re.compile(r"\bglow::|\bkhronos_egl::|\begl[A-Z]|\bwgl[A-Z]"),
                (API_ADAPTER_ROOTS["opengl"],),
            ),
            (
                re.compile(r"windows::Win32::Graphics::Direct3D11|D3D11CreateDevice"),
                (API_ADAPTER_ROOTS["d3d11"],),
            ),
            (
                re.compile(r"windows::Win32::Graphics::Dxgi|CreateDXGIFactory"),
                (
                    API_ADAPTER_ROOTS["d3d11"],
                    SRC / "native/presentation/graphics/d3d12",
                ),
            ),
        )
        for path in rust_files(SRC):
            text = source(path)
            for pattern, roots in checks:
                if not pattern.search(text):
                    continue
                with self.subTest(source=relative(path), marker=pattern.pattern):
                    self.assertTrue(any(path.is_relative_to(root) for root in roots))

        # 组合根只消费中立 parity 端口；API 诊断与故障恢复证明归对应 Adapter。
        composition = source(SRC / "graphics_parity.rs")
        parity_contract = source(SRC / "platform/presentation/rhi/parity.rs")
        opengl_parity = source(API_ADAPTER_ROOTS["opengl"] / "adapter/parity.rs")
        vulkan_parity = source(API_ADAPTER_ROOTS["vulkan"] / "adapter/parity.rs")
        self.assertIn("trait WsiParityAdapter", parity_contract)
        self.assertIn("impl WsiParityFramePresenter", composition)
        for marker in (
            "window_surface_replacements_for_test",
            "eglSwapBuffers",
            "EGLSurface",
            "OpenGlSurfaceFault",
        ):
            self.assertNotIn(marker, composition)
            self.assertIn(marker, opengl_parity)
        for marker in (
            "VulkanSurfaceFaultForParity",
            "OUT_OF_DATE",
            "SUBOPTIMAL",
            "inject_surface_fault_for_parity_test",
        ):
            self.assertNotIn(marker, composition)
            self.assertIn(marker, vulkan_parity)

        factory = "\n".join(source(path) for path in rust_files(SRC / "native/factory"))
        self.assertNotRegex(factory, r"\b(?:ash|vk|glow|khronos_egl)::")
        self.assertNotRegex(factory, r"::(?:adapter|context|pipeline|raster)::")

        implementation = re.compile(r"impl(?:<[^>]+>)?\s+Graphics(?:Device|Surface)\s+for")
        for path in rust_files(SRC):
            if implementation.search(source(path)):
                with self.subTest(implementation=relative(path)):
                    self.assertTrue(any(path.is_relative_to(root) for root in API_ADAPTER_ROOTS.values()))

        # 每个生产 API adapter 只能实现一份 Device 与一份 Surface 角色。
        for name, root in API_ADAPTER_ROOTS.items():
            adapter = "\n".join(without_comments(source(path)) for path in rust_files(root))
            with self.subTest(adapter_role_implementations=name):
                self.assertEqual(len(implementation.findall(adapter)), 2)

    def test_vulkan_first_and_gpu_recipe_never_selects_pixel_upload(self) -> None:
        for name in ("linux", "windows", "macos"):
            path = SRC / f"native/factory/registry_{name}.rs"
            text = source(path)
            blocks = re.findall(r"GraphicsBackendEntry\s*\{(.*?)\n\s*\}", text, re.DOTALL)
            priorities = [int(value) for value in re.findall(r"priority:\s*(\d+)", text)]
            vulkan = next(block for block in blocks if "GraphicsApi::Vulkan" in block)
            vulkan_priority = int(re.search(r"priority:\s*(\d+)", vulkan).group(1))
            with self.subTest(registry=name):
                self.assertEqual(vulkan_priority, 100)
                self.assertEqual(vulkan_priority, max(priorities))
                self.assertEqual(priorities.count(vulkan_priority), 1)

        runtime = source(SRC / "draw/renderer/runtime.rs")
        gpu_branch = runtime[runtime.index("GraphicsRecipeOwner::Gpu(owner)") :]
        gpu_branch = gpu_branch[: gpu_branch.index("GraphicsRecipeOwner::PixelUpload(owner)")]
        self.assertIn("GpuBackend::new_gpu_only(owner)", gpu_branch)
        self.assertNotIn("PixelUploadPresentation", gpu_branch)

        frame_plan = source(SRC / "draw/backend/frame_plan_execution.rs")
        self.assertNotIn("PixelUpload", frame_plan)

    def test_all_native_harnesses_consume_one_canonical_pipeline_spec(self) -> None:
        pipeline = source(SRC / "platform/presentation/rhi/pipeline.rs")
        enum_body = pipeline[pipeline.index("enum PipelineKind") :]
        enum_body = enum_body[: enum_body.index("}")]
        variants = tuple(re.findall(r"^\s*([A-Z][A-Za-z0-9_]*),\s*$", enum_body, re.MULTILINE))
        self.assertEqual(variants, PIPELINE_KINDS)

        shared = source(SRC / "draw/backend/rhi_renderer_consistency.rs")
        spec = shared[shared.index("CONSISTENCY_PIPELINES") :]
        spec = spec[: spec.index("];", spec.index("["))]
        canonical = tuple(re.findall(r"PipelineKind::([A-Za-z0-9_]+)", spec))
        self.assertEqual(canonical, PIPELINE_KINDS)
        self.assertIn("CONSISTENCY_PIPELINES.map(scene_for_pipeline)", shared)

        harnesses = (
            ROOT / "tests/unit/native/presentation/graphics/vulkan/adapter/context/rhi_device__gpu_parity_tests.rs",
            ROOT / "tests/unit/native/presentation/graphics/opengl/raster/rhi_device__gpu_parity_tests.rs",
            ROOT / "tests/unit/native/presentation/graphics/d3d11/adapter/context/rhi_device__gpu_parity_tests.rs",
        )
        for path in harnesses:
            text = source(path)
            with self.subTest(harness=relative(path)):
                self.assertIn("canonical_scenes", text)
                self.assertIn("validate_canonical_scenes", text)
                self.assertIn("blur_subregion_scenario", text)
                self.assertIn("sample.tolerance.amount()", text)
                self.assertNotIn("CONSISTENCY_PIPELINES", text)
                self.assertNotRegex(text, r"fn\s+canonical_scenes\s*\(")
                self.assertNotRegex(text, r"fn\s+blur_subregion_scenario\s*\(")

    def test_all_three_adapters_consume_shared_surface_lifecycle(self) -> None:
        for name, root in API_ADAPTER_ROOTS.items():
            text = "\n".join(source(path) for path in rust_files(root))
            with self.subTest(adapter=name):
                self.assertIn("RhiSurfaceLifecycle", text)
                self.assertIn("RhiSurfaceLifecycle::uninitialized", text)
                self.assertIsNone(PRIVATE_SURFACE_STATE.search(text))
                self.assertNotRegex(text, r"(?:struct|enum)\s+RhiSurfaceLifecycle\b")

    def test_architecture_document_names_current_physical_boundaries(self) -> None:
        document = source(ROOT / "docs/架构/graphics/source-boundary.md")
        for marker in (
            "src/app",
            "src/ui",
            "src/draw",
            "src/platform/presentation/rhi",
            "src/platform/composition_root.rs",
            "src/native/presentation/graphics",
            "UI → Drawing Engine → platform 通用 RHI/Surface 合同 → native 原生 adapter",
            "共同构成 platform 层",
            "真实 Windows D3D11 尚未执行",
        ):
            with self.subTest(marker=marker):
                self.assertIn(marker, document)


if __name__ == "__main__":
    unittest.main()
