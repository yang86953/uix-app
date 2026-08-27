// 声明本文件只编译配置使用文档，不启动应用或访问设置文件。
#![allow(dead_code)]

// 隔离 settings-service 围栏中的应用设置服务装配。
mod settings_service {
    // 引入文档承诺的公开 Application 与数据服务 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "settings-service";

    // 编译设置路径、服务解析、typed 读写与显式保存链。
    fn documented_main() {
        // 配置应用组合根及其设置服务。
        App::new()
            // 显式选择设置文件路径。
            .settings("app.settings.json")
            // 在应用启动后解析共享设置服务。
            .on_start(|handle| {
                // 在服务不存在时安全结束启动逻辑。
                let Some(settings) = handle.resolve::<SettingsService>() else {
                    // 保持 Settings 为 opt-in 服务。
                    return;
                };
                // 读取可选字符串设置。
                let theme: Option<String> = settings.get("theme");
                // 消费读取结果以保持示例无新增警告。
                let _ = theme;
                // 只修改内存中的主题设置。
                settings.set("theme", "dark");
                // 读取 typed 启动次数并提供缺省值。
                let launches = settings.get_typed_or("launches", 0_u32).unwrap_or(0);
                // 只修改内存中的 typed 启动次数。
                settings.set_typed("launches", launches + 1);
                // 通过显式保存提交当前设置。
                if let Err(error) = settings.save() {
                    // 向终端报告 typed 保存错误。
                    eprintln!("save settings failed: {error}");
                }
            })
            // 保留文档运行入口供编译器检查。
            .run();
    }
}

// 隔离 settings-typed-scalars 围栏中的标量 codec。
mod settings_typed_scalars {
    // 引入文档承诺的公开 Error 与 SettingsService。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "settings-typed-scalars";

    // 编译 typed 标量的可选、默认与必需读取路径。
    fn read_launch_count(settings: &SettingsService) -> std::result::Result<u32, Error> {
        // 以规范文本形式写入无符号整数。
        settings.set_typed("launches", 42_u32);
        // 读取存在时返回 Some 的 typed 标量。
        let optional: Option<u32> = settings.get_typed("launches")?;
        // 读取缺失键并使用 typed 默认值。
        let with_default = settings.get_typed_or("missing", 0_u32)?;
        // 要求存在且类型匹配的启动次数。
        let required = settings.require_typed::<u32>("launches")?;
        // 核对可选与默认读取的公开语义。
        assert_eq!((optional, with_default), (Some(42), 0));
        // 返回必需读取的 typed 值。
        Ok(required)
    }
}

// 隔离 settings-structured 围栏中的 serde 结构体 codec。
#[cfg(feature = "settings-serde")]
mod settings_structured {
    // 引入文档承诺的公开 Error 与 SettingsService。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "settings-structured";

    // 声明按单个设置 key 编解码的应用偏好。
    #[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
    // 保存主题、窗口宽度与最近文件列表。
    struct AppPrefs {
        // 保存主题名称。
        theme: String,
        // 保存逻辑窗口宽度。
        window_width: f64,
        // 保存最近文件路径列表。
        recent_files: Vec<String>,
    }

    // 编译结构体的可选读取、写入与必需读取路径。
    fn store_preferences(settings: &SettingsService) -> std::result::Result<(), Error> {
        // 读取已有偏好或使用结构体默认值。
        let mut preferences = settings
            // 从独立 key 解码结构体。
            .get_struct::<AppPrefs>("preferences")?
            // 在 key 缺失时采用默认偏好。
            .unwrap_or_default();
        // 只修改内存中的主题字段。
        preferences.theme = "dark".into();
        // 把完整结构体编码回同一独立 key。
        settings.set_struct("preferences", &preferences)?;
        // 要求该 key 存在并可完整解码。
        let persisted = settings.require_struct::<AppPrefs>("preferences")?;
        // 核对结构体字段完成 round trip。
        assert_eq!(persisted.theme, "dark");
        // 返回成功且不触发磁盘保存。
        Ok(())
    }
}

// 隔离 graphics-backend 围栏中的后端选择策略。
mod graphics_backend {
    // 引入文档承诺的公开 App 与 GraphicsBackend。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "graphics-backend";

    // 编译省略后端的自动策略与显式后端选择。
    fn compile_example() {
        // 省略 graphics_backend 以保留框架自动选择策略。
        let _automatic = App::new();

        // 默认 feature 集中的 Vulkan 是三平台参考后端。
        #[cfg(feature = "vulkan")]
        let _vulkan = App::new().graphics_backend(GraphicsBackend::Vulkan);

        // 在默认 feature 集中显式固定 Direct3D 11。
        #[cfg(feature = "d3d11")]
        let _d3d11 = App::new()
            // 使用编译期存在的公开后端变体。
            .graphics_backend(GraphicsBackend::Direct3D11);

        // D3D12 只在使用方显式启用同名 feature 后进入公开选择面。
        #[cfg(feature = "d3d12")]
        let _d3d12 = App::new().graphics_backend(GraphicsBackend::Direct3D12);

        // Metal 只在使用方显式启用同名 feature 后进入公开选择面。
        #[cfg(feature = "metal")]
        let _metal = App::new().graphics_backend(GraphicsBackend::Metal);
    }
}

