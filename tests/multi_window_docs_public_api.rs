// 声明本文件只编译多窗口使用文档，不创建窗口或运行事件循环。
#![allow(dead_code)]

// 隔离 multi-window-open 围栏中的次窗创建与共享主题。
mod multi_window_open {
    // 引入文档承诺的公开 AppHandle、状态与 View prelude。
    use uix_app::prelude::*;

    // 编译回调次窗创建与后续共享主题更新的独立所有权。
    fn compile_example(handle: &AppHandle, dark: bool) {
        // 为按钮回调克隆独立的 Application 多窗门面。
        let window_handle = handle.clone();
        // 构造按需打开设置窗口的按钮。
        let _open_button = button("打开设置窗口").on_click_fn(move || {
            // 创建只由新 WindowSession 消费的拥有型配置。
            let _ = window_handle.open_window(WindowConfig::new("设置", 480, 360, || {
                // 构造设置窗口的独立根视图。
                column((label("设置面板"),))
            }));
        });

        // 创建可在多个窗口共享的业务主题状态。
        let _theme = State::new(false);
        // 通过仍由调用方持有的原句柄更新应用主题。
        let _ = handle.set_theme(if dark {
            // 选择内建深色主题。
            Theme::antd_dark()
        } else {
            // 选择内建浅色主题。
            Theme::antd_light()
        });
    }
}

// 隔离 multi-window-lifecycle 围栏中的窗口存活观测与激活请求。
mod multi_window_lifecycle {
    // 引入文档承诺的公开 AppHandle 与 typed Result。
    use uix_app::prelude::*;

    // 编译窗口句柄的瞬时存活快照与 owner-thread 激活请求。
    fn compile_example(handle: &AppHandle) -> Result<(), Error> {
        // 存活值只用于决定创建或激活分支，不充当跨线程租约。
        if handle.is_open() {
            // 激活请求保持 typed 失败，不泄漏原生窗口对象。
            handle.request_activate()?;
        }
        // 返回公开 Result，要求使用方显式处理关闭竞态。
        Ok(())
    }
}

// 隔离 multi-window-control 围栏中的标准窗口交互。
mod multi_window_control {
    // 引入文档承诺的公开窗口控制与 View prelude。
    use uix_app::prelude::*;

    // 编译标准控制组合、具名关闭控件、拖拽区域与缩放热区。
    fn compile_example() {
        // 按标准顺序创建最小化、最大化与关闭控件。
        let controls = window_controls(true, true, true);
        // 消费标准控制组合以保持纯编译示例。
        let _ = controls;

        // 为深色标题栏表面创建指定图标前景色的控制组合。
        let dark_controls =
            window_controls_with_icon_color(true, true, true, "#E7F2ED".into());
        // 消费定制前景色控制组合以保持纯编译示例。
        let _ = dark_controls;

        // 创建带稳定无障碍名称的定制关闭控件。
        let close = window_control_named(WindowControl::Close, "关闭窗口", button("×"));
        // 消费定制关闭控件以保持纯编译示例。
        let _ = close;

        // 创建不覆盖交互控件的自定义标题栏拖拽区域。
        let drag_region = window_drag_region(
            // 使用固定逻辑高度的标题栏内容。
            column((label("自定义标题栏"),)).height(40.0),
        );
        // 消费拖拽区域节点以保持纯编译示例。
        let _ = drag_region;

        // 创建右下角透明窗口缩放热区。
        let resize_region = window_resize_region(
            // 使用公共八方向枚举声明精确缩放方向。
            WindowResizeEdge::BottomRight,
            // 以逻辑尺寸声明由布局定位的命中区域。
            space(12.0).width(12.0),
        );
        // 消费缩放热区节点以保持纯编译示例。
        let _ = resize_region;
    }
}

// 隔离 multi-window-config 围栏中的延迟窗口配置。
mod multi_window_config {
    // 引入文档承诺的公开 Application、WindowConfig 与 View prelude。
    use uix_app::prelude::*;

    // 编译拥有型窗口配置到 on_start 延迟打开的移动语义。
    fn documented_main() {
        // 创建声明式设置窗口配置。
        let settings_window = WindowConfig::new("设置", 480, 360, settings_page)
            // 为设置窗口启用自定义标题栏。
            .custom_title_bar(true);

        // 配置主窗口与按需次窗创建。
        App::new()
            // 安装主窗口业务根视图。
            .root(main_page)
            // 把拥有型设置窗口配置移动进一次性启动回调。
            .on_start(move |handle| {
                // 在应用启动后按需打开设置窗口。
                let _ = handle.open_window(settings_window);
            })
            // 保留文档运行入口供编译器检查。
            .run();
    }

    // 声明设置窗口根视图工厂。
    fn settings_page() -> ViewNode {
        // 返回稳定的设置页内容。
        label("设置")
    }

    // 声明主窗口根视图工厂。
    fn main_page() -> ViewNode {
        // 返回稳定的主页面内容。
        label("主页")
    }
}
