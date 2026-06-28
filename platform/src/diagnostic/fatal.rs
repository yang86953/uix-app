// ============================================================================
// platform/src/diagnostic/fatal.rs — 崩溃处理
//
// Crash dump, abort handler, panic integration.
// ============================================================================

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Once;

use crate::error::{Error, Errc, ErrorSeverity};
use crate::diagnostic::collector::Collector;
use crate::diagnostic::Timestamp;
use crate::log::{Level, Logger};

static FATAL_INSTALLED: Once = Once::new();

/// Install the global panic hook. Converts panics into diagnostic errors,
/// dumps the collector snapshot, and flushes log sinks before abort.
pub fn install_fatal_handler() {
    FATAL_INSTALLED.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let msg_str: String = if let Some(s) = info.payload().downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic payload".to_string()
            };

            let loc_file = info.location().map(|l| l.file().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let loc_line = info.location().map(|l| l.line()).unwrap_or(0);
            let panic_msg = format!("PANIC at {}:{}: {}", loc_file, loc_line, msg_str);

            Logger::instance().log(
                Level::Fatal,
                panic_msg.clone(),
                "<panic>",
                0,
                Vec::new(),
            );

            Collector::instance().collect(Error::fatal(Errc::Unknown, panic_msg.clone()));

            if msg_str.contains("platform only supports Windows") {
                Collector::instance().collect(Error::fatal(
                    Errc::PlatformError,
                    "compiled for unsupported target",
                ));
            }

            dump_crash_report();

            default_hook(info);
            std::process::abort();
        }));
    });
}

/// Write collector snapshot to uix_crash.log and flush all log sinks.
pub fn dump_crash_report() {
    let snapshot = Collector::instance().snapshot();
    let report = format!(
        "\
╔══════════════════════════════════════════════════╗
║            UIX CRASH REPORT                     ║
╠══════════════════════════════════════════════════╣
║ Timestamp : {}  ║
║ Collected : {} errors ({} stored)               ║
╠══════════════════════════════════════════════════╣
{}
╚══════════════════════════════════════════════════╝
",
        Timestamp::from_system_time(snapshot.timestamp).format_datetime_utc(),
        snapshot.total_collected,
        snapshot.stored_count,
        snapshot
    );

    let path = Path::new("uix_crash.log");
    if let Ok(mut f) = fs::File::create(path) {
        let _ = f.write_all(report.as_bytes());
        let _ = f.flush();
    }

    eprintln!("{}", report);

    Logger::instance().flush();
}

/// Trigger a fatal abort with the given error. Dumps crash report and exits.
pub fn fatal_abort(error: Error) -> ! {
    let mut err = error;
    if !err.severity().is_fatal() {
        err = err.set_severity(ErrorSeverity::Fatal);
    }
    Logger::instance().log(
        Level::Fatal,
        err.to_string(),
        err.file(),
        err.line(),
        Vec::new(),
    );
    Collector::instance().collect(err);
    dump_crash_report();
    std::process::abort();
}

/// If an error is fatal, abort. Otherwise, return it for further handling.
pub fn abort_if_fatal(err: Error) -> Error {
    if err.severity().should_abort() {
        fatal_abort(err);
    }
    err
}

/// Convenience: collect an error, and if it's fatal, immediately abort.
pub fn collect_or_abort(err: Error) {
    let severity = err.severity();
    Collector::instance().collect(err);
    if severity.should_abort() {
        dump_crash_report();
        std::process::abort();
    }
}
