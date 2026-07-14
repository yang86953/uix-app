use crate::native::backends::windows::platform::WindowsPlatform;
use crate::native::shared::OsEventSource;
use crate::native::traits::event::{UiEventPayload, UiEventType};
use crate::native::traits::{IWindowManager, Platform};
use crate::tests::common::*;
use windows::core::GUID;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ActivateKeyboardLayout, GetFocus, GetKeyboardLayout, LoadKeyboardLayoutW, SendInput,
    SetActiveWindow, SetFocus, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, HKL, INPUT, INPUT_0, INPUT_KEYBOARD,
    KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KLF_ACTIVATE, KLF_SETFORPROCESS, VIRTUAL_KEY,
    VK_SPACE,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, ITfInputProcessorProfileMgr, GUID_TFCAT_TIP_KEYBOARD,
    TF_INPUTPROCESSORPROFILE, TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE, TF_IPPMF_FORPROCESS,
    TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
};

const ZH_CN_LANG_ID: u16 = 0x0804;
const MICROSOFT_PINYIN_CLSID: GUID = GUID::from_u128(0xe7ea138f_69f8_11d7_a6ea_00065b844310);
const MICROSOFT_PINYIN_PROFILE: GUID = GUID::from_u128(0xe7ea1390_69f8_11d7_a6ea_00065b844311);

struct ProcessPinyinProfile {
    manager: ITfInputProcessorProfileMgr,
    previous_layout: windows::Win32::UI::Input::KeyboardAndMouse::HKL,
}

impl ProcessPinyinProfile {
    fn activate() -> windows::core::Result<Self> {
        // SAFETY: TSF 会话已在当前 STA 线程初始化 COM；该系统类在进程内创建。
        let manager: ITfInputProcessorProfileMgr = unsafe {
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?
        };
        let previous_layout = unsafe { GetKeyboardLayout(0) };
        let flags = ACTIVATE_KEYBOARD_LAYOUT_FLAGS(KLF_ACTIVATE.0 | KLF_SETFORPROCESS.0);
        // SAFETY: 静态 UTF-16 字符串以 NUL 结尾，布局切换限制在测试进程。
        unsafe { LoadKeyboardLayoutW(windows::core::w!("00000804"), flags)? };
        let profile_flags = TF_IPPMF_FORPROCESS | TF_IPPMF_DONTCARECURRENTINPUTLANGUAGE;
        // SAFETY: GUID 指针在同步调用期间有效；输入处理器 profile 要求空 HKL。
        if let Err(error) = unsafe {
            manager.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                ZH_CN_LANG_ID,
                &MICROSOFT_PINYIN_CLSID,
                &MICROSOFT_PINYIN_PROFILE,
                HKL(std::ptr::null_mut()),
                profile_flags,
            )
        } {
            let _ = unsafe { ActivateKeyboardLayout(previous_layout, KLF_SETFORPROCESS) };
            return Err(error);
        }

        let profile = Self {
            manager,
            previous_layout,
        };
        let mut active = TF_INPUTPROCESSORPROFILE::default();
        // SAFETY: active 是有效输出缓冲区，category GUID 为静态值。
        unsafe {
            profile
                .manager
                .GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &mut active)?
        };
        if active.clsid != MICROSOFT_PINYIN_CLSID || active.guidProfile != MICROSOFT_PINYIN_PROFILE
        {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x8000_4005_u32 as i32),
                "Microsoft Pinyin profile did not become active",
            ));
        }

        Ok(profile)
    }
}

impl Drop for ProcessPinyinProfile {
    fn drop(&mut self) {
        // SAFETY: 仅撤销本测试进程激活的 profile，并恢复进入测试前的布局。
        let _ = unsafe {
            self.manager.DeactivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                ZH_CN_LANG_ID,
                &MICROSOFT_PINYIN_CLSID,
                &MICROSOFT_PINYIN_PROFILE,
                HKL(std::ptr::null_mut()),
                TF_IPPMF_FORPROCESS,
            )
        };
        let _ = unsafe { ActivateKeyboardLayout(self.previous_layout, KLF_SETFORPROCESS) };
    }
}

