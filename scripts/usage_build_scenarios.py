"""定义使用方构建采集器的稳定场景矩阵。"""

# 启用未来注解，避免运行时解析类型前向引用。
from __future__ import annotations

# 导入数据类声明与不可变场景复制工具。
from dataclasses import dataclass, replace
# 导入路径类型。
from pathlib import Path

# 记录 uix-demo 除日志订阅外必须启用的既有能力组合。
DEMO_BASE_FEATURES = "d3d11,image-codecs,qrcode,form-pattern,rich-text,charts,table,navigation,feedback,tree-widgets"
# 记录在相同演示能力组合上额外启用日志订阅的对照集合。
DEMO_LOGGING_FEATURES = f"{DEMO_BASE_FEATURES},demo-logging"
# 记录所有可独立选择的图形 backend feature，供最小与负向场景统一排除。
GRAPHICS_BACKEND_FEATURES = ("d3d11", "d3d12", "metal", "opengles", "vulkan")
# 记录没有专属第三方 package、必须逐场景断言根 feature 的源码能力。
PURE_SOURCE_CAPABILITY_FEATURES = (
    # 富文本通过实现、公开辅助函数与内部分派共同形成源码边界。
    "rich-text",
    # 图表通过基础与高级组件族共同形成源码边界。
    "charts",
    # 表格通过基础与泛型组件共同形成源码边界。
    "table",
    # 导航通过基础与泛型导航组件共同形成源码边界。
    "navigation",
    # 反馈通过组件、全局门面与应用覆盖层共同形成源码边界。
    "feedback",
    # 树组件通过展示树、节点模型、树选择器与快照共同形成源码边界。
    "tree-widgets",
)
# 记录 D3D11 启用后必须进入 windows package 的精确 API feature。
D3D11_WINDOWS_PACKAGE_FEATURES = (
    # Direct3D 基础类型属于 D3D11 实现依赖。
    ("windows", "Win32_Graphics_Direct3D"),
    # shader 编译 API 属于 D3D11 实现依赖。
    ("windows", "Win32_Graphics_Direct3D_Fxc"),
    # Direct3D 11 API 是该 backend 的核心依赖。
    ("windows", "Win32_Graphics_Direct3D11"),
    # DXGI factory 与 adapter API 属于 D3D11 实现依赖。
    ("windows", "Win32_Graphics_Dxgi"),
    # DXGI 公共格式与描述类型属于 D3D11 实现依赖。
    ("windows", "Win32_Graphics_Dxgi_Common"),
)
# 记录只有 D3D11 选择面才应启用的 Windows API feature。
D3D11_ONLY_WINDOWS_PACKAGE_FEATURES = (
    # Direct3D 11 API 不得被 D3D12 或其他 backend 选择面合并。
    ("windows", "Win32_Graphics_Direct3D11"),
)
# 记录 D3D12 选择面声明的完整 Windows API feature 集。
D3D12_WINDOWS_PACKAGE_FEATURES = (
    # Direct3D 基础类型由 D3D12 清单 feature 显式启用。
    ("windows", "Win32_Graphics_Direct3D"),
    # 当前 D3D12 legacy pipeline 仍声明 shader 编译 API。
    ("windows", "Win32_Graphics_Direct3D_Fxc"),
    # Direct3D 12 API 是该选择面的专属核心 feature。
    ("windows", "Win32_Graphics_Direct3D12"),
    # DXGI factory 与 swapchain 类型由 D3D12 feature 启用。
    ("windows", "Win32_Graphics_Dxgi"),
    # DXGI 公共格式与描述类型由 D3D12 feature 启用。
    ("windows", "Win32_Graphics_Dxgi_Common"),
    # 当前 D3D12 清单同时声明基础安全 API。
    ("windows", "Win32_Security"),
)
# 记录只有 D3D12 选择面才应启用的 Windows API feature。
D3D12_ONLY_WINDOWS_PACKAGE_FEATURES = (
    # Direct3D 12 API 不得泄漏到 D3D11 或其他 backend 入口。
    ("windows", "Win32_Graphics_Direct3D12"),
    # 基础安全 API 当前只由 D3D12 或 Agent 显式声明。
    ("windows", "Win32_Security"),
)
# 汇总图形选择面可能向 windows package 合并的全部 API feature。
GRAPHICS_WINDOWS_PACKAGE_FEATURES = (
    # 复用完整 D3D11 feature 集作为公共与 D3D11 专属部分。
    *D3D11_WINDOWS_PACKAGE_FEATURES,
    # 追加 D3D12 专属 feature，避免重复公共 Direct3D/DXGI 项。
    *D3D12_ONLY_WINDOWS_PACKAGE_FEATURES,
)


