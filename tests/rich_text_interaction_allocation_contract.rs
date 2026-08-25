//! 验证单段与跨段富文本选择都不会为逻辑源投影建立临时字符串。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::prelude::{RichText, RichTextSegment, RichTextStyle};
use uix::ui::{EventHandler, EventResult, KeyCode, KeyMod, SystemEvent};

// 只在显式测量窗口内统计当前测试进程的堆申请。
struct CountingAllocator;

// 控制当前是否记录堆申请。
static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
// 保存最近一次测量窗口中的堆申请次数。
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

// 在一个最小窗口内测量闭包触发的堆申请。
fn measure<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let result = operation();
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    (result, ALLOCATION_COUNT.load(Ordering::Relaxed))
}

#[test]
fn selection_avoids_temporary_source_allocations() {
    let content = format!("{}👩🏽‍💻", "富文本交互 ".repeat(1024));
    let mut rich = RichText::new()
        .selectable(true)
        .content(vec![RichTextSegment::Text {
            content: content.clone(),
            style: RichTextStyle::default(),
        }]);
    let select_all = SystemEvent::KeyDown {
        key: KeyCode::A,
        mods: KeyMod::CTRL,
    };

    let (event_result, event_allocations) = measure(|| rich.on_event(&select_all));
    assert_eq!(event_result, EventResult::Handled);
    eprintln!("单段富文本全选堆申请次数: {event_allocations}");
    assert_eq!(event_allocations, 0, "全选不得拼接临时逻辑源文本");

    let (selected, copy_allocations) = measure(|| rich.selected_text());
    assert_eq!(selected.as_deref(), Some(content.as_str()));
    eprintln!("单段富文本复制堆申请次数: {copy_allocations}");
    assert_eq!(copy_allocations, 1, "复制只应申请最终返回字符串");

    // 把组合音标与 ZWJ emoji 故意拆到不同样式段，覆盖真实跨段字素簇边界。
    let segments = vec![
        RichTextSegment::Text {
            content: "a".to_owned(),
            style: RichTextStyle::default(),
        },
        RichTextSegment::Text {
            content: "\u{0301}".to_owned(),
            style: RichTextStyle {
                bold: true,
                ..RichTextStyle::default()
            },
        },
        RichTextSegment::Code {
            content: "👩🏽‍".to_owned(),
        },
        RichTextSegment::Link {
            content: "💻".to_owned(),
            url: "https://example.test".to_owned(),
        },
        RichTextSegment::Text {
            content: "尾声".repeat(1024),
            style: RichTextStyle::default(),
        },
    ];
    let expected = "a\u{0301}👩🏽‍💻".to_owned() + &"尾声".repeat(1024);
    let mut rich = RichText::new().selectable(true).content(segments);
    let select_all = SystemEvent::KeyDown {
        key: KeyCode::A,
        mods: KeyMod::CTRL,
    };

    let (event_result, event_allocations) = measure(|| rich.on_event(&select_all));
    assert_eq!(event_result, EventResult::Handled);
    assert_eq!(rich.selected_text().as_deref(), Some(expected.as_str()));
    eprintln!("多段富文本全选堆申请次数: {event_allocations}");
    assert_eq!(event_allocations, 0, "跨段全选不得拼接临时逻辑源文本");
}
