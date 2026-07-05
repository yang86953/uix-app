//! log — Level / Record / Sink / Logger 测试。

use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use uix::core::diagnostic::Timestamp;
use uix::core::log::{self, CallbackSink, ConsoleSink, FileSink, Level, Logger, Record, Sink};

fn ts_from_millis(ms: u64) -> Timestamp {
    Timestamp::from_system_time(UNIX_EPOCH + Duration::from_millis(ms))
}

#[test]
fn level_ordering() {
    assert!(Level::Trace < Level::Debug);
    assert!(Level::Debug < Level::Info);
    assert!(Level::Info < Level::Warn);
    assert!(Level::Warn < Level::Error);
    assert!(Level::Error < Level::Fatal);
}

#[test]
fn level_name() {
    assert_eq!(Level::Trace.name(), "TRACE");
    assert_eq!(Level::Debug.name(), "DEBUG");
    assert_eq!(Level::Info.name(), "INFO");
    assert_eq!(Level::Warn.name(), "WARN");
    assert_eq!(Level::Error.name(), "ERROR");
    assert_eq!(Level::Fatal.name(), "FATAL");
}

#[test]
fn level_display() {
    assert_eq!(format!("{}", Level::Warn), "WARN");
}

#[test]
fn record_display_without_attributes() {
    let rec = Record {
        level: Level::Info,
        timestamp: ts_from_millis(3600000),
        file: "test.rs",
        line: 42,
        message: "hello".into(),
        attributes: Vec::new(),
        sequence: 1,
    };
    let s = rec.to_string();
    assert!(s.contains("hello"));
    assert!(s.contains("INFO"));
    assert!(s.contains("test.rs"));
    assert!(s.contains("42"));
}

#[test]
fn record_display_with_attributes() {
    let rec = Record {
        level: Level::Warn,
        timestamp: ts_from_millis(7200000),
        file: "app.rs",
        line: 99,
        message: "warning msg".into(),
        attributes: vec![("key".into(), "val".into())],
        sequence: 2,
    };
    let s = rec.to_string();
    assert!(s.contains("key=val"));
}

