//! D4-I1：设施的公开所有权合同；不启动窗口、后台线程或真实业务服务。
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use uix::prelude::*;

#[test]
fn app_registration_and_container_clones_preserve_explicit_service_ownership() {
    let service = Arc::new(AtomicUsize::new(1));
    let weak = Arc::downgrade(&service);
    let app = App::new()
        .singleton(service.clone())
        .singleton("value".to_owned());
    drop(service);
    let mut container = app.container().clone();
    let resolved = container.resolve_clone::<Arc<AtomicUsize>>().unwrap();
    assert!(Arc::ptr_eq(
        &resolved,
        app.container().resolve::<Arc<AtomicUsize>>().unwrap()
    ));
    resolved.store(2, Ordering::Release);
    assert_eq!(
        app.container()
            .resolve::<Arc<AtomicUsize>>()
            .unwrap()
            .load(Ordering::Acquire),
        2
    );
    // resolve_clone 遵循 T::clone；普通值不因注册为 singleton 而变成共享可变对象。
    let mut copied = container.resolve_clone::<String>().unwrap();
    copied.push_str(" changed");
    assert_eq!(container.resolve::<String>().unwrap(), "value");
    assert!(container.resolve::<u64>().is_none());
    container.remove::<Arc<AtomicUsize>>();
    assert!(container.resolve::<Arc<AtomicUsize>>().is_none());
    assert!(app.container().has::<Arc<AtomicUsize>>());
    drop(app);
    assert!(weak.upgrade().is_some(), "解析出的 Arc 仍拥有服务");
    drop(resolved);
    assert!(weak.upgrade().is_none(), "最后一个显式所有者释放服务");
}

#[test]
fn locale_provider_is_scoped_but_business_translations_are_process_owned_snapshots() {
    let original = use_locale();
    // 本测试进程中仅此用例使用资源表；不依赖或改写真实应用的翻译数据。
    set_translations(&[("d4.save", "Save")]);
    let text_before = t!("d4.save");
    let tree = LocaleProvider::new(en_us())
        .child(|| {
            assert!(use_locale() == en_us());
            let child = LocaleProvider::new(zh_cn())
                .child(|| {
                    assert!(use_locale() == zh_cn());
                    assert_eq!(t!("d4.save"), "Save", "Locale 不隐式切换业务资源表");
                    label("子树")
                })
                .build();
            assert!(use_locale() == en_us());
            child
        })
        .build();
    assert!(use_locale() == original);
    drop(tree);
    assert_eq!(t!("d4.save"), "Save", "子树释放不清空进程资源表");
    register_translations(&[("d4.save", "保存"), ("d4.cancel", "取消")]);
    assert_eq!(t!("d4.save"), "保存");
    assert_eq!(text_before, "Save", "既有 String 不被资源表写入反向修改");
    set_translations(&[("d4.cancel", "Cancel")]);
    assert_eq!(t!("d4.save"), "d4.save", "整体替换移除旧 key，缺失返回 key");
    assert_eq!(t!("d4.cancel"), "Cancel");
    set_translations(&[]);
}

#[test]
fn timer_drop_cancel_and_detach_have_distinct_capture_owners() {
    let app = App::new();
    let marker = Arc::new(());
    let weak = Arc::downgrade(&marker);
    let timer = app.run_interval(Duration::from_secs(60), move || {
        let _ = &marker;
    });
    assert!(weak.upgrade().is_some());
    drop(timer);
    assert!(weak.upgrade().is_none(), "Drop 取消并释放未执行的捕获");

    let marker = Arc::new(());
    let weak = Arc::downgrade(&marker);
    let timer = app.run_after(Duration::from_secs(60), move || drop(marker));
    timer.cancel();
    assert!(weak.upgrade().is_none(), "显式 cancel 释放待执行任务");

    let marker = Arc::new(());
    let weak = Arc::downgrade(&marker);
    app.run_after(Duration::from_secs(60), move || drop(marker))
        .detach();
    assert!(
        weak.upgrade().is_some(),
        "detach 将余下生命周期交给调度 owner"
    );
    drop(app);
    assert!(weak.upgrade().is_none(), "无其他 owner 时应用队列释放捕获");
}

#[test]
fn standalone_effect_requires_an_owner_and_explicit_tick_after_notification() {
    let source = State::new(0usize);
    let observed = Arc::new(AtomicUsize::new(0));
    let marker = Arc::new(());
    let weak = Arc::downgrade(&marker);
    let effect_source = source.clone();
    let effect_observed = observed.clone();
    let effect = Effect::new(move || {
        let _ = &marker;
        effect_observed.store(effect_source.get() + 1, Ordering::Release);
    });
    assert_eq!(observed.load(Ordering::Acquire), 1);
    let owner = effect.clone();
    drop(effect);
    source.set(4);
    assert!(owner.has_pending());
    assert_eq!(
        observed.load(Ordering::Acquire),
        1,
        "独立 Effect 不自建任务执行器"
    );
    assert!(owner.tick());
    assert_eq!(observed.load(Ordering::Acquire), 5);
    drop(owner);
    assert!(
        weak.upgrade().is_none(),
        "最后一个 Effect owner 释放闭包和订阅租约"
    );
    source.set(9);
    assert_eq!(observed.load(Ordering::Acquire), 5);
}
