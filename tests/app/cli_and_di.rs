//! app 域 — CLI 与 DI 集成测试。

use uix::app::{Cli, CliArgs, Container};

// ════════════════════════════════════════════════════════════════════════════
// cli 测试
// ════════════════════════════════════════════════════════════════════════════

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
    let code1 = cli.run(&["prog".to_string(), "build".to_string()]);
    let code2 = cli.run(&["prog".to_string(), "test".to_string()]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
}

#[test]
fn program_name_set_from_argv() {
    let mut cli = Cli::new();
    let _ = cli.parse(&["myapp".to_string(), "build".to_string()]);
    assert_eq!(cli.program_name(), "myapp");
}

// ════════════════════════════════════════════════════════════════════════════
// di 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn new_container_is_empty() {
    let c = Container::new();
    assert!(!c.has::<String>());
    assert!(!c.has::<i32>());
}

#[test]
fn singleton_register_and_resolve() {
    let mut c = Container::new();
    c.singleton("hello".to_string());
    c.singleton(42i32);

    assert!(c.has::<String>());
    assert!(c.has::<i32>());

    assert_eq!(c.resolve::<String>(), Some(&"hello".to_string()));
    assert_eq!(c.resolve::<i32>(), Some(&42i32));
}

#[test]
fn resolve_mut_allows_modification() {
    let mut c = Container::new();
    c.singleton(10i32);

    let val = c.resolve_mut::<i32>();
    assert!(val.is_some());
    *val.unwrap() = 20;

    assert_eq!(c.resolve::<i32>(), Some(&20i32));
}

#[test]
fn resolve_nonexistent_returns_none() {
    let c = Container::new();
    assert!(c.resolve::<String>().is_none());
    assert!(c.resolve::<f64>().is_none());
}

#[test]
fn remove_clears_singleton() {
    let mut c = Container::new();
    c.singleton(42i32);
    assert!(c.has::<i32>());
    c.remove::<i32>();
    assert!(!c.has::<i32>());
}

#[test]
fn multiple_singletons_independent() {
    let mut c = Container::new();
    c.singleton(1i32);
    c.singleton(2.0f64);
    c.singleton("text".to_string());

    assert_eq!(c.resolve::<i32>(), Some(&1i32));
    assert_eq!(c.resolve::<f64>(), Some(&2.0f64));
    assert_eq!(c.resolve::<String>(), Some(&"text".to_string()));

    c.remove::<i32>();
    assert!(c.resolve::<i32>().is_none());
    assert!(c.resolve::<f64>().is_some());
    assert!(c.resolve::<String>().is_some());
}

#[test]
fn singleton_clone_trait() {
    let mut c = Container::new();
    c.singleton(vec![1, 2, 3]);
    assert_eq!(c.resolve::<Vec<i32>>(), Some(&vec![1, 2, 3]));
}

#[test]
fn default_container_is_empty() {
    let c: Container = Default::default();
    assert!(!c.has::<i32>());
}
