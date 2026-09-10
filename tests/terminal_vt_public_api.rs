//! Documented terminal capabilities, exercised through a real, private Linux PTY.
#![cfg(all(target_os = "linux", feature = "terminal"))]

use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use uix_app::prelude::*;

struct TerminalProbe {
    session: TerminalSession,
    output: mpsc::Receiver<()>,
}

impl TerminalProbe {
    fn new(script: &str, cols: u16, rows: u16) -> Self {
        let (sender, output) = mpsc::sync_channel(1);
        let session = TerminalSession::spawn(&TerminalSessionConfig {
            command: vec![
                "/bin/sh".into(),
                "-c".into(),
                format!("stty -echo; {script}"),
            ],
            cols,
            rows,
            env: vec![
                ("ENV".into(), String::new()),
                ("BASH_ENV".into(), String::new()),
            ],
            on_output: Some(Arc::new(move || {
                let _ = sender.try_send(());
            })),
            ..TerminalSessionConfig::default()
        })
        .expect("private PTY starts");
        Self { session, output }
    }

    fn wait(&self, predicate: impl Fn(&TerminalSession) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.session.pump();
            if predicate(&self.session) {
                return;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "terminal deadline: {:?}, {:?}",
                self.session.status(),
                self.lines()
            );
            let _ = self.output.recv_timeout(remaining);
        }
    }

    fn lines(&self) -> Vec<String> {
        self.session
            .rows()
            .iter()
            .map(|row| row.plain().trim_end().to_owned())
            .collect()
    }

    fn history(&self) -> Vec<String> {
        self.session
            .scrollback_rows(0, usize::MAX)
            .iter()
            .map(|row| row.plain().trim_end().to_owned())
            .collect()
    }

    fn advance(&self) {
        self.session.write(b"\n").expect("continue synthetic child");
    }

    fn finished(&self) {
        self.wait(|session| session.status() != TerminalSessionStatus::Running);
        assert_eq!(
            self.session.status(),
            TerminalSessionStatus::Exited { code: Some(0) }
        );
    }
}

#[test]
fn alternate_screen_1049_preserves_primary_cursor_and_attributes_across_resize() {
    let probe = TerminalProbe::new(
        "printf '\\033[31mMAIN\\033[3;4H\\033[?1049h\\033[H\\033[32mALT'; read -r step; \
         printf '\\033[?1049hB'; read -r step; \
         printf '\\033[?1049lZ\\033[?1049lY'",
        12,
        5,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT"));
    assert_eq!(probe.lines(), ["ALT", "", "", "", ""]);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("ALTB"));
    probe.session.resize(10, 4).unwrap();
    assert_eq!(probe.session.screen_size(), (10, 4));
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["MAIN", "", "   ZY", ""]);
    assert_eq!(probe.session.cursor(), (2, 5));
    let rows = probe.session.rows();
    let restored = rows[2]
        .spans
        .iter()
        .find(|span| span.text.contains('Z'))
        .unwrap();
    assert_eq!(restored.style.fg, TerminalColorSpec::Palette(1));
    assert_eq!(probe.session.ignored_sequences(), 0);
}

#[test]
fn alternate_47_retains_content_and_1047_clears_only_alternate_on_exit() {
    let probe = TerminalProbe::new(
        "printf 'MAIN\\033[?47h\\033[HALT\\033[?47l\\033[H'; read -r step; \
         printf '\\033[?47h\\033[1;4H2'; read -r step; \
         printf '\\033[?1047l\\033[H'; read -r step; \
         printf '\\033[?47h\\033[2;1HREADY'",
        12,
        4,
    );
    probe.wait(|session| {
        session.cursor() == (0, 0) && session.rows()[0].plain().starts_with("MAIN")
    });
    assert_eq!(probe.lines(), ["MAIN", "", "", ""]);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT2"));
    assert_eq!(probe.lines(), ["ALT2", "", "", ""]);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("MAIN"));
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["", "READY", "", ""]);
    assert_eq!(probe.session.ignored_sequences(), 0);
}

#[test]
fn cursor_save_restore_includes_style_and_delayed_wrap() {
    for (save, restore) in [("\\0337", "\\0338"), ("\\033[?1048h", "\\033[?1048l")] {
        let probe = TerminalProbe::new(
            &format!("printf '\\033[31m123456{save}\\033[H\\033[32mX{restore}Z'"),
            6,
            3,
        );
        probe.finished();
        assert_eq!(probe.lines(), ["X23456", "Z", ""]);
        assert_eq!(probe.session.cursor(), (1, 1));
        assert_eq!(
            probe.session.rows()[1].spans[0].style.fg,
            TerminalColorSpec::Palette(1)
        );
    }
}

#[test]
fn scrolling_region_protects_header_and_footer_for_linefeed_and_reverse_index() {
    let probe = TerminalProbe::new(
        "printf 'HEAD\\033[2;1HA\\033[3;1HB\\033[4;1HC\\033[5;1HFOOT\\033[2;4r\\033[4;1H\nD'; read -r step; \
         printf '\\033[2;1H\\033MZ'",
        8,
        5,
    );
    probe.wait(|session| session.rows()[3].plain().starts_with('D'));
    assert_eq!(probe.lines(), ["HEAD", "B", "C", "D", "FOOT"]);
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["HEAD", "Z", "B", "C", "FOOT"]);
    assert_eq!(probe.session.ignored_sequences(), 0);
}