# 描述一个独立的使用方基线入口。
@dataclass(frozen=True)
class Scenario:
    # 记录报告中使用的稳定名称。
    name: str
    # 记录 fixture 的 Cargo 清单路径。
    manifest: Path
    # 记录该入口覆盖的能力范围。
    description: str
    # 记录 metadata、check 与 build 共用的 Cargo feature 参数。
    feature_args: tuple[str, ...] = ()
    # 记录 check 与 build 使用的 Cargo target 参数。
    build_args: tuple[str, ...] = ()
    # 允许单个场景覆盖 rustc host target，未设置时使用主机目标。
    target: str | None = None
    # 标记正向跨目标场景只执行类型检查，避免要求宿主机具备目标链接器。
    check_only: bool = False
    # 记录 resolved graph 中必须出现的专属 package。
    required_packages: tuple[str, ...] = ()
    # 记录 resolved graph 中必须缺席的未选 package。
    forbidden_packages: tuple[str, ...] = ()
    # 记录 resolved package 上必须启用的精确 feature。
    required_package_features: tuple[tuple[str, str], ...] = ()
    # 记录 resolved package 上必须关闭的精确 feature。
    forbidden_package_features: tuple[tuple[str, str], ...] = ()
    # 记录 uix resolve 节点中必须启用的 capability feature。
    required_uix_features: tuple[str, ...] = ()
    # 记录 uix resolve 节点中必须关闭的 capability feature。
    forbidden_uix_features: tuple[str, ...] = ()
    # 标记该场景是否必须在公开入口编译阶段失败。
    expected_compile_failure: bool = False
    # 记录 compile-fail 场景必须出现的错误片段。
    expected_error_fragments: tuple[str, ...] = ()


# 为单个场景补齐全部纯源码 capability 的正向或负向断言。
def _complete_pure_source_feature_guards(scenario: Scenario) -> Scenario:
    # 汇总场景已经明确要求或禁止的根 feature。
    guarded = {*scenario.required_uix_features, *scenario.forbidden_uix_features}
    # 按稳定能力顺序找出尚未声明边界的纯源码 feature。
    missing = tuple(feature for feature in PURE_SOURCE_CAPABILITY_FEATURES if feature not in guarded)
    # 已完整声明的场景保持原对象与原顺序。
    if not missing:
        return scenario
    # 缺失项统一追加到禁止列表，防止未选能力静默进入解析图。
    return replace(
        scenario,
        forbidden_uix_features=(*scenario.forbidden_uix_features, *missing),
    )


# 返回仓库根目录。
def project_root() -> Path:
    # 当前脚本位于仓库根目录下的 scripts 目录。
    return Path(__file__).resolve().parents[1]


