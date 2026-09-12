// 各项自带 cfg(test) 门控，在 tsf_text_store.rs 模块作用域内 include! 展开。

#[cfg(test)]
static TEST_PANIC_NEXT_CALLBACK: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
fn panic_if_requested(operation: &str) {
    if TEST_PANIC_NEXT_CALLBACK.swap(false, Ordering::SeqCst) {
        panic!("test panic in TSF ABI callback: {operation}");
    }
}