#[test]
fn insert_delete_and_explicit_scroll_are_bounded_by_the_region() {
    for (operation, expected) in [
        ("\\033[3;1H\\033[L", vec!["HEAD", "A", "", "B", "FOOT"]),
        ("\\033[3;1H\\033[M", vec!["HEAD", "A", "C", "", "FOOT"]),
        ("\\033[65535S", vec!["HEAD", "", "", "", "FOOT"]),
        ("\\033[65535T", vec!["HEAD", "", "", "", "FOOT"]),
        (
            "\\033[1;1H\\033[65535L",
            vec!["HEAD", "A", "B", "C", "FOOT"],
        ),
        (
            "\\033[5;1H\\033[65535M",
            vec!["HEAD", "A", "B", "C", "FOOT"],
        ),
    ] {
        let probe = TerminalProbe::new(
            &format!(
                "printf 'HEAD\\033[2;1HA\\033[3;1HB\\033[4;1HC\\033[5;1HFOOT\\033[2;4r{operation}'"
            ),
            8,
            5,
        );
        probe.finished();
        assert_eq!(probe.lines(), expected, "operation {operation}");
        assert_eq!(probe.session.ignored_sequences(), 0);
    }
}

#[test]
fn origin_and_autowrap_modes_apply_and_invalid_margins_preserve_the_region() {
    let probe = TerminalProbe::new(
        "printf 'HEAD\\033[5;1HFOOT\\033[2;4r\\033[?6hA\\033[99;1HB\\033[3;3r\\033[1;2HZ\\033[?6l\\033[1;5HQ\\033[?7l123456'; read -r step; \
         printf '\\033[?7h\\033[HabcdefZ'",
        6,
        5,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("HEADQ6"));
    assert_eq!(probe.lines(), ["HEADQ6", "AZ", "", "B", "FOOT"]);
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["abcdef", "ZZ", "", "B", "FOOT"]);
    assert_eq!(probe.session.ignored_sequences(), 0);
}

#[test]
fn unsupported_private_and_intermediate_sequences_do_not_execute_public_operations() {
    let probe = TerminalProbe::new(
        "printf 'KEEP\\033[>2J\\033[?2J\\033[2$J\\033#8\\033[>1;1H'",
        8,
        3,
    );
    probe.finished();
    assert_eq!(probe.lines(), ["KEEP", "", ""]);
    assert_eq!(probe.session.cursor(), (0, 4));
    assert_eq!(probe.session.ignored_sequences(), 5);
}

#[test]
fn alternate_1049_reentry_clears_old_content_and_reset_discards_both_screens() {
    let probe = TerminalProbe::new(
        "printf 'MAIN\\033[?1049h\\033[HOLD\\033[?1049l\\033[?1049h\\033[2;1HNEW'; read -r step; \
         printf '\\033[2;3r\\033[?6;7l\\033cRESET\\033[?47h\\033[2;1HFRESH'",
        10,
        4,
    );
    probe.wait(|session| session.rows()[1].plain().starts_with("NEW"));
    assert_eq!(probe.lines(), ["", "NEW", "", ""]);
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["", "FRESH", "", ""]);
    assert_eq!(probe.session.ignored_sequences(), 0);
}

#[test]
fn alternate_cursor_save_does_not_overwrite_primary_saved_cursor() {
    let probe = TerminalProbe::new(
        "printf '\\033[31mMAIN\\033[3;3H\\033[?1049h\\033[H\\033[32mALT\\0337\\033[4;1HTEMP\\0338X\\033[?1049lZ'",
        10,
        5,
    );
    probe.finished();
    assert_eq!(probe.lines(), ["MAIN", "", "  Z", "", ""]);
    assert_eq!(probe.session.cursor(), (2, 3));
    assert_eq!(
        probe.session.rows()[2]
            .spans
            .iter()
            .find(|span| span.text.contains('Z'))
            .unwrap()
            .style
            .fg,
        TerminalColorSpec::Palette(1)
    );
}

#[test]
fn region_reset_and_resize_restore_full_screen_scrolling() {
    let probe = TerminalProbe::new(
        "printf 'HEAD\\033[2;1HA\\033[3;1HB\\033[4;1HFOOT\\033[2;3r'; read -r step; \
         printf '\\033[r\\033[4;1H\\nNEXT'",
        8,
        4,
    );
    probe.wait(|session| {
        session.rows()[3].plain().starts_with("FOOT") && session.cursor() == (0, 0)
    });
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["A", "B", "FOOT", "NEXT"]);

    let probe = TerminalProbe::new(
        "printf 'HEAD\\033[2;1HA\\033[3;1HB\\033[4;1HFOOT\\033[2;3r'; read -r step; \
         printf '\\033[5;1HEND\\nNEXT'",
        8,
        4,
    );
    probe.wait(|session| {
        session.rows()[3].plain().starts_with("FOOT") && session.cursor() == (0, 0)
    });
    probe.session.resize(8, 5).unwrap();
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["A", "B", "FOOT", "END", "NEXT"]);
}

#[test]
fn oversized_grid_is_rejected_without_mutating_either_screen() {
    let result = TerminalSession::spawn(&TerminalSessionConfig {
        command: vec!["/bin/sh".into(), "-c".into(), "exit 0".into()],
        cols: u16::MAX,
        rows: u16::MAX,
        ..TerminalSessionConfig::default()
    });
    assert!(matches!(result, Err(error) if error.code() == uix_app::core::Errc::InsufficientResources));

    let probe = TerminalProbe::new(
        "printf 'MAIN\\033[?1049h\\033[HALT'; read -r step; printf '\\033[?1049l'",
        8,
        4,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT"));
    let before = probe.session.rows();
    assert_eq!(
        probe.session.resize(u16::MAX, u16::MAX).unwrap_err().code(),
        uix_app::core::Errc::InsufficientResources
    );
    assert_eq!(probe.session.screen_size(), (8, 4));
    assert_eq!(probe.session.rows(), before);
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["MAIN", "", "", ""]);
}

