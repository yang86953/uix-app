//! Theme wrapper and TokenProvider supertrait — aggregates all token sub-traits.
//!
//! `TokenProvider` combines `IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, and `IBoxShadowTokens` into a single supertrait
//! with the `is_dark()` mode query. `Theme` wraps an `Arc<dyn TokenProvider>`
//! for runtime-polymorphic token injection.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use uix_graphics::Color;
use super::color_tokens::{IColorTokens, ShadowToken};
use super::design_tokens::DesignTokens;
use super::spacing_tokens::{IBoxShadowTokens, ISpacingTokens};
use super::typography_tokens::ITypographyTokens;
use crate::style::Style;
use uix_platform::EdgeInsets;

// ════════════════════════════════════════════════════════════════════════════
// TokenProvider — supertrait 聚合全部子 trait
// ════════════════════════════════════════════════════════════════════════════

/// Abstract design token provider — full Ant Design 5 token surface.
///
/// Aggregates domain-specific sub-traits via supertrait bounds.
/// Implement this trait (or implement all sub-traits separately) to inject
/// a custom design system. All color and shadow methods require explicit
/// implementation; typography, spacing, motion, and breakpoint methods
/// provide Ant Design 5 light mode defaults.
pub trait TokenProvider:
    IColorTokens + ITypographyTokens + ISpacingTokens + IBoxShadowTokens + Send + Sync
{
    /// Whether the active theme is dark mode.
    fn is_dark(&self) -> bool {
        false
    }

    // ── 默认样式构造器（从主题 token 推导 widget 默认外观）──

    /// 获取容器默认样式。
    fn style_container(&self) -> Style {
        Style {
            background: None,
            border_color: None,
            border_width: 0.0,
            border_radius: self.border_radius(),
            color: self.color_text(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }

    /// 获取标签默认样式。
    fn style_label(&self) -> Style {
        Style {
            color: self.color_text(),
            font_size: self.font_size(),
            padding: EdgeInsets::new(2.0, 0.0, 0.0, 0.0),
            ..Style::default()
        }
    }

    /// 获取默认按钮样式（Default 变体）。
    fn style_button_default(&self) -> Style {
        Style {
            background: None,
            border_color: Some(self.color_border()),
            border_width: 1.0,
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: self.color_text(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }

    /// 获取主按钮样式（Primary 变体）。
    fn style_button_primary(&self) -> Style {
        Style {
            background: Some(self.color_primary()),
            border_color: Some(self.color_primary()),
            border_width: 1.0,
            border_radius: self.border_radius(),
            padding: EdgeInsets::new(15.0, 0.0, 15.0, 0.0),
            color: self.color_white(),
            font_size: self.font_size(),
            ..Style::default()
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Theme
// ════════════════════════════════════════════════════════════════════════════

/// Theme wraps a token provider with light/dark mode.
/// Custom themes are injected by providing a `TokenProvider` implementation.
#[derive(Clone)]
pub struct Theme {
    provider: Arc<dyn TokenProvider>,
}

impl Theme {
    /// 通过已有 TokenProvider 创建 Theme（值语义，内部包装为 Arc）。
    pub fn new(provider: impl TokenProvider + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }

    /// 通过已有的 Arc<dyn TokenProvider> 创建 Theme。
    /// 与 `new()` 的区别在于不会重新包装 Arc，适用于需要共享同一 provider 的场景。
    pub fn from_arc(provider: Arc<dyn TokenProvider>) -> Self {
        Self { provider }
    }

    pub fn antd_light() -> Self {
        Self::new(super::design_tokens::DesignTokens::antd_light())
    }

    pub fn antd_dark() -> Self {
        Self::new(super::design_tokens::DesignTokens::antd_dark())
    }

    pub fn tokens(&self) -> &dyn TokenProvider {
        &*self.provider
    }

    pub fn is_dark(&self) -> bool {
        self.provider.is_dark()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::antd_light()
    }
}

impl std::fmt::Debug for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Theme")
            .field("is_dark", &self.is_dark())
            .finish()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// DynTokens — 运行时动态切换的 DesignTokens 包装器
// ════════════════════════════════════════════════════════════════════════════

/// 运行时可切换的 DesignTokens 包装器。
///
/// 内部持有 `RwLock<DesignTokens>`，可在运行时通过 `set_light()` / `set_dark()`
/// 切换暗/亮模式。实现了所有 token trait，每次方法调用时从 RwLock 读取最新值。
///
/// 配合 `Theme::new()` 使用，可实现运行时主题切换而无需重建 widget 树：
/// 将 `Arc<DynTokens>` 传入 `Theme::new()`，然后通过 `dyn_tokens.set_mode()` 切换，
/// 渲染循环每帧从 `ctx.tokens()` 获取的颜色会自动更新。
///
/// # 线程安全
///
/// `DynTokens` 是 `Send + Sync`，内部使用 `RwLock` 保证读-写安全，读操作可并发。
#[derive(Debug)]
pub struct DynTokens {
    inner: RwLock<DesignTokens>,
    /// 是否开启了系统主题跟随
    auto_follow: AtomicBool,
}

impl DynTokens {
    /// 创建新的 DynTokens，初始使用指定的 DesignTokens。
    pub fn new(tokens: DesignTokens) -> Self {
        Self { inner: RwLock::new(tokens), auto_follow: AtomicBool::new(false) }
    }

    /// 切换到亮色模式。
    pub fn set_light(&self) {
        self.auto_follow.store(false, Ordering::Relaxed);
        let new = DesignTokens::antd_light();
        *self.inner.write().unwrap_or_else(|e| e.into_inner()) = new;
    }

    /// 切换到暗色模式。
    pub fn set_dark(&self) {
        self.auto_follow.store(false, Ordering::Relaxed);
        let new = DesignTokens::antd_dark();
        *self.inner.write().unwrap_or_else(|e| e.into_inner()) = new;
    }

    /// 根据 `dark` 参数切换模式（true=暗色，false=亮色）。
    pub fn set_mode(&self, dark: bool) {
        self.auto_follow.store(false, Ordering::Relaxed);
        let new = if dark { DesignTokens::antd_dark() } else { DesignTokens::antd_light() };
        *self.inner.write().unwrap_or_else(|e| e.into_inner()) = new;
    }

    /// 获取当前是否为暗色模式。
    pub fn is_dark(&self) -> bool {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).is_dark
    }

    /// 获取当前 DesignTokens 的快照（clone 副本）。
    /// 用于在不持有锁的情况下读取所有 token 值。
    pub fn snapshot(&self) -> DesignTokens {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 设置为任意自定义 DesignTokens。
    /// 用于运行时切换非标准主题（如樱花、极光等自定义预设）。
    pub fn set_custom(&self, tokens: DesignTokens) {
        self.auto_follow.store(false, Ordering::Relaxed);
        *self.inner.write().unwrap_or_else(|e| e.into_inner()) = tokens;
    }

    /// 是否开启了系统主题自动跟随
    pub fn is_auto_following(&self) -> bool {
        self.auto_follow.load(Ordering::Relaxed)
    }

    /// 启动系统主题自动跟随。
    ///
    /// 在后台线程中每隔 `poll_interval_secs` 秒调用 `is_dark_fn` 检测系统主题，
    /// 当系统主题切换时自动切换暗/亮模式。
    ///
    /// # 参数
    /// - `is_dark_fn`: 返回 `true` 表示系统当前为暗色模式的检测函数
    /// - `poll_interval_secs`: 轮询间隔（秒），建议 2~5 秒
    ///
    /// # 线程安全
    ///
    /// 此方法会 spawn 一个后台线程，该线程通过 `RwLock` 安全地切换主题。
    /// 调用 `set_light()` / `set_dark()` / `set_mode()` / `set_custom()` 会
    /// 自动关闭自动跟随。
    ///
    /// # 示例
    ///
    /// ```ignore
    /// let platform = create_platform().unwrap();
    /// let dt = Arc::new(DynTokens::new(DesignTokens::antd_dark()));
    /// dt.follow_system_theme(
    ///     Box::new(move || platform.display().is_dark_mode()),
    ///     3,
    /// );
    /// ```
    pub fn follow_system_theme(
        self: &Arc<Self>,
        is_dark_fn: Box<dyn Fn() -> bool + Send>,
        poll_interval_secs: u64,
    ) {
        self.auto_follow.store(true, Ordering::Relaxed);
        let this = Arc::clone(self);
        std::thread::spawn(move || {
            let interval = std::time::Duration::from_secs(poll_interval_secs.max(1));
            let mut last_known_dark = is_dark_fn();

            // 初始同步：设置与系统一致
            let current_dark = this.inner.read()
                .unwrap_or_else(|e| e.into_inner()).is_dark;
            if current_dark != last_known_dark {
                this.set_mode(last_known_dark);
            }

            loop {
                std::thread::sleep(interval);

                if !this.auto_follow.load(Ordering::Relaxed) {
                    break; // 手动切换主题时退出跟随
                }

                let current = is_dark_fn();
                if current != last_known_dark {
                    last_known_dark = current;
                    let new = if current {
                        DesignTokens::antd_dark()
                    } else {
                        DesignTokens::antd_light()
                    };
                    match this.inner.write() {
                        Ok(mut guard) => *guard = new,
                        Err(e) => *e.into_inner() = new,
                    }
                }
            }
        });
    }
}

// ── 辅助宏：从 RwLock 中读取 Copy 类型的字段 ──
// 使用 unwrap_or_else 处理 RwLock 被 poisoned 的情况，
// 从 PoisonError 中恢复内部值。
macro_rules! read_copy {
    ($self:expr, $field:ident) => {
        $self.inner.read().unwrap_or_else(|e| e.into_inner()).$field
    };
}

impl IColorTokens for DynTokens {
    fn color_primary(&self) -> Color { read_copy!(self, color_primary) }
    fn color_primary_hover(&self) -> Color { read_copy!(self, color_primary_hover) }
    fn color_primary_active(&self) -> Color { read_copy!(self, color_primary_active) }
    fn color_primary_bg(&self) -> Color { read_copy!(self, color_primary_bg) }
    fn color_primary_border(&self) -> Color { read_copy!(self, color_primary_border) }
    fn color_bg_container(&self) -> Color { read_copy!(self, color_bg_container) }
    fn color_bg_elevated(&self) -> Color { read_copy!(self, color_bg_elevated) }
    fn color_bg_raised(&self) -> Color { read_copy!(self, color_bg_raised) }
    fn color_bg_overlay(&self) -> Color { read_copy!(self, color_bg_overlay) }
    fn color_bg_layout(&self) -> Color { read_copy!(self, color_bg_layout) }
    fn color_bg_spotlight(&self) -> Color { read_copy!(self, color_bg_spotlight) }
    fn color_bg_mask(&self) -> Color { read_copy!(self, color_bg_mask) }
    fn color_border(&self) -> Color { read_copy!(self, color_border) }
    fn color_border_secondary(&self) -> Color { read_copy!(self, color_border_secondary) }
    fn color_fill(&self) -> Color { read_copy!(self, color_fill) }
    fn color_fill_secondary(&self) -> Color { read_copy!(self, color_fill_secondary) }
    fn color_fill_tertiary(&self) -> Color { read_copy!(self, color_fill_tertiary) }
    fn color_fill_quaternary(&self) -> Color { read_copy!(self, color_fill_quaternary) }
    fn color_text(&self) -> Color { read_copy!(self, color_text) }
    fn color_text_secondary(&self) -> Color { read_copy!(self, color_text_secondary) }
    fn color_text_tertiary(&self) -> Color { read_copy!(self, color_text_tertiary) }
    fn color_text_quaternary(&self) -> Color { read_copy!(self, color_text_quaternary) }
    fn color_white(&self) -> Color { read_copy!(self, color_white) }
    fn color_black(&self) -> Color { read_copy!(self, color_black) }
    fn color_shadow(&self) -> Color { read_copy!(self, color_shadow) }
    fn color_shadow_secondary(&self) -> Color { read_copy!(self, color_shadow_secondary) }
    fn color_success(&self) -> Color { read_copy!(self, color_success) }
    fn color_success_bg(&self) -> Color { read_copy!(self, color_success_bg) }
    fn color_success_border(&self) -> Color { read_copy!(self, color_success_border) }
    fn color_warning(&self) -> Color { read_copy!(self, color_warning) }
    fn color_warning_bg(&self) -> Color { read_copy!(self, color_warning_bg) }
    fn color_warning_border(&self) -> Color { read_copy!(self, color_warning_border) }
    fn color_error(&self) -> Color { read_copy!(self, color_error) }
    fn color_error_bg(&self) -> Color { read_copy!(self, color_error_bg) }
    fn color_error_border(&self) -> Color { read_copy!(self, color_error_border) }
    fn color_info(&self) -> Color { read_copy!(self, color_info) }
    fn color_info_bg(&self) -> Color { read_copy!(self, color_info_bg) }
    fn color_info_border(&self) -> Color { read_copy!(self, color_info_border) }
    fn color_link(&self) -> Color { read_copy!(self, color_link) }
    fn color_link_hover(&self) -> Color { read_copy!(self, color_link_hover) }
    fn color_link_active(&self) -> Color { read_copy!(self, color_link_active) }
}

impl ITypographyTokens for DynTokens {
    // &str 字段在 antd_light 和 antd_dark 中值相同，直接返回静态字符串
    fn font_family(&self) -> &str {
        "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial"
    }
    fn font_size_sm(&self) -> f32 { read_copy!(self, font_size_sm) }
    fn font_size(&self) -> f32 { read_copy!(self, font_size) }
    fn font_size_lg(&self) -> f32 { read_copy!(self, font_size_lg) }
    fn font_size_xl(&self) -> f32 { read_copy!(self, font_size_xl) }
    fn font_size_heading_1(&self) -> f32 { read_copy!(self, font_size_heading_1) }
    fn font_size_heading_2(&self) -> f32 { read_copy!(self, font_size_heading_2) }
    fn font_size_heading_3(&self) -> f32 { read_copy!(self, font_size_heading_3) }
    fn font_size_heading_4(&self) -> f32 { read_copy!(self, font_size_heading_4) }
    fn font_size_heading_5(&self) -> f32 { read_copy!(self, font_size_heading_5) }
    fn font_weight_regular(&self) -> f32 { read_copy!(self, font_weight_regular) }
    fn font_weight_medium(&self) -> f32 { read_copy!(self, font_weight_medium) }
    fn font_weight_semibold(&self) -> f32 { read_copy!(self, font_weight_semibold) }
    fn font_weight_bold(&self) -> f32 { read_copy!(self, font_weight_bold) }
    fn line_height(&self) -> f32 { read_copy!(self, line_height) }
}

impl ISpacingTokens for DynTokens {
    fn padding_xss(&self) -> f32 { read_copy!(self, padding_xss) }
    fn padding_xs(&self) -> f32 { read_copy!(self, padding_xs) }
    fn padding_sm(&self) -> f32 { read_copy!(self, padding_sm) }
    fn padding(&self) -> f32 { read_copy!(self, padding) }
    fn padding_md(&self) -> f32 { read_copy!(self, padding_md) }
    fn padding_lg(&self) -> f32 { read_copy!(self, padding_lg) }
    fn padding_xl(&self) -> f32 { read_copy!(self, padding_xl) }
    fn border_radius(&self) -> f32 { read_copy!(self, border_radius) }
    fn border_radius_sm(&self) -> f32 { read_copy!(self, border_radius_sm) }
    fn border_radius_lg(&self) -> f32 { read_copy!(self, border_radius_lg) }
    fn border_radius_xl(&self) -> f32 { read_copy!(self, border_radius_xl) }
    fn border_radius_round(&self) -> f32 { read_copy!(self, border_radius_round) }
    fn control_height_sm(&self) -> f32 { read_copy!(self, control_height_sm) }
    fn control_height(&self) -> f32 { read_copy!(self, control_height) }
    fn control_height_lg(&self) -> f32 { read_copy!(self, control_height_lg) }
    // motion & screen 字段在 antd_light/dark 中值相同，也使用 RwLock
    fn motion_duration_fast(&self) -> f32 { read_copy!(self, motion_duration_fast) }
    fn motion_duration_mid(&self) -> f32 { read_copy!(self, motion_duration_mid) }
    fn motion_duration_slow(&self) -> f32 { read_copy!(self, motion_duration_slow) }
    fn motion_easing_default(&self) -> &str {
        // antd_light 和 antd_dark 值相同
        "cubic-bezier(0.25, 0.1, 0.25, 1)"
    }
    fn motion_easing_in(&self) -> &str {
        "cubic-bezier(0.42, 0, 1, 1)"
    }
    fn motion_easing_out(&self) -> &str {
        "cubic-bezier(0, 0, 0.58, 1)"
    }
    fn motion_easing_in_out(&self) -> &str {
        "cubic-bezier(0.42, 0, 0.58, 1)"
    }
    fn screen_xs(&self) -> f32 { read_copy!(self, screen_xs) }
    fn screen_sm(&self) -> f32 { read_copy!(self, screen_sm) }
    fn screen_md(&self) -> f32 { read_copy!(self, screen_md) }
    fn screen_lg(&self) -> f32 { read_copy!(self, screen_lg) }
    fn screen_xl(&self) -> f32 { read_copy!(self, screen_xl) }
    fn screen_xxl(&self) -> f32 { read_copy!(self, screen_xxl) }
}

impl IBoxShadowTokens for DynTokens {
    fn box_shadow(&self) -> ShadowToken { read_copy!(self, box_shadow) }
    fn box_shadow_secondary(&self) -> ShadowToken { read_copy!(self, box_shadow_secondary) }
}

impl TokenProvider for DynTokens {
    fn is_dark(&self) -> bool {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).is_dark
    }
}
