// 引入父组件的查找入口。
use super::{chunk_a, chunk_b, chunk_c, find_icon, icon_chunk};

// 三个分片的首尾附近名称都必须保持可查找。
#[test]
fn finds_icons_across_all_generated_chunks() {
    // 验证首分片中的名称。
    assert!(find_icon("a-arrow-down").is_some());
    // 验证中分片中的名称。
    assert!(find_icon("pause").is_some());
    // 验证末分片中的名称。
    assert!(find_icon("zoom-out").is_some());
    // 未知名称不得伪造图标字符。
    assert_eq!(find_icon("uix-icon-does-not-exist"), None);
}

// 拆分不得丢失条目或破坏任一分片的二分查找顺序。
#[test]
fn generated_chunks_preserve_count_and_global_order() {
    // 按运行时搜索顺序取得三个静态分片。
    let chunks = [
        // 取得首分片。
        chunk_a::ICON_MAP_CHUNK,
        // 取得中分片。
        chunk_b::ICON_MAP_CHUNK,
        // 取得末分片。
        chunk_c::ICON_MAP_CHUNK,
    ];
    // 原始生成表共有 2025 个名称映射。
    assert_eq!(chunks.iter().map(|chunk| chunk.len()).sum::<usize>(), 2025);
    // 将相邻条目的有序性扩展到分片边界。
    let globally_sorted = chunks
        // 依次读取所有分片。
        .iter()
        // 展平为原始全表顺序。
        .flat_map(|chunk| chunk.iter())
        // 只提取图标名称。
        .map(|entry| entry.0)
        // 保存为测试专用名称序列。
        .collect::<Vec<_>>();
    // 每个相邻名称都必须严格递增，避免重复或边界错位。
    assert!(globally_sorted.windows(2).all(|pair| pair[0] < pair[1]));
}

// 名称必须先定位到唯一分片，避免绘制热路径重复搜索无关分片。
#[test]
fn lookup_routes_to_one_ordered_chunk() {
    // 路由边界必须与生成分片的真实首项一致。
    assert_eq!(chunk_b::ICON_MAP_CHUNK[0].0, "ethernet-port");
    assert_eq!(chunk_c::ICON_MAP_CHUNK[0].0, "pen-line");
    // 首、中、末段名称分别只选择对应静态切片。
    assert!(std::ptr::eq(icon_chunk("eraser"), chunk_a::ICON_MAP_CHUNK));
    assert!(std::ptr::eq(icon_chunk("home"), chunk_b::ICON_MAP_CHUNK));
    assert!(std::ptr::eq(
        icon_chunk("zoom-out"),
        chunk_c::ICON_MAP_CHUNK
    ));
}
