// 引入被测规范化、标量参考和运行时 SIMD 分派。
use super::{normalize_upload_payload, swap_bgra_to_rgba, swap_bgra_to_rgba_scalar};
// 复用测试二进制中的唯一全局分配探针。
use crate::test_allocation_probe::{allocation_stats, AllocationStats};
// 引入共享纹理格式。
use crate::platform::presentation::rhi::TextureFormat;
// 引入黑盒与稳态计时。
use std::hint::black_box;
use std::time::Instant;

const PROFILE_WIDTH: usize = 1200;
const PROFILE_HEIGHT: usize = 800;
const PROFILE_BYTES: usize = PROFILE_WIDTH * PROFILE_HEIGHT * 4;
const PROFILE_UPDATES: usize = 240;
const PROFILE_ROUNDS: usize = 9;

// 构造包含透明、半透明和不透明像素的确定性预乘 BGRA 图片。
fn profile_bgra_payload() -> Vec<u8> {
    let mut payload = vec![0_u8; PROFILE_BYTES];
    for (index, pixel) in payload.chunks_exact_mut(4).enumerate() {
        let alpha = match index % 3 {
            0 => 0,
            1 => 128,
            _ => 255,
        };
        // 颜色通道不超过 alpha，保持预乘颜色输入约束。
        pixel.copy_from_slice(&[alpha / 4, alpha / 2, alpha / 8, alpha]);
    }
    payload
}

// 用 PERF-017 的原始复制加标量交换形成同条件参考帧。
fn scalar_payload(data: &[u8]) -> Vec<u8> {
    let mut rgba = data.to_vec();
    swap_bgra_to_rgba_scalar(&mut rgba);
    rgba
}

// 用产品运行时分派形成同条件候选帧。
fn dispatched_payload(data: &[u8]) -> Vec<u8> {
    let mut rgba = data.to_vec();
    swap_bgra_to_rgba(&mut rgba);
    rgba
}

// 在当前 CPU 支持时隔离测量 AVX2 intrinsic，不绕过能力门禁执行。
#[cfg(target_arch = "x86_64")]
fn avx2_payload(data: &[u8]) -> Option<Vec<u8>> {
    if !std::arch::is_x86_feature_detected!("avx2") {
        return None;
    }
    let mut rgba = data.to_vec();
    // SAFETY: 上方运行时检测已证明当前 CPU 支持 AVX2。
    unsafe { super::swap_bgra_to_rgba_avx2(&mut rgba) };
    Some(rgba)
}

// 在当前 CPU 支持时隔离测量 SSSE3 intrinsic，不绕过能力门禁执行。
#[cfg(target_arch = "x86_64")]
fn ssse3_payload(data: &[u8]) -> Option<Vec<u8>> {
    if !std::arch::is_x86_feature_detected!("ssse3") {
        return None;
    }
    let mut rgba = data.to_vec();
    // SAFETY: 上方运行时检测已证明当前 CPU 支持 SSSE3。
    unsafe { super::swap_bgra_to_rgba_ssse3(&mut rgba) };
    Some(rgba)
}

// 取得九轮稳态样本的中位数。
fn median(mut samples: [u128; PROFILE_ROUNDS]) -> u128 {
    samples.sort_unstable();
    samples[PROFILE_ROUNDS / 2]
}

// 测量一个完整复制加转换实现的单帧分配。
fn allocation_fields(stats: AllocationStats) -> (usize, usize, usize, usize) {
    (stats.count, stats.bytes, stats.peak_live, stats.final_live)
}

// 把架构条件封装在编译期，非 x86_64 不引用不可用的检测宏。
#[cfg(target_arch = "x86_64")]
fn runtime_simd_support() -> (bool, bool) {
    (
        std::arch::is_x86_feature_detected!("avx2"),
        std::arch::is_x86_feature_detected!("ssse3"),
    )
}

// 非 x86_64 构建固定报告标量后备。
#[cfg(not(target_arch = "x86_64"))]
fn runtime_simd_support() -> (bool, bool) {
    (false, false)
}

