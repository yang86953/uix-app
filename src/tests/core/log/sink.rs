use crate::core::diagnostic::Timestamp;
use crate::core::log::{FileSink, Level, Record, Sink};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn temp_log_path() -> std::path::PathBuf {
    let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!("uix-file-sink-{}-{id}", std::process::id()));
    fs::create_dir_all(&directory).expect("create file sink test directory");
    directory.join("app.log")
}

fn record(message: &str) -> Record {
    Record {
        level: Level::Info,
        timestamp: Timestamp::from_system_time(UNIX_EPOCH),
        file: "test.rs",
        line: 42,
        message: message.to_owned(),
        attributes: Vec::new(),
        sequence: 0,
    }
}

#[test]
fn record_json_escapes_every_string_field() {
    let record = Record {
        level: Level::Info,
        timestamp: Timestamp::from_system_time(UNIX_EPOCH),
        file: "bad\"file\n.rs",
        line: 42,
        message: "quote \" slash \\ newline\n tab\t control \u{1}".to_owned(),
        attributes: vec![("key\"".to_owned(), "value\n\\".to_owned())],
        sequence: 7,
    };

    assert_eq!(
        record.to_json(),
        "{\"ts\":\"1970-01-01T00:00:00.000Z\",\"level\":\"INFO\",\"msg\":\"quote \\\" slash \\\\ newline\\n tab\\t control \\u0001\",\"file\":\"bad\\\"file\\n.rs\",\"line\":42,\"seq\":7,\"attrs\":{\"key\\\"\":\"value\\n\\\\\"}}"
    );
}

#[test]
fn file_sink_rotation_releases_the_file_lock_and_caps_backups() {
    let path = temp_log_path();
    let path_text = path.to_string_lossy().into_owned();
    fs::write(format!("{path_text}.8"), "eighth").expect("seed eighth backup");
    fs::write(format!("{path_text}.9"), "oldest").expect("seed oldest backup");
    let mut sink = FileSink::new(path_text.clone()).expect("create file sink");
    sink.set_max_size(1);

    sink.write(&record("rotate"));
    drop(sink);

    assert_eq!(
        fs::read_to_string(format!("{path_text}.9")).expect("read ninth backup"),
        "eighth"
    );
    assert!(fs::read_to_string(format!("{path_text}.1"))
        .expect("read first backup")
        .contains("\"msg\":\"rotate\""));
    assert_eq!(fs::metadata(&path).expect("read active log").len(), 0);

    fs::remove_dir_all(path.parent().expect("log parent")).expect("remove test directory");
}
