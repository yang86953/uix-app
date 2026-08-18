//! FontService 对确定性字体包的安装事务。

// 引入统一错误分类和字体句柄。
use crate::core::{Errc, Error, Result};
// 引入 graphics/resources 拥有的字体包值。
use crate::draw::resources::font::FontBundle;

// 复用父模块唯一的字体服务 owner。
use super::FontService;

// 把字体包加载流程保持在 resources Module 内，不让 Application 操作字体句柄。
impl FontService {
    // 在全新 FontService 上安装主字体与有序 fallback；失败时不发布注册表状态。
    pub(crate) fn install_font_bundle(&mut self, bundle: &FontBundle) -> Result<()> {
        // 在触碰文本后端前完成全部纯值约束验证。
        bundle.validate()?;
        // 确定性启动配置只允许安装到尚未发布任何字体的服务。
        if !self.registry.is_empty() || !self.fallback_handles.is_empty() {
            // 拒绝覆盖 live 句柄，避免布局缓存引用被静默重定向。
            return Err(Error::new(
                // 重复安装违反 FontService 生命周期。
                Errc::InvalidState,
                // 诊断不泄漏字体内容或平台路径。
                "font bundle must be installed before any other font",
            ));
        }

        // 暂存尚未发布到 FontService 注册表的后端句柄与同源资产。
        let mut loaded = Vec::with_capacity(bundle.fallback_count().saturating_add(1));
        // 严格按主字体、fallback 的声明顺序进入同一个文本后端。
        for asset in bundle.assets() {
            // 后端取得共享只读所有权，字体包配置随后可从 Application 容器释放。
            match self.text_backend.load_font_shared(asset.shared_bytes()) {
                // 成功句柄在全部资产完成前保持未发布状态。
                Ok(handle) => loaded.push((handle, asset)),
                // 任一解析失败都回滚本次已经创建的后端字体资源。
                Err(error) => {
                    // 逐项释放尚未进入注册表的句柄。
                    for (handle, _) in loaded.drain(..) {
                        // 后端卸载保持句柄代际不可复用，不制造半安装回退链。
                        self.text_backend.unload_font(&handle);
                    }
                    // 保留字体解析器给出的 typed 失败分类。
                    return Err(error);
                }
            }
        }

        // 验证后字体包必有主字体；仍用 typed guard 防止未来迭代器契约漂移。
        let Some((primary_handle, primary_asset)) = loaded.first().copied() else {
            // 缺少主字体属于 resources 内部状态破坏。
            return Err(Error::new(
                // 使用 InvalidState 区分调用方字体字节错误。
                Errc::InvalidState,
                // 稳定诊断指向字体包安装事务。
                "validated font bundle produced no primary asset",
            ));
        };
        // 保存实际安装的主字体族，供默认样式与显式 family 解析共用。
        self.primary_family = primary_asset.family().to_owned();
        // 标记主字体来自显式应用配置，不允许后续系统字体覆盖。
        self.user_family_set = true;
        // 只有全部字体解析成功后才原子发布主字体句柄与元数据。
        self.install_primary_font(
            // 发布主字体的稳定后端句柄。
            primary_handle,
            // 注册调用方声明的主字体族名称。
            primary_asset.family().to_owned(),
            // 内嵌字体没有平台文件路径。
            None,
        );
        // 其余已验证句柄按原声明顺序发布为 fallback。
        for (handle, asset) in loaded.iter().skip(1).copied() {
            // 为显式 family 样式注册同一字体身份。
            self.register_font(handle, asset.family().to_owned(), None);
            // 直接追加已验证句柄，最后统一同步一次文本后端。
            self.fallback_handles.push(handle);
        }
        // 一次发布完整回退链，避免后端观察中间顺序。
        self.sync_fallback_fonts();
        // 安装事务已完整建立确定性字体事实。
        Ok(())
    }
}

// 验证字体包的发布顺序和失败回滚语义。
#[cfg(test)]
mod tests {
    // 复用父模块私有状态与安装入口。
    use super::*;
    // 引入稳定字体句柄以检查失败回滚后的后端状态。
    use crate::draw::FontHandle;

    // 使用仓库已授权并可由真实文本后端解析的字体 fixture。
    const TEST_FONT: &[u8] = include_bytes!("../../../../../assets/fonts/lucide.ttf");

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
}
