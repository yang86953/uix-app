use crate::ui::component::config::ComponentConfig;
use crate::ui::component::locale::Locale;
use std::cell::RefCell;

/// 在 View 构建时捕获、随节点保存的 Provider 上下文。
#[derive(Clone, Default, PartialEq)]
pub(crate) struct ProviderContext {
    pub(crate) config: ComponentConfig,
    pub(crate) locale: Locale,
}

thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static PROVIDER_CONTEXT_STACK: RefCell<Vec<ProviderContext>> = const { RefCell::new(Vec::new()) };
}

struct ProviderContextGuard;

impl Drop for ProviderContextGuard {
    fn drop(&mut self) {
        PROVIDER_CONTEXT_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

pub(crate) fn current_provider_context() -> ProviderContext {
    PROVIDER_CONTEXT_STACK.with(|stack| stack.borrow().last().cloned().unwrap_or_default())
}

pub(crate) fn with_provider_context<T>(context: &ProviderContext, f: impl FnOnce() -> T) -> T {
    PROVIDER_CONTEXT_STACK.with(|stack| stack.borrow_mut().push(context.clone()));
    let _guard = ProviderContextGuard;
    f()
}

pub(crate) fn with_component_config<T>(config: &ComponentConfig, f: impl FnOnce() -> T) -> T {
    let mut context = current_provider_context();
    context.config = config.clone();
    with_provider_context(&context, f)
}

pub(crate) fn with_component_locale<T>(locale: &Locale, f: impl FnOnce() -> T) -> T {
    let mut context = current_provider_context();
    context.locale = locale.clone();
    with_provider_context(&context, f)
}
