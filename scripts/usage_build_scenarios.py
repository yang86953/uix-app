"""定义使用方构建采集器的稳定场景矩阵。"""

# 启用未来注解，避免运行时解析类型前向引用。
from __future__ import annotations

# 导入数据类工具。
from dataclasses import dataclass
# 导入路径类型。
from pathlib import Path

# 记录 uix-demo 除日志订阅外必须启用的既有能力组合。
DEMO_BASE_FEATURES = "d3d11,image-codecs,qrcode,form-pattern,rich-text"
# 记录在相同演示能力组合上额外启用日志订阅的对照集合。
DEMO_LOGGING_FEATURES = f"{DEMO_BASE_FEATURES},demo-logging"


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
    # 记录 uix resolve 节点中必须启用的 capability feature。
    required_uix_features: tuple[str, ...] = ()
    # 记录 uix resolve 节点中必须关闭的 capability feature。
    forbidden_uix_features: tuple[str, ...] = ()
    # 标记该场景是否必须在公开入口编译阶段失败。
    expected_compile_failure: bool = False
    # 记录 compile-fail 场景必须出现的错误片段。
    expected_error_fragments: tuple[str, ...] = ()


# 返回仓库根目录。
def project_root() -> Path:
    # 当前脚本位于仓库根目录下的 scripts 目录。
    return Path(__file__).resolve().parents[1]


# 返回 ODC-01/ODC-07 的独立 fixture 与根二进制场景定义。
def scenario_specs(root: Path) -> list[Scenario]:
    # 返回最小、默认、单能力、根二进制与禁用公开面入口。
    return [
        # 最小入口关闭所有默认 feature。
        Scenario(
            name="minimal",
            manifest=root / "fixtures" / "usage-build" / "minimal" / "Cargo.toml",
            description="关闭默认 feature 的最小使用方入口",
            # 最小入口必须排除全部已独立裁剪的专属依赖及已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
            # 最小入口必须证明非默认源码与数据能力未被 Cargo 选择。
            forbidden_uix_features=("agent-control", "rich-text", "settings-serde"),
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
                # 已删除的 bytemuck 直接边不得在未启用图片能力时回归。
                "bytemuck",
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
            # Linux 最小入口同样不得选择 Agent、富文本与设置能力。
            forbidden_uix_features=("agent-control", "rich-text", "settings-serde"),
        # 结束 Linux 最小场景定义。
        ),
        # 默认入口覆盖当前 Windows 默认 D3D11 能力。
        Scenario(
            name="d3d11-default",
            manifest=root / "fixtures" / "usage-build" / "d3d11-default" / "Cargo.toml",
            description="使用当前默认 feature 的图形入口",
            # 默认兼容集合必须包含三项能力依赖与演示日志订阅器。
            required_packages=("image", "qrcode", "regex", "tracing-subscriber"),
            # 已删除的死依赖在默认入口中也必须保持缺席。
            forbidden_packages=("raw-window-handle", "serde_json"),
            # 默认兼容集合必须继续包含富文本公开能力。
            required_uix_features=("rich-text",),
            # Agent 控制不属于默认兼容集合。
            forbidden_uix_features=("agent-control",),
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
            forbidden_packages=("raw-window-handle", "serde_json", "tracing-subscriber"),
            # 演示基础组合必须显式保留富文本组件能力。
            required_uix_features=("rich-text",),
            # 演示日志对照不得意外启用 Agent 控制。
            forbidden_uix_features=("agent-control",),
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
            forbidden_packages=("raw-window-handle", "serde_json"),
            # 日志对照只能增加订阅器，富文本能力必须与禁用场景一致。
            required_uix_features=("rich-text",),
            # 日志启用场景同样不得合并 Agent 控制。
            forbidden_uix_features=("agent-control",),
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
            # 同时排除共享 JSON 依赖无法区分的两个源码能力。
            forbidden_uix_features=("agent-control", "rich-text"),
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
            forbidden_uix_features=("rich-text", "settings-serde"),
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
        ),
        # 表单 pattern 单能力入口只打开正则规则 capability。
        Scenario(
            name="form-pattern",
            manifest=root / "fixtures" / "usage-build" / "form-pattern" / "Cargo.toml",
            description="只打开 form-pattern capability 的入口",
            required_packages=("regex",),
            # 表单正则入口不得合并其他 capability 依赖或已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "raw-window-handle", "serde_json", "tracing-subscriber"),
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
        # 结束富文本正向场景定义。
        ),
        # 二维码禁用入口必须证明公开类型无法绕过 capability。
        Scenario(
            name="qrcode-disabled",
            manifest=root / "fixtures" / "usage-build" / "qrcode-disabled" / "Cargo.toml",
            description="关闭 qrcode capability 的公开入口 compile-fail",
            # 禁用入口必须排除全部专属依赖及已删除的死依赖。
            forbidden_packages=("bytemuck", "image", "qrcode", "regex", "raw-window-handle", "serde_json", "tracing-subscriber"),
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
            forbidden_uix_features=("rich-text",),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 同时绑定组件类型与解析辅助函数的缺失诊断。
            expected_error_fragments=("unresolved import", "RichText", "parse_rich_text"),
        # 结束富文本负向场景定义。
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
            forbidden_uix_features=("agent-control",),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定公开 builder 方法缺失的稳定诊断片段。
            expected_error_fragments=("no method named", "enable_agent_control"),
        # 结束 Agent 控制负向场景定义。
        ),
    ]
