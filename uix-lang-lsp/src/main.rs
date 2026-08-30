//! `uix-lang-ls`：UIX Lang 语言服务器 stdio 入口。
//!
//! 实现位于共享的 `uix_lang_lsp` System；`uix lsp` 子命令同样委托该实现。

fn main() -> std::process::ExitCode {
    match uix_lang_lsp::run_stdio() {
        Ok(code) => std::process::ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
