// 复用父模块私有状态与安装入口。
use super::*;
// 引入稳定字体句柄以检查失败回滚后的后端状态。
use crate::draw::FontHandle;

// 使用仓库已授权并可由真实文本后端解析的字体 fixture。
const TEST_FONT: &[u8] = include_bytes!("../../../../../../assets/fonts/lucide.ttf");

// 验证主字体与 fallback 按声明顺序发布到同一 FontService。
#[test]
fn font_bundle_installs_primary_and_ordered_fallbacks_from_identical_bytes() {
    // 构造包含一项回退的确定性字体包。
    let bundle = FontBundle::new("UIX Primary", TEST_FONT)
        // 使用同一有效字体数据隔离测试对宿主字体文件的依赖。
        .with_fallback("UIX Fallback", TEST_FONT);
    // 创建尚未装载任何字体的资源 owner。
    let mut service = FontService::new();
    // 执行唯一字体包安装事务。
    let result = service.install_font_bundle(&bundle);
    // 有效资产必须完整安装。
    assert!(result.is_ok());
    // 主字体句柄必须登记调用方声明的族名。
    assert_eq!(
        service.font_family(&service.loaded_font_handle),
        Some("UIX Primary")
    );
    // 显式回退链必须只包含声明的一项字体。
    assert_eq!(service.fallback_count(), 1);
    // 第一项回退句柄必须登记第二个族名。
    assert_eq!(
        service.font_family(&service.fallback_chain()[0]),
        Some("UIX Fallback")
    );
    // 内嵌字体不得伪造平台文件路径。
    assert!(service.font_path(&service.loaded_font_handle).is_none());
}

// 验证后续字体解析失败不会发布半条字体链。
#[test]
fn font_bundle_parse_failure_rolls_back_unpublished_backend_handles() {
    // 构造有效主字体后接无效回退字节的失败事务。
    let bundle = FontBundle::new("UIX Primary", TEST_FONT)
        // 非空载荷通过值约束，但必须由真实解析器返回 FormatError。
        .with_fallback("Broken Fallback", [0_u8, 1_u8]);
    // 创建全新 FontService 以观察安装前后状态。
    let mut service = FontService::new();
    // 尝试安装包含无效字体的完整包。
    let result = service.install_font_bundle(&bundle);
    // 解析错误必须作为 typed failure 返回。
    assert!(matches!(result, Err(error) if error.code() == Errc::FormatError));
    // 失败事务不得发布任何注册表槽位。
    assert_eq!(service.font_count(), 0);
    // 失败事务不得发布 fallback 链。
    assert_eq!(service.fallback_count(), 0);
    // 已成功解析但尚未发布的主字体句柄必须被后端卸载。
    assert!(!service.is_valid(&FontHandle::new(0)));
}
