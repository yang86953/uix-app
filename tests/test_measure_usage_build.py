"""验证使用方构建采集器的解析图归一化。"""

# 导入标准单元测试框架。
import unittest
# 导入环境变量隔离补丁工具。
from unittest.mock import patch

# 导入被测采集模块。
from scripts import measure_usage_build


# 覆盖 capability 依赖断言使用的解析图辅助函数。
class ResolvedGraphTests(unittest.TestCase):
    # 构造同时包含激活与未激活 package 的 Cargo metadata 摘要。
    def metadata(self) -> dict:
        # 返回两个实际节点和一个仅存在于清单信息中的 optional package。
        return {
            "packages": [
                {"id": "root", "name": "root", "version": "1.0.0", "source": None},
                {"id": "active", "name": "active", "version": "2.0.0", "source": "registry"},
                {"id": "optional", "name": "optional", "version": "3.0.0", "source": "registry"},
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": "root",
                        "deps": [{"name": "active", "pkg": "active"}],
                        "features": ["selected"],
                    },
                    {"id": "active", "deps": [], "features": []},
                ]
            },
        }

    # 确认依赖存在性断言只观察实际 resolve 节点。
    def test_resolved_package_names_exclude_inactive_optional_package(self) -> None:
        # 提取实际解析到的 package 名称。
        names = measure_usage_build.resolved_package_names(self.metadata())
        # 未激活的 optional package 不得进入结果。
        self.assertEqual(names, {"root", "active"})

    # 确认 capability 断言只读取指定 package 的实际 resolve feature。
    def test_resolved_package_features_use_selected_resolve_node(self) -> None:
        # 提取根 package 当前真实选择的 feature。
        features = measure_usage_build.resolved_package_features(self.metadata(), "root")
        # 结果必须包含 resolve 节点的 selected，且不猜测未激活 feature。
        self.assertEqual(features, {"selected"})

    # 确认 package feature 正反断言只观察实际 resolve feature。
    def test_package_feature_assertions_use_selected_resolve_features(self) -> None:
        # 构造一个存在 feature 与一个不存在 feature 的断言集合。
        assertions = (("root", "selected"), ("root", "missing"))
        # 必需断言只应报告没有被 Cargo 选择的 feature。
        self.assertEqual(
            measure_usage_build.missing_required_package_features(
                self.metadata(), assertions
            ),
            ["root/missing"],
        )
        # 禁用断言只应报告真实进入解析图的 feature。
        self.assertEqual(
            measure_usage_build.present_forbidden_package_features(
                self.metadata(), assertions
            ),
            ["root/selected"],
        )

    # 确认结构化摘要使用实际依赖边与启用 feature。
    def test_resolved_graph_summary_uses_resolve_edges(self) -> None:
        # 生成稳定排序的解析图摘要。
        summary = measure_usage_build.resolved_graph_summary(self.metadata())
        # 摘要的 package 和节点数量必须一致。
        self.assertEqual(summary["package_count"], 2)
        # 取出根 package 的实际解析记录。
        root = next(package for package in summary["packages"] if package["name"] == "root")
        # 根 package 只记录实际激活的依赖边。
        self.assertEqual(root["dependencies"], ["active"])
        # 根 package 同时记录 Cargo 选择的 feature。
        self.assertEqual(root["features"], ["selected"])

    # 确认 compile-fail 门禁同时检查所有必需诊断片段。
    def test_missing_error_fragments_reports_incomplete_diagnostics(self) -> None:
        # 构造只包含类型名、不包含错误类别的编译诊断。
        result = {"_stdout": "", "_stderr": "QRCode is unavailable"}
        # 检查两个预期诊断片段。
        missing = measure_usage_build.missing_error_fragments(
            result, ("unresolved import", "QRCode")
        )
        # 仅未出现的错误类别必须被报告。
        self.assertEqual(missing, ["unresolved import"])

    # 确认演示日志场景分别绑定禁用与单能力根二进制构建参数。
    def test_demo_logging_scenarios_bind_root_binary_and_feature_graph(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 文档声明的 schema v6 矩阵必须保持二十一个独立执行场景。
        self.assertEqual(len(scenarios), 21)
        # 按稳定名称索引场景。
        by_name = {scenario.name: scenario for scenario in scenarios}
        # 读取关闭演示日志的根二进制场景。
        disabled = by_name["demo-logging-disabled"]
        # 禁用场景必须显式保留演示基础能力但不含日志 feature。
        self.assertEqual(disabled.feature_args, ("--no-default-features", "--features", measure_usage_build.DEMO_BASE_FEATURES))
        # 禁用场景必须只构建 uix-demo。
        self.assertEqual(disabled.build_args, ("--bin", "uix-demo"))
        # 禁用场景必须阻止日志订阅器进入解析图。
        self.assertIn("tracing-subscriber", disabled.forbidden_packages)
        # 禁用场景仍必须要求演示使用的三项能力依赖存在。
        self.assertEqual(disabled.required_packages, ("image", "qrcode", "regex"))
        # 演示基础组合必须显式选择 D3D11 与无专属 package 的富文本 capability。
        self.assertEqual(disabled.required_uix_features, ("d3d11", "rich-text"))
        # 读取只启用演示日志的根二进制场景。
        enabled = by_name["demo-logging"]
        # 启用场景必须在同一基础组合上增加 demo-logging capability。
        self.assertEqual(enabled.feature_args, ("--no-default-features", "--features", measure_usage_build.DEMO_LOGGING_FEATURES))
        # 启用场景必须要求基础依赖与日志订阅器共同进入解析图。
        self.assertEqual(enabled.required_packages, ("image", "qrcode", "regex", "tracing-subscriber"))
        # 日志对照不得改变演示所需的 D3D11 与富文本 capability。
        self.assertEqual(enabled.required_uix_features, ("d3d11", "rich-text"))
        # 两个根二进制场景必须使用同一根清单以便比较。
        self.assertEqual(enabled.manifest, disabled.manifest)

    # 确认 D3D11 与 OpenGL ES 正反场景同时覆盖依赖、feature 与公开变体。
    def test_graphics_backend_scenarios_bind_dependency_and_public_api_guards(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 按稳定名称索引场景。
        by_name = {scenario.name: scenario for scenario in scenarios}
        # 读取只启用 D3D11 的正向入口。
        d3d11 = by_name["d3d11"]
        # 正向入口必须要求 uix 实际选择 d3d11 feature。
        self.assertEqual(d3d11.required_uix_features, ("d3d11",))
        # D3D11 必须要求完整的 windows package API feature 集。
        self.assertEqual(
            d3d11.required_package_features,
            measure_usage_build.D3D11_WINDOWS_PACKAGE_FEATURES,
        )
        # D3D11 单 backend 入口不得解析 OpenGL ES 的 glow package。
        self.assertIn("glow", d3d11.forbidden_packages)
        # 读取关闭 D3D11 的负向入口。
        d3d11_disabled = by_name["d3d11-disabled"]
        # 负向入口必须禁止全部 backend feature 意外进入解析图。
        self.assertEqual(
            d3d11_disabled.forbidden_uix_features,
            measure_usage_build.GRAPHICS_BACKEND_FEATURES,
        )
        # D3D11 的 Win32 API feature 在禁用入口中必须全部缺席。
        self.assertEqual(
            d3d11_disabled.forbidden_package_features,
            measure_usage_build.D3D11_WINDOWS_PACKAGE_FEATURES,
        )
        # 负向诊断必须绑定缺失变体与公开名称。
        self.assertEqual(
            d3d11_disabled.expected_error_fragments,
            ("no variant", "Direct3D11"),
        )
        # 读取只启用 OpenGL ES 的正向入口。
        opengles = by_name["opengles"]
        # OpenGL ES 正向入口必须解析 glow package。
        self.assertEqual(opengles.required_packages, ("glow",))
        # metadata 必须证明 uix 实际选择 opengles feature。
        self.assertEqual(opengles.required_uix_features, ("opengles",))
        # OpenGL ES 不得合并 D3D11 的 windows API feature。
        self.assertEqual(
            opengles.forbidden_package_features,
            measure_usage_build.D3D11_WINDOWS_PACKAGE_FEATURES,
        )
        # 读取关闭 OpenGL ES 的负向入口。
        opengles_disabled = by_name["opengles-disabled"]
        # 禁用入口必须阻止 glow package 误入解析图。
        self.assertIn("glow", opengles_disabled.forbidden_packages)
        # 负向诊断必须绑定缺失变体与公开名称。
        self.assertEqual(
            opengles_disabled.expected_error_fragments,
            ("no variant", "OpenGlEs"),
        )

    # 确认富文本正反场景同时覆盖 feature 解析与公开面收缩。
    def test_rich_text_scenarios_bind_feature_and_public_api_guards(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 按稳定名称索引场景。
        by_name = {scenario.name: scenario for scenario in scenarios}
        # 读取只启用富文本能力的正向入口。
        enabled = by_name["rich-text"]
        # 正向入口必须要求 Cargo 实际选择 rich-text feature。
        self.assertEqual(enabled.required_uix_features, ("rich-text",))
        # 正向入口不得依赖其他可选第三方 capability package。
        self.assertIn("image", enabled.forbidden_packages)
        # 读取关闭富文本能力的负向入口。
        disabled = by_name["rich-text-disabled"]
        # 负向入口必须禁止 rich-text feature 意外进入解析图。
        self.assertEqual(disabled.forbidden_uix_features, ("rich-text",))
        # 负向入口必须以公开导入失败结束。
        self.assertTrue(disabled.expected_compile_failure)
        # 诊断必须同时覆盖组件类型和解析辅助函数。
        self.assertEqual(
            disabled.expected_error_fragments,
            ("unresolved import", "RichText", "parse_rich_text"),
        )

    # 确认 Agent 与设置能力共享 JSON package 时仍保持 feature 与公开面隔离。
    def test_agent_control_scenarios_bind_shared_json_and_public_api_guards(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 按稳定名称索引场景。
        by_name = {scenario.name: scenario for scenario in scenarios}
        # 读取设置序列化单能力入口。
        settings = by_name["settings-serde"]
        # 设置入口必须解析 serde 与共享 serde_json package。
        self.assertEqual(settings.required_packages, ("serde", "serde_json"))
        # 设置入口必须只选择自己的 uix feature。
        self.assertEqual(settings.required_uix_features, ("settings-serde",))
        # 共享 package 不得使 Agent feature 意外进入解析图。
        self.assertIn("agent-control", settings.forbidden_uix_features)
        # 读取只启用 Agent 控制的正向入口。
        enabled = by_name["agent-control"]
        # Agent 入口必须解析共享 JSON package。
        self.assertEqual(enabled.required_packages, ("serde_json",))
        # Agent 不需要设置能力独占的 serde derive 直接边。
        self.assertIn("serde", enabled.forbidden_packages)
        # metadata 必须证明 Agent feature 已启用。
        self.assertEqual(enabled.required_uix_features, ("agent-control",))
        # Agent 入口不得合并设置序列化 feature。
        self.assertIn("settings-serde", enabled.forbidden_uix_features)
        # 读取复用相同公开入口的 Linux release 场景。
        linux = by_name["agent-control-linux"]
        # Linux Agent 场景必须显式覆盖为 GNU 目标。
        self.assertEqual(linux.target, "x86_64-unknown-linux-gnu")
        # Linux Agent 必须执行最终 release 链接而不是降级为 check。
        self.assertFalse(linux.check_only)
        # Linux Agent 必须解析 Unix、Wayland 与共享 JSON package。
        self.assertEqual(linux.required_packages, ("libc", "serde_json", "wayland-client"))
        # Linux 解析图必须排除 Windows API package。
        self.assertIn("windows", linux.forbidden_packages)
        # Linux 入口必须选择相同的 Agent 根 feature。
        self.assertEqual(linux.required_uix_features, ("agent-control",))
        # 两个目标必须复用同一 fixture，避免公开 API 差异污染对照。
        self.assertEqual(linux.manifest, enabled.manifest)
        # 读取关闭 Agent 控制的负向入口。
        disabled = by_name["agent-control-disabled"]
        # 负向入口必须排除 Agent 独占的 JSON 直接依赖。
        self.assertIn("serde_json", disabled.forbidden_packages)
        # 负向入口必须禁止 Agent feature 意外进入解析图。
        self.assertEqual(disabled.forbidden_uix_features, ("agent-control",))
        # 负向入口必须以公开 builder 方法不可用结束。
        self.assertTrue(disabled.expected_compile_failure)
        # 诊断必须绑定缺失方法和稳定公开方法名。
        self.assertEqual(
            disabled.expected_error_fragments,
            ("no method named", "enable_agent_control"),
        )

    # 确认共享 JSON package 只进入显式启用设置或 Agent 的场景。
    def test_serde_json_is_scoped_to_settings_and_agent_control(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 记录当前允许解析 serde_json 的三个能力入口。
        json_scenarios = {"settings-serde", "agent-control", "agent-control-linux"}
        # 逐场景核对共享 package 的正反断言。
        for scenario in scenarios:
            # 三个显式能力入口必须要求 JSON package 存在。
            if scenario.name in json_scenarios:
                # 共享 package 必须进入对应正向解析图。
                self.assertIn("serde_json", scenario.required_packages)
                # 当前场景已经完成正向分支核对。
                continue
            # 其余场景必须防止 Agent 或设置能力依赖误入。
            self.assertIn("serde_json", scenario.forbidden_packages)

    # 确认删除的 bytemuck 直接边只在未启用图片能力的场景中执行 package 缺席断言。
    def test_bytemuck_is_forbidden_without_image_codecs(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 逐场景核对图片能力与 bytemuck 断言的对应关系。
        for scenario in scenarios:
            # 启用 image 的场景允许其合法传递依赖继续存在。
            if "image" in scenario.required_packages:
                # 不得把上游 image 的 bytemuck 传递边误判为回归。
                self.assertNotIn("bytemuck", scenario.forbidden_packages)
                # 当前场景已经完成对应分支核对。
                continue
            # 未启用 image 时，bytemuck 不得由 uix 的直接边重新进入解析图。
            self.assertIn("bytemuck", scenario.forbidden_packages)

    # 确认 Linux 最小场景覆盖目标、依赖边界与正向检查命令。
    def test_minimal_linux_scenario_binds_target_and_platform_graph(self) -> None:
        # 读取当前仓库的全部场景定义。
        scenarios = measure_usage_build.scenario_specs(measure_usage_build.project_root())
        # 按稳定名称索引场景。
        by_name = {scenario.name: scenario for scenario in scenarios}
        # 读取显式绑定 Linux GNU 目标的最小场景。
        linux = by_name["minimal-linux"]
        # 场景必须覆盖宿主 target，形成真实非 Windows 轴。
        self.assertEqual(linux.target, "x86_64-unknown-linux-gnu")
        # Windows 宿主不具备 Linux 链接器时仍须执行真实类型检查。
        self.assertTrue(linux.check_only)
        # Linux 平台基础依赖必须进入目标过滤后的解析图。
        self.assertEqual(linux.required_packages, ("libc", "wayland-client"))
        # Windows API 与宏展开根依赖不得进入 Linux 解析图。
        self.assertIn("windows", linux.forbidden_packages)
        # 精确的 windows-core package 同样必须缺席。
        self.assertIn("windows-core", linux.forbidden_packages)
        # 以 dry-run 生成目标感知命令而不创建构建产物。
        record = measure_usage_build.measure_scenario(
            # 传入 Linux 最小场景。
            linux,
            # 传入仓库根目录。
            measure_usage_build.project_root(),
            # 使用占位 Cargo 命令。
            "cargo",
            # 提供不同的 Windows host，验证场景 target 确实覆盖它。
            "x86_64-pc-windows-gnu",
            # 要求命令包含 locked 门禁。
            True,
            # 跳过前置清理以缩短命令列表。
            False,
            # 跳过后置清理以缩短命令列表。
            False,
            # 启用 dry-run，禁止外部副作用。
            True,
        )
        # 报告必须表达正向类型检查语义。
        self.assertEqual(record["expected_outcome"], "check-success")
        # 报告必须保存场景覆盖后的 Linux target。
        self.assertEqual(record["target"], "x86_64-unknown-linux-gnu")
        # 找到目标过滤的 metadata 命令。
        metadata_step = next(step for step in record["steps"] if step["name"] == "metadata")
        # 找到正向 cargo check 命令。
        check_step = next(step for step in record["steps"] if step["name"] == "check")
        # metadata 必须按 Linux target 过滤解析图。
        self.assertIn("--filter-platform x86_64-unknown-linux-gnu", metadata_step["command"])
        # 类型检查必须使用同一个 Linux target。
        self.assertIn("--target x86_64-unknown-linux-gnu", check_step["command"])
        # 跨目标检查场景不得伪造 release 构建步骤。
        self.assertNotIn("build-release", {step["name"] for step in record["steps"]})

    # 确认 Linux Agent release 必须绑定并探针显式交叉 linker。
    def test_agent_control_linux_release_records_explicit_linker(self) -> None:
        # 读取仓库根目录和完整场景矩阵。
        root = measure_usage_build.project_root()
        # 按稳定名称索引场景。
        by_name = {
            scenario.name: scenario
            for scenario in measure_usage_build.scenario_specs(root)
        }
        # 读取 Linux Agent release 场景。
        linux = by_name["agent-control-linux"]
        # 计算 Cargo 为 Linux GNU 目标读取的标准 linker 环境变量名。
        linker_environment = measure_usage_build.target_linker_environment_name(linux.target)
        # 环境变量名必须符合 Cargo 的全大写下划线约定。
        self.assertEqual(
            linker_environment,
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
        )
        # 用空覆盖模拟 Windows 交叉宿主缺少 Linux linker 的真实配置。
        with patch.dict(
            measure_usage_build.os.environ,
            {linker_environment: ""},
            clear=False,
        ):
            # 禁用清理并执行非 dry-run；门禁必须在任何 Cargo 调用前失败。
            missing_linker_record = measure_usage_build.measure_scenario(
                # 传入 Linux Agent release 场景。
                linux,
                # 传入仓库根目录。
                root,
                # 使用不会被执行的占位 Cargo 命令。
                "cargo-must-not-run",
                # 提供不同的 Windows host，触发跨目标 linker 门禁。
                "x86_64-pc-windows-gnu",
                # locked 参数在前置门禁失败前不会进入命令。
                True,
                # 跳过前置清理，确保测试没有外部副作用。
                False,
                # 跳过后置清理，确保测试没有外部副作用。
                False,
                # 使用真实模式验证缺失 linker 会被拒绝。
                False,
            )
        # 缺少 linker 的跨目标 release 必须保持失败状态。
        self.assertEqual(missing_linker_record["status"], "failed")
        # 失败原因必须指出调用方需要设置的标准环境变量。
        self.assertIn(linker_environment, missing_linker_record["error"])
        # 前置 linker 门禁不得执行任何 Cargo 或构建步骤。
        self.assertEqual(missing_linker_record["steps"], [])
        # 临时注入稳定占位 linker，验证证据记录而不执行外部工具。
        with patch.dict(
            measure_usage_build.os.environ,
            {linker_environment: "zigcc"},
            clear=False,
        ):
            # 以 dry-run 生成跨目标 release 命令与 linker 探针。
            record = measure_usage_build.measure_scenario(
                # 传入 Linux Agent release 场景。
                linux,
                # 传入仓库根目录。
                root,
                # 使用占位 Cargo 命令。
                "cargo",
                # 提供不同的 Windows host，触发跨目标 linker 门禁。
                "x86_64-pc-windows-gnu",
                # 要求命令包含 locked 门禁。
                True,
                # 跳过前置清理以缩短命令列表。
                False,
                # 跳过后置清理以缩短命令列表。
                False,
                # 启用 dry-run，禁止外部副作用。
                True,
            )
        # 报告必须表达最终 release 构建语义。
        self.assertEqual(record["expected_outcome"], "build-success")
        # 报告必须保存显式 linker 环境变量和值。
        self.assertEqual(
            record["cross_target_linker"]["environment"], linker_environment
        )
        # 报告必须保存本轮实际使用的 linker 覆盖。
        self.assertEqual(record["cross_target_linker"]["override"], "zigcc")
        # dry-run 也必须生成可复核的 linker 版本探针命令。
        linker_step = next(
            step for step in record["steps"] if step["name"] == "linker-probe"
        )
        # 探针必须调用同一个显式 linker。
        self.assertEqual(linker_step["command"], "zigcc --version")
        # 找到 Linux release 构建步骤。
        build_step = next(
            step for step in record["steps"] if step["name"] == "build-release"
        )
        # 最终构建必须绑定 Linux GNU target。
        self.assertIn("--target x86_64-unknown-linux-gnu", build_step["command"])
        # Linux Agent release 不得退化为正向 check。
        self.assertNotIn("check", {step["name"] for step in record["steps"]})

    # 确认 rustc 详细版本输出能稳定提取 host target。
    def test_parse_rustc_host_target(self) -> None:
        # 构造包含真实格式 host 字段的详细版本输出。
        verbose_version = "rustc 1.97.1\nbinary: rustc\nhost: x86_64-pc-windows-gnu\n"
        # 解析结果必须等于报告和 Cargo 命令使用的 target triple。
        self.assertEqual(
            measure_usage_build.parse_rustc_host_target(verbose_version),
            "x86_64-pc-windows-gnu",
        )
        # 缺少 host 字段时不得猜测目标平台。
        self.assertIsNone(measure_usage_build.parse_rustc_host_target("rustc 1.97.1\n"))

    # 确认 dry-run 同时绑定 metadata 过滤目标和实际构建目标。
    def test_measure_scenario_binds_target_to_metadata_and_build(self) -> None:
        # 读取仓库根目录供场景清单定位。
        root = measure_usage_build.project_root()
        # 构造不执行外部命令的最小场景。
        scenario = measure_usage_build.Scenario(
            # 使用稳定测试名称。
            name="target-binding",
            # 复用现有最小 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "minimal" / "Cargo.toml",
            # 说明该场景只验证命令生成。
            description="验证 target 参数绑定",
        # 结束测试场景定义。
        )
        # 以 dry-run 生成完整场景命令而不创建构建产物。
        record = measure_usage_build.measure_scenario(
            # 传入测试场景。
            scenario,
            # 传入仓库根目录。
            root,
            # 使用占位 Cargo 命令。
            "cargo",
            # 绑定稳定的测试 host target。
            "x86_64-pc-windows-gnu",
            # 要求命令包含 locked 门禁。
            True,
            # 跳过前置清理以缩短命令列表。
            False,
            # 跳过后置清理以缩短命令列表。
            False,
            # 启用 dry-run，禁止外部副作用。
            True,
        )
        # 找到 metadata 命令记录。
        metadata_step = next(step for step in record["steps"] if step["name"] == "metadata")
        # 找到 release build 命令记录。
        build_step = next(step for step in record["steps"] if step["name"] == "build-release")
        # metadata 必须按同一 target 过滤 resolved graph。
        self.assertIn("--filter-platform x86_64-pc-windows-gnu", metadata_step["command"])
        # release build 必须显式使用同一 target。
        self.assertIn("--target x86_64-pc-windows-gnu", build_step["command"])
        # 场景报告必须保存最终生效的 target。
        self.assertEqual(record["target"], "x86_64-pc-windows-gnu")


# 允许直接运行本文件执行测试。
if __name__ == "__main__":
    # 把 unittest 退出状态传递给调用方。
    unittest.main()