#[test]
fn linefeed_and_reverse_index_cancel_pending_wrap_without_resetting_column() {
    for (motion, expected) in [
        ("\\n", vec!["123456", "     X", ""]),
        ("\\033D", vec!["123456", "     X", ""]),
        ("\\033M", vec!["     X", "123456", ""]),
    ] {
        let probe = TerminalProbe::new(&format!("stty -opost; printf '123456{motion}X'"), 6, 3);
        probe.finished();
        assert_eq!(probe.lines(), expected, "motion {motion}");
    }
}

#[test]
fn resizing_alternate_does_not_restore_a_half_width_glyph_on_primary() {
    let probe = TerminalProbe::new(
        "printf 'ab中\\033[?1049h\\033[HALT'; read -r step; printf '\\033[?1049l'",
        6,
        3,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT"));
    probe.session.resize(3, 3).unwrap();
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["ab", "", ""]);
    assert_eq!(probe.session.cursor(), (0, 2));
}

#[cfg(feature = "test-harness")]
#[test]
fn terminal_screen_semantics_follow_the_active_buffer_without_recreating_the_session() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'MAIN'; read -r step; printf '\\033[?1049h\\033[HALT'; read -r step; printf '\\033[?1049l'",
        40,
        8,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((480.0, 240.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[0].plain().starts_with("MAIN"));
    app.settle().unwrap();
    assert!(app.text("screen").unwrap().contains("MAIN"));
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT"));
    app.settle().unwrap();
    let alternate = app.text("screen").unwrap();
    assert!(alternate.contains("ALT") && !alternate.contains("MAIN"));
    probe.advance();
    probe.finished();
    app.settle().unwrap();
    let primary = app.text("screen").unwrap();
    assert!(primary.contains("MAIN") && !primary.contains("ALT"));
}

#[cfg(feature = "test-harness")]
#[test]
fn terminal_screen_wheel_can_revisit_output_that_left_the_live_grid() {
    use uix_app::core::Point;
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.settle().unwrap();
    let live = app.text("screen").unwrap();
    assert!(!live.contains("line0") && live.contains("line4"));
    app.scroll("screen", Point::new(0.0, 1000.0))
        .expect("terminal advertises scrollback");
    let historical = app.text("screen").unwrap();
    assert!(
        historical.contains("line0") && !historical.contains("line4"),
        "{historical}"
    );
    app.scroll("screen", Point::new(0.0, -1000.0)).unwrap();
    assert!(app.text("screen").unwrap().contains("line4"));
    probe.advance();
    probe.finished();
}

#[test]
fn scrollback_preserves_order_styles_wide_characters_and_bounded_ranges() {
    let probe = TerminalProbe::new(
        "printf '\\033[1;31m红Z\\033[0m\\nsecond\\nthird\\nfourth'",
        8,
        2,
    );
    probe.finished();
    assert_eq!(probe.session.scrollback_limit(), 1000);
    assert_eq!(probe.session.scrollback_len(), 2);
    assert_eq!(probe.history(), ["红Z", "second"]);
    assert_eq!(probe.lines(), ["third", "fourth"]);
    let first = &probe.session.scrollback_rows(0, 1)[0];
    assert_eq!(first.spans[0].style.fg, TerminalColorSpec::Palette(1));
    assert!(first.spans[0].style.bold);
    assert_eq!(probe.session.scrollback_rows(1, usize::MAX).len(), 1);
    assert!(
        probe
            .session
            .scrollback_rows(usize::MAX, usize::MAX)
            .is_empty()
    );
    assert!(probe.session.scrollback_rows(0, 0).is_empty());
    let before = probe.session.rows();
    let err = probe.session.set_scrollback_limit(100001).unwrap_err();
    assert_eq!(err.code(), uix_app::core::Errc::InvalidArgument);
    assert_eq!(probe.session.scrollback_limit(), 1000);
    assert_eq!(probe.history(), ["红Z", "second"]);
    probe.session.set_scrollback_limit(1).unwrap();
    assert_eq!(probe.history(), ["second"]);
    probe.session.clear_scrollback();
    assert!(probe.history().is_empty());
    assert_eq!(probe.session.rows(), before);
}

