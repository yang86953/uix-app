//! 显式 feature 驱动的 Linux Wayland/EGL/OpenGL ES 真实窗口呈现测试。

#![cfg(target_os = "linux")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

// 仅在 paced 生产帧边界统计真实堆申请，不把平台与 GPU 初始化计入样本。
struct CountingAllocator;

static COUNT_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

// 更新当前测量区间内仍存活的申请字节与峰值。
fn record_allocation(size: usize) {
    ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

// 只回收测量区间内可见的 live 字节，防御区间前申请在区间内释放。
fn record_deallocation(size: usize) {
    let _ = LIVE_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
        Some(live.saturating_sub(size))
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            record_allocation(layout.size());
        }
        // SAFETY: 原样把有效 Layout 委托给系统分配器。
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            record_deallocation(layout.size());
        }
        // SAFETY: 指针与 Layout 来自同一系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size >= layout.size() {
                let growth = new_size - layout.size();
                let live = LIVE_BYTES.fetch_add(growth, Ordering::Relaxed) + growth;
                PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                record_deallocation(layout.size() - new_size);
            }
        }
        // SAFETY: 指针和旧 Layout 来自系统分配器，新尺寸由调用方提供。
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy, Debug)]
struct FrameSample {
    elapsed: Duration,
    allocations: usize,
    allocated_bytes: usize,
    peak_live_bytes: usize,
}

// 清空上帧统计并开启下一个真实 paced frame 的窄测量区间。
fn begin_frame_measurement() {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    COUNT_ALLOCATIONS.store(true, Ordering::Release);
}

// 关闭计数后冻结本帧的耗时、申请和峰值事实。
fn end_frame_measurement(elapsed: Duration) -> FrameSample {
    COUNT_ALLOCATIONS.store(false, Ordering::Release);
    FrameSample {
        elapsed,
        allocations: ALLOCATION_COUNT.load(Ordering::Relaxed),
        allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
        peak_live_bytes: PEAK_LIVE_BYTES.load(Ordering::Relaxed),
    }
}

fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

// 从真实 UI WidgetRender 入口闭合共享 FramePlan 到 EGL window surface。
#[test]
fn ui_drawing_frame_plan_presents_through_real_opengl_es_wsi() {
    let samples = RefCell::new(Vec::new());
    uix::__run_opengl_wsi_production_chain_profile(begin_frame_measurement, |elapsed| {
        samples.borrow_mut().push(end_frame_measurement(elapsed));
    });
    let samples = samples.into_inner();
    assert_eq!(
        samples.len(),
        6,
        "真实八帧中的后六帧必须进入 paced 测量区间"
    );
    let elapsed_ns = median(
        samples
            .iter()
            .map(|sample| sample.elapsed.as_nanos())
            .collect(),
    );
    let allocations = median(
        samples
            .iter()
            .map(|sample| sample.allocations as u128)
            .collect(),
    );
    let allocated_bytes = median(
        samples
            .iter()
            .map(|sample| sample.allocated_bytes as u128)
            .collect(),
    );
    let peak_live_bytes = median(
        samples
            .iter()
            .map(|sample| sample.peak_live_bytes as u128)
            .collect(),
    );
    eprintln!(
        "PROFILE real_wsi_frame: samples={}; median_ns={elapsed_ns}; allocations={allocations}; allocated_bytes={allocated_bytes}; peak_live_bytes={peak_live_bytes}",
        samples.len(),
    );
    assert!(
        allocations <= 50,
        "稳态真实 WSI 帧不得恢复第二份 FramePlanCommand 扩容"
    );
    assert!(
        allocated_bytes <= 8_500,
        "稳态真实 WSI 帧累计申请字节不得超过已验证预算"
    );
    assert!(
        peak_live_bytes <= 3_400,
        "稳态真实 WSI 帧峰值 live 字节不得超过已验证预算"
    );
}
