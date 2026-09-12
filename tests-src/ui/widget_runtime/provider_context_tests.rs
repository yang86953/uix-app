//! `ui/widget_runtime/provider_context.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::theme::TokenPatch;
use crate::ui::widget_runtime::locale::en_us;
use crate::ui::widgets::Button;

#[test]
fn default_context_reuses_one_immutable_snapshot() {
    let first = ProviderContext::default();
    let second = ProviderContext::default();

    assert!(Arc::ptr_eq(&first.values, &second.values));
    assert_eq!(
        std::mem::size_of::<ProviderContext>(),
        std::mem::size_of::<Arc<ProviderContextValues>>()
    );
}

#[test]
fn shared_non_reflexive_snapshot_uses_identity() {
    let config = WidgetConfig::new().widget_tokens::<Button>(TokenPatch {
        font_size: Some(f32::NAN),
        ..TokenPatch::default()
    });
    let context = ProviderContext::default().with_config(&config);
    let shared = context.clone();

    assert!(Arc::ptr_eq(&context.values, &shared.values));
    assert!(context == shared);

    // 不同快照仍走值比较，NaN 的非反身语义不被身份快返扩大。
    let independent = ProviderContext::default().with_config(&config);
    assert!(!Arc::ptr_eq(&context.values, &independent.values));
    assert!(context != independent);
}

#[test]
fn independent_equal_snapshots_keep_value_semantics() {
    let first = ProviderContext {
        values: Arc::new(ProviderContextValues::default()),
    };
    let second = ProviderContext {
        values: Arc::new(ProviderContextValues::default()),
    };

    assert!(!Arc::ptr_eq(&first.values, &second.values));
    assert!(first == second);
}

#[test]
fn nested_overrides_restore_outer_snapshot() {
    let outer_config = WidgetConfig::new().disabled(true);
    let inner_locale = en_us();

    with_widget_config(&outer_config, || {
        let outer = current_provider_context();
        assert!(outer.config().disabled);
        assert!(outer.locale() == &Locale::default());

        with_widget_locale(&inner_locale, || {
            let inner = current_provider_context();
            assert!(inner.config().disabled);
            assert!(inner.locale() == &inner_locale);
            assert!(!Arc::ptr_eq(&outer.values, &inner.values));
        });

        let restored = current_provider_context();
        assert!(Arc::ptr_eq(&outer.values, &restored.values));
    });

    assert!(!current_provider_context().config().disabled);
}

#[test]
fn panic_restores_previous_snapshot() {
    // 故意 panic 与 crash hook 测试共享进程全局 hook，必须持锁串行。
    let _hook_series = crate::diagnostics::mod_tests::PANIC_HOOK_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let outer = current_provider_context();
    let overridden = WidgetConfig::new().disabled(true);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_widget_config(&overridden, || panic!("测试 ProviderContext 展开恢复"));
    }));

    assert!(result.is_err());
    let restored = current_provider_context();
    assert!(Arc::ptr_eq(&outer.values, &restored.values));
}
