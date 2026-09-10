#![cfg(all(feature = "test-harness", feature = "terminal"))]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use uix_app::prelude::*;
use uix_app::ui::test_harness::TestApp;

#[test]
fn terminal_declared_size_updates_without_losing_the_command_draft() {
    let size = State::new((240.0_f32, 120.0_f32));
    let root_size = size.clone();
    let submitted = Rc::new(RefCell::new(String::new()));
    let observed = submitted.clone();
    let mut app = TestApp::new((640.0, 480.0), move || {
        let (width, height) = root_size.get();
        let observed = observed.clone();
        column((embed(Terminal::new().on_command(move |text| {
            *observed.borrow_mut() = text.to_owned();
        }))
        .width(width)
        .height(height)
        .flex_shrink(0.0)
        .automation_id("console"),))
        .width(600.0)
        .align(AlignItems::Start)
    });
    let frame = app.snapshot().find("console").unwrap().frame;
    assert_eq!((frame.w, frame.h), (240.0, 120.0));
    app.focus("console").unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput {
        text: "草稿".to_owned(),
    })
    .unwrap();
    size.set((300.0, 160.0));
    app.settle().unwrap();
    let frame = app.snapshot().find("console").unwrap().frame;
    assert_eq!((frame.w, frame.h), (300.0, 160.0));
    app.press_key(KeyCode::Enter, KeyMod::NONE).unwrap();
    assert_eq!(&*submitted.borrow(), "草稿");
}

#[test]
fn terminal_participates_in_flex_layout() {
    let app = TestApp::new((600.0, 300.0), || {
        row((
            embed(Terminal::new())
                .width(100.0)
                .height(100.0)
                .flex_grow(1.0)
                .automation_id("grow"),
            embed(Terminal::new())
                .width(100.0)
                .height(100.0)
                .flex_grow(0.0)
                .flex_shrink(0.0)
                .automation_id("fixed"),
        ))
        .width(600.0)
    });
    assert_eq!(app.snapshot().find("grow").unwrap().frame.w, 500.0);
    assert_eq!(app.snapshot().find("fixed").unwrap().frame.w, 100.0);
}

// ── 真实终端会话（Linux PTY 最小闭环）──