#[test]
fn scrollback_row_cell_and_disabled_limits_are_enforced_before_retaining_output() {
    // The largest supported row limit is exact, not a hint; no unbounded archive.
    let probe = TerminalProbe::new(
        "printf 'READY'; read -r step; i=0; while [ $i -lt 100005 ]; do printf 'x\\n'; i=$((i+1)); done",
        8,
        2,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    probe.session.set_scrollback_limit(100000).unwrap();
    probe.advance();
    probe.finished();
    assert_eq!(probe.session.scrollback_len(), 100000);
    assert_eq!(
        probe.session.scrollback_rows(0, 1)[0].plain().trim_end(),
        "x"
    );
    probe.session.set_scrollback_limit(0).unwrap();
    assert_eq!(probe.session.scrollback_len(), 0);

    let wide = TerminalProbe::new(
        "printf 'READY'; read -r step; i=0; while [ $i -lt 200 ]; do printf '\\033[65535S'; i=$((i+1)); done; read -r step; printf 'END\\n\\n'",
        4096,
        2,
    );
    wide.wait(|session| session.rows()[0].plain().starts_with("READY"));
    wide.session.set_scrollback_limit(100000).unwrap();
    wide.advance();
    wide.wait(|session| session.scrollback_len() == 256);
    assert_eq!(wide.session.scrollback_len(), 1048576 / 4096);
    wide.session.set_scrollback_limit(0).unwrap();
    wide.advance();
    wide.finished();
    assert_eq!(wide.session.scrollback_len(), 0);
}

#[test]
fn only_primary_full_screen_upward_scrolls_enter_history() {
    for (operation, expected) in [
        ("\\033[3;1H\\n", vec!["A"]),
        ("\\033[3;1H\\033D", vec!["A"]),
        ("\\033[S", vec!["A"]),
        ("\\033[65535S", vec!["A", "B", "C"]),
        ("\\033[H\\033[M", vec![]),
        ("\\033[H\\033[L", vec![]),
        ("\\033[T", vec![]),
        ("\\033[2;3r\\033[3;1H\\n\\033[S", vec![]),
        ("\\033[?1049h\\033[3;1H\\n\\033[S\\033[?1049l", vec![]),
    ] {
        let probe = TerminalProbe::new(
            &format!("printf 'A\\033[2;1HB\\033[3;1HC{operation}'"),
            8,
            3,
        );
        probe.finished();
        assert_eq!(probe.history(), expected, "{operation}");
    }
    let wrapped = TerminalProbe::new("printf '0123456789ABCD'", 6, 2);
    wrapped.finished();
    assert_eq!(wrapped.history(), ["012345"]);
}

#[test]
fn erase_saved_lines_and_reset_have_distinct_live_screen_and_host_limit_contracts() {
    let probe = TerminalProbe::new(
        "printf 'one\\ntwo\\nthree'; read -r step; printf '\\033[3J'; read -r step; \
         printf '\\nfour'; read -r step; printf '\\033[2J'; read -r step; printf '\\033cRESET'",
        8,
        2,
    );
    probe.wait(|session| session.rows()[1].plain().starts_with("three"));
    probe.session.set_scrollback_limit(7).unwrap();
    let live = probe.session.rows();
    probe.advance();
    probe.wait(|session| session.scrollback_len() == 0);
    assert_eq!(probe.session.rows(), live, "ED3 does not erase live cells");
    probe.advance();
    probe.wait(|session| session.rows()[1].plain().starts_with("four"));
    assert_eq!(probe.history(), ["two"]);
    probe.advance();
    probe.wait(|session| {
        session
            .rows()
            .iter()
            .all(|row| row.plain().trim().is_empty())
    });
    assert_eq!(probe.history(), ["two"], "ED2 does not erase saved lines");
    probe.advance();
    probe.finished();
    assert_eq!(probe.lines(), ["RESET", ""]);
    assert_eq!(probe.session.scrollback_len(), 0);
    assert_eq!(
        probe.session.scrollback_limit(),
        7,
        "RIS preserves host policy"
    );
}

#[cfg(feature = "test-harness")]
#[test]
fn history_view_stays_anchored_on_output_and_clamps_to_oldest_surviving_row() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step; \
         printf '\\nline5'; read -r step; printf '\\nline6\\nline7\\nline8\\nline9'; read -r step",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    probe.session.set_scrollback_limit(3).unwrap();
    app.focus("screen").unwrap();
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    let historical = app.text("screen").unwrap();
    assert!(historical.contains("line0") && !historical.contains("line4"));
    probe.advance();
    probe.wait(|session| session.rows()[2].plain().starts_with("line5"));
    app.settle().unwrap();
    assert_eq!(app.text("screen").unwrap(), historical);
    probe.advance();
    probe.wait(|session| session.rows()[2].plain().starts_with("line9"));
    app.settle().unwrap();
    let clamped = app.text("screen").unwrap();
    assert!(
        clamped.contains("line4") && !clamped.contains("line0") && !clamped.contains("line9"),
        "{clamped}"
    );
    app.press_key(KeyCode::End, KeyMod::SHIFT).unwrap();
    assert!(app.text("screen").unwrap().contains("line9"));
    probe.advance();
    probe.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn browsing_keys_stay_local_and_actual_input_returns_to_live_output() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; IFS= read -r value; \
         if [ \"$value\" != 'ok' ]; then exit 23; fi; printf '\\nACCEPT'",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.focus("screen").unwrap();
    app.press_key(KeyCode::PageUp, KeyMod::SHIFT).unwrap();
    assert!(app.text("screen").unwrap().contains("line0"));
    app.press_key(KeyCode::PageDown, KeyMod::SHIFT).unwrap();
    assert!(app.text("screen").unwrap().contains("line4"));
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    app.press_key(KeyCode::End, KeyMod::SHIFT).unwrap();
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput { text: "ok".into() })
        .unwrap();
    assert!(app.text("screen").unwrap().contains("line4"));
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    probe.finished();
    assert!(probe.lines().iter().any(|line| line == "ACCEPT"));
}