// 锁定 BGRA 载荷进入 RGBA8 存储前只交换红蓝通道。
#[test]
fn normalizes_bgra_payload_to_rgba_storage() {
    // 构造两个包含不同通道值的 BGRA 像素。
    let bgra = [0x33, 0x22, 0x11, 0x44, 0x77, 0x66, 0x55, 0x88];
    // 执行 Adapter 私有规范化。
    let rgba = normalize_upload_payload(TextureFormat::Bgra8Unorm, &bgra);
    // 每个像素只交换首尾颜色通道，alpha 不变。
    assert_eq!(
        rgba.as_ref(),
        [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
    );
}

// 锁定透明度与首尾像素在运行时 SIMD 分派下仍保持逐字节结果。
#[test]
fn normalizes_alpha_and_edge_pixels_byte_exactly() {
    let bgra = [
        0x00, 0x00, 0x00, 0x00, 0x20, 0x40, 0x10, 0x80, 0x40, 0x80, 0x20, 0xFF,
    ];
    let normalized = normalize_upload_payload(TextureFormat::Bgra8Unorm, &bgra);
    assert_eq!(
        normalized.as_ref(),
        [0x00, 0x00, 0x00, 0x00, 0x10, 0x40, 0x20, 0x80, 0x20, 0x80, 0x40, 0xFF]
    );
}

// 覆盖空、短、非 16/32 字节尾部以及非四字节尾部的边界访问。
#[test]
fn simd_dispatch_matches_scalar_for_all_short_tails() {
    for byte_len in 0..=67 {
        let source: Vec<u8> = (0..byte_len).map(|value| value as u8).collect();
        let mut expected = source.clone();
        swap_bgra_to_rgba_scalar(&mut expected);
        let mut actual = source;
        swap_bgra_to_rgba(&mut actual);
        assert_eq!(actual, expected, "尾部长度 {byte_len} 必须与标量一致");
    }
}

// 未对齐地址必须使用 loadu/storeu 安全转换且不破坏两侧 guard。
#[test]
fn simd_dispatch_handles_unaligned_input_without_touching_guards() {
    let mut storage: Vec<u8> = (0..=66).map(|value| value as u8).collect();
    let before_guard = storage[0];
    let after_guard = storage[66];
    let mut expected = storage[1..66].to_vec();
    swap_bgra_to_rgba_scalar(&mut expected);
    swap_bgra_to_rgba(&mut storage[1..66]);
    assert_eq!(&storage[1..66], expected);
    assert_eq!(storage[0], before_guard);
    assert_eq!(storage[66], after_guard);
}

// 每个显式 x86_64 intrinsic 在受支持 CPU 上都必须等价于标量参考。
#[cfg(target_arch = "x86_64")]
#[test]
fn supported_intrinsics_match_scalar_reference() {
    let source: Vec<u8> = (0..132).map(|value| (value * 17) as u8).collect();
    let expected = scalar_payload(&source);
    if let Some(actual) = avx2_payload(&source) {
        assert_eq!(actual, expected);
    }
    if let Some(actual) = ssse3_payload(&source) {
        assert_eq!(actual, expected);
    }
}

// 锁定已经是 RGBA 的载荷保持零复制与原序。
#[test]
fn borrows_rgba_payload_without_channel_changes() {
    // 构造一个 RGBA 像素。
    let rgba = [0x11, 0x22, 0x33, 0x44];
    // 执行无需转换的 Adapter 路径。
    let normalized = normalize_upload_payload(TextureFormat::Rgba8Unorm, &rgba);
    // RGBA 载荷必须保持原始字节顺序。
    assert_eq!(normalized.as_ref(), rgba);
    // 未转换路径必须继续借用调用方切片。
    assert!(matches!(normalized, std::borrow::Cow::Borrowed(_)));
}

// 沿用 PERF-017 场景，对标量、运行时分派和可用 intrinsic 做可比取样。
#[test]
#[ignore = "性能取样需独占测试进程，避免全局分配统计受到并行测试干扰"]
fn profile_opengl_bgra_simd_upload() {
    let bgra = profile_bgra_payload();
    let expected = scalar_payload(&bgra);
    assert_eq!(dispatched_payload(&bgra), expected);
    #[cfg(target_arch = "x86_64")]
    {
        if let Some(actual) = avx2_payload(&bgra) {
            assert_eq!(actual, expected);
        }
        if let Some(actual) = ssse3_payload(&bgra) {
            assert_eq!(actual, expected);
        }
    }

    for _ in 0..32 {
        black_box(scalar_payload(black_box(&bgra)));
        black_box(dispatched_payload(black_box(&bgra)));
    }

    let mut scalar_samples = [0_u128; PROFILE_ROUNDS];
    let mut dispatch_samples = [0_u128; PROFILE_ROUNDS];
    #[cfg(target_arch = "x86_64")]
    let mut avx2_samples = [0_u128; PROFILE_ROUNDS];
    #[cfg(target_arch = "x86_64")]
    let mut ssse3_samples = [0_u128; PROFILE_ROUNDS];
    for round in 0..PROFILE_ROUNDS {
        let start = Instant::now();
        for _ in 0..PROFILE_UPDATES {
            black_box(scalar_payload(black_box(&bgra)));
        }
        scalar_samples[round] = start.elapsed().as_nanos() / PROFILE_UPDATES as u128;

        let start = Instant::now();
        for _ in 0..PROFILE_UPDATES {
            black_box(dispatched_payload(black_box(&bgra)));
        }
        dispatch_samples[round] = start.elapsed().as_nanos() / PROFILE_UPDATES as u128;

        #[cfg(target_arch = "x86_64")]
        {
            let start = Instant::now();
            for _ in 0..PROFILE_UPDATES {
                black_box(avx2_payload(black_box(&bgra)));
            }
            avx2_samples[round] = start.elapsed().as_nanos() / PROFILE_UPDATES as u128;

            let start = Instant::now();
            for _ in 0..PROFILE_UPDATES {
                black_box(ssse3_payload(black_box(&bgra)));
            }
            ssse3_samples[round] = start.elapsed().as_nanos() / PROFILE_UPDATES as u128;
        }
    }

    let scalar_allocations = allocation_stats(|| {
        black_box(scalar_payload(black_box(&bgra)));
    });
    let dispatch_allocations = allocation_stats(|| {
        black_box(dispatched_payload(black_box(&bgra)));
    });
    #[cfg(target_arch = "x86_64")]
    let avx2_allocations = allocation_stats(|| {
        black_box(avx2_payload(black_box(&bgra)));
    });
    #[cfg(target_arch = "x86_64")]
    let ssse3_allocations = allocation_stats(|| {
        black_box(ssse3_payload(black_box(&bgra)));
    });
    let scalar_ns = median(scalar_samples);
    let dispatch_ns = median(dispatch_samples);
    let scalar_throughput = PROFILE_BYTES as u128 * 1_000_000_000 / scalar_ns;
    let dispatch_throughput = PROFILE_BYTES as u128 * 1_000_000_000 / dispatch_ns;
    let (scalar_count, scalar_bytes, scalar_peak, scalar_final) =
        allocation_fields(scalar_allocations);
    let (dispatch_count, dispatch_bytes, dispatch_peak, dispatch_final) =
        allocation_fields(dispatch_allocations);
    #[cfg(target_arch = "x86_64")]
    let (avx2_ns, ssse3_ns) = (median(avx2_samples), median(ssse3_samples));
    #[cfg(target_arch = "x86_64")]
    let (avx2_count, avx2_bytes, avx2_peak, avx2_final) = allocation_fields(avx2_allocations);
    #[cfg(target_arch = "x86_64")]
    let (ssse3_count, ssse3_bytes, ssse3_peak, ssse3_final) = allocation_fields(ssse3_allocations);
    #[cfg(not(target_arch = "x86_64"))]
    let (avx2_ns, ssse3_ns) = (0_u128, 0_u128);
    #[cfg(not(target_arch = "x86_64"))]
    let (avx2_count, avx2_bytes, avx2_peak, avx2_final) = (0, 0, 0, 0);
    #[cfg(not(target_arch = "x86_64"))]
    let (ssse3_count, ssse3_bytes, ssse3_peak, ssse3_final) = (0, 0, 0, 0);
    let (avx2_supported, ssse3_supported) = runtime_simd_support();
    eprintln!(
        "PROFILE opengl_bgra_simd scalar_ns={scalar_ns} dispatch_ns={dispatch_ns} avx2_ns={avx2_ns} ssse3_ns={ssse3_ns} scalar_bytes_per_second={scalar_throughput} dispatch_bytes_per_second={dispatch_throughput} payload_bytes={PROFILE_BYTES} scalar_allocs={scalar_count} scalar_bytes={scalar_bytes} scalar_peak={scalar_peak} scalar_final={scalar_final} dispatch_allocs={dispatch_count} dispatch_bytes={dispatch_bytes} dispatch_peak={dispatch_peak} dispatch_final={dispatch_final} avx2_allocs={avx2_count} avx2_bytes={avx2_bytes} avx2_peak={avx2_peak} avx2_final={avx2_final} ssse3_allocs={ssse3_count} ssse3_bytes={ssse3_bytes} ssse3_peak={ssse3_peak} ssse3_final={ssse3_final} avx2_supported={} ssse3_supported={}",
        avx2_supported,
        ssse3_supported,
    );
}
