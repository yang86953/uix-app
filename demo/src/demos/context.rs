//! Dashboard 共享上下文 — 跨页 State 与可执行验收控制。

use std::sync::{Arc, Mutex};
use uix::prelude::*;

#[derive(Clone)]
pub struct ThemeControl {
    handle: Arc<Mutex<Option<AppHandle>>>,
    dark: State<bool>,
    follow_system_theme: bool,
}

impl Default for ThemeControl {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ThemeControl {
    pub fn new(follow_system_theme: bool) -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
            dark: State::new(false),
            follow_system_theme,
        }
    }

    pub fn set_handle(&self, handle: AppHandle) {
        *self.handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    pub fn handle(&self) -> Option<AppHandle> {
        self.handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn is_dark(&self) -> bool {
        self.dark.get()
    }

    pub fn follows_system_theme(&self) -> bool {
        self.follow_system_theme
    }

    pub fn toggle(&self) -> bool {
        if self.follow_system_theme {
            return false;
        }
        let next = !self.is_dark();
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(handle) = handle else {
            return false;
        };
        if handle
            .set_theme(if next {
                Theme::antd_dark()
            } else {
                Theme::antd_light()
            })
            .is_err()
        {
            return false;
        }
        self.dark.set(next);
        true
    }
}

pub const GRAPHICS_RECOVERY_READY: &str = "图形恢复验收：等待注入";
pub const GRAPHICS_RECOVERY_PENDING: &str = "图形恢复验收：已注入，等待恢复后交互";
pub const GRAPHICS_RECOVERY_VERIFIED: &str = "图形恢复验收：恢复后交互成功";

#[derive(Clone)]
pub struct GraphicsRecoveryControl {
    handle: Arc<Mutex<Option<AppHandle>>>,
    status: State<String>,
    enabled: bool,
}

impl GraphicsRecoveryControl {
    pub fn new(enabled: bool) -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
            status: State::new(GRAPHICS_RECOVERY_READY.to_string()),
            enabled,
        }
    }

    pub fn set_handle(&self, handle: AppHandle) {
        *self.handle.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn status(&self) -> State<String> {
        self.status.clone()
    }

    pub fn can_inject(&self) -> bool {
        self.status.get() == GRAPHICS_RECOVERY_READY
    }

    pub fn can_verify(&self) -> bool {
        self.status.get() == GRAPHICS_RECOVERY_PENDING
    }

    pub fn inject_device_lost(&self) {
        let Some(handle) = self
            .handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        else {
            self.status
                .set("图形恢复验收：注入失败（AppHandle 尚未就绪）".to_string());
            return;
        };

        #[cfg(feature = "test-harness")]
        match handle.inject_graphics_device_lost_for_test() {
            Ok(()) => self.status.set(GRAPHICS_RECOVERY_PENDING.to_string()),
            Err(error) => self
                .status
                .set(format!("图形恢复验收：注入失败（{}）", error.short_what())),
        }

        #[cfg(not(feature = "test-harness"))]
        {
            let _ = handle;
            self.status
                .set("图形恢复验收：注入失败（未启用 test-harness）".to_string());
        }
    }

    pub fn verify_recovered_interaction(&self) {
        if self.can_verify() {
            self.status.set(GRAPHICS_RECOVERY_VERIFIED.to_string());
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameworkLocale {
    #[default]
    ZhCn,
    EnUs,
}

#[derive(Clone)]
pub struct FrameworkControl {
    locale: State<FrameworkLocale>,
}

impl Default for FrameworkControl {
    fn default() -> Self {
        Self {
            locale: State::new(FrameworkLocale::default()),
        }
    }
}

impl FrameworkControl {
    pub fn locale_state(&self) -> State<FrameworkLocale> {
        self.locale.clone()
    }

    pub fn locale(&self) -> Locale {
        match self.locale.get() {
            FrameworkLocale::ZhCn => zh_cn(),
            FrameworkLocale::EnUs => en_us(),
        }
    }
}

/// 传入各分类页的共享运行时 State。
pub struct DemoCtx<'a> {
    pub tk: &'a DesignTokens,
    pub timer_ticks: &'a State<u32>,
    /// 当前页索引；覆盖清单等页用于跳转导航。
    pub active_page: Option<&'a State<usize>>,
    home_count: Option<&'a State<i32>>,
    runtime_count: Option<&'a State<i32>>,
    theme_control: Option<&'a ThemeControl>,
    graphics_recovery_control: Option<&'a GraphicsRecoveryControl>,
    framework_control: Option<&'a FrameworkControl>,
}

impl<'a> DemoCtx<'a> {
    pub fn new(
        tk: &'a DesignTokens,
        timer_ticks: &'a State<u32>,
        active_page: Option<&'a State<usize>>,
    ) -> Self {
        Self {
            tk,
            timer_ticks,
            active_page,
            home_count: None,
            runtime_count: None,
            theme_control: None,
            graphics_recovery_control: None,
            framework_control: None,
        }
    }

    pub fn with_counters(
        mut self,
        home_count: &'a State<i32>,
        runtime_count: &'a State<i32>,
    ) -> Self {
        self.home_count = Some(home_count);
        self.runtime_count = Some(runtime_count);
        self
    }

    pub fn with_theme_control(mut self, theme_control: &'a ThemeControl) -> Self {
        self.theme_control = Some(theme_control);
        self
    }

    pub fn with_graphics_recovery_control(
        mut self,
        graphics_recovery_control: &'a GraphicsRecoveryControl,
    ) -> Self {
        self.graphics_recovery_control = Some(graphics_recovery_control);
        self
    }

    pub fn with_framework_control(mut self, framework_control: &'a FrameworkControl) -> Self {
        self.framework_control = Some(framework_control);
        self
    }

    pub fn home_count(&self) -> State<i32> {
        self.home_count.cloned().unwrap_or_else(|| State::new(0))
    }

    pub fn runtime_count(&self) -> State<i32> {
        self.runtime_count.cloned().unwrap_or_else(|| State::new(0))
    }

    pub fn theme_control(&self) -> Option<&ThemeControl> {
        self.theme_control
    }

    pub fn graphics_recovery_control(&self) -> Option<&GraphicsRecoveryControl> {
        self.graphics_recovery_control
    }

    pub fn framework_control(&self) -> FrameworkControl {
        self.framework_control.cloned().unwrap_or_default()
    }
}
