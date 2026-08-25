//! 验证输入删除与按词移动事件不建立临时 Unicode 边界容器。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::prelude::Input;
use uix::ui::{EventHandler, EventResult, KeyCode, KeyMod, SystemEvent};

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

fn key(key: KeyCode, mods: KeyMod) -> SystemEvent {
    SystemEvent::KeyDown { key, mods }
}

fn measure(input: &mut Input, event: &SystemEvent) -> (EventResult, usize) {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let result = input.on_event(event);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    (result, ALLOCATION_COUNT.load(Ordering::Relaxed))
}

#[test]
fn edit_events_avoid_temporary_index_allocations() {
    let long_value = format!("{}👩🏽‍💻", "word ".repeat(1024));
    let backspace = key(KeyCode::Backspace, KeyMod::NONE);
    let delete = key(KeyCode::Delete, KeyMod::NONE);
    let ctrl_left = key(KeyCode::Left, KeyMod::CTRL);
    let ctrl_right = key(KeyCode::Right, KeyMod::CTRL);
    let ctrl_a = key(KeyCode::A, KeyMod::CTRL);

    let mut backward = Input::new("").with_value(long_value.clone());
    let (backward_result, backward_allocations) = measure(&mut backward, &backspace);
    assert_eq!(backward_result, EventResult::Handled);
    assert!(backward.current_value().ends_with(' '));
    eprintln!("Backspace 堆申请次数: {backward_allocations}");

    let mut forward = Input::new("").with_value(long_value.clone());
    assert_eq!(
        forward.on_event(&key(KeyCode::Home, KeyMod::NONE)),
        EventResult::Handled
    );
    let (forward_result, forward_allocations) = measure(&mut forward, &delete);
    assert_eq!(forward_result, EventResult::Handled);
    assert!(forward.current_value().starts_with("ord "));
    eprintln!("Delete 堆申请次数: {forward_allocations}");

    let mut selection = Input::new("").with_value(long_value.clone());
    assert_eq!(selection.on_event(&ctrl_a), EventResult::Handled);
    let (selection_result, selection_allocations) = measure(&mut selection, &backspace);
    assert_eq!(selection_result, EventResult::Handled);
    assert_eq!(selection.current_value(), "");
    eprintln!("全选删除堆申请次数: {selection_allocations}");

    let mut movement = Input::new("").with_value(long_value);
    let (movement_result, movement_allocations) = measure(&mut movement, &ctrl_left);
    assert_eq!(movement_result, EventResult::Handled);
    eprintln!("Ctrl+Left 堆申请次数: {movement_allocations}");

    assert_eq!(
        movement.on_event(&key(KeyCode::Home, KeyMod::NONE)),
        EventResult::Handled
    );
    let (right_result, right_allocations) = measure(&mut movement, &ctrl_right);
    assert_eq!(right_result, EventResult::Handled);
    eprintln!("Ctrl+Right 堆申请次数: {right_allocations}");

    assert_eq!(
        backward_allocations, 1,
        "Backspace 只应分配 Change 事件负载"
    );
    assert_eq!(forward_allocations, 1, "Delete 只应分配 Change 事件负载");
    assert_eq!(selection_allocations, 0, "全选删除空值不得建立临时索引");
    assert_eq!(movement_allocations, 0, "按词移动不得建立临时字符向量");
    assert_eq!(right_allocations, 0, "向右按词移动不得建立临时字符向量");
}
