//! UIX Lang Zed 扩展：为 `uix-lang-ls` 提供语言服务器命令适配。

use zed::settings::LspSettings;
use zed_extension_api::{self as zed, Command, LanguageServerId, Worktree};

struct UixLangExtension;

impl zed::Extension for UixLangExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> zed::Result<Command> {
        // 优先使用用户 settings 中 lsp.uix-lang-ls.binary 指定的二进制。
        let lsp_settings = LspSettings::for_worktree(language_server_id.as_ref(), worktree)?;
        if let Some(binary) = lsp_settings.binary {
            if let Some(path) = binary.path {
                return Ok(Command {
                    command: path,
                    args: binary.arguments.unwrap_or_default(),
                    env: binary
                        .env
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                });
            }
        }
        // 兜底：使用 PATH 上的 uix-lang-ls。
        Ok(Command {
            command: "uix-lang-ls".to_string(),
            args: vec![],
            env: vec![],
        })
    }
}

zed::register_extension!(UixLangExtension);
