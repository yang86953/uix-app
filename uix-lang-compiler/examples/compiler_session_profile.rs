//! 测量 `CompilerSession` 稳定 overlay 命中的耗时与分配。

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

use uix_lang_compiler::{CompileTarget, CompilerSession};

struct CountingAllocator;

static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 完整转发调用方提供的合法布局给系统分配器。
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: 指针和布局来自对应的系统分配器调用。
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 指针和旧布局来自系统分配器，新尺寸由调用方提供。
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            if new_size >= layout.size() {
                record_allocation(new_size - layout.size());
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_pointer
    }
}

fn record_allocation(bytes: usize) {
    ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
    ALLOCATED_BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

fn reset_counters() -> usize {
    let live = LIVE_BYTES.load(Ordering::Relaxed);
    ALLOCATION_CALLS.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(live, Ordering::Relaxed);
    live
}

fn profile_source(nodes: usize) -> String {
    let mut source = String::with_capacity(nodes * 48);
    source.push_str("<Column gap=\"4px\">\n");
    for index in 0..nodes {
        source.push_str("  <Text automationId=\"item-");
        source.push_str(&index.to_string());
        source.push_str("\">稳定内容</Text>\n");
    }
    source.push_str("</Column>\n");
    source
}

fn dependency_source(nodes: usize) -> String {
    let mut source = String::with_capacity(nodes * 48);
    source.push_str("@export('Helper')\n<Widget name=\"Helper\"><Column gap=\"4px\">\n");
    for index in 0..nodes {
        source.push_str("  <Text automationId=\"item-");
        source.push_str(&index.to_string());
        source.push_str("\">稳定内容</Text>\n");
    }
    source.push_str("</Column></Widget>\n<Helper />\n");
    source
}

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(200);
    let nodes = std::env::args()
        .nth(2)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(500);
    let scenario = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "single".to_string());
    let root = PathBuf::from("/tmp/uix-compiler-session-profile/root.uix");
    let mut overlays = BTreeMap::new();
    if scenario == "multi" {
        overlays.insert(
            root.clone(),
            "@import('./helper.uix', 'Helper')\n<Column><Helper /></Column>\n".to_string(),
        );
        overlays.insert(root.with_file_name("helper.uix"), dependency_source(nodes));
    } else {
        overlays.insert(root.clone(), profile_source(nodes));
    }
    let mut session = CompilerSession::new();

    session
        .check_file_with_overlays(&root, &overlays, CompileTarget::View)
        .expect("预热编译必须成功");

    let baseline_live = reset_counters();
    let started = Instant::now();
    for _ in 0..iterations {
        let output = session
            .check_file_with_overlays(&root, &overlays, CompileTarget::View)
            .expect("稳定 overlay 必须持续命中");
        black_box(output);
    }
    let elapsed = started.elapsed();
    let allocation_calls = ALLOCATION_CALLS.load(Ordering::Relaxed);
    let allocated_bytes = ALLOCATED_BYTES.load(Ordering::Relaxed);
    let peak_live = PEAK_LIVE_BYTES.load(Ordering::Relaxed);

    println!("iterations={iterations}");
    println!("nodes={nodes}");
    println!("scenario={scenario}");
    println!("elapsed_ns={}", elapsed.as_nanos());
    println!(
        "ns_per_iteration={}",
        elapsed.as_nanos() / iterations as u128
    );
    println!("allocation_calls={allocation_calls}");
    println!(
        "allocation_calls_per_iteration={}",
        allocation_calls / iterations as u64
    );
    println!("allocated_bytes={allocated_bytes}");
    println!(
        "allocated_bytes_per_iteration={}",
        allocated_bytes / iterations as u64
    );
    println!(
        "peak_live_bytes_delta={}",
        peak_live.saturating_sub(baseline_live)
    );
}
