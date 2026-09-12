// 各项自带原 cfg 门控（uix_gpu_parity_* 或含 test/生产分支的 any 组合），
// 在源文件模块作用域 include! 展开。

// 确定性验证连续帧与代际切换的 present 完成所有权，不创建原生对象。
#[cfg(uix_gpu_parity_vulkan)]
pub(crate) fn run_present_completion_contract_test() {
    let mut completion = PresentCompletion::empty();
    completion.replace_generation(vec![false; 3]);
    assert_eq!(
        completion
            .release_count_for_acquire(0, 2)
            .unwrap_or_else(|error| panic!("first image acquire failed: {error}")),
        0
    );
    completion
        .mark_presented(1)
        .unwrap_or_else(|error| panic!("present history update failed: {error}"));
    assert_eq!(
        completion
            .release_count_for_acquire(1, 2)
            .unwrap_or_else(|error| panic!("reacquired image lookup failed: {error}")),
        2
    );
    completion.on_submission_queued(2);
    completion.on_submission_queued(1);
    assert_eq!(completion.completed_submission_count(), 2);
    assert_eq!(completion.completed_submission_count(), 0);

    completion.replace_generation(
        allocate_presented_images(2)
            .unwrap_or_else(|error| panic!("replacement generation failed: {error}")),
    );
    assert_eq!(
        completion
            .release_count_for_acquire(0, 3)
            .unwrap_or_else(|error| panic!("new generation acquire failed: {error}")),
        0
    );
    assert!(completion.release_count_for_acquire(2, 3).is_err());
}
