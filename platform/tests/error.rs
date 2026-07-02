//! uix-platform crate 集成测试（error 模块）。

use uix_platform::error::{
    collect_errors, collect_values, try_invoke, Error, ErrorSeverity, Errc,
    ResultErrorExt, ResultExt, ResultVoidExt, make_error,
};
use std::hash::{DefaultHasher, Hash, Hasher};

// ════════════════════════════════════════════════════════════════════════════
// 构造与访问器
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn error_new_creates_with_correct_code_and_message() {
    let err = Error::new(Errc::NotFound, "资源未找到");
    assert_eq!(err.code(), Errc::NotFound);
    assert_eq!(err.message(), "资源未找到");
}

#[test]
fn error_new_default_severity_is_error() {
    let err = Error::new(Errc::InvalidArgument, "test");
    assert_eq!(err.severity(), ErrorSeverity::Error);
}

#[test]
fn error_new_empty_message() {
    let err = Error::new(Errc::None, "");
    assert_eq!(err.message(), "");
    assert_eq!(err.code(), Errc::None);
}

#[test]
fn default_creates_none_error() {
    let err = Error::default();
    assert_eq!(err.code(), Errc::None);
    assert_eq!(err.message(), "");
}

#[test]
fn accessors_return_correct_values() {
    let err = Error::new(Errc::OutOfRange, "too big");
    assert_eq!(err.code(), Errc::OutOfRange);
    assert_eq!(err.message(), "too big");
    assert_eq!(err.severity(), ErrorSeverity::Error);
    assert!(err.file().contains("error.rs") || err.file().contains("tests"));
    assert!(err.line() > 0);
}

// ════════════════════════════════════════════════════════════════════════════
// 严重级别
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn with_severity_sets_correct_severity() {
    let err = Error::with_severity(Errc::NotFound, "warn", ErrorSeverity::Warning);
    assert_eq!(err.severity(), ErrorSeverity::Warning);
}

#[test]
fn warn_constructor_sets_warning() {
    let err = Error::warn(Errc::Timeout, "timed out");
    assert_eq!(err.severity(), ErrorSeverity::Warning);
}

#[test]
fn info_constructor_sets_info() {
    let err = Error::info(Errc::None, "informational");
    assert_eq!(err.severity(), ErrorSeverity::Info);
}

#[test]
fn fatal_constructor_sets_fatal() {
    let err = Error::fatal(Errc::OutOfRange, "fatal error");
    assert_eq!(err.severity(), ErrorSeverity::Fatal);
}

#[test]
fn set_severity_changes_severity() {
    let err = Error::new(Errc::NotFound, "test").set_severity(ErrorSeverity::Info);
    assert_eq!(err.severity(), ErrorSeverity::Info);
}

// ════════════════════════════════════════════════════════════════════════════
// 源码位置
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn with_location_preserves_file_and_line() {
    let err = Error::with_location(Errc::NotFound, "test", "my_file.rs", 42);
    assert_eq!(err.file(), "my_file.rs");
    assert_eq!(err.line(), 42);
}

// ════════════════════════════════════════════════════════════════════════════
// 原因链
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn with_source_chains_errors() {
    let inner = Error::new(Errc::NotFound, "inner");
    let outer = Error::new(Errc::InvalidArgument, "outer").with_source(inner);
    assert_eq!(outer.depth(), 1);
    assert!(outer.source_error().is_some());
}

#[test]
fn depth_zero_for_no_source() {
    let err = Error::new(Errc::None, "");
    assert_eq!(err.depth(), 0);
}

#[test]
fn depth_counts_chain_length() {
    let e1 = Error::new(Errc::NotFound, "e1");
    let e2 = Error::new(Errc::InvalidArgument, "e2").with_source(e1);
    let e3 = Error::new(Errc::Unknown, "e3").with_source(e2);
    assert_eq!(e3.depth(), 2);
}

#[test]
fn root_cause_returns_deepest_error() {
    let e1 = Error::new(Errc::NotFound, "root");
    let e2 = Error::new(Errc::InvalidArgument, "middle").with_source(e1);
    let e3 = Error::new(Errc::Unknown, "top").with_source(e2);
    let root = e3.root_cause();
    assert!(root.is(Errc::NotFound));
    assert_eq!(root.message(), "root");
}

#[test]
fn root_cause_of_single_error_is_self() {
    let err = Error::new(Errc::NotFound, "alone");
    let root = err.root_cause();
    assert!(root.is(Errc::NotFound));
}

