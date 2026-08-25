//! 验证一次性文字事件索引查询不构造字符与字素簇边界堆表。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use uix::draw::resources::font::text_index::{
    BoundaryBias, CharIndex, TextIndexCursor, TextIndexMap,
};

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
fn one_shot_text_index_cursor_eliminates_boundary_table_allocations() {
    let text = "A👨‍👩‍👧‍👦e\u{301}中🙂".repeat(64);

    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    let map = TextIndexMap::new(&text);
    let normalized = map.normalize_char(CharIndex(17), BoundaryBias::Nearest);
    COUNT_ALLOCATIONS.store(false, Ordering::Release);

    assert!(normalized.0 <= map.char_len().0);
    let allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);
    eprintln!("一次性文字索引表堆申请次数: {allocations}");
    assert!(allocations >= 2, "基线应捕获字符与字素簇边界表申请");

    // 预先构造只含字符串借用的游标。
    let cursor = TextIndexCursor::new(&text);
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
    // 覆盖文字事件热路径使用的全部流式查询。
    std::hint::black_box(cursor.normalize_char(CharIndex(17), BoundaryBias::Nearest));
    std::hint::black_box(cursor.normalize_selection(CharIndex(2), CharIndex(18)));
    std::hint::black_box(cursor.previous_grapheme_boundary(CharIndex(18)));
    std::hint::black_box(cursor.next_grapheme_boundary(CharIndex(2)));
    std::hint::black_box(cursor.char_to_byte(CharIndex(18)));
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    let cursor_allocations = ALLOCATION_COUNT.load(Ordering::Relaxed);

    eprintln!("一次性文字索引游标堆申请次数: {cursor_allocations}");
    assert_eq!(cursor_allocations, 0, "借用游标不得建立临时边界表");
}
