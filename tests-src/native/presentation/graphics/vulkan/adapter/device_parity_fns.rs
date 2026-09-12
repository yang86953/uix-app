// 各项自带原 cfg 门控，在源文件模块作用域 include! 展开。
// VulkanDevice 的验证观测/故障注入扩展 impl 与共享设备合同验证自由函数。

impl VulkanDevice {
    #[cfg(uix_gpu_parity_vulkan)]
    fn identity(&self) -> usize {
        self as *const Self as usize
    }

    #[cfg(any(test, uix_gpu_parity_vulkan, all(windows, feature = "vulkan")))]
    pub(super) fn mark_lost(&self) {
        self.loss.mark_for_test(self.fault_reporter.is_some());
    }
}

// 在真实 Vulkan device 上验证共享、peer loss、重取、旧代释放与 queue 身份合同。
#[cfg(uix_gpu_parity_vulkan)]
pub(super) fn run_shared_device_contract_test() {
    let runtime = VulkanRuntime::acquire().expect("shared-device runtime must be created");
    let selection = select_headless_test_queue(runtime.instance())
        .expect("a graphics queue with production Vulkan extensions must exist");
    let adapter = selection.info.diagnostic_summary();

    let first = runtime
        .acquire_device(selection.clone())
        .expect("first shared device must be created");
    let peer = runtime
        .acquire_device(selection.clone())
        .expect("healthy shared device must be reused");
    assert!(Rc::ptr_eq(&first, &peer));

    let first_identity = first.identity();
    first
        .with_queue("shared-device identity test", |_queue| {
            let reentry = peer
                .with_queue("peer concurrent submit test", |_queue| ())
                .expect_err("one shared queue must reject re-entry");
            assert_eq!(reentry.code(), crate::core::Errc::InvalidState);
        })
        .expect("outer shared queue lease must be valid");
    let submit_status = first
        .with_queue("shared-device empty submit", |queue| unsafe {
            first
                .device()
                .queue_submit(queue, &[], vk::Fence::null())
                .and_then(|()| first.device().queue_wait_idle(queue))
        })
        .expect("shared queue lease must serialize native submit");
    first
        .observe(submit_status.map_err(|error| vk_err("shared-device empty submit", error)))
        .expect("serialized native submit must complete");

    first.mark_lost();
    for owner in [&first, &peer] {
        let error = owner
            .ensure_healthy()
            .expect_err("every old-device peer must observe the same typed loss");
        assert_eq!(error.code(), crate::core::Errc::GraphicsDeviceLost);
        let queue_error = owner
            .with_queue("submit after peer loss", |_queue| ())
            .expect_err("lost peer must fail before touching the native queue");
        assert_eq!(queue_error.code(), crate::core::Errc::GraphicsDeviceLost);
    }

    let old = Rc::downgrade(&first);
    let replacement = runtime
        .acquire_device(selection.clone())
        .expect("lost device must be replaced");
    assert_ne!(first_identity, replacement.identity());
    assert!(old.upgrade().is_some());
    let replacement_peer = runtime
        .acquire_device(selection)
        .expect("healthy replacement must be shared");
    assert!(Rc::ptr_eq(&replacement, &replacement_peer));

    drop(first);
    assert!(old.upgrade().is_some());
    drop(peer);
    assert!(old.upgrade().is_none());
    replacement
        .with_queue("replacement identity test", |_queue| ())
        .expect("replacement queue must have an independent serial identity");

    eprintln!(
        "Vulkan shared device verified: {adapter}; old={first_identity:#x}; new={:#x}",
        replacement.identity()
    );
}