#[test]
fn source_error_returns_none_for_no_source() {
    let err = Error::new(Errc::None, "");
    assert!(err.source_error().is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// 判断方法
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn is_none_true_when_code_is_none() {
    let err = Error::new(Errc::None, "");
    assert!(err.is_none());
}

#[test]
fn is_none_false_when_code_is_not_none() {
    let err = Error::new(Errc::NotFound, "");
    assert!(!err.is_none());
}

#[test]
fn has_value_true_when_code_is_none() {
    let err = Error::new(Errc::None, "");
    assert!(err.has_value());
}

#[test]
fn has_value_false_when_code_is_not_none() {
    let err = Error::new(Errc::NotFound, "");
    assert!(!err.has_value());
}

#[test]
fn is_checks_code_equality() {
    let err = Error::new(Errc::InvalidArgument, "test");
    assert!(err.is(Errc::InvalidArgument));
    assert!(!err.is(Errc::NotFound));
}

// ════════════════════════════════════════════════════════════════════════════
// 便利工厂方法
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn invalid_arg_creates_invalid_argument() {
    let err = Error::invalid_arg("bad arg");
    assert!(err.is(Errc::InvalidArgument));
    assert_eq!(err.message(), "bad arg");
}

#[test]
fn not_found_creates_not_found() {
    let err = Error::not_found("file missing");
    assert!(err.is(Errc::NotFound));
    assert_eq!(err.message(), "file missing");
}

#[test]
fn invalid_state_creates_invalid_state() {
    let err = Error::invalid_state("state error");
    assert!(err.is(Errc::InvalidState));
    assert_eq!(err.message(), "state error");
}

#[test]
fn not_implemented_creates_not_implemented() {
    let err = Error::not_implemented("todo");
    assert!(err.is(Errc::NotImplemented));
}

#[test]
fn io_error_creates_io_error() {
    let err = Error::io_error("disk full");
    assert!(err.is(Errc::IoError));
}

#[test]
fn write_failure_creates_write_failure() {
    let err = Error::write_failure("write failed");
    assert!(err.is(Errc::WriteFailure));
}

#[test]
fn unknown_creates_unknown() {
    let err = Error::unknown("something happened");
    assert!(err.is(Errc::Unknown));
}

// ════════════════════════════════════════════════════════════════════════════
// 格式化输出
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn short_what_includes_severity_code_and_location() {
    let err = Error::new(Errc::InvalidArgument, "bad input");
    let s = err.short_what();
    assert!(s.contains("[ERROR]"));
    assert!(s.contains("invalid_argument"));
    assert!(s.contains("error.rs"));
}

#[test]
fn short_what_with_source_does_not_include_source() {
    let inner = Error::new(Errc::NotFound, "inner");
    let outer = Error::new(Errc::InvalidArgument, "outer").with_source(inner);
    let s = outer.short_what();
    assert!(!s.contains("inner"));
    assert!(!s.contains("cause"));
}

#[test]
fn what_includes_category_code_message_and_location() {
    let err = Error::new(Errc::InvalidArgument, "bad input");
    let s = err.what();
    assert!(s.contains("general"));
    assert!(s.contains("invalid_argument"));
    assert!(s.contains("bad input"));
    assert!(s.contains("error.rs"));
}

#[test]
fn what_with_source_includes_cause() {
    let inner = Error::new(Errc::NotFound, "inner cause");
    let outer = Error::new(Errc::InvalidArgument, "outer").with_source(inner);
    let s = outer.what();
    assert!(s.contains("cause"));
    assert!(s.contains("inner cause"));
    assert!(s.contains("not_found"));
}

#[test]
fn display_delegates_to_what() {
    let err = Error::new(Errc::InvalidArgument, "display test");
    let display = format!("{}", err);
    assert_eq!(display, err.what());
}

// ════════════════════════════════════════════════════════════════════════════
// PartialEq / Eq / Hash
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn partial_eq_same_code_and_message() {
    let a = Error::new(Errc::NotFound, "test");
    let b = Error::new(Errc::NotFound, "test");
    assert_eq!(a, b);
}

#[test]
fn partial_eq_different_code() {
    let a = Error::new(Errc::NotFound, "test");
    let b = Error::new(Errc::InvalidArgument, "test");
    assert_ne!(a, b);
}

#[test]
fn partial_eq_different_message() {
    let a = Error::new(Errc::NotFound, "message a");
    let b = Error::new(Errc::NotFound, "message b");
    assert_ne!(a, b);
}

#[test]
fn partial_eq_ignores_severity_file_line_timestamp_source() {
    let a = Error::info(Errc::NotFound, "test");
    let b = Error::fatal(Errc::NotFound, "test");
    assert_eq!(a, b);
}

#[test]
fn hash_consistent_with_partial_eq() {
    let a = Error::new(Errc::NotFound, "hash test");
    let b = Error::new(Errc::NotFound, "hash test");
    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    let hash_a = hasher.finish();
    let mut hasher = DefaultHasher::new();
    b.hash(&mut hasher);
    let hash_b = hasher.finish();
    assert_eq!(hash_a, hash_b);
}

#[test]
fn hash_different_for_different_codes() {
    let a = Error::new(Errc::NotFound, "test");
    let b = Error::new(Errc::Unknown, "test");
    let mut hasher = DefaultHasher::new();
    a.hash(&mut hasher);
    let hash_a = hasher.finish();
    let mut hasher = DefaultHasher::new();
    b.hash(&mut hasher);
    let hash_b = hasher.finish();
    assert_ne!(hash_a, hash_b);
}

// ════════════════════════════════════════════════════════════════════════════
// From<std::io::Error>
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn from_io_error_not_found() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
    let err: Error = io_err.into();
    assert_eq!(err.code(), Errc::FileNotFound);
}

#[test]
fn from_io_error_permission_denied() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let err: Error = io_err.into();
    assert_eq!(err.code(), Errc::AccessDenied);
}

