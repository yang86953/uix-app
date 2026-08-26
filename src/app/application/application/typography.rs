//! Application 组合根的字体配置注入与启动组装。

// 引入应用 DI 容器以消费一次性字体配置。
use crate::app::application::di::Container;
// 引入统一 Result 与 resources System 的公开字体契约。
use crate::core::Result;
use crate::draw::{FontBundle, FontService};
// 引入平台字体发现窄接口，仅供未配置字体包的兼容策略使用。
use crate::platform::services::FontSystemInfo;

// 复用父模块的 Application 组合根类型。
use super::App;

// 为 App builder 提供显式确定性字体配置入口。
impl App {
    /// 配置随应用分发的主字体与有序 fallback，并禁用平台字体发现。
    pub fn font_bundle(mut self, bundle: FontBundle) -> Self {
        // 字体包作为组装期配置进入应用容器，首次 GUI 启动时被消费并释放。
        self.container.singleton(bundle);
        // 返回可继续链式配置的同一 App builder。
        self
    }
}

// 创建只包含正文与 fallback 的 FontService，返回是否使用了确定性字体包。
fn initialize_text_font_service<T>(
    // 可变借用组合根容器，以便安装后释放大字体配置字节。
    container: &mut Container,
    // 借用平台字体发现能力，但确定性分支不得调用它。
    system_info: &T,
    // 返回唯一 FontService owner 与字体来源事实。
) -> Result<(FontService, bool)>
where
    T: FontSystemInfo + ?Sized,
{
    // 轻量克隆只复制字体资产 Arc，安装完成后原配置即可释放。
    let configured_bundle = container.resolve_clone::<FontBundle>();
    // 一次性配置不得继续留在多窗口 DI 容器中占用第二份大字体引用。
    container.remove::<FontBundle>();
    // 创建尚未发布任何字体句柄的 resources owner。
    let mut font_service = FontService::new();
    // 显式字体包与平台兼容策略是互斥分支。
    if let Some(bundle) = configured_bundle {
        // 任一资产无效都原样返回 typed failure，禁止静默选择系统字体。
        font_service.install_font_bundle(&bundle)?;
        // 返回已经建立确定性字体身份的服务。
        return Ok((font_service, true));
    }
    // 未配置字体包时保留既有 OS-native 字体发现兼容行为。
    font_service.load_default_system_font(14.0, system_info);
    // 标记当前文本外观允许随平台字体策略变化。
    Ok((font_service, false))
}

// 在任何窗口创建前组装完整应用字体服务。
pub(super) fn initialize_font_service<T>(
    // 消费 App 组合根中的一次性字体配置。
    container: &mut Container,
    // 仅在兼容分支借用平台字体发现能力。
    system_info: &T,
    // 返回后续所有窗口共享的唯一字体服务 owner。
) -> Result<FontService>
where
    T: FontSystemInfo + ?Sized,
{
    // 记录字体解析与服务组装耗时，不包含窗口或图形设备创建。
    let started_at = std::time::Instant::now();
    // 先建立正文与 fallback 的单一来源。
    let (mut font_service, deterministic) = initialize_text_font_service(container, system_info)?;
    // Icon 依赖仓库随包 Lucide PUA 字形，且不改变正文主字体句柄。
    crate::ui::widgets::icon::init_static_lucide_font(
        // 所有平台编译同一份已纳入仓库的图标字体数据。
        include_bytes!("../../../../assets/fonts/lucide.ttf"),
        // 图标字体进入同一个 FontService 生命周期。
        &mut font_service,
    );
    // 记录字体来源，便于跨平台验收区分确定性模式与 OS-native 模式。
    tracing::info!(
        // 使用稳定布尔字段供结构化日志与测试环境采集。
        deterministic_font_bundle = deterministic,
        // 记录完整组装耗时。
        elapsed_ms = started_at.elapsed().as_millis(),
        // 保留简短的人类可读事件名称。
        "startup fonts ready before window creation"
    );
    // 交付所有窗口共享的唯一 FontService owner。
    Ok(font_service)
}

// 验证确定性分支不会触碰平台字体发现。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/app/application/application/typography__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
