// 引入当前模块全部类型。
use super::*;

// 单样本必须同时关闭 coverage 转换并保留全样本掩码。
#[test]
fn single_sample_disables_coverage_and_keeps_full_mask() {
    // 读取当前唯一共享状态。
    let state = PipelineMultisampleState::SingleSample;
    // 禁止 alpha-to-coverage 修改像素覆盖率。
    assert!(!state.alpha_to_coverage_enabled());
    // 禁止 Adapter 启用多样本 rasterizer。
    assert!(!state.raster_multisample_enabled());
    // 输出合并阶段必须允许全部样本位。
    assert_eq!(state.sample_mask(), u32::MAX);
}