// 隔离 cli-mode 围栏中的无窗口应用模式。
mod cli_mode {
    // 引入文档承诺的公开 App 与 AppMode。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "cli-mode";

    // 编译 CLI 模式的启动回调与运行入口。
    fn documented_main() {
        // 配置不创建窗口的应用入口。
        App::new()
            // 显式选择 CLI 生命周期。
            .mode(AppMode::CLI)
            // 注册直接执行的启动逻辑。
            .on_start(|_| {
                // 输出稳定的命令行提示。
                println!("命令行模式运行");
            })
            // 保留文档运行入口供编译器检查。
            .run();
    }
}

// 隔离 settings-typed-struct 围栏中的组合根理想用法。
#[cfg(feature = "settings-serde")]
mod settings_typed_struct {
    // 引入文档承诺的公开 Application 与设置 prelude。
    use uix::prelude::*;

    // 暴露当前模块对应的文档编译标识。
    pub(super) const COMPILE_ID: &str = "settings-typed-struct";

    // 声明组合根示例使用的应用偏好结构体。
    #[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
    // 保存可按独立 key 编解码的偏好字段。
    struct AppPrefs {
        // 保存主题名称。
        theme: String,
    }

    // 编译设置、图形后端与 typed 错误处理的组合根。
    fn documented_main() {
        // 配置应用级服务与窗口后端策略。
        App::new()
            // 显式启用设置文件路径。
            .settings("app.settings.json")
            // 使用默认 feature 集导出的 D3D11 变体。
            .graphics_backend(GraphicsBackend::Direct3D11)
            // 在启动回调中解析并更新设置服务。
            .on_start(|handle| {
                // 在服务不存在时安全结束启动逻辑。
                let Some(settings) = handle.resolve::<SettingsService>() else {
                    // 保持设置服务的 opt-in 边界。
                    return;
                };
                // 只在结构体 key 存在且解码成功时修改偏好。
                if let Ok(Some(mut preferences)) =
                    // 从单个 key 读取结构化偏好。
                    settings.get_struct::<AppPrefs>("preferences")
                {
                    // 更新内存中的主题字段。
                    preferences.theme = "dark".into();
                    // 单独处理结构体编码的 typed 错误。
                    if let Err(error) = settings.set_struct("preferences", &preferences) {
                        // 向终端报告结构体写入错误。
                        eprintln!("save preferences failed: {error}");
                    }
                }
                // 通过显式保存提交全部内存设置。
                if let Err(error) = settings.save() {
                    // 向终端报告持久化错误。
                    eprintln!("save settings failed: {error}");
                }
            })
            // 保留文档运行入口供编译器检查。
            .run();
    }
}

// 运行无原生副作用的标记测试，让 Cargo 显式执行本编译消费者。
#[test]
// 确认本批外部消费者覆盖默认四个围栏与两个 opt-in 围栏。
fn config_rust_fences_compile_as_external_consumers() {
    // 收集当前 feature 图中已经由编译器类型检查的公开示例标识。
    let compile_ids = [
        // 登记应用设置服务围栏。
        settings_service::COMPILE_ID,
        // 登记 typed 标量围栏。
        settings_typed_scalars::COMPILE_ID,
        // 仅在 settings-serde 下登记结构体 codec 围栏。
        #[cfg(feature = "settings-serde")]
        settings_structured::COMPILE_ID,
        // 登记图形后端选择围栏。
        graphics_backend::COMPILE_ID,
        // 登记 CLI 模式围栏。
        cli_mode::COMPILE_ID,
        // 仅在 settings-serde 下登记 typed 组合根围栏。
        #[cfg(feature = "settings-serde")]
        settings_typed_struct::COMPILE_ID,
    ];
    // 运行阶段核对消费者覆盖标识与 Markdown 围栏一致。
    assert_eq!(
        // 使用实际模块暴露的标识作为结果。
        compile_ids,
        // 使用配置文档当前声明的稳定标识作为期望。
        [
            // 应用设置服务围栏标识。
            "settings-service",
            // typed 标量围栏标识。
            "settings-typed-scalars",
            // settings-serde 下的结构体 codec 围栏标识。
            #[cfg(feature = "settings-serde")]
            "settings-structured",
            // 图形后端选择围栏标识。
            "graphics-backend",
            // CLI 模式围栏标识。
            "cli-mode",
            // settings-serde 下的 typed 组合根围栏标识。
            #[cfg(feature = "settings-serde")]
            "settings-typed-struct",
        ],
    );
}