fn keyboard_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_keys(keys: &[VIRTUAL_KEY]) {
    let mut inputs = Vec::with_capacity(keys.len() * 2);
    for key in keys {
        inputs.push(keyboard_input(*key, KEYBD_EVENT_FLAGS(0)));
        inputs.push(keyboard_input(*key, KEYEVENTF_KEYUP));
    }
    // SAFETY: INPUT 数组在同步调用期间有效，结构尺寸与 Win32 ABI 一致。
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    assert_eq!(
        sent as usize,
        inputs.len(),
        "SendInput must enqueue every key"
    );
}

fn request_foreground_focus(hwnd: HWND) {
    // Windows 只允许满足前台策略的线程抢占焦点；临时合并输入队列后立即解除。
    let foreground = unsafe { GetForegroundWindow() };
    let current_thread = unsafe { GetCurrentThreadId() };
    let foreground_thread = if foreground.0.is_null() {
        0
    } else {
        unsafe { GetWindowThreadProcessId(foreground, None) }
    };
    let attached = foreground_thread != 0
        && foreground_thread != current_thread
        && unsafe { AttachThreadInput(current_thread, foreground_thread, true) }.as_bool();

    // SAFETY: hwnd 是当前线程创建的活动窗口；返回值由后续状态轮询统一验证。
    unsafe {
        let _ = BringWindowToTop(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        if attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
    }
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
        )
    })
}

#[test]
#[ignore = "requires an interactive Windows desktop with Microsoft Pinyin installed"]
fn microsoft_pinyin_foreground_composition_commits_once() {
    static FOREGROUND_IME_LOCK: Mutex<()> = Mutex::new(());
    let _serial = FOREGROUND_IME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut platform = WindowsPlatform::new();
    let mut window = platform
        .create_window("UIX Microsoft Pinyin foreground proof", 420, 160)
        .expect("create foreground IME window");
    let window_id = window.window_id();
    let hwnd = HWND(window.native_handle().native_window());

    window.show().expect("show foreground IME window");
    window.raise().expect("raise foreground IME window");
    platform
        .text_input()
        .set_target_window(window_id, hwnd.0)
        .expect("select foreground text input target");
    platform.text_input().start().expect("start TSF input");
    platform
        .text_input()
        .set_cursor_rect(Rect::new(24.0, 48.0, 2.0, 24.0))
        .expect("set TSF caret rect");

    let profile = ProcessPinyinProfile::activate().expect("activate Microsoft Pinyin profile");
    request_foreground_focus(hwnd);

    let focus_deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < focus_deadline {
        assert!(platform.dispatch_timeout(Duration::from_millis(20)));
        if unsafe { GetForegroundWindow() } == hwnd && unsafe { GetFocus() } == hwnd {
            break;
        }
    }
    assert_eq!(unsafe { GetForegroundWindow() }, hwnd);
    assert_eq!(unsafe { GetFocus() }, hwnd);
    while platform.next_event().is_some() {}

    let mut keys = "zhong"
        .bytes()
        .map(|byte| VIRTUAL_KEY(u16::from(byte.to_ascii_uppercase())))
        .collect::<Vec<_>>();
    keys.push(VK_SPACE);
    send_keys(&keys);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut types = Vec::new();
    let mut updates = Vec::new();
    let mut commits = Vec::new();
    while Instant::now() < deadline && commits.is_empty() {
        assert!(platform.dispatch_timeout(Duration::from_millis(25)));
        while let Some(event) = platform.next_event() {
            assert_eq!(event.window_id, Some(window_id));
            types.push(event.type_);
            match event.payload {
                UiEventPayload::ImeComposition(update) => updates.push(update.text),
                UiEventPayload::TextInput(commit) => commits.push(commit.text),
                _ => {}
            }
        }
    }

    platform.text_input().stop().expect("stop TSF input");
    drop(profile);
    window.close().expect("close foreground IME window");
    assert!(platform.dispatch_pending());

    assert!(types.contains(&UiEventType::ImeCompositionStart));
    assert!(types.contains(&UiEventType::ImeCompositionUpdate));
    assert!(types.contains(&UiEventType::ImeCompositionEnd));
    assert!(!updates.is_empty());
    assert_eq!(
        commits.len(),
        1,
        "IME must commit exactly once: {commits:?}"
    );
    assert!(!commits[0].is_empty());
    assert!(
        contains_cjk(&commits[0]),
        "expected CJK commit: {commits:?}"
    );
    assert_ne!(commits[0].to_ascii_lowercase(), "zhong");
}
