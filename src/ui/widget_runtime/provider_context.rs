use crate::ui::widget_runtime::config::WidgetConfig;
use crate::ui::widget_runtime::locale::Locale;
use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::{Arc, OnceLock};

/// 在 View 构建时捕获、由同一子树节点共享的 Provider 上下文快照。
#[derive(Clone)]
pub struct ProviderContext {
    values: Arc<ProviderContextValues>,
}

/// 配置与语言共同组成一次不可变 Provider 快照。
#[derive(Clone, Default, PartialEq)]
struct ProviderContextValues {
    config: WidgetConfig,
    locale: Locale,
}

impl PartialEq for ProviderContext {
    fn eq(&self, other: &Self) -> bool {
        // 同一不可变快照必定未变；仅不同快照继续保留完整值比较语义。
        Arc::ptr_eq(&self.values, &other.values) || self.values == other.values
    }
}

impl Default for ProviderContext {
    fn default() -> Self {
        // 默认快照跨节点共享，避免每次脱离 Provider 构造组件时复制完整语言表。
        static DEFAULT_VALUES: OnceLock<Arc<ProviderContextValues>> = OnceLock::new();
        Self {
            values: Arc::clone(
                DEFAULT_VALUES.get_or_init(|| Arc::new(ProviderContextValues::default())),
            ),
        }
    }
}

impl ProviderContext {
    pub(crate) fn config(&self) -> &WidgetConfig {
        &self.values.config
    }

    pub(crate) fn locale(&self) -> &Locale {
        &self.values.locale
    }

    fn with_config(&self, config: &WidgetConfig) -> Self {
        let mut context = self.clone();
        // Provider 覆写生成新快照，既有节点继续读取捕获时的不可变值。
        Arc::make_mut(&mut context.values).config = config.clone();
        context
    }

    fn with_locale(&self, locale: &Locale) -> Self {
        let mut context = self.clone();
        // Locale 与配置共同保持同一次子树快照的值语义。
        Arc::make_mut(&mut context.values).locale = locale.clone();
        context
    }
}

thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static PROVIDER_CONTEXT_STACK: RefCell<Vec<ProviderContextFrame>> = const { RefCell::new(Vec::new()) };
}

/// 同步闭包有效期内借用 ProviderContext，不参与共享快照的引用计数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProviderContextFrame(NonNull<ProviderContext>);

impl ProviderContextFrame {
    fn clone_context(self) -> ProviderContext {
        // SAFETY: frame 只由 `with_provider_context` 从活跃引用创建，且守卫在
        // 该同步闭包返回或展开前必定弹出 frame，因此读取期间来源仍然存活。
        unsafe { self.0.as_ref().clone() }
    }
}

struct ProviderContextGuard {
    frame: ProviderContextFrame,
}

impl Drop for ProviderContextGuard {
    fn drop(&mut self) {
        PROVIDER_CONTEXT_STACK.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert_eq!(popped, Some(self.frame));
        });
    }
}

pub(crate) fn current_provider_context() -> ProviderContext {
    PROVIDER_CONTEXT_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .copied()
            .map(ProviderContextFrame::clone_context)
            .unwrap_or_default()
    })
}

pub fn with_provider_context<T>(context: &ProviderContext, f: impl FnOnce() -> T) -> T {
    let frame = ProviderContextFrame(NonNull::from(context));
    PROVIDER_CONTEXT_STACK.with(|stack| stack.borrow_mut().push(frame));
    let _guard = ProviderContextGuard { frame };
    f()
}

pub(crate) fn with_widget_config<T>(config: &WidgetConfig, f: impl FnOnce() -> T) -> T {
    let context = current_provider_context().with_config(config);
    with_provider_context(&context, f)
}

pub(crate) fn with_widget_locale<T>(locale: &Locale, f: impl FnOnce() -> T) -> T {
    let context = current_provider_context().with_locale(locale);
    with_provider_context(&context, f)
}

#[cfg(test)]
#[path = "../../../tests-src/ui/widget_runtime/provider_context_tests.rs"]
mod tests;

