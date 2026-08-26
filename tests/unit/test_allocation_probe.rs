//! lib 单元测试二进制共享的独占分配测量探针。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// 测量窗口之外完全转发系统 allocator，不改变产品构建。
struct CountingAllocator;

static MEASURING: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 记录一次新分配并更新当前与峰值 live 字节。
fn record_allocation(size: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 完整转发调用方提供的有效 Layout，返回值语义保持 System allocator 契约。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 完整转发调用方提供的有效 Layout，零初始化语义由 System allocator 保证。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if MEASURING.load(Ordering::Relaxed) {
            LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: ptr 与 Layout 来自同一 allocator 的既有分配，按 GlobalAlloc 契约原样转发。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if MEASURING.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size >= layout.size() {
                let added = new_size - layout.size();
                let live = LIVE_BYTES.fetch_add(added, Ordering::Relaxed) + added;
                PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        // SAFETY: ptr、旧 Layout 与新大小按 GlobalAlloc::realloc 契约原样转发。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

// lib 单元测试只能声明一个全局 allocator，由中立测试支持边界唯一拥有。
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// 保存一个封闭测量窗口的分配与 live 内存结果。
#[derive(Clone, Copy)]
pub(crate) struct AllocationStats {
    pub(crate) count: usize,
    pub(crate) bytes: usize,
    pub(crate) peak_live: usize,
    pub(crate) final_live: usize,
}

// 独占执行一次同步动作并返回该窗口的分配统计。
pub(crate) fn allocation_stats<F: FnOnce()>(action: F) -> AllocationStats {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    MEASURING.store(true, Ordering::Release);
    action();
    MEASURING.store(false, Ordering::Release);
    AllocationStats {
        count: ALLOCATION_COUNT.load(Ordering::Relaxed),
        bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        final_live: LIVE_BYTES.load(Ordering::Relaxed),
    }
}
