//! FontService 对确定性字体包的安装事务。

// 引入统一错误分类和字体句柄。
use crate::core::{Errc, Error, Result};
// 引入 graphics/resources 拥有的字体包值。
use crate::draw::resources::font::FontBundle;
use crate::draw::resources::font::bundle::FontAssetSource;

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
            // 静态资产保持借用，动态资产克隆 Arc；两条路径都不复制已拥有的数据。
            let load_result = match asset.source() {
                FontAssetSource::Static(bytes) => self.text_backend.load_font_static(bytes),
                FontAssetSource::Shared(bytes) => self
                    .text_backend
                    .load_font_shared(std::sync::Arc::clone(bytes)),
            };
            match load_result {
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
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/draw/resources/font/font_service/bundle_install__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