# 返回 ODC-01/ODC-07 的独立 fixture 与根二进制场景定义。
def scenario_specs(root: Path) -> list[Scenario]:
    # 返回最小、默认、单能力、根二进制与禁用公开面入口。
    scenarios = [
        # 最小入口关闭所有默认 feature。
        Scenario(
            name="minimal",
            manifest=root / "fixtures" / "usage-build" / "minimal" / "Cargo.toml",
            description="关闭默认 feature 的最小使用方入口",
            # 最小入口必须排除全部已独立裁剪的专属依赖及已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 最小入口必须排除全部 D3D11/D3D12 图形 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # 最小入口必须证明 backend、非默认源码与数据能力未被 Cargo 选择。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        ),
        # 复用最小入口建立真实的非 Windows 编译轴。
        Scenario(
            # 使用稳定名称区分同一 fixture 的 Linux 目标证据。
            name="minimal-linux",
            # 复用最小使用方清单，避免 feature 集差异污染目标对照。
            manifest=root / "fixtures" / "usage-build" / "minimal" / "Cargo.toml",
            # 说明该入口验证 Linux GNU 目标的基础平台依赖边界。
            description="关闭默认 feature 的 Linux GNU 目标最小入口",
            # 显式覆盖宿主目标，确保 metadata 与 check 使用同一 Linux triple。
            target="x86_64-unknown-linux-gnu",
            # 跨目标只要求真实类型检查，不要求 Windows 宿主具备 Linux 链接器。
            check_only=True,
            # Linux 最小入口必须解析 Unix 与 Wayland 基础依赖。
            required_packages=("libc", "wayland-client"),
            # Linux 最小入口必须排除 Windows 依赖、未选能力和已删除死依赖。
            forbidden_packages=(
                # 图片编解码能力未启用。
                "image",
                # Vulkan backend 的专属依赖未启用。
                "ash",
                # 已删除的 bytemuck 直接边不得在未启用图片能力时回归。
                "bytemuck",
                # OpenGL ES backend 的跨平台入口依赖未启用。
                "glow",
                # Linux EGL loader 只应随 OpenGL ES backend 启用。
                "khronos-egl",
                # 二维码能力未启用。
                "qrcode",
                # 表单正则能力未启用。
                "regex",
                # 已删除的窗口句柄死依赖不得回归。
                "raw-window-handle",
                # Agent 与设置序列化共享的 JSON 依赖未启用。
                "serde_json",
                # 演示日志订阅能力未启用。
                "tracing-subscriber",
                # Windows API package 不得进入 Linux 解析图。
                "windows",
                # Windows 宏展开根依赖不得进入 Linux 解析图。
                "windows-core",
            # 结束 Linux 禁用依赖集合。
            ),
            # Linux 最小入口同样不得选择 backend、Agent、富文本与设置能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 Linux 最小场景定义。
        ),
        # 默认入口覆盖当前 Windows 默认 D3D11 能力。
        Scenario(
            name="d3d11-default",
            manifest=root / "fixtures" / "usage-build" / "d3d11-default" / "Cargo.toml",
            description="使用当前默认 feature 的图形入口",
            # 默认兼容集合必须包含三项能力依赖与演示日志订阅器。
            required_packages=("image", "qrcode", "regex", "tracing-subscriber"),
            # 未选 backend 与已删除的死依赖在默认入口中必须保持缺席。
            forbidden_packages=("ash", "glow", "raw-window-handle", "serde_json"),
            # 默认 D3D11 必须选择完整的 Win32 图形 API feature 集。
            required_package_features=D3D11_WINDOWS_PACKAGE_FEATURES,
            # 默认 D3D11 入口不得合并 D3D12 专属 Windows API feature。
            forbidden_package_features=D3D12_ONLY_WINDOWS_PACKAGE_FEATURES,
            # 默认兼容集合必须继续包含 D3D11 与富文本公开能力。
            required_uix_features=("d3d11", "charts", "feedback", "navigation", "rich-text", "table", "tree-widgets"),
            # Agent 控制与 OpenGL ES 不属于默认兼容集合。
            forbidden_uix_features=("agent-control", "opengles"),
        ),
        # D3D11 单 backend 入口只打开对应实现与公开选择面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="d3d11",
            # 指向 D3D11 正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "d3d11" / "Cargo.toml",
            # 说明该入口只覆盖 D3D11 backend capability。
            description="只打开 d3d11 backend capability 的入口",
            # 单 backend 入口不得合并其他 backend 与使用方 capability 依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # D3D11 必须精确选择其 Win32 API feature 集。
            required_package_features=D3D11_WINDOWS_PACKAGE_FEATURES,
            # 单 D3D11 入口不得合并 D3D12 专属 Windows API feature。
            forbidden_package_features=D3D12_ONLY_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明 uix 实际选择了 D3D11 feature。
            required_uix_features=("d3d11",),
            # 单 backend 入口不得合并其他图形实现或源码能力。
            forbidden_uix_features=("d3d12", "metal", "opengles", "vulkan", "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 D3D11 正向场景定义。
        ),
        # OpenGL ES 单 backend 入口只打开对应实现与公开选择面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="opengles",
            # 指向 OpenGL ES 正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "opengles" / "Cargo.toml",
            # 说明该入口只覆盖 OpenGL ES backend capability。
            description="只打开 opengles backend capability 的入口",
            # OpenGL ES 跨平台实现必须解析 glow package。
            required_packages=("glow",),
            # 单 backend 入口不得合并其他 backend 与使用方 capability 依赖。
            forbidden_packages=("ash", "bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # OpenGL ES 入口不得合并 D3D11/D3D12 的 Win32 API feature 集。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明 uix 实际选择了 OpenGL ES feature。
            required_uix_features=("opengles",),
            # 单 backend 入口不得合并其他图形实现或源码能力。
            forbidden_uix_features=("d3d11", "d3d12", "metal", "vulkan", "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 OpenGL ES 正向场景定义。
        ),
        # D3D12 当前只验证公开选择面与清单依赖，不宣称生产 registry 可用。
        Scenario(
            # 使用稳定名称显式区分选择面与生产 backend。
            name="d3d12-selection",
            # 三种未生产化 backend 复用同一个选择面 fixture。
            manifest=root / "fixtures" / "usage-build" / "backend-selection" / "Cargo.toml",
            # 说明该入口只覆盖 D3D12 公开变体与 Cargo feature 边界。
            description="只验证 d3d12 公开选择面与依赖图，不作为生产 backend 证据",
            # 选择 fixture 的 D3D12 转发 feature。
            feature_args=("--no-default-features", "--features", "d3d12-selection"),
            # D3D12 选择面不得合并其他 backend 或 capability 依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # D3D12 必须精确选择其清单声明的 Windows API feature 集。
            required_package_features=D3D12_WINDOWS_PACKAGE_FEATURES,
            # D3D12 入口不得合并 D3D11 专属 API feature。
            forbidden_package_features=D3D11_ONLY_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明 uix 实际选择 D3D12 feature。
            required_uix_features=("d3d12",),
            # 单选择面入口不得合并其他 backend 与源码 capability。
            forbidden_uix_features=("d3d11", "metal", "opengles", "vulkan", "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 D3D12 选择面正向场景定义。
        ),
        # Vulkan 当前验证公开选择面、ash 与可编译源码，但不宣称 registry 可用。
        Scenario(
            # 使用稳定名称显式区分选择面与生产 backend。
            name="vulkan-selection",
            # 复用统一 backend 选择面 fixture。
            manifest=root / "fixtures" / "usage-build" / "backend-selection" / "Cargo.toml",
            # 说明该入口只覆盖 Vulkan 公开变体与 Cargo feature 边界。
            description="只验证 vulkan 公开选择面、ash 与构建图，不作为生产 backend 证据",
            # 选择 fixture 的 Vulkan 转发 feature。
            feature_args=("--no-default-features", "--features", "vulkan-selection"),
            # Vulkan feature 必须解析其专属 ash package。
            required_packages=("ash",),
            # Vulkan 选择面不得合并其他 capability 依赖或死依赖。
            forbidden_packages=("bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # Vulkan 不得合并 D3D11/D3D12 的 Windows API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明 uix 实际选择 Vulkan feature。
            required_uix_features=("vulkan",),
            # 单选择面入口不得合并其他 backend 与源码 capability。
            forbidden_uix_features=("d3d11", "d3d12", "metal", "opengles", "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 Vulkan 选择面正向场景定义。
        ),
        # Metal 当前只验证无专属 package 的公开选择面，不宣称 Windows 生产实现。
        Scenario(
            # 使用稳定名称显式区分选择面与生产 backend。
            name="metal-selection",
            # 复用统一 backend 选择面 fixture。
            manifest=root / "fixtures" / "usage-build" / "backend-selection" / "Cargo.toml",
            # 说明该入口只覆盖 Metal 公开变体与空 feature 边界。
            description="只验证 metal 公开选择面与空依赖 feature，不作为生产 backend 证据",
            # 选择 fixture 的 Metal 转发 feature。
            feature_args=("--no-default-features", "--features", "metal-selection"),
            # Metal 选择面不得引入其他 backend/capability package 或死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # Metal 空 feature 不得合并任何 D3D Windows API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明 uix 实际选择 Metal feature。
            required_uix_features=("metal",),
            # 单选择面入口不得合并其他 backend 与源码 capability。
            forbidden_uix_features=("d3d11", "d3d12", "opengles", "vulkan", "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 Metal 选择面正向场景定义。
        ),
        # 演示日志禁用入口直接构建根清单二进制。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="demo-logging-disabled",
            # 指向包含 uix-demo 的根清单。
            manifest=root / "Cargo.toml",
            # 说明该入口验证保持演示基础能力但关闭订阅器的二进制。
            description="保持演示基础能力并关闭 demo-logging 的二进制入口",
            # 关闭默认集合并显式启用演示所需的非日志能力。
            feature_args=("--no-default-features", "--features", DEMO_BASE_FEATURES),
            # 只构建演示二进制目标。
            build_args=("--bin", "uix-demo"),
            # 禁用入口必须保留演示基础能力依赖。
            required_packages=("image", "qrcode", "regex"),
            # 禁用入口必须排除死依赖与日志订阅器。
            forbidden_packages=("ash", "glow", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 演示使用的 D3D11 必须选择完整 Win32 图形 API feature 集。
            required_package_features=D3D11_WINDOWS_PACKAGE_FEATURES,
            # 演示基础组合不得合并 D3D12 专属 Windows API feature。
            forbidden_package_features=D3D12_ONLY_WINDOWS_PACKAGE_FEATURES,
            # 演示基础组合必须显式保留 D3D11 与富文本组件能力。
            required_uix_features=("d3d11", "charts", "feedback", "navigation", "rich-text", "table", "tree-widgets"),
            # 演示日志对照不得意外启用 Agent 控制或 OpenGL ES。
            forbidden_uix_features=("agent-control", "opengles"),
        # 结束演示日志禁用场景定义。
        ),
        # 演示日志单能力入口直接构建根清单二进制。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="demo-logging",
            # 指向包含 uix-demo 的根清单。
            manifest=root / "Cargo.toml",
            # 说明该入口在相同演示基础组合上启用日志订阅能力。
            description="保持演示基础能力并打开 demo-logging 的二进制入口",
            # 关闭默认集合并显式启用演示基础能力及日志 capability。
            feature_args=("--no-default-features", "--features", DEMO_LOGGING_FEATURES),
            # 只构建演示二进制目标。
            build_args=("--bin", "uix-demo"),
            # 启用入口必须解析演示基础依赖与日志订阅器。
            required_packages=("image", "qrcode", "regex", "tracing-subscriber"),
            # 演示日志入口不得重新引入已删除的死依赖。
            forbidden_packages=("ash", "glow", "raw-window-handle", "serde_json"),
            # 日志启用入口必须保持相同的 D3D11 Win32 feature 集。
            required_package_features=D3D11_WINDOWS_PACKAGE_FEATURES,
            # 日志启用对照不得合并 D3D12 专属 Windows API feature。
            forbidden_package_features=D3D12_ONLY_WINDOWS_PACKAGE_FEATURES,
            # 日志对照只能增加订阅器，D3D11 与富文本能力必须保持一致。
            required_uix_features=("d3d11", "charts", "feedback", "navigation", "rich-text", "table", "tree-widgets"),
            # 日志启用场景同样不得合并 Agent 控制或 OpenGL ES。
            forbidden_uix_features=("agent-control", "opengles"),
        # 结束演示日志正向场景定义。
        ),
        # 单能力入口只打开设置序列化能力。
        Scenario(
            name="settings-serde",
            manifest=root / "fixtures" / "usage-build" / "settings-serde" / "Cargo.toml",
            description="只打开 settings-serde capability 的入口",
            # 设置序列化必须同时解析 serde 与共享 JSON package。
            required_packages=("serde", "serde_json"),
            # 设置序列化入口不得合并其他能力依赖或已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "tracing-subscriber"),
            # metadata 必须证明只选择设置序列化而非 Agent 控制。
            required_uix_features=("settings-serde",),
            # 同时排除共享 JSON 依赖无法区分的其他源码能力。
            forbidden_uix_features=("agent-control", "charts", "feedback", "navigation", "rich-text", "table"),
        ),
        # Agent 控制单能力入口只打开应用 builder 与本机 IPC 实现。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="agent-control",
            # 指向 Agent 控制正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "agent-control" / "Cargo.toml",
            # 说明该入口只覆盖 Agent 控制 capability。
            description="只打开 agent-control capability 的入口",
            # Agent 控制必须解析与设置能力共享的 JSON package。
            required_packages=("serde_json",),
            # Agent 入口不得合并设置 derive、其他能力依赖或死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了 Agent 控制 feature。
            required_uix_features=("agent-control",),
            # 共享 JSON package 不得掩盖设置或富文本 feature 的误入。
            forbidden_uix_features=("charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        ),
        # 复用 Agent 使用方入口建立 Linux GNU 最终链接证据。
        Scenario(
            # 使用稳定名称区分同一 fixture 的 Linux release 结果。
            name="agent-control-linux",
            # 复用 Agent 控制正向 fixture，避免公开 API 差异污染目标对照。
            manifest=root / "fixtures" / "usage-build" / "agent-control" / "Cargo.toml",
            # 说明该入口覆盖 Linux Agent capability 与最终 ELF 链接。
            description="只打开 agent-control capability 的 Linux GNU release 入口",
            # 显式覆盖宿主目标，让 metadata 与 release 构建绑定同一 Linux triple。
            target="x86_64-unknown-linux-gnu",
            # Linux Agent 必须同时解析 Unix 平台、Wayland 基础与共享 JSON package。
            required_packages=("libc", "serde_json", "wayland-client"),
            # Linux Agent 不得合并其他 capability、Windows 平台依赖或死依赖。
            forbidden_packages=(
                # Vulkan backend 未启用。
                "ash",
                # 未启用图片能力时不得恢复 bytemuck 直接边。
                "bytemuck",
                # OpenGL ES backend 未启用。
                "glow",
                # 图片编解码能力未启用。
                "image",
                # Linux EGL loader 只应随 OpenGL ES backend 启用。
                "khronos-egl",
                # 二维码能力未启用。
                "qrcode",
                # 已删除的窗口句柄死依赖不得回归。
                "raw-window-handle",
                # 表单正则能力未启用。
                "regex",
                # 设置序列化独占的 serde derive package 不得被共享 JSON 掩盖。
                "serde",
                # 演示日志订阅能力未启用。
                "tracing-subscriber",
                # Windows API package 不得进入 Linux 解析图。
                "windows",
                # Windows 宏展开根依赖不得进入 Linux 解析图。
                "windows-core",
            # 结束 Linux Agent 禁用依赖集合。
            ),
            # Linux Agent 不得合并任何 D3D11/D3D12 Win32 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明 uix 实际选择 Agent 控制 feature。
            required_uix_features=("agent-control",),
            # Linux Agent 单能力入口不得合并 backend、富文本或设置能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束 Linux Agent release 场景定义。
        ),
        # 图片编解码单能力入口只打开对应文件格式 capability。
        # 创建图片编解码正向使用方场景。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="image-codecs",
            # 指向图片编解码正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "image-codecs" / "Cargo.toml",
            # 说明该入口只覆盖图片编解码能力。
            description="只打开 image-codecs capability 的入口",
            # 图片编解码入口必须解析精确 image package。
            required_packages=("image",),
            # 图片编解码入口不得合并其他 capability 依赖或已删除的死依赖。
            forbidden_packages=("qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 图片编解码入口不得合并无专属 package 的图表、反馈、导航或表格 capability。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
        # 结束图片编解码正向场景定义。
        ),
        # 二维码单能力入口只打开对应组件 capability。
        Scenario(
            name="qrcode",
            manifest=root / "fixtures" / "usage-build" / "qrcode" / "Cargo.toml",
            description="只打开 qrcode capability 的入口",
            required_packages=("qrcode",),
            # 二维码入口不得合并其他 capability 依赖或已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 二维码入口不得合并无专属 package 的图表、反馈、导航或表格 capability。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
        ),
        # 表单 pattern 单能力入口只打开正则规则 capability。
        Scenario(
            name="form-pattern",
            manifest=root / "fixtures" / "usage-build" / "form-pattern" / "Cargo.toml",
            description="只打开 form-pattern capability 的入口",
            required_packages=("regex",),
            # 表单正则入口不得合并其他 capability 依赖或已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 表单正则入口不得合并无专属 package 的图表、反馈、导航或表格 capability。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
        ),
        # 富文本单能力入口只打开组件、解析、布局与快照公开面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="rich-text",
            # 指向富文本正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "rich-text" / "Cargo.toml",
            # 说明该入口只覆盖富文本 capability。
            description="只打开 rich-text capability 的入口",
            # 富文本能力不应合并其他可选依赖或已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了富文本 feature。
            required_uix_features=("rich-text",),
            # 富文本入口不得合并同为纯源码能力的图表、反馈、导航或表格 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
        # 结束富文本正向场景定义。
        ),
        # 图表单能力入口只打开基础与高级图表公开面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="charts",
            # 指向图表正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "charts" / "Cargo.toml",
            # 说明该入口同时覆盖基础图表与高级图表构造器。
            description="只打开 charts capability 的基础与高级图表入口",
            # 图表纯源码能力不得合并其他可选依赖或已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了图表 feature。
            required_uix_features=("charts",),
            # 单图表入口不得合并 backend、Agent、富文本或设置能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束图表正向场景定义。
        ),
        # 表格单能力入口只打开基础与泛型表格公开面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="table",
            # 指向表格正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "table" / "Cargo.toml",
            # 说明该入口同时覆盖基础表格与泛型数据表格构造器。
            description="只打开 table capability 的基础与泛型表格入口",
            # 表格纯源码能力不得合并其他可选依赖或已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了表格 feature。
            required_uix_features=("table",),
            # 单表格入口不得合并 backend、Agent、图表、反馈、导航、富文本或设置能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde"),
        # 结束表格正向场景定义。
        ),
        # 导航单能力入口覆盖完整组件族及泛型页面绑定公开面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="navigation",
            # 指向导航正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "navigation" / "Cargo.toml",
            # 说明该入口同时覆盖基础导航组件与泛型导航容器。
            description="只打开 navigation capability 的基础与泛型导航入口",
            # 导航纯源码能力不得合并其他可选依赖或已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了导航 feature。
            required_uix_features=("navigation",),
            # 单导航入口不得合并 backend、Agent、图表、反馈、富文本、表格或设置能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "charts", "feedback", "rich-text", "settings-serde", "table"),
        # 结束导航正向场景定义。
        ),
        # 反馈单能力入口覆盖基础组件、弹层组件与全局门面公开面。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="feedback",
            # 指向反馈正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "feedback" / "Cargo.toml",
            # 说明该入口同时覆盖 Alert、Modal 与全局消息通知门面。
            description="只打开 feedback capability 的组件族与全局门面入口",
            # 反馈纯源码能力不得合并其他可选依赖或已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了反馈 feature。
            required_uix_features=("feedback",),
            # 单反馈入口不得合并 backend、Agent、图表、导航、富文本、表格或设置能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "charts", "navigation", "rich-text", "settings-serde", "table"),
        # 结束反馈正向场景定义。
        ),
        # 树组件族单能力入口覆盖展示树、树选择器与公开模型。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="tree-widgets",
            # 指向树组件族正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "tree-widgets" / "Cargo.toml",
            # 说明该入口同时覆盖展示、输入与快照公开面。
            description="只打开 tree-widgets capability 的展示树与树选择器入口",
            # 树组件纯源码能力不得合并其他可选依赖或已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 实际选择了树组件 feature。
            required_uix_features=("tree-widgets",),
            # 单树组件入口不得合并 backend、Agent 或其他纯源码能力。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "agent-control", "charts", "feedback", "navigation", "rich-text", "settings-serde", "table"),
        # 结束树组件正向场景定义。
        ),
        # D3D11 禁用入口必须证明公开 backend 变体无法绕过 feature。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="d3d11-disabled",
            # 指向 D3D11 负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "d3d11-disabled" / "Cargo.toml",
            # 说明该入口必须在公开 backend 变体处编译失败。
            description="关闭 d3d11 backend capability 的公开变体 compile-fail",
            # 禁用入口必须排除 backend、使用方能力依赖与死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # D3D11 禁用入口不得选择任何 D3D11/D3D12 Win32 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明全部 backend feature 都保持关闭。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定缺失枚举变体的稳定诊断与公开名称。
            expected_error_fragments=("no variant", "Direct3D11"),
        # 结束 D3D11 负向场景定义。
        ),
        # OpenGL ES 禁用入口必须证明公开 backend 变体无法绕过 feature。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="opengles-disabled",
            # 指向 OpenGL ES 负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "opengles-disabled" / "Cargo.toml",
            # 说明该入口必须在公开 backend 变体处编译失败。
            description="关闭 opengles backend capability 的公开变体 compile-fail",
            # 禁用入口必须排除 backend、使用方能力依赖与死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # OpenGL ES 禁用入口同样不得合并 D3D11/D3D12 Win32 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明全部 backend feature 都保持关闭。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定缺失枚举变体的稳定诊断与公开名称。
            expected_error_fragments=("no variant", "OpenGlEs"),
        # 结束 OpenGL ES 负向场景定义。
        ),
        # D3D12 禁用入口必须证明公开选择变体无法绕过 uix feature。
        Scenario(
            # 使用稳定名称区分禁用选择面场景。
            name="d3d12-selection-disabled",
            # 复用统一 backend 选择面 fixture，但只启用不转发 uix feature 的分支。
            manifest=root / "fixtures" / "usage-build" / "backend-selection" / "Cargo.toml",
            # 说明该入口必须在 D3D12 公开变体处编译失败。
            description="关闭 d3d12 feature 的公开选择变体 compile-fail",
            # 只启用 fixture 自身的禁用分支，不向 uix 转发 D3D12。
            feature_args=("--no-default-features", "--features", "d3d12-selection-disabled"),
            # 禁用入口必须排除 backend、能力依赖与死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 禁用入口不得合并任何 D3D 图形 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明全部 backend feature 关闭。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定缺失枚举变体与公开名称。
            expected_error_fragments=("no variant", "Direct3D12"),
        # 结束 D3D12 选择面负向场景定义。
        ),
        # Vulkan 禁用入口必须证明公开选择变体与 ash 同步消失。
        Scenario(
            # 使用稳定名称区分禁用选择面场景。
            name="vulkan-selection-disabled",
            # 复用统一 backend 选择面 fixture 的禁用分支。
            manifest=root / "fixtures" / "usage-build" / "backend-selection" / "Cargo.toml",
            # 说明该入口必须在 Vulkan 公开变体处编译失败。
            description="关闭 vulkan feature 的公开选择变体 compile-fail",
            # 只启用 fixture 自身的禁用分支，不向 uix 转发 Vulkan。
            feature_args=("--no-default-features", "--features", "vulkan-selection-disabled"),
            # Vulkan 禁用入口必须排除 ash 与其他能力依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 禁用入口不得合并任何 D3D 图形 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明全部 backend feature 关闭。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定缺失枚举变体与公开名称。
            expected_error_fragments=("no variant", "Vulkan"),
        # 结束 Vulkan 选择面负向场景定义。
        ),
        # Metal 禁用入口必须证明空依赖 feature 仍控制公开变体。
        Scenario(
            # 使用稳定名称区分禁用选择面场景。
            name="metal-selection-disabled",
            # 复用统一 backend 选择面 fixture 的禁用分支。
            manifest=root / "fixtures" / "usage-build" / "backend-selection" / "Cargo.toml",
            # 说明该入口必须在 Metal 公开变体处编译失败。
            description="关闭 metal feature 的公开选择变体 compile-fail",
            # 只启用 fixture 自身的禁用分支，不向 uix 转发 Metal。
            feature_args=("--no-default-features", "--features", "metal-selection-disabled"),
            # Metal 禁用入口必须排除其他 backend/capability package 与死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 禁用入口不得合并任何 D3D 图形 API feature。
            forbidden_package_features=GRAPHICS_WINDOWS_PACKAGE_FEATURES,
            # metadata 必须证明全部 backend feature 关闭。
            forbidden_uix_features=(*GRAPHICS_BACKEND_FEATURES, "charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定缺失枚举变体与公开名称。
            expected_error_fragments=("no variant", "Metal"),
        # 结束 Metal 选择面负向场景定义。
        ),
        # 二维码禁用入口必须证明公开类型无法绕过 capability。
        Scenario(
            name="qrcode-disabled",
            manifest=root / "fixtures" / "usage-build" / "qrcode-disabled" / "Cargo.toml",
            description="关闭 qrcode capability 的公开入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 禁用入口必须证明图表、反馈、导航与表格 feature 没有随其他源码能力误入。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            expected_compile_failure=True,
            expected_error_fragments=("unresolved import", "QRCode"),
        ),
        # 表单 pattern 禁用入口必须证明 builder 方法无法绕过 capability。
        Scenario(
            name="form-pattern-disabled",
            manifest=root / "fixtures" / "usage-build" / "form-pattern-disabled" / "Cargo.toml",
            description="关闭 form-pattern capability 的公开方法 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 禁用入口必须证明图表、反馈、导航与表格 feature 没有随其他源码能力误入。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            expected_compile_failure=True,
            expected_error_fragments=("no method named", "validate_pattern"),
        ),
        # 图片编解码禁用入口必须证明公开方法无法绕过 capability。
        # 创建图片编解码负向使用方场景。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="image-codecs-disabled",
            # 指向图片编解码负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "image-codecs-disabled" / "Cargo.toml",
            # 说明该入口必须在公开方法编译阶段失败。
            description="关闭 image-codecs capability 的公开方法 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 禁用入口必须证明图表、反馈、导航与表格 feature 没有随其他源码能力误入。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定稳定的缺失方法诊断片段。
            expected_error_fragments=("no method named", "load_from_bytes"),
        # 结束图片编解码负向场景定义。
        ),
        # 富文本禁用入口必须证明类型与辅助函数都无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="rich-text-disabled",
            # 指向富文本负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "rich-text-disabled" / "Cargo.toml",
            # 说明该入口必须在公开导入阶段失败。
            description="关闭 rich-text capability 的公开入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择富文本 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "rich-text", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定组件类型与解析辅助函数的缺失诊断。
            expected_error_fragments=("unresolved import", "RichText", "parse_rich_text"),
        # 结束富文本负向场景定义。
        ),
        # 图表禁用入口必须证明基础与高级组件类型都无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="charts-disabled",
            # 指向图表负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "charts-disabled" / "Cargo.toml",
            # 说明该入口必须在公开导入阶段失败。
            description="关闭 charts capability 的基础与高级图表入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择图表 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定基础图表与高级图表类型的缺失诊断。
            expected_error_fragments=("unresolved import", "BarChart", "Gauge"),
        # 结束图表负向场景定义。
        ),
        # 表格禁用入口必须证明基础与泛型表格类型都无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="table-disabled",
            # 指向表格负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "table-disabled" / "Cargo.toml",
            # 说明该入口必须在公开导入阶段失败。
            description="关闭 table capability 的基础与泛型表格入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择图表或表格 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定基础表格与泛型表格类型的缺失诊断。
            expected_error_fragments=("unresolved import", "Table", "DataTable"),
        # 结束表格负向场景定义。
        ),
        # 导航禁用入口必须证明基础与泛型导航类型都无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="navigation-disabled",
            # 指向导航负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "navigation-disabled" / "Cargo.toml",
            # 说明该入口必须在公开导入阶段失败。
            description="关闭 navigation capability 的基础与泛型导航入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择图表、反馈、导航或表格 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定基础面包屑与泛型导航容器的缺失诊断。
            expected_error_fragments=("unresolved import", "Breadcrumb", "Navigation"),
        # 结束导航负向场景定义。
        ),
        # 反馈禁用入口必须证明组件族与全局门面都无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="feedback-disabled",
            # 指向反馈负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "feedback-disabled" / "Cargo.toml",
            # 说明该入口必须在组件与门面公开导入阶段失败。
            description="关闭 feedback capability 的组件族与全局门面 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择反馈及其他纯源码 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定基础组件、弹层组件与两个全局门面函数的缺失诊断。
            expected_error_fragments=("unresolved import", "Alert", "Modal", "message", "notify"),
        # 结束反馈负向场景定义。
        ),
        # 树组件禁用入口必须证明展示、输入与快照公开面都无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="tree-widgets-disabled",
            # 指向树组件族负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "tree-widgets-disabled" / "Cargo.toml",
            # 说明该入口必须在公开导入阶段失败。
            description="关闭 tree-widgets capability 的展示树与树选择器入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("ash", "bytemuck", "glow", "image", "khronos-egl", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择树组件及其他纯源码 feature。
            forbidden_uix_features=("charts", "feedback", "navigation", "rich-text", "table", "tree-widgets"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定展示、输入、拖拽与快照公开模型的缺失诊断。
            expected_error_fragments=("unresolved import", "DropPosition", "SnapshotTreeNode", "Tree", "TreeSelect"),
        # 结束树组件负向场景定义。
        ),
        # Agent 控制禁用入口必须证明应用 builder 方法无法绕过 capability。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="agent-control-disabled",
            # 指向 Agent 控制负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "agent-control-disabled" / "Cargo.toml",
            # 说明该入口必须在公开方法编译阶段失败。
            description="关闭 agent-control capability 的公开方法 compile-fail",
            # 禁用入口必须排除共享序列化依赖、其他能力依赖与死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde", "serde_json", "tracing-subscriber"),
            # metadata 必须证明 uix 没有选择 Agent 控制 feature。
            forbidden_uix_features=("agent-control", "charts", "feedback", "navigation", "table"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定公开 builder 方法缺失的稳定诊断片段。
            expected_error_fragments=("no method named", "enable_agent_control"),
        # 结束 Agent 控制负向场景定义。
        ),
    ]
    # 返回已经补齐全部纯源码 capability 断言的稳定场景列表。
    return [_complete_pure_source_feature_guards(scenario) for scenario in scenarios]
