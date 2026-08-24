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
    // 先按生成表的稳定首项边界定位唯一分片，避免每次绘制最多执行三次二分查找。
    let entries = icon_chunk(name);
    // 目标分片内部保持名称升序，只执行一次二分查找。
    entries
        .binary_search_by_key(&name, |entry| entry.0)
        // 命中时直接返回与名称绑定的静态 PUA 字符。
        .ok()
        .map(|index| entries[index].1)
}

// 根据三个全局有序分片的首项选择唯一查找区间。
fn icon_chunk(name: &str) -> &'static [(&'static str, &'static str)] {
    // 中段从 ethernet-port 开始，末段从 pen-line 开始。
    if name < "ethernet-port" {
        chunk_a::ICON_MAP_CHUNK
    } else if name < "pen-line" {
        chunk_b::ICON_MAP_CHUNK
    } else {
        chunk_c::ICON_MAP_CHUNK
    }
}

// 验证拆分后的查找边界与未知名称行为。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/general/icon_map__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
