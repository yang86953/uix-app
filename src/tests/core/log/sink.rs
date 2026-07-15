use crate::core::diagnostic::Timestamp;
use crate::core::log::{Level, Record};
use std::time::UNIX_EPOCH;

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
        "{\"ts\":\"1970-01-01T00:00:00.000Z\",\"level\":\"INFO\",\"msg\":\"quote \\\" slash \\\\ newline\\n tab\\t control \\u0001\",\"file\":\"bad\\\"file\\n.rs\",\"line\":42,\"attrs\":{\"key\\\"\":\"value\\n\\\\\"}}"
    );
}
