// 声明本文件只编译运行保障使用文档，不安装 subscriber 或启动应用。
#![allow(dead_code)]

// 隔离 diagnostics-report 围栏中的报告与恢复订阅。
mod diagnostics_report {
    // 引入文档承诺的公开 typed Error prelude。
    use uix::prelude::*;
    // 引入公开 Diagnostics runtime 与配置。
    use uix::diagnostics::{Diagnostics, DiagnosticsConfig, RecoveryAction};

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "diagnostics-report";

    // 编译有界报告与 owner 登记恢复动作的公开路径。
    fn compile_example() {
        // 创建使用默认固定容量的 Diagnostics runtime。
        let diagnostics = Diagnostics::new(DiagnosticsConfig::default());

        // 报告保留类别与来源边界的 typed Error。
        let _report_id =
            // 把平台错误交给共享诊断 runtime。
            diagnostics.report(Error::new(Errc::PlatformError, "failed to open file"));

        // 为图形设备丢失登记资源 owner 的恢复动作。
        let _subscription = diagnostics.on_error(Errc::GraphicsDeviceLost, |_error| {
            // 文档示例把实际资源重建留给 owner 实现。
            RecoveryAction::Recovered
        });
    }
}

// 隔离 diagnostics-tracing 围栏中的宿主 subscriber 配置。
mod diagnostics_tracing {
    // 引入宿主选择的环境过滤器。
    use tracing_subscriber::EnvFilter;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "diagnostics-tracing";

    // 编译宿主拥有 subscriber、过滤与输出位置的配置链。
    fn install_subscriber() {
        // 创建格式化 subscriber 并应用宿主环境过滤策略。
        tracing_subscriber::fmt()
            // 从宿主环境解析过滤指令。
            .with_env_filter(EnvFilter::from_default_env())
            // 安装进程级 subscriber。
            .init();
    }
}

// 隔离 diagnostics-repro 围栏中的显式复现清单导出。
mod diagnostics_repro {
    // 引入公开 Diagnostics runtime。
    use uix::diagnostics::Diagnostics;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "diagnostics-repro";

    // 编译宿主显式选择目录并处理 typed 写入结果的公开路径。
    fn save_repro(diagnostics: &Diagnostics) -> Result<(), uix::core::Error> {
        // 原子导出固定 schema 的有界复现清单。
        let path = diagnostics.write_debug_repro_manifest("logs/repro")?;
        // 路径仅由显式调用方使用，不进入复现清单内容。
        let _ = path.display();
        // 返回 typed 成功结果。
        Ok(())
    }
}

// 隔离 error-typed-business 围栏中的业务错误包装。
mod error_typed_business {
    // 引入文档承诺的公开 Error 与设置服务 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "error-typed-business";

    // 编译保存失败到业务责任边界错误的 typed 转换。
    fn save_document(settings: &SettingsService) -> Result<(), Error> {
        // 显式保存并在失败时添加业务责任边界。
        settings.save().map_err(|error| {
            // 保留原 typed Error 作为原因链而非只格式化文本。
            Error::new(Errc::WriteFailure, "save failed").with_source(error)
        })
    }
}

// 隔离 diagnostics-app-config 围栏中的 Application 配置。
mod diagnostics_app_config {
    // 引入文档承诺的公开 Application prelude。
    use uix::prelude::*;
    // 引入公开回溯策略与诊断配置。
    use uix::diagnostics::{BacktracePolicy, DiagnosticsConfig};

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "diagnostics-app-config";

    // 编译有界报告、崩溃目录与回溯策略的组合根配置。
    fn documented_main() {
        // 配置应用级 Diagnostics runtime。
        App::new()
            // 安装宿主选择的诊断策略。
            .diagnostics(
                // 从安全默认配置开始。
                DiagnosticsConfig::default()
                    // 限制内存中保留的报告数量。
                    .report_capacity(256)
                    // 配置 panic 报告原子写入目录。
                    .crash_report_directory("logs/crash")
                    // 为错误与致命报告采集受限回溯。
                    .backtrace(BacktracePolicy::ErrorsAndFatal),
            )
            // 保留文档运行入口供编译器检查。
            .run();
    }
}

// 隔离 diagnostics-runtime-inject 围栏中的共享 runtime 注入。
mod diagnostics_runtime_inject {
    // 引入文档承诺的公开 Application prelude。
    use uix::prelude::*;
    // 引入宿主显式持有的 Diagnostics runtime 与配置。
    use uix::diagnostics::{Diagnostics, DiagnosticsConfig};

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "diagnostics-runtime-inject";

    // 编译宿主保留共享句柄并向 Application 注入同一 runtime。
    fn documented_main() {
        // 创建宿主持有的 Diagnostics runtime。
        let diagnostics = Diagnostics::new(DiagnosticsConfig::default());

        // 配置应用复用已构建的共享 runtime。
        App::new()
            // 克隆轻量共享句柄注入 Application。
            .diagnostics_runtime(diagnostics.clone())
            // 保留文档运行入口供编译器检查。
            .run();
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖运行保障文档的六个真实围栏。
fn runtime_guarantees_rust_fences_compile_as_external_consumers() {
    // 收集六个已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记诊断报告与恢复订阅围栏。
        diagnostics_report::COMPILE_ID,
        // 登记宿主 tracing 配置围栏。
        diagnostics_tracing::COMPILE_ID,
        // 登记复现清单导出围栏。
        diagnostics_repro::COMPILE_ID,
        // 登记业务 typed 错误围栏。
        error_typed_business::COMPILE_ID,
        // 登记 Application 诊断配置围栏。
        diagnostics_app_config::COMPILE_ID,
        // 登记共享 runtime 注入围栏。
        diagnostics_runtime_inject::COMPILE_ID,
    ];
    // 运行阶段核对消费者覆盖标识与 Markdown 围栏一致。
    assert_eq!(
        // 使用实际模块暴露的标识作为结果。
        compile_ids,
        // 使用运行保障文档当前声明的稳定标识作为期望。
        [
            // 诊断报告与恢复订阅围栏标识。
            "diagnostics-report",
            // 宿主 tracing 配置围栏标识。
            "diagnostics-tracing",
            // 复现清单导出围栏标识。
            "diagnostics-repro",
            // 业务 typed 错误围栏标识。
            "error-typed-business",
            // Application 诊断配置围栏标识。
            "diagnostics-app-config",
            // 共享 runtime 注入围栏标识。
            "diagnostics-runtime-inject",
        ],
    );
}