#[cfg(feature = "test-harness")]
#[test]
fn alternate_output_pauses_primary_history_view_and_returns_without_polluting_it() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step; \
         printf '\\033[?1049h\\033[HALT0\\nALT1\\nALT2\\nALT3'; read -r step; \
         printf '\\033[?1049l'; read -r step",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.focus("screen").unwrap();
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    let historical = app.text("screen").unwrap();
    let history = probe.history();
    probe.advance();
    probe.wait(|session| session.rows()[2].plain().starts_with("ALT3"));
    app.settle().unwrap();
    assert!(app.text("screen").unwrap().contains("ALT3"));
    assert!(!app.text("screen").unwrap().contains("line0"));
    assert!(
        app.scroll("screen", uix_app::core::Point::new(0.0, 1000.0))
            .is_err(),
        "alternate view must not advertise primary scrollback"
    );
    assert_eq!(probe.history(), history);
    probe.advance();
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.settle().unwrap();
    assert_eq!(app.text("screen").unwrap(), historical);
    probe.advance();
    probe.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn history_rows_keep_original_width_while_the_view_clips_and_pads_on_resize() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new("printf 'ab中\\nnext\\nlive'; read -r step", 6, 2);
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[1].plain().starts_with("live"));
    app.focus("screen").unwrap();
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    let stored = probe.session.scrollback_rows(0, 1);
    assert_eq!(stored[0].plain(), "ab中  ");
    probe.session.resize(3, 2).unwrap();
    app.settle().unwrap();
    let clipped = app.text("screen").unwrap();
    assert!(
        clipped.starts_with("ab\n") && !clipped.contains('中'),
        "{clipped}"
    );
    assert_eq!(probe.session.scrollback_rows(0, 1), stored);
    probe.session.resize(8, 2).unwrap();
    app.settle().unwrap();
    // Semantic text intentionally trims trailing cells; stored rows above do not.
    assert!(app.text("screen").unwrap().starts_with("ab中\n"));
    probe.advance();
    probe.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn views_share_output_but_not_history_navigation_and_rebinding_resets_the_anchor() {
    use uix_app::ui::test_harness::TestApp;
    let first = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step",
        12,
        3,
    );
    let second = TerminalProbe::new(
        "printf 'new0\\nnew1\\nnew2\\nnew3\\nnew4'; read -r step",
        12,
        3,
    );
    first.wait(|session| session.rows()[2].plain().starts_with("line4"));
    second.wait(|session| session.rows()[2].plain().starts_with("new4"));
    let sessions = [first.session.clone(), second.session.clone()];
    let selected = State::new(0usize);
    let index = selected.clone();
    let mut app = TestApp::new((640.0, 200.0), move || {
        row((
            embed(TerminalScreen::new(&sessions[index.get()]))
                .width(300.0)
                .automation_id("left"),
            embed(TerminalScreen::new(&sessions[0]))
                .width(300.0)
                .automation_id("right"),
        ))
    });
    app.focus("left").unwrap();
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    let historical = app.text("left").unwrap();
    assert!(historical.contains("line0"));
    assert!(app.text("right").unwrap().contains("line4"));
    // Rebuilding with the same session preserves the left view's runtime state.
    selected.set(0);
    app.settle().unwrap();
    assert_eq!(app.text("left").unwrap(), historical);
    selected.set(1);
    app.settle().unwrap();
    let rebound = app.text("left").unwrap();
    assert!(
        rebound.contains("new4") && !rebound.contains("new0"),
        "{rebound}"
    );
    first.advance();
    first.finished();
    second.advance();
    second.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn fractional_wheel_motion_accumulates_from_the_live_edge() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.settle().unwrap();
    for _ in 0..40 {
        // Raw SystemEvent is already positive-down (unlike TestApp::scroll's
        // native convention); each event is less than one row.
        app.dispatch_system_event(&SystemEvent::Wheel {
            pos: uix_app::core::Point::new(50.0, 50.0),
            delta: uix_app::core::Point::new(0.0, -0.05),
        })
        .unwrap();
    }
    let historical = app.text("screen").unwrap();
    assert!(
        historical.contains("line0") && !historical.contains("line4"),
        "{historical}"
    );
    probe.advance();
    probe.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn clearing_history_releases_the_view_anchor_and_new_output_follows_live() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step; printf '\\nline5'; read -r step",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.focus("screen").unwrap();
    app.press_key(KeyCode::Home, KeyMod::SHIFT).unwrap();
    assert!(app.text("screen").unwrap().contains("line0"));
    probe.session.clear_scrollback();
    app.settle().unwrap();
    let live = app.text("screen").unwrap();
    assert!(live.contains("line4") && !live.contains("line0"));
    probe.advance();
    probe.wait(|session| session.rows()[2].plain().starts_with("line5"));
    app.settle().unwrap();
    assert!(app.text("screen").unwrap().contains("line5"));
    probe.advance();
    probe.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn wheel_targeting_an_offset_view_does_not_scroll_its_sibling() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'line0\\nline1\\nline2\\nline3\\nline4'; read -r step",
        12,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((640.0, 200.0), move || {
        row((
            embed(TerminalScreen::new(&session))
                .width(300.0)
                .automation_id("left"),
            embed(TerminalScreen::new(&session))
                .width(300.0)
                .automation_id("right"),
        ))
    });
    probe.wait(|session| session.rows()[2].plain().starts_with("line4"));
    app.settle().unwrap();
    assert!(app.snapshot().find("right").unwrap().frame.x > 0.0);
    app.scroll("right", uix_app::core::Point::new(0.0, 1000.0))
        .unwrap();
    assert!(app.text("right").unwrap().contains("line0"));
    assert!(app.text("left").unwrap().contains("line4"));
    probe.advance();
    probe.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn application_cursor_key_reaches_the_child_as_ss3_not_csi() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf '\\033[?1hREADY'; IFS= read -r value; printf '\\r\\n'; printf '%s' \"$value\" | od -An -tx1 | tr -d ' \\n'",
        40,
        4,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    app.focus("screen").unwrap();
    app.press_key(KeyCode::Up, KeyMod::NONE).unwrap();
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    probe.finished();
    assert_eq!(probe.lines()[1], "1b4f41");
}

