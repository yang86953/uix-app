use std::collections::HashMap;

/// 解析后的命令行参数。
#[derive(Debug, Clone)]
pub struct CliArgs {
    pub command: String,
    pub positional: Vec<String>,
    pub options: HashMap<String, String>,
}

impl CliArgs {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        match self.options.get(key) {
            Some(s) => s.as_str(),
            None => default,
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.options.contains_key(key)
    }
}

/// CLI 命令处理器。
pub(crate) type CommandHandler = fn(&CliArgs) -> i32;

/// 非 GUI 操作的命令行接口。
#[derive(Default)]
pub struct Cli {
    commands: HashMap<String, CommandHandler>,
    descriptions: HashMap<String, String>,
    default_handler: Option<CommandHandler>,
    program_name: String,
}

impl Cli {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一条命令。
    pub fn command(&mut self, name: &str, handler: CommandHandler, description: &str) -> &mut Self {
        self.commands.insert(name.to_string(), handler);
        if !description.is_empty() {
            self.descriptions
                .insert(name.to_string(), description.to_string());
        }
        self
    }

    /// 设置默认处理器（当没有命令匹配时使用）。
    pub fn default_command(&mut self, handler: CommandHandler) -> &mut Self {
        self.default_handler = Some(handler);
        self
    }

    /// 解析并运行 CLI。
    /// 返回退出码。
    pub fn run(&mut self, argv: &[String]) -> i32 {
        let args = self.parse(argv);

        // 无命令或显式请求帮助：打印帮助并正常退出。
        if args.command.is_empty() || args.command == "help" {
            self.print_help();
            return 0;
        }

        // 优先查找已注册命令，其次回退到默认处理器。
        if let Some(&handler) = self.commands.get(&args.command) {
            return handler(&args);
        }

        if let Some(handler) = self.default_handler {
            return handler(&args);
        }

        // 未知命令：记录错误并以非零退出码结束。
        tracing::error!("Unknown command: {}", args.command);
        self.print_help();
        1
    }

    pub fn parse(&mut self, argv: &[String]) -> CliArgs {
        let mut result = CliArgs {
            command: String::new(),
            positional: Vec::new(),
            options: HashMap::new(),
        };

        if argv.is_empty() {
            return result;
        }

        self.program_name = argv[0].clone();

        let mut i = 1;
        // 第一个非选项参数视为命令名。
        if i < argv.len() && !argv[i].starts_with('-') {
            result.command = argv[i].clone();
            i += 1;
        }

        while i < argv.len() {
            let arg = &argv[i];
            // 帮助请求：把命令改写为 help 以便统一处理。
            if arg == "--help" || arg == "-h" {
                result.command = "help".to_string();
                i += 1;
            } else if arg.starts_with("--") {
                // 长选项：`--key=value` 拆成键值对，否则视为布尔开关。
                let opt = arg.trim_start_matches("--");
                if let Some(eq_pos) = opt.find('=') {
                    let key = &opt[..eq_pos];
                    let val = &opt[eq_pos + 1..];
                    result.options.insert(key.to_string(), val.to_string());
                } else {
                    result.options.insert(opt.to_string(), "true".to_string());
                }
                i += 1;
            } else if arg.starts_with('-') && arg.len() > 1 {
                // 短选项：后跟非选项参数时消费为选项值，否则视为布尔开关。
                let key = &arg[1..];
                if i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                    result.options.insert(key.to_string(), argv[i + 1].clone());
                    i += 2;
                } else {
                    result.options.insert(key.to_string(), "true".to_string());
                    i += 1;
                }
            } else {
                // 其余参数归入位置参数列表。
                result.positional.push(arg.clone());
                i += 1;
            }
        }

        result
    }

    fn print_help(&self) {
        println!("Usage: {} <command> [options]\n", self.program_name);
        println!("Commands:");
        for (name, desc) in &self.descriptions {
            println!("  {:<20} {}", name, desc);
        }
        println!("  {:<20} Show this help message", "help");
    }

    pub fn program_name(&self) -> &str {
        &self.program_name
    }
    pub fn commands(&self) -> &HashMap<String, CommandHandler> {
        &self.commands
    }
}
