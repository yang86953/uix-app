//! RHI 采样器共享值对象的完整类型化语义测试。

// 复用父模块公开给本 crate 的采样器契约类型。
use super::{SamplerAddressMode, SamplerDesc, SamplerFilter, SamplerMipMode};

// 验证两个命名构造器只改变过滤策略并共同冻结寻址与 mip 语义。
#[test]
fn named_clamp_constructors_share_address_and_mip_semantics() {
    // 创建共享契约规定的线性 clamp sampler。
    let linear = SamplerDesc::linear_clamp();
    // 创建共享契约规定的最近点 clamp sampler。
    let nearest = SamplerDesc::nearest_clamp();
    // 两个构造器必须分别表达线性过滤与最近点过滤。
    assert_eq!(linear.filter(), SamplerFilter::Linear);
    // 最近点构造器必须表达最近点过滤。
    assert_eq!(nearest.filter(), SamplerFilter::Nearest);
    // 两个构造器必须共同冻结边缘寻址。
    assert_eq!(linear.address_mode(), SamplerAddressMode::ClampToEdge);
    // 最近点构造器也必须冻结边缘寻址。
    assert_eq!(nearest.address_mode(), SamplerAddressMode::ClampToEdge);
    // 两个构造器必须共同冻结单级 mip 语义。
    assert_eq!(linear.mip_mode(), SamplerMipMode::SingleLevel);
    // 最近点构造器也必须冻结单级 mip 语义。
    assert_eq!(nearest.mip_mode(), SamplerMipMode::SingleLevel);
    // 两个描述的差异必须仅限于过滤策略。
    assert_ne!(linear.filter(), nearest.filter());
}

// 验证三个封闭枚举只接受当前架构规定的状态，不依赖 Debug 文本。
#[test]
fn sampler_enums_are_exhaustive_at_the_shared_boundary() {
    // 过滤枚举的两个架构状态必须被穷尽匹配。
    let filter_states = [SamplerFilter::Linear, SamplerFilter::Nearest];
    // 逐一匹配过滤枚举，新增状态将使该契约测试无法静默通过。
    for filter in filter_states {
        // 仅接受架构规定的线性或最近点过滤状态。
        match filter {
            // 接受线性过滤状态。
            SamplerFilter::Linear => {}
            // 接受最近点过滤状态。
            SamplerFilter::Nearest => {}
        }
    }
    // 边缘寻址枚举当前只有一个架构状态，匹配必须穷尽该枚举。
    match SamplerAddressMode::ClampToEdge {
        // 接受边缘寻址状态。
        SamplerAddressMode::ClampToEdge => {}
    }
    // 单级 mip 枚举当前只有一个架构状态，匹配必须穷尽该枚举。
    match SamplerMipMode::SingleLevel {
        // 接受单级 mip 状态。
        SamplerMipMode::SingleLevel => {}
    }
}