#[test]
fn from_io_error_timed_out() {
    let io_err = std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout");
    let err: Error = io_err.into();
    assert_eq!(err.code(), Errc::Timeout);
}

#[test]
fn from_io_error_invalid_input() {
    let io_err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid");
    let err: Error = io_err.into();
    assert_eq!(err.code(), Errc::InvalidArgument);
}

#[test]
fn from_io_error_unknown_kind() {
    let io_err = std::io::Error::new(std::io::ErrorKind::Other, "other");
    let err: Error = io_err.into();
    assert_eq!(err.code(), Errc::IoError);
}

#[test]
fn from_io_error_write_zero() {
    let io_err = std::io::Error::new(std::io::ErrorKind::WriteZero, "write zero");
    let err: Error = io_err.into();
    assert_eq!(err.code(), Errc::WriteFailure);
}

// ════════════════════════════════════════════════════════════════════════════
// Errc::category
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn errc_category_general() {
    assert_eq!(Errc::None.category(), "general");
    assert_eq!(Errc::Unknown.category(), "general");
    assert_eq!(Errc::InvalidArgument.category(), "general");
    assert_eq!(Errc::NotImplemented.category(), "general");
}

#[test]
fn errc_category_io() {
    assert_eq!(Errc::IoError.category(), "io");
    assert_eq!(Errc::FileNotFound.category(), "io");
    assert_eq!(Errc::WriteFailure.category(), "io");
    assert_eq!(Errc::EndOfFile.category(), "io");
}

#[test]
fn errc_category_network() {
    assert_eq!(Errc::NetworkError.category(), "network");
    assert_eq!(Errc::ConnectionRefused.category(), "network");
    assert_eq!(Errc::TlsError.category(), "network");
}

#[test]
fn errc_category_protocol() {
    assert_eq!(Errc::ProtocolError.category(), "protocol");
    assert_eq!(Errc::InvalidState.category(), "protocol");
    assert_eq!(Errc::ParseError.category(), "protocol");
}

#[test]
fn errc_category_concurrency() {
    assert_eq!(Errc::DeadlockDetected.category(), "concurrency");
    assert_eq!(Errc::TaskAbandoned.category(), "concurrency");
}

#[test]
fn errc_category_platform() {
    assert_eq!(Errc::PlatformError.category(), "platform");
    assert_eq!(Errc::WindowCreationFailed.category(), "platform");
}

#[test]
fn errc_category_application() {
    assert_eq!(Errc::AppDomainBase.category(), "application");
}

// ════════════════════════════════════════════════════════════════════════════
// Errc::to_io_kind
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn errc_to_io_kind_invalid_argument() {
    assert_eq!(
        Errc::InvalidArgument.to_io_kind(),
        Some(std::io::ErrorKind::InvalidInput)
    );
}

#[test]
fn errc_to_io_kind_not_found() {
    assert_eq!(Errc::NotFound.to_io_kind(), Some(std::io::ErrorKind::NotFound));
    assert_eq!(
        Errc::FileNotFound.to_io_kind(),
        Some(std::io::ErrorKind::NotFound)
    );
}

#[test]
fn errc_to_io_kind_permission_denied() {
    assert_eq!(
        Errc::PermissionDenied.to_io_kind(),
        Some(std::io::ErrorKind::PermissionDenied)
    );
    assert_eq!(
        Errc::AccessDenied.to_io_kind(),
        Some(std::io::ErrorKind::PermissionDenied)
    );
}