// 真会话测试共享串行锁：/proc/self/fd 计数与其他按字节断言不能被并行
// 会话的 fd 与回显输出污染。
#[cfg(target_os = "linux")]
static PTY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// 在超时内反复 pump 直到谓词成立；返回是否命中。
#[cfg(target_os = "linux")]
fn pump_until(
    session: &TerminalSession,
    timeout: Duration,
    predicate: impl Fn(&TerminalSession) -> bool,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        session.pump();
        if predicate(session) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

// 屏幕全部行拼接的纯文本。
#[cfg(target_os = "linux")]
fn screen_text(session: &TerminalSession) -> String {
    session
        .rows()
        .iter()
        .map(TerminalRow::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(target_os = "linux")]
fn spawn_command(command: &[&str]) -> TerminalSession {
    TerminalSession::spawn(&TerminalSessionConfig {
        command: command.iter().map(|text| text.to_string()).collect(),
        ..TerminalSessionConfig::default()
    })
    .expect("terminal session should spawn on Linux")
}

#[cfg(target_os = "linux")]
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|entries| entries.count())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_session_streams_bidirectionally_and_reports_exit() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = spawn_command(&["cat"]);
    // 回显验证：输入经 PTY 到内核行规程，echo 立即回到屏幕状态。
    session.write(b"hello ").unwrap();
    session.write("终端".as_bytes()).unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains("hello 终端")
        }),
        "echo output should reach the screen, got: {}",
        screen_text(&session)
    );

    // 回车提交当前行（cat 读到整行后回写），行首 Ctrl+D 才是 EOF。
    session.write(b"\r").unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).matches("hello 终端").count() >= 2
        }),
        "cat should echo the committed line again, screen: {}",
        screen_text(&session)
    );
    session.write(b"\x04").unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            session.status() == TerminalSessionStatus::Exited { code: Some(0) }
        }),
        "cat should exit with code 0 after EOF, status: {:?}",
        session.status()
    );
    // 退出后的写入返回类型化失败而不是 panic。
    assert!(session.write(b"x").is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_session_parses_ansi_color_utf8_and_cursor_control() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let script = "printf 'red\\033[31mA\\033[0m|\\033[1;32mG\\033[0m|plain\\nsecond\\r\\033[2GX\\033[D\\033[Dy'; printf '\\033[4;10Hmark\\033[32mG\\033[0m'";
    let session = spawn_command(&["sh", "-c", script]);
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            session.status() != TerminalSessionStatus::Running
                && screen_text(session).contains("mark")
        }),
        "script should finish and position text, screen: {}",
        screen_text(&session)
    );

    let rows = session.rows();
    // 第一行：红色 A、粗体绿色 G 与默认色分段（行文本是完整网格行）。
    let first = &rows[0];
    assert_eq!(first.plain().trim_end(), "redA|G|plain");
    let red_span = first
        .spans
        .iter()
        .find(|span| span.text.contains('A'))
        .expect("red span");
    assert_eq!(red_span.style.fg, TerminalColorSpec::Palette(1));
    let green_span = first
        .spans
        .iter()
        .find(|span| span.text.contains('G'))
        .expect("green span");
    assert_eq!(green_span.style.fg, TerminalColorSpec::Palette(2));
    assert!(green_span.style.bold);
    // 第二行：\r 后重写列 2 为 X，两次 CUB 后 y 覆盖行首。
    assert_eq!(rows[1].plain().trim_end(), "yXcond");
    // 第四行第 10 列：mark + 绿色 G。
    assert_eq!(rows[3].plain().trim_end(), "         markG");
    let g_span = rows[3]
        .spans
        .iter()
        .find(|span| span.text == "G")
        .expect("positioned G span");
    assert_eq!(g_span.style.fg, TerminalColorSpec::Palette(2));
    // 最终光标停在 markG 之后。
    let (cursor_row, cursor_col) = session.cursor();
    assert_eq!((cursor_row, cursor_col), (3, 14));
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_session_reports_initial_window_size_to_child() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = TerminalSession::spawn(&TerminalSessionConfig {
        command: vec!["sh".to_string(), "-c".to_string(), "stty size".to_string()],
        cols: 57,
        rows: 19,
        ..TerminalSessionConfig::default()
    })
    .expect("stty session should spawn");
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains("19 57")
        }),
        "child should observe the configured window size, screen: {}",
        screen_text(&session)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_session_resize_propagates_to_running_child() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = spawn_command(&["sh", "-c", "sleep 0.3 && stty size"]);
    session.resize(60, 20).expect("resize should apply");
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains("20 60")
        }),
        "child should observe the resized window, screen: {}",
        screen_text(&session)
    );
    assert_eq!(session.screen_size(), (60, 20));
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_session_drop_releases_file_descriptors() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = open_fd_count();
    {
        let session = spawn_command(&["sleep", "30"]);
        session.write(b"ignored").unwrap();
        // 控制句柄与全部克隆 drop 后完成关闭、SIGHUP 与子进程回收。
        let clone = session.clone();
        drop(session);
        drop(clone);
    }
    // 读线程异步收尾：短暂等待后 fd 数量应回到基线。
    let start = Instant::now();
    while open_fd_count() > before && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        open_fd_count(),
        before,
        "session drop must close pty and pipe descriptors"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_session_drop_releases_a_saturated_input_queue_and_all_pty_descriptors() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = open_fd_count();
    let child_pid;
    let closing;
    {
        let session = spawn_command(&[
            "/bin/sh",
            "-c",
            "stty raw -echo; printf 'READY %s' \"$$\"; exec sleep 30",
        ]);
        assert!(pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains("READY")
        }));
        child_pid = session.rows()[0]
            .plain()
            .trim()
            .strip_prefix("READY ")
            .unwrap()
            .parse::<u32>()
            .unwrap();
        session.write(&vec![b'A'; 1024 * 1024]).unwrap();
        let mut full = false;
        for _ in 0..64 {
            match session.write(&[b'B'; 65536]) {
                Ok(()) => {}
                Err(error) if error.code() == uix_app::core::Errc::WouldBlock => {
                    full = true;
                    break;
                }
                Err(error) => panic!("unexpected input error: {error:?}"),
            }
        }
        assert!(full, "child is not reading; input must be bounded");
        let clone = session.clone();
        closing = Instant::now();
        drop(session);
        assert!(std::path::Path::new(&format!("/proc/{child_pid}")).exists());
        drop(clone);
    }
    // Close cancels queued input rather than waiting for a stalled child to consume it.
    assert!(closing.elapsed() < Duration::from_secs(5));
    assert!(
        !std::path::Path::new(&format!("/proc/{child_pid}")).exists(),
        "last handle drop must reap the child, not just close its PTY descriptors"
    );
    assert_eq!(open_fd_count(), before);
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_screen_forwards_keys_and_ime_text_through_the_session() {
    let _guard = PTY_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = spawn_command(&["cat"]);
    let builder_session = session.clone();
    let mut app = TestApp::new((640.0, 400.0), move || {
        embed(TerminalScreen::new(&builder_session)).automation_id("live")
    });
    app.focus("live").unwrap();

    // 中文经 TextInput 进入 PTY；行规程 echo 立即回显（宽字符占两列）。
    app.dispatch_system_event(&SystemEvent::TextInput {
        text: "你好".to_owned(),
    })
    .unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains("你好")
        }),
        "ime text should echo through the session, screen: {}",
        screen_text(&session)
    );

    // 方向键编码为 \x1b[D 写入 PTY：canonical echo 以 caret 形式回显字面序列，
    // 屏幕出现 "^[[D" 即证明字节确实到达 PTY（VT 解释由会话测试覆盖）。
    app.press_key(KeyCode::Left, KeyMod::NONE).unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains("^[[D")
        }),
        "arrow key bytes should reach the pty, screen: {}",
        screen_text(&session)
    );

    // Tab 是子进程输入而不是焦点导航：字节到达后行规程把光标推进到
    // 下一个制表位（caret 输出停在列 8；内核列计数与 VT 宽字符列存在
    // 固有偏差，断言只要求光标越过 caret 位置）。
    app.press_key(KeyCode::Tab, KeyMod::NONE).unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            session.cursor().1 > 8
        }),
        "tab bytes should reach the pty instead of moving focus, cursor: {:?} screen: {}",
        session.cursor(),
        screen_text(&session)
    );

    // Ctrl+M 即回车提交整行；按住区间内平台跟随的 TextInput("a") 必须被抑制。
    app.dispatch_system_event(&SystemEvent::KeyDown {
        key: KeyCode::M,
        mods: KeyMod::CTRL,
    })
    .unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput {
        text: "a".to_owned(),
    })
    .unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            !screen_text(session).contains('a')
        }),
        "ctrl combination text must be suppressed, screen: {}",
        screen_text(&session)
    );

    // 控制组合的文本抑制只覆盖按住区间：KeyUp 后的 TextInput 不再被
    // 误杀（Agent 注入与无伴随文本的平台都依赖这一时序）。
    app.dispatch_system_event(&SystemEvent::KeyUp {
        key: KeyCode::M,
        mods: KeyMod::CTRL,
    })
    .unwrap();
    app.dispatch_system_event(&SystemEvent::TextInput {
        text: "z".to_owned(),
    })
    .unwrap();
    assert!(
        pump_until(&session, Duration::from_secs(5), |session| {
            screen_text(session).contains('z')
        }),
        "text after key-up must reach the pty, screen: {}",
        screen_text(&session)
    );

    // 组件快照把屏幕文本暴露给自动化通道。
    let visible = app.text("live").unwrap();
    assert!(visible.contains("你好"), "snapshot text: {visible}");
}