#[cfg(feature = "test-harness")]
#[test]
fn bracketed_paste_frames_the_paste_event_but_not_typed_text() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf '\\033[?2004hREADY'; IFS= read -r value; printf '\\r\\n'; printf '%s' \"$value\" | od -An -tx1 | tr -d ' \\n'",
        80,
        4,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    app.focus("screen").unwrap();
    app.dispatch_system_event(&SystemEvent::Paste { text: "中".into() })
        .unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput { text: "X".into() })
        .unwrap();
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    probe.finished();
    assert_eq!(probe.lines()[1], "1b5b3230307ee4b8ad1b5b3230317e58");
}

#[test]
fn cursor_visibility_private_mode_is_a_supported_nonprinting_sequence() {
    let probe = TerminalProbe::new("printf '\\033[?25lHIDDEN\\033[?25h'", 12, 3);
    probe.finished();
    assert_eq!(probe.lines(), ["HIDDEN", "", ""]);
    assert_eq!(probe.session.ignored_sequences(), 0);
}

#[test]
fn terminal_modes_are_session_state_not_screen_or_saved_cursor_state() {
    let probe = TerminalProbe::new(
        "printf 'START'; read -r step; printf '\\033[2J\\033[H\\033[?1;2004h\\033[?25lON'; read -r step; \
         printf '\\033[?1049h\\033[HALT'; read -r step; \
         printf '\\0337\\033[?1;2004l\\033[?25h\\0338\\033[HDEFAULT'; read -r step; \
         printf '\\033[?1;2004h\\033[?25l\\033cRIS'",
        16,
        4,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("START"));
    assert_eq!(probe.session.modes(), TerminalModes::default());
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("ON"));
    let on = probe.session.modes();
    assert!(on.application_cursor_keys && on.bracketed_paste && !on.cursor_visible);
    probe.session.resize(20, 5).unwrap();
    assert_eq!(probe.session.clone().modes(), on);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT"));
    assert_eq!(probe.session.modes(), on);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("DEFAULT"));
    assert_eq!(
        probe.session.modes(),
        TerminalModes::default(),
        "DECRC must not restore input/visibility modes"
    );
    probe.advance();
    probe.finished();
    assert_eq!(
        probe.session.modes(),
        TerminalModes::default(),
        "RIS resets modes"
    );
    assert!(probe.lines()[0].starts_with("RIS"));
}

#[test]
fn mode_dispatch_rejects_wrong_prefixes_and_colon_subparameters() {
    let probe = TerminalProbe::new(
        "printf '\\033[?1;25;2004h\\033[>1;25;2004l\\033[1;25;2004l\\033[?1:2lREADY'",
        16,
        3,
    );
    probe.finished();
    let modes = probe.session.modes();
    assert!(modes.application_cursor_keys && modes.cursor_visible && modes.bracketed_paste);
    assert_eq!(probe.session.ignored_sequences(), 3);
    assert_eq!(probe.lines(), ["READY", "", ""]);
}

#[test]
fn modes_change_independently_and_survive_returning_to_primary_without_affecting_another_session() {
    let other = TerminalProbe::new("printf 'OTHER'; read -r step", 16, 3);
    other.wait(|session| session.rows()[0].plain().starts_with("OTHER"));
    let probe = TerminalProbe::new(
        "printf '\\033[?1hONE'; read -r step; \
         printf '\\033[?25l\\033[HTWO'; read -r step; \
         printf '\\033[?2004h\\033[?1049h\\033[HALT'; read -r step; \
         printf '\\033[?1049l\\033[HBACK'; read -r step; \
         printf '\\033[?1l\\033[HPLAIN'",
        16,
        3,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("ONE"));
    let one = probe.session.modes();
    assert!(one.application_cursor_keys && one.cursor_visible && !one.bracketed_paste);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("TWO"));
    let two = probe.session.modes();
    assert!(two.application_cursor_keys && !two.cursor_visible && !two.bracketed_paste);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("ALT"));
    let all = probe.session.modes();
    assert!(all.application_cursor_keys && !all.cursor_visible && all.bracketed_paste);
    probe.advance();
    probe.wait(|session| session.rows()[0].plain().starts_with("BACK"));
    assert_eq!(probe.session.modes(), all);
    probe.advance();
    probe.finished();
    let plain = probe.session.modes();
    assert!(!plain.application_cursor_keys && !plain.cursor_visible && plain.bracketed_paste);
    assert_eq!(other.session.modes(), TerminalModes::default());
    other.advance();
    other.finished();
}

