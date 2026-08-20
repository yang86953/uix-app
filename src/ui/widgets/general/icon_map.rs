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
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/general/icon_map__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
