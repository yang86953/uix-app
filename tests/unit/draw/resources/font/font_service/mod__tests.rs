// 引入被测试的字体服务类型。
use super::FontService;
// 引入用于构造稳定槽位编号的字体句柄类型。
use crate::draw::FontHandle;

// 验证卸载不会让无效槽位继续持有族名和路径字符串。
#[test]
// 使用未被后端占用的句柄，隔离注册表元数据清理语义。
fn unload_releases_registry_metadata() {
    // 创建默认字体服务。
    let mut service = FontService::new();
    // 选择一个稳定但尚未使用的槽位编号。
    let handle = FontHandle::new(3);
    // 写入带路径的字体元数据，模拟路径字体注册。
    service.register_font(
        handle,
        "retained-family".to_owned(),
        Some("retained-path.ttf".to_owned()),
    );
    // 执行卸载，验证后端无效句柄也不会阻止元数据清理。
    service.unload_font(&handle);
    // 确认族名字符串已清空。
    assert!(service.registry[handle.0 as usize].face.family.is_empty());
    // 确认路径字符串所有权已释放。
    assert!(service.registry[handle.0 as usize].face.path.is_none());
}

// 验证字体族解析遵守顺序、大小写、通用族和稳定后备。
#[test]
fn font_family_resolves_first_registered_match_or_current_fallback() {
    // 创建默认字体服务。
    let mut service = FontService::new();
    // 注册两个稳定族名槽位。
    let preferred = FontHandle::new(3);
    // 为首选句柄登记混合大小写族名。
    service.register_font(preferred, "Segoe UI".to_owned(), None);
    // 注册低优先级候选。
    let later = FontHandle::new(4);
    // 为后备句柄登记另一族名。
    service.register_font(later, "Arial".to_owned(), None);
    // 定义调用方当前字体句柄。
    let fallback = FontHandle::new(9);
    // 缺失项后的小写名称必须命中首选注册字体。
    assert_eq!(
        service.resolve_font_families(["Missing", "segoe ui"], fallback),
        preferred
    );
    // 首项 generic family 必须立即保留当前字体，不继续选择后项。
    assert_eq!(
        service.resolve_font_families(["sans-serif", "Arial"], fallback),
        fallback
    );
    // 全部缺失时必须返回调用方当前字体。
    assert_eq!(
        service.resolve_font_families(["Missing"], fallback),
        fallback
    );
}
