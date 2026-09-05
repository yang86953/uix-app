// 声明本文件只编译终端使用文档，不启动窗口、计时器或外部 I/O。
#![allow(dead_code)]

// 隔离 terminal-basic 围栏中的基础终端配置。
mod terminal_basic {
    // 引入文档承诺的终端组件公开 prelude。
    use uix::prelude::*;

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
    use uix::prelude::*;

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
    use uix::prelude::*;

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
