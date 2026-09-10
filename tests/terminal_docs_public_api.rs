// 声明本文件只编译终端使用文档，不启动窗口、计时器或外部 I/O。
#![allow(dead_code)]

// 隔离 terminal-basic 围栏中的基础终端配置。
mod terminal_basic {
    // 引入文档承诺的终端组件公开 prelude。
    use uix_app::prelude::*;

    // 编译带输出行、提示符与命令回调的基础终端。
    fn compile_example() {
        // 构造单行欢迎输出与空命令回调的终端组件。
        let _terminal = embed(
            // 用默认提示符与回调创建终端。
            Terminal::new()
                // 提供一行初始输出。
                .lines(vec![TerminalLine::text("UIX 控制台就绪")])
                // 覆盖默认提示符。
                .prompt("$ ")
                // 命令执行在应用域，回调保持空实现。
                .on_command(|_command| {}),
        );
    }
}

// 隔离 terminal-styled 围栏中的语义着色输出行。
mod terminal_styled {
    // 引入文档承诺的终端着色模型公开 prelude。
    use uix_app::prelude::*;

    // 编译按语义颜色拼接文本段的输出行。
    fn compile_example() {
        // 构造成功、警告与错误三行语义输出。
        let _terminal = embed(
            // 用多行着色输出创建终端组件。
            Terminal::new().lines(vec![
                // 默认色前缀接成功色结论。
                TerminalLine::new()
                    .push("[build] ")
                    .styled("done", TerminalColor::Success)
                    .push(" in 1.2s"),
                // 警告色标记警告前缀。
                TerminalLine::new()
                    .styled("warning:", TerminalColor::Warning)
                    .push(" 未指定输出目录"),
                // 错误色标记错误前缀。
                TerminalLine::new()
                    .styled("error:", TerminalColor::Error)
                    .push(" 清单缺失，构建终止"),
            ]),
        );
    }
}

// 隔离 terminal-command 围栏中的运行期缓冲与提示符替换。
mod terminal_command {
    // 引入文档承诺的终端运行期公开 prelude。
    use uix_app::prelude::*;

    // 编译运行期提示符替换、缓冲重建与历史读取。
    fn compile_example() {
        // 构造默认提示符的终端实例。
        let mut console = Terminal::new().prompt("$ ");
        // 运行期替换提示符文本。
        console.set_prompt("# ");
        // 运行期用新输出行替换整个回看缓冲。
        console.set_lines(vec![TerminalLine::text("缓冲已重建")]);
        // 读取按提交顺序保存的历史命令。
        let _history: &[String] = console.history();
    }
}

// 隔离 terminal-live-session 围栏中的真实会话启动与投影。
mod terminal_live_session {
    // 引入文档承诺的真实终端公开 prelude。
    use uix_app::prelude::*;

    // 编译显式 spawn 与纯投影视图构建。
    fn compile_example() {
        // 取得应用句柄即可组装读取唤醒回调。
        fn spawn_session(handle: &AppHandle) -> Result<TerminalSession, uix_app::core::Error> {
            // 复制窗口句柄供读线程唤醒。
            let wakeup = handle.clone();
            // 使用默认 shell 命令与窗口尺寸。
            let mut config = TerminalSessionConfig::default();
            // 输出到达时经 post_to_ui 回 UI 线程。
            config.on_output = Some(std::sync::Arc::new(move || {
                wakeup.post_to_ui(|| {
                    // 应用在 UI 线程调用 session.pump() 消费新输出。
                });
            }));
            // 显式创建会话；视图构建永远不会创建进程。
            TerminalSession::spawn(&config)
        }

        // 视图只投影已存在的会话。
        fn live_view(session: &TerminalSession) -> ViewNode {
            embed(TerminalScreen::new(session))
        }

        let _ = (
            spawn_session as fn(&AppHandle) -> Result<TerminalSession, uix_app::core::Error>,
            live_view as fn(&TerminalSession) -> ViewNode,
        );
    }
}

// 隔离 terminal-live-query 围栏中的读写、尺寸与状态查询。
mod terminal_live_query {
    // 引入文档承诺的会话运行期公开 prelude。
    use uix_app::prelude::*;

    // 编译写入、resize 与屏幕投影查询。
    fn compile_example() {
        // 查询与驱动只接收已存在的会话句柄。
        fn inspect_and_drive(session: &TerminalSession) -> Result<(), uix_app::core::Error> {
            // 按键字节与文本直接写入 PTY。
            session.write(b"ls --color=auto\r")?;
            // 显式同步窗口网格尺寸。
            session.resize(100, 30)?;
            // 屏幕投影：行快照、光标与网格尺寸。
            let rows: Vec<String> = session.rows().iter().map(TerminalRow::plain).collect();
            let (row, column) = session.cursor();
            let (cols, lines) = session.screen_size();
            let running = matches!(session.status(), TerminalSessionStatus::Running);
            let _ = (rows, row, column, cols, lines, running);
            Ok(())
        }

        let _ = inspect_and_drive as fn(&TerminalSession) -> Result<(), uix_app::core::Error>;
    }
}

// 编译公开滚回合同：历史属于会话，滚动位置不属于共享会话。
mod terminal_scrollback {
    use uix_app::prelude::*;

    fn inspect_history(session: &TerminalSession) -> Result<(), uix_app::core::Error> {
        session.set_scrollback_limit(2_000)?;
        let retained = session.scrollback_len();
        let limit = session.scrollback_limit();
        let oldest: Vec<TerminalRow> = session.scrollback_rows(0, 20);
        let active_grid: Vec<TerminalRow> = session.rows();
        let _ = (retained, limit, oldest, active_grid);
        session.clear_scrollback();
        Ok(())
    }
}

// 编译会话模式查询、受模式控制的粘贴与原始输入的区别。
mod terminal_input_modes {
    use uix_app::prelude::*;

    fn inspect_and_paste(session: &TerminalSession) -> Result<(), uix_app::core::Error> {
        let modes: TerminalModes = session.modes();
        let _ = (
            modes.application_cursor_keys,
            modes.cursor_visible,
            modes.bracketed_paste,
        );
        // 由会话按 2004 模式添加包围并过滤非排版控制字符。
        session.paste("第一行\n第二行")?;
        // 原始 write 不过滤控制字节，也不自动添加粘贴包围。
        session.write(b"\x03")?;
        Ok(())
    }
}