#[cfg(feature = "test-harness")]
#[test]
fn all_six_cursor_keys_switch_between_normal_and_application_encodings() {
    use uix_app::ui::test_harness::TestApp;
    for (mode, expected) in [
        ("\\033[?1l", "1b5b411b5b421b5b431b5b441b5b481b5b46"),
        ("\\033[?1h", "1b4f411b4f421b4f431b4f441b4f481b4f46"),
    ] {
        let probe = TerminalProbe::new(
            &format!(
                "printf '{mode}READY'; IFS= read -r value; printf '\\r\\n'; printf '%s' \"$value\" | od -An -tx1 | tr -d ' \\n'"
            ),
            80,
            3,
        );
        let session = probe.session.clone();
        let mut app = TestApp::new((320.0, 120.0), move || {
            embed(TerminalScreen::new(&session)).automation_id("screen")
        });
        probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
        app.focus("screen").unwrap();
        for key in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::Left,
            KeyCode::Home,
            KeyCode::End,
        ] {
            app.press_key(key, KeyMod::NONE).unwrap();
        }
        app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
        probe.finished();
        assert_eq!(probe.lines()[1], expected, "mode {mode}");
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(feature = "test-harness")]
#[test]
fn modified_cursor_function_and_edit_keys_use_xterm_parameters_even_in_application_mode() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf '\\033[?1hREADY'; IFS= read -r value; printf '\\r\\n'; printf '%s' \"$value\" | od -An -tx1 | tr -d ' \\n'",
        1024,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    app.focus("screen").unwrap();
    let mut expected = String::new();
    for (mods, parameter) in [
        (KeyMod::SHIFT, 2),
        (KeyMod::ALT, 3),
        (KeyMod::SHIFT | KeyMod::ALT, 4),
        (KeyMod::CTRL, 5),
        (KeyMod::SHIFT | KeyMod::CTRL, 6),
        (KeyMod::ALT | KeyMod::CTRL, 7),
        (KeyMod::SHIFT | KeyMod::ALT | KeyMod::CTRL, 8),
    ] {
        for (key, sequence) in [
            (KeyCode::Right, format!("\x1b[1;{parameter}C")),
            (KeyCode::F1, format!("\x1b[1;{parameter}P")),
            (KeyCode::F5, format!("\x1b[15;{parameter}~")),
            (KeyCode::Delete, format!("\x1b[3;{parameter}~")),
        ] {
            app.press_key(key, mods).unwrap();
            expected.push_str(&sequence);
        }
    }
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    probe.finished();
    assert_eq!(probe.lines()[1], hex_bytes(expected.as_bytes()));
}

#[cfg(feature = "test-harness")]
#[test]
fn alt_text_uses_the_platform_text_event_and_super_input_does_not_leak() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'READY'; IFS= read -r value; printf '\\r\\n'; printf '%s' \"$value\" | od -An -tx1 | tr -d ' \\n'",
        80,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((320.0, 120.0), move || {
        embed(TerminalScreen::new(&session)).automation_id("screen")
    });
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    app.focus("screen").unwrap();
    // press_key also releases the key; inject a real down/text/up sequence here.
    for (mods, text) in [
        (KeyMod::ALT, "中"),
        (KeyMod::ALT | KeyMod::SHIFT, "X"),
        (KeyMod::SUPER, "leak"),
    ] {
        app.dispatch_system_event(&SystemEvent::KeyDown {
            key: KeyCode::X,
            mods,
        })
        .unwrap();
        app.dispatch_system_event(&SystemEvent::TextInput { text: text.into() })
            .unwrap();
        app.dispatch_system_event(&SystemEvent::KeyUp {
            key: KeyCode::X,
            mods,
        })
        .unwrap();
    }
    app.press_key(KeyCode::Right, KeyMod::SUPER).unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput { text: "ok".into() })
        .unwrap();
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    probe.finished();
    assert_eq!(probe.lines()[1], hex_bytes("\x1b中\x1bXok".as_bytes()));
}

#[test]
fn paste_preserves_unicode_and_line_layout_but_cannot_embed_a_closing_marker() {
    for bracketed in [false, true] {
        let text = "A\t\n中\rB\x1b[201~C\u{009b}D\x03\x7f";
        let clean = "A\t\n中\rB[201~CD";
        let expected = if bracketed {
            format!("\x1b[200~{clean}\x1b[201~typed")
        } else {
            format!("{clean}typed")
        };
        let mode = if bracketed {
            "\\033[?2004h"
        } else {
            "\\033[?2004l"
        };
        let probe = TerminalProbe::new(
            &format!(
                "stty raw -echo; printf '{mode}READY'; value=$(head -c {} | od -An -tx1 | tr -d ' \\n'); printf '\\r\\n%s' \"$value\"",
                expected.len()
            ),
            128,
            3,
        );
        probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
        probe.session.paste("").unwrap();
        probe.session.paste("\0\x1b\u{009b}").unwrap();
        probe.session.paste(text).unwrap();
        probe.session.write(b"typed").unwrap();
        probe.finished();
        assert_eq!(probe.lines()[1], hex_bytes(expected.as_bytes()));
    }
}

// A private fixture controls when the synthetic child starts consuming PTY input.
// Test data live under this source repository's build tree, never the business workspace.
struct InputFixture {
    dir: std::path::PathBuf,
    release: std::fs::File,
}

impl InputFixture {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/continuous-audit-20260908")
            .join(format!(
                "input-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(dir.join("release"))
                .status()
                .unwrap()
                .success()
        );
        let release = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.join("release"))
            .unwrap();
        Self { dir, release }
    }

    fn script(&self, modes: &str) -> String {
        format!(
            "stty raw -echo; printf '{modes}READY'; IFS= read -r count < '{}/release'; \
             head -c \"$count\" > '{}/actual'; \
             if ! cmp -s '{}/expected' '{}/actual'; then printf '\\r\\nMISMATCH'; exit 41; fi; \
             printf '\\r\\nDRAINED'; value=$(head -c 3); \
             if [ \"$value\" != END ]; then printf '\\r\\nEXTRA'; exit 42; fi; printf '\\r\\nCOMPLETE'",
            self.dir.display(),
            self.dir.display(),
            self.dir.display(),
            self.dir.display()
        )
    }

    fn release(&mut self, expected: &[u8]) {
        use std::io::Write;
        std::fs::write(self.dir.join("expected"), expected).unwrap();
        writeln!(self.release, "{}", expected.len()).unwrap();
    }
}

