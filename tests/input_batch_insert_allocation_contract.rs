//! 验证批量文字输入只保留值扩容与事件负载所需的堆申请。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::prelude::Input;
use uix::ui::{EventHandler, EventResult, SystemEvent};

struct CountingAllocator;

static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 指针与 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: 指针与旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn long_text_input_uses_only_owned_value_and_event_payload_allocations() {
    // 事件载荷在测量前构造，避免把平台消息所有权计入组件编辑成本。
    let inserted = "中🙂a".repeat(1024);
    let event = SystemEvent::TextInput {
        text: inserted.clone(),
    };
    let mut input = Input::new("").with_value("prefix");

    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let result = input.on_event(&event);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert_eq!(result, EventResult::Handled);
    assert_eq!(input.current_value().len(), "prefix".len() + inserted.len());
    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("长文本批量输入堆申请次数: {allocations}");
    assert_eq!(
        allocations, 2,
        "长文本插入只应为值扩容和 Change 事件负载各申请一次"
    );

    // 最大长度落在 emoji 内部时，仍应无临时字符向量或裁剪字符串。
    let limited_inserted = "a👩🏽‍💻".repeat(1024);
    let limited_event = SystemEvent::TextInput {
        text: limited_inserted,
    };
    let mut limited = Input::new("").max_length(1024);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let limited_result = limited.on_event(&limited_event);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert_eq!(limited_result, EventResult::Handled);
    assert_eq!(limited.current_value().chars().count(), 1021);
    assert!(limited.current_value().ends_with('a'));
    let limited_allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("带字素簇长度限制的批量输入堆申请次数: {limited_allocations}");
    assert_eq!(
        limited_allocations, 2,
        "长度裁剪仍只应为值扩容和 Change 事件负载各申请一次"
    );

    // 非法控制字符确实需要一份过滤副本，但不得恢复逐字符扩容。
    let filtered_event = SystemEvent::TextInput {
        text: "甲\n乙\t🙂".repeat(1024),
    };
    let mut filtered = Input::new("");
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let filtered_result = filtered.on_event(&filtered_event);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert_eq!(filtered_result, EventResult::Handled);
    assert_eq!(filtered.current_value().chars().count(), 3 * 1024);
    let filtered_allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("需过滤控制字符的批量输入堆申请次数: {filtered_allocations}");
    assert_eq!(
        filtered_allocations, 3,
        "过滤分支只应额外拥有一份必要的紧凑输入副本"
    );
}