#[test]
fn errc_to_io_kind_timeout() {
    assert_eq!(Errc::Timeout.to_io_kind(), Some(std::io::ErrorKind::TimedOut));
    assert_eq!(
        Errc::ConnectionTimeout.to_io_kind(),
        Some(std::io::ErrorKind::TimedOut)
    );
}

#[test]
fn errc_to_io_kind_write_failure() {
    assert_eq!(
        Errc::WriteFailure.to_io_kind(),
        Some(std::io::ErrorKind::WriteZero)
    );
}

#[test]
fn errc_to_io_kind_none_returns_none() {
    assert_eq!(Errc::None.to_io_kind(), None);
}

#[test]
fn errc_to_io_kind_unknown_returns_none() {
    assert_eq!(Errc::Unknown.to_io_kind(), None);
}

#[test]
fn errc_to_io_kind_not_implemented_returns_none() {
    assert_eq!(Errc::NotImplemented.to_io_kind(), None);
}

// ════════════════════════════════════════════════════════════════════════════
// ErrorSeverity
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn error_severity_is_fatal() {
    assert!(ErrorSeverity::Fatal.is_fatal());
    assert!(!ErrorSeverity::Error.is_fatal());
    assert!(!ErrorSeverity::Warning.is_fatal());
    assert!(!ErrorSeverity::Info.is_fatal());
}

#[test]
fn error_severity_should_abort() {
    assert!(ErrorSeverity::Fatal.should_abort());
    assert!(!ErrorSeverity::Error.should_abort());
}

#[test]
fn error_severity_display() {
    assert_eq!(format!("{}", ErrorSeverity::Info), "INFO");
    assert_eq!(format!("{}", ErrorSeverity::Warning), "WARN");
    assert_eq!(format!("{}", ErrorSeverity::Error), "ERROR");
    assert_eq!(format!("{}", ErrorSeverity::Fatal), "FATAL");
}

#[test]
fn error_severity_ordering() {
    assert!(ErrorSeverity::Info < ErrorSeverity::Warning);
    assert!(ErrorSeverity::Warning < ErrorSeverity::Error);
    assert!(ErrorSeverity::Error < ErrorSeverity::Fatal);
}

// ════════════════════════════════════════════════════════════════════════════
// Errc Display
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn errc_display() {
    assert_eq!(format!("{}", Errc::None), "none");
    assert_eq!(format!("{}", Errc::NotFound), "not_found");
    assert_eq!(format!("{}", Errc::InvalidArgument), "invalid_argument");
    assert_eq!(format!("{}", Errc::IoError), "io_error");
    assert_eq!(format!("{}", Errc::WindowCreationFailed), "window_creation_failed");
}

// ════════════════════════════════════════════════════════════════════════════
// make_error 工厂函数
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn make_error_creates_error() {
    let err = make_error(Errc::NotFound, "via factory");
    assert_eq!(err.code(), Errc::NotFound);
    assert_eq!(err.message(), "via factory");
}

// ════════════════════════════════════════════════════════════════════════════
// ResultExt
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn result_ext_has_value_on_ok() {
    let res: Result<i32, Error> = Ok(42);
    assert!(res.has_value());
}

#[test]
fn result_ext_has_value_on_err() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, ""));
    assert!(!res.has_value());
}

#[test]
fn result_ext_has_error_on_err() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, ""));
    assert!(res.has_error());
}

#[test]
fn result_ext_has_error_on_ok() {
    let res: Result<i32, Error> = Ok(42);
    assert!(!res.has_error());
}

#[test]
fn result_ext_value_ok_returns_some() {
    let res: Result<i32, Error> = Ok(42);
    assert_eq!(res.value(), Some(&42));
}

#[test]
fn result_ext_value_err_returns_none() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, ""));
    assert!(res.value().is_none());
}

#[test]
fn result_ext_value_mut_modifies_inner() {
    let mut res: Result<i32, Error> = Ok(10);
    if let Some(v) = res.value_mut() {
        *v = 20;
    }
    assert_eq!(res.value(), Some(&20));
}

#[test]
fn result_ext_into_value_ok_returns_some() {
    let res: Result<i32, Error> = Ok(42);
    assert_eq!(res.into_value(), Some(42));
}

#[test]
fn result_ext_into_value_err_returns_none() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, ""));
    assert!(res.into_value().is_none());
}

#[test]
fn result_ext_error_ok_returns_none() {
    let res: Result<i32, Error> = Ok(42);
    assert!(res.error().is_none());
}

#[test]
fn result_ext_error_err_returns_some() {
    let err = Error::new(Errc::NotFound, "test err");
    let res: Result<i32, Error> = Err(err);
    assert!(res.error().is_some());
    assert_eq!(res.error().unwrap().code(), Errc::NotFound);
}