impl Drop for InputFixture {
    fn drop(&mut self) {
        for file in ["release", "expected", "actual"] {
            let _ = std::fs::remove_file(self.dir.join(file));
        }
        let _ = std::fs::remove_dir(&self.dir);
    }
}

#[test]
fn accepted_large_paste_remains_complete_and_ordered_when_the_child_initially_does_not_read() {
    let mut fixture = InputFixture::new();
    let probe = TerminalProbe::new(&fixture.script("\\033[?2004h"), 80, 4);
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    let text = "line\t中\r\n".repeat(16384);
    let expected = format!("\x1b[200~{text}\x1b[201~tail");
    probe
        .session
        .paste(&text)
        .expect("large paste accepted without waiting for the child");
    probe.session.write(b"tail").unwrap();
    fixture.release(expected.as_bytes());
    probe.wait(|session| session.rows()[1].plain().starts_with("DRAINED"));
    probe.session.write(b"END").unwrap();
    probe.finished();
    assert_eq!(probe.lines()[2], "COMPLETE");
}

#[test]
fn bracketed_paste_accepts_exactly_one_mib_including_its_framing() {
    let mut fixture = InputFixture::new();
    let probe = TerminalProbe::new(&fixture.script("\\033[?2004h"), 80, 4);
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    let text = "A".repeat(1048576 - 12);
    let expected = format!("\x1b[200~{text}\x1b[201~");
    assert_eq!(expected.len(), 1048576);
    probe.session.paste(&text).unwrap();
    fixture.release(expected.as_bytes());
    probe.wait(|session| session.rows()[1].plain().starts_with("DRAINED"));
    probe.session.write(b"END").unwrap();
    probe.finished();
    assert_eq!(probe.lines()[2], "COMPLETE");
}

#[test]
fn input_backpressure_rejects_whole_messages_and_recovers_after_the_child_drains() {
    let mut fixture = InputFixture::new();
    let probe = TerminalProbe::new(&fixture.script("\\033[?2004h"), 80, 4);
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    let mut accepted = Vec::new();
    let mut refused = false;
    for index in 0..64u8 {
        let text = char::from(b'A' + index % 26).to_string().repeat(65536);
        match probe.session.paste(&text) {
            Ok(()) => {
                accepted.extend_from_slice(b"\x1b[200~");
                accepted.extend_from_slice(text.as_bytes());
                accepted.extend_from_slice(b"\x1b[201~");
            }
            Err(error) if error.code() == uix_app::core::Errc::WouldBlock => {
                refused = true;
                break;
            }
            Err(error) => panic!("unexpected input failure: {error:?}"),
        }
    }
    assert!(
        refused,
        "stalled input must reach a bounded admission limit"
    );
    assert!(!accepted.is_empty());
    assert_eq!(
        probe
            .session
            .write(&vec![b'R'; 1048577])
            .unwrap_err()
            .code(),
        uix_app::core::Errc::InsufficientResources
    );
    fixture.release(&accepted);
    probe.wait(|session| session.rows()[1].plain().starts_with("DRAINED"));
    probe.session.write(b"END").unwrap();
    probe.finished();
    assert_eq!(
        probe.lines()[2],
        "COMPLETE",
        "rejected bytes must never arrive after accepted input"
    );
    assert_eq!(
        probe.session.write(b"closed").unwrap_err().code(),
        uix_app::core::Errc::InvalidOperation
    );
}

#[test]
fn oversized_bracketed_paste_is_rejected_before_sending_any_opening_marker() {
    let probe = TerminalProbe::new(
        "stty raw -echo; printf '\\033[?2004hREADY'; value=$(head -c 2); if [ \"$value\" != OK ]; then exit 43; fi; printf '\\r\\nCOMPLETE'",
        80,
        3,
    );
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    let text = "A".repeat(1048576 - 11);
    assert_eq!(
        probe.session.paste(&text).unwrap_err().code(),
        uix_app::core::Errc::InsufficientResources
    );
    probe.session.write(b"OK").unwrap();
    probe.finished();
    assert_eq!(probe.lines()[1], "COMPLETE");
}

#[cfg(feature = "test-harness")]
#[test]
fn focus_changes_expire_pending_alt_and_suppressed_text_state() {
    use uix_app::ui::test_harness::TestApp;
    let probe = TerminalProbe::new(
        "printf 'READY'; IFS= read -r value; printf '\\r\\n'; printf '%s' \"$value\" | od -An -tx1 | tr -d ' \\n'",
        80,
        3,
    );
    let session = probe.session.clone();
    let mut app = TestApp::new((640.0, 200.0), move || {
        row((
            embed(TerminalScreen::new(&session))
                .width(300.0)
                .automation_id("one"),
            embed(TerminalScreen::new(&session))
                .width(300.0)
                .automation_id("two"),
        ))
    });
    probe.wait(|session| session.rows()[0].plain().starts_with("READY"));
    for (mods, text) in [(KeyMod::ALT, "x"), (KeyMod::SUPER, "y")] {
        app.focus("one").unwrap();
        app.dispatch_system_event(&SystemEvent::KeyDown {
            key: KeyCode::X,
            mods,
        })
        .unwrap();
        app.focus("two").unwrap();
        app.focus("one").unwrap();
        app.dispatch_system_event(&SystemEvent::TextInput { text: text.into() })
            .unwrap();
    }
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    probe.finished();
    assert_eq!(probe.lines()[1], "7879");
}
