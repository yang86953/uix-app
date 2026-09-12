// 各项自带 cfg(test) 门控，在源文件模块作用域内 include! 展开。

// 测试只统计当前线程确有隐藏子项时的过滤分配，避免并行测试相互污染。
#[cfg(test)]
thread_local! {
    static FILTERED_CHILD_ALLOCATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_filtered_child_allocation_count() {
    FILTERED_CHILD_ALLOCATION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn filtered_child_allocation_count() -> usize {
    FILTERED_CHILD_ALLOCATION_COUNT.get()
}
