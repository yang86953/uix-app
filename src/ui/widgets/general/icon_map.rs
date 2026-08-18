//! Lucide 图标名称到 PUA 字符的只读查找组件。
//!
//! 重新生成需同步 assets/fonts/lucide.ttf 与三个有序分片。

// 按名称范围拆分生成表，使每个代码文件保持可治理规模。
mod chunk_a;
// 引入中段名称分片。
mod chunk_b;
// 引入末段名称分片。
mod chunk_c;

// 在有序分片中查找图标字符，不向调用方暴露存储布局。
pub(crate) fn find_icon(name: &str) -> Option<&'static str> {
    // 依次搜索三个互不重叠的名称范围。
    for entries in [
        // 搜索 A 到 E 分片。
        chunk_a::ICON_MAP_CHUNK,
        // 搜索 E 到 P 分片。
        chunk_b::ICON_MAP_CHUNK,
        // 搜索 P 到 Z 分片。
        chunk_c::ICON_MAP_CHUNK,
    ] {
        // 每个生成分片内部保持名称升序，可继续使用二分查找。
        if let Ok(index) = entries.binary_search_by_key(&name, |entry| entry.0) {
            // 返回与命中名称绑定的静态 PUA 字符。
            return Some(entries[index].1);
        }
    }
    // 三个分片都未命中时交给 Icon 组件执行诊断与兜底。
    None
}

// 验证拆分后的查找边界与未知名称行为。
#[cfg(test)]
mod tests {
    // 引入父组件的查找入口。
    use super::{chunk_a, chunk_b, chunk_c, find_icon};

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
}