#[test]
fn result_ext_value_or_returns_value_on_ok() {
    let res: Result<i32, Error> = Ok(42);
    assert_eq!(res.value_or(0), 42);
}

#[test]
fn result_ext_value_or_returns_default_on_err() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, ""));
    assert_eq!(res.value_or(99), 99);
}

#[test]
fn result_ext_err_message_returns_some_on_err() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, "custom msg"));
    assert_eq!(res.err_message(), Some("custom msg"));
}

#[test]
fn result_ext_err_message_returns_none_on_ok() {
    let res: Result<i32, Error> = Ok(42);
    assert!(res.err_message().is_none());
}

#[test]
fn result_ext_err_code_returns_some_on_err() {
    let res: Result<i32, Error> = Err(Error::new(Errc::NotFound, ""));
    assert_eq!(res.err_code(), Some(Errc::NotFound));
}

#[test]
fn result_ext_err_code_returns_none_on_ok() {
    let res: Result<i32, Error> = Ok(42);
    assert!(res.err_code().is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// ResultErrorExt
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn result_error_ext_ok_value() {
    let res: Result<i32, Error> = ResultErrorExt::ok_value(42);
    assert_eq!(res.value(), Some(&42));
}

#[test]
fn result_error_ext_err_error() {
    let err = Error::new(Errc::NotFound, "wrapped");
    let res: Result<i32, Error> = ResultErrorExt::err_error(err);
    assert!(res.has_error());
    assert_eq!(res.err_code(), Some(Errc::NotFound));
}

#[test]
fn result_error_ext_fail() {
    let res: Result<i32, Error> = ResultErrorExt::fail(Errc::InvalidArgument, "failed");
    assert!(res.has_error());
    assert_eq!(res.err_message(), Some("failed"));
}

// ════════════════════════════════════════════════════════════════════════════
// ResultVoidExt
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn result_void_ext_ok_void() {
    let res: Result<(), Error> = ResultVoidExt::ok_void();
    assert!(res.has_value());
}

// ════════════════════════════════════════════════════════════════════════════
// try_invoke
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn try_invoke_success_returns_value() {
    let result = try_invoke(|| 42);
    assert!(result.has_value());
    assert_eq!(result.into_value(), Some(42));
}

#[test]
fn try_invoke_panic_returns_error() {
    let result = try_invoke(|| -> i32 {
        panic!("intentional panic in test");
    });
    assert!(result.has_error());
    assert_eq!(result.err_code(), Some(Errc::Unknown));
}

#[test]
fn try_invoke_with_side_effects() {
    let mut val = 0;
    let result = try_invoke(|| {
        val = 10;
        val
    });
    assert!(result.has_value());
    assert_eq!(result.into_value(), Some(10));
}

// ════════════════════════════════════════════════════════════════════════════
// collect_values / collect_errors
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn collect_values_returns_all_oks() {
    let results: Vec<Result<i32, Error>> = vec![
        Ok(1),
        Err(Error::new(Errc::NotFound, "e1")),
        Ok(3),
        Err(Error::new(Errc::Unknown, "e2")),
        Ok(5),
    ];
    let values = collect_values(results);
    assert_eq!(values, vec![1, 3, 5]);
}

#[test]
fn collect_values_empty_input() {
    let results: Vec<Result<i32, Error>> = vec![];
    let values = collect_values(results);
    assert!(values.is_empty());
}

#[test]
fn collect_values_all_errors() {
    let results: Vec<Result<i32, Error>> = vec![
        Err(Error::new(Errc::NotFound, "e1")),
        Err(Error::new(Errc::Unknown, "e2")),
    ];
    let values = collect_values(results);
    assert!(values.is_empty());
}

#[test]
fn collect_errors_returns_all_errs() {
    let results: Vec<Result<i32, Error>> = vec![
        Ok(1),
        Err(Error::new(Errc::NotFound, "e1")),
        Ok(3),
        Err(Error::new(Errc::Unknown, "e2")),
    ];
    let errors = collect_errors(results);
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].code(), Errc::NotFound);
    assert_eq!(errors[1].code(), Errc::Unknown);
}

#[test]
fn collect_errors_empty_input() {
    let results: Vec<Result<i32, Error>> = vec![];
    let errors = collect_errors(results);
    assert!(errors.is_empty());
}

#[test]
fn collect_errors_all_oks() {
    let results: Vec<Result<i32, Error>> = vec![Ok(1), Ok(2), Ok(3)];
    let errors = collect_errors(results);
    assert!(errors.is_empty());
}