#[test]
fn record_to_json_without_attributes() {
    let rec = Record {
        level: Level::Error,
        timestamp: ts_from_millis(1700000000000),
        file: "mod.rs",
        line: 10,
        message: "fail".into(),
        attributes: Vec::new(),
        sequence: 0,
    };
    let json = rec.to_json();
    assert!(json.contains(r#""level":"ERROR""#));
    assert!(json.contains(r#""msg":"fail""#));
}

#[test]
fn record_to_json_with_attributes() {
    let rec = Record {
        level: Level::Debug,
        timestamp: ts_from_millis(1800000000000),
        file: "lib.rs",
        line: 5,
        message: "debug info".into(),
        attributes: vec![("a".into(), "1".into()), ("b".into(), "2".into())],
        sequence: 3,
    };
    let json = rec.to_json();
    assert!(json.contains(r#""attrs":{"#));
    assert!(json.contains(r#""a":"1""#));
}

#[test]
fn console_sink_new_default_level() {
    let sink = ConsoleSink::new(true);
    assert_eq!(sink.level(), Level::Warn);
}

#[test]
fn console_sink_new_with_level() {
    let sink = ConsoleSink::new_with_level(true, Level::Debug);
    assert_eq!(sink.level(), Level::Debug);
}

#[test]
fn console_sink_new_trace() {
    let sink = ConsoleSink::new_trace(true);
    assert_eq!(sink.level(), Level::Trace);
}

#[test]
fn console_sink_set_level() {
    let sink = ConsoleSink::new(true);
    sink.set_level(Level::Info);
    assert_eq!(sink.level(), Level::Info);
}

#[test]
fn console_sink_passes() {
    let sink = ConsoleSink::new_with_level(true, Level::Warn);
    assert!(!sink.passes(Level::Info));
    assert!(sink.passes(Level::Warn));
    assert!(sink.passes(Level::Error));
}

#[test]
fn console_sink_flush() {
    let sink = ConsoleSink::new(true);
    sink.flush();
}

#[test]
fn callback_sink_callback_invoked() {
    let captured = Arc::new(Mutex::new(None::<String>));
    let c = captured.clone();
    let sink = CallbackSink::new(move |rec: &Record| {
        *c.lock().unwrap() = Some(rec.message.clone());
    });
    let rec = Record {
        level: Level::Info,
        timestamp: Timestamp::now(),
        file: "test.rs",
        line: 1,
        message: "callback works".into(),
        attributes: Vec::new(),
        sequence: 0,
    };
    sink.write(&rec);
    let msg = captured.lock().unwrap().take();
    assert_eq!(msg.as_deref(), Some("callback works"));
}

#[test]
fn callback_sink_level() {
    let sink = CallbackSink::new(|_: &Record| {});
    assert_eq!(sink.level(), Level::Trace);
    sink.set_level(Level::Error);
    assert_eq!(sink.level(), Level::Error);
    assert!(sink.passes(Level::Error));
    assert!(!sink.passes(Level::Info));
}

#[test]
fn file_sink_create_and_write() {
    let dir = std::env::temp_dir();
    let path = dir.join("uix_test_file_sink.log");
    let _ = std::fs::remove_file(&path);
    let sink = FileSink::new(path.to_string_lossy().to_string()).expect("file sink");
    let rec = Record {
        level: Level::Info,
        timestamp: Timestamp::now(),
        file: "test.rs",
        line: 1,
        message: "file write test".into(),
        attributes: Vec::new(),
        sequence: 0,
    };
    sink.write(&rec);
    sink.flush();
    let content = std::fs::read_to_string(&path).expect("read log file");
    assert!(content.contains("file write test"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn file_sink_set_level() {
    let dir = std::env::temp_dir();
    let path = dir.join("uix_test_file_level.log");
    let _ = std::fs::remove_file(&path);
    let sink = FileSink::new(path.to_string_lossy().to_string()).expect("file sink");
    assert_eq!(sink.level(), Level::Info);
    sink.set_level(Level::Warn);
    assert_eq!(sink.level(), Level::Warn);
    let _ = std::fs::remove_file(&path);
}

/// Logger 是全局单例，依赖它的测试合并在一个函数中顺序执行。
#[test]
fn logger_sequential() {
    let logger = Logger::instance();
    logger.clear_sinks();

    // ── singleton ──
    let a = Logger::instance() as *const Logger;
    let b = Logger::instance() as *const Logger;
    assert_eq!(a, b);

    // ── get / set level ──
    logger.set_level(Level::Trace);
    assert_eq!(logger.get_level(), Level::Trace);
    logger.set_level(Level::Error);
    assert_eq!(logger.get_level(), Level::Error);

    // ── add / remove sink ──
    logger.clear_sinks();
    let sink = Arc::new(ConsoleSink::new_trace(true));
    let ptr = Arc::as_ptr(&sink) as *const dyn Sink;
    logger.add_sink(sink);
    logger.remove_sink(ptr);

    // ── log respects level ──
    logger.clear_sinks();
    let captured = Arc::new(Mutex::new(Vec::new()));
    let c = captured.clone();
    logger.add_sink(Arc::new(CallbackSink::new(move |rec: &Record| {
        c.lock().unwrap_or_else(|e| e.into_inner()).push(rec.level);
    })));
    logger.set_level(Level::Warn);
    logger.log(
        Level::Info,
        "should not appear".into(),
        "test.rs",
        1,
        Vec::new(),
    );
    assert!(captured
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_empty());
    logger.log(
        Level::Error,
        "should appear".into(),
        "test.rs",
        2,
        Vec::new(),
    );
    assert_eq!(captured.lock().unwrap_or_else(|e| e.into_inner()).len(), 1);

    // ── convenience methods ──
    logger.clear_sinks();
    let captured2 = Arc::new(Mutex::new(Vec::new()));
    let c2 = captured2.clone();
    logger.add_sink(Arc::new(CallbackSink::new(move |rec: &Record| {
        c2.lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((rec.level, rec.message.clone()));
    })));
    logger.set_level(Level::Trace);
    logger.trace("trace msg".into(), "t.rs", 1);
    logger.debug("debug msg".into(), "d.rs", 2);
    logger.info("info msg".into(), "i.rs", 3);
    logger.warn("warn msg".into(), "w.rs", 4);
    logger.error("error msg".into(), "e.rs", 5);
    logger.fatal("fatal msg".into(), "f.rs", 6);
    {
        let msgs = captured2.lock().unwrap_or_else(|e| e.into_inner());
        // 至少包含自己写入的 6 条；其他并行测试可能写入额外日志
        assert!(msgs.len() >= 6, "expected >=6 logs, got {}", msgs.len());
        // 验证自己的 6 条日志都存在且顺序正确
        let own_levels: Vec<Level> = vec![
            Level::Trace,
            Level::Debug,
            Level::Info,
            Level::Warn,
            Level::Error,
            Level::Fatal,
        ];
        let own_msgs: Vec<&(Level, String)> = msgs
            .iter()
            .filter(|(_, msg)| {
                matches!(
                    msg.as_str(),
                    "trace msg" | "debug msg" | "info msg" | "warn msg" | "error msg" | "fatal msg"
                )
            })
            .collect();
        assert_eq!(own_msgs.len(), 6, "should find exactly 6 own messages");
        for (i, m) in own_msgs.iter().enumerate() {
            assert_eq!(m.0, own_levels[i], "own message {} level mismatch", i);
        }
    }

    // ── log_error ──
    logger.clear_sinks();
    let captured3 = Arc::new(Mutex::new(None::<String>));
    let c3 = captured3.clone();
    logger.add_sink(Arc::new(CallbackSink::new(move |rec: &Record| {
        *c3.lock().unwrap_or_else(|e| e.into_inner()) = Some(rec.message.clone());
    })));
    logger.set_level(Level::Trace);
    let err = uix::native::make_error(uix::native::Errc::None, "ignored message");
    logger.log_error(&err, Level::Info);
    {
        let msg = captured3.lock().unwrap_or_else(|e| e.into_inner()).take();
        assert!(msg.as_deref().unwrap_or("").contains("none"));
    }

    // ── flush ──
    logger.flush();

    // ── 清理 ──
    logger.clear_sinks();
}

#[test]
fn global_log_functions() {
    // 无需 sink 也能执行，只是测试不 panic
    log::set_level(Level::Trace);
    log::trace_fn("trace test");
    log::debug_fn("debug test");
    log::info_fn("info test");
    log::warn_fn("warn test");
    log::error_fn("error test");
    log::fatal_fn("fatal test");
    log::flush();
}

#[test]
fn log_error_does_not_panic() {
    let err = uix::native::make_error(uix::native::Errc::PlatformError, "test error");
    log::log_error(&err, Level::Warn);
}

#[test]
fn record_sequence() {
    let rec = Record {
        level: Level::Info,
        timestamp: Timestamp::now(),
        file: "seq.rs",
        line: 1,
        message: "seq".into(),
        attributes: Vec::new(),
        sequence: 42,
    };
    assert_eq!(rec.sequence, 42);
}
