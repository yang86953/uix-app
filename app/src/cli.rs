use std::collections::HashMap;

/// Parsed command-line arguments.
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

/// CLI command handler.
pub type CommandHandler = fn(&CliArgs) -> i32;

/// Command-line interface for non-GUI operation.
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

    /// Register a command.
    pub fn command(&mut self, name: &str, handler: CommandHandler, description: &str) -> &mut Self {
        self.commands.insert(name.to_string(), handler);
        if !description.is_empty() {
            self.descriptions
                .insert(name.to_string(), description.to_string());
        }
        self
    }

    /// Set the default handler (when no command matches).
    pub fn default_command(&mut self, handler: CommandHandler) -> &mut Self {
        self.default_handler = Some(handler);
        self
    }

    /// Parse and run the CLI.
    /// Returns the exit code.
    pub fn run(&mut self, argv: &[String]) -> i32 {
        let args = self.parse(argv);

        if args.command.is_empty() || args.command == "help" {
            self.print_help();
            return 0;
        }

        if let Some(&handler) = self.commands.get(&args.command) {
            return handler(&args);
        }

        if let Some(handler) = self.default_handler {
            return handler(&args);
        }

        log::error!("Unknown command: {}", args.command);
        self.print_help();
        1
    }

    fn parse(&mut self, argv: &[String]) -> CliArgs {
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
        // First non-option argument is the command
        if i < argv.len() && !argv[i].starts_with('-') {
            result.command = argv[i].clone();
            i += 1;
        }

        while i < argv.len() {
            let arg = &argv[i];
            if arg == "--help" || arg == "-h" {
                result.command = "help".to_string();
                i += 1;
            } else if arg.starts_with("--") {
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
                let key = &arg[1..];
                if i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                    result.options.insert(key.to_string(), argv[i + 1].clone());
                    i += 2;
                } else {
                    result.options.insert(key.to_string(), "true".to_string());
                    i += 1;
                }
            } else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_command() {
        let mut cli = Cli::new();
        let args = cli.parse(&["prog".to_string(), "build".to_string()]);
        assert_eq!(args.command, "build");
        assert!(args.positional.is_empty());
        assert!(args.options.is_empty());
    }

    #[test]
    fn parse_long_option_with_value() {
        let mut cli = Cli::new();
        let args = cli.parse(&[
            "prog".to_string(),
            "run".to_string(),
            "--name=value".to_string(),
        ]);
        assert_eq!(args.command, "run");
        assert_eq!(args.get("name"), Some("value"));
    }

    #[test]
    fn parse_long_option_without_value() {
        let mut cli = Cli::new();
        let args = cli.parse(&["prog".to_string(), "--verbose".to_string()]);
        assert_eq!(args.command, "");
        assert_eq!(args.get("verbose"), Some("true"));
    }

    #[test]
    fn parse_short_option_with_value() {
        let mut cli = Cli::new();
        let args = cli.parse(&[
            "prog".to_string(),
            "-o".to_string(),
            "output.txt".to_string(),
        ]);
        assert_eq!(args.get("o"), Some("output.txt"));
    }

    #[test]
    fn parse_short_option_without_value() {
        let mut cli = Cli::new();
        let args = cli.parse(&["prog".to_string(), "-v".to_string()]);
        assert_eq!(args.get("v"), Some("true"));
    }

    #[test]
    fn parse_positional_args() {
        let mut cli = Cli::new();
        let args = cli.parse(&[
            "prog".to_string(),
            "cmd".to_string(),
            "file1".to_string(),
            "file2".to_string(),
        ]);
        assert_eq!(args.command, "cmd");
        assert_eq!(args.positional, vec!["file1", "file2"]);
    }

    #[test]
    fn parse_help_flag() {
        let mut cli = Cli::new();
        let args = cli.parse(&["prog".to_string(), "--help".to_string()]);
        assert_eq!(args.command, "help");

        let args2 = cli.parse(&["prog".to_string(), "-h".to_string()]);
        assert_eq!(args2.command, "help");
    }

    #[test]
    fn parse_empty_argv() {
        let mut cli = Cli::new();
        let args = cli.parse(&[] as &[String]);
        assert_eq!(args.command, "");
        assert!(args.positional.is_empty());
    }

    #[test]
    fn get_or_returns_default() {
        let mut cli = Cli::new();
        let args = cli.parse(&["prog".to_string()]);
        assert_eq!(args.get_or("nonexistent", "default"), "default");
        assert_eq!(args.get_or("nonexistent", ""), "");
    }

    #[test]
    fn has_checks_option_existence() {
        let mut cli = Cli::new();
        let args = cli.parse(&["prog".to_string(), "--flag".to_string()]);
        assert!(args.has("flag"));
        assert!(!args.has("other"));
    }

    #[test]
    fn run_registered_command() {
        static CALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn handler(_: &CliArgs) -> i32 {
            CALLED.store(true, std::sync::atomic::Ordering::SeqCst);
            0
        }

        let mut cli = Cli::new();
        cli.command("build", handler, "Build the project");
        let code = cli.run(&["prog".to_string(), "build".to_string()]);
        assert_eq!(code, 0);
        assert!(CALLED.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn run_unknown_command_returns_1() {
        fn handler(_: &CliArgs) -> i32 {
            0
        }
        let mut cli = Cli::new();
        cli.command("build", handler, "");
        let code = cli.run(&["prog".to_string(), "unknown".to_string()]);
        assert_eq!(code, 1);
    }

    #[test]
    fn run_help_command_returns_0() {
        fn handler(_: &CliArgs) -> i32 {
            42
        }
        let mut cli = Cli::new();
        cli.command("build", handler, "Build");
        let code = cli.run(&["prog".to_string(), "help".to_string()]);
        assert_eq!(code, 0);
    }

    #[test]
    fn default_handler_invoked_on_unknown() {
        fn default(_: &CliArgs) -> i32 {
            99
        }
        fn handler(_: &CliArgs) -> i32 {
            0
        }
        let mut cli = Cli::new();
        cli.command("build", handler, "");
        cli.default_command(default);
        let code = cli.run(&["prog".to_string(), "unknown".to_string()]);
        assert_eq!(code, 99);
    }

    #[test]
    fn parse_mixed_options_and_positional() {
        let mut cli = Cli::new();
        let args = cli.parse(&[
            "prog".to_string(),
            "deploy".to_string(),
            "--env=prod".to_string(),
            "--verbose".to_string(),
            "target_host".to_string(),
        ]);
        assert_eq!(args.command, "deploy");
        assert_eq!(args.get("env"), Some("prod"));
        assert_eq!(args.get("verbose"), Some("true"));
        assert_eq!(args.positional, vec!["target_host"]);
    }

    #[test]
    fn command_description_stored() {
        let mut cli = Cli::new();
        cli.command("build", |_| 0, "Compile the project");
        cli.command("test", |_| 0, "Run tests");

        assert!(cli.descriptions.contains_key("build"));
        assert!(cli.descriptions.contains_key("test"));
        assert_eq!(
            cli.descriptions.get("build").unwrap(),
            "Compile the project"
        );
    }

    #[test]
    fn program_name_set_from_argv() {
        let mut cli = Cli::new();
        let _ = cli.parse(&["myapp".to_string(), "build".to_string()]);
        assert_eq!(cli.program_name(), "myapp");
    }
}
