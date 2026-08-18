//! Application 组合根的字体配置注入与启动组装。

// 引入应用 DI 容器以消费一次性字体配置。
use crate::app::application::di::Container;
// 引入统一 Result 与 resources System 的公开字体契约。
use crate::core::Result;
use crate::draw::{FontBundle, FontService};
// 引入平台系统信息窄接口，仅供未配置字体包的兼容策略使用。
use crate::native::capabilities::system::ISystemInfo;

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
fn initialize_text_font_service(
    // 可变借用组合根容器，以便安装后释放大字体配置字节。
    container: &mut Container,
    // 借用平台字体发现能力，但确定性分支不得调用它。
    system_info: &dyn ISystemInfo,
    // 返回唯一 FontService owner 与字体来源事实。
) -> Result<(FontService, bool)> {
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
pub(super) fn initialize_font_service(
    // 消费 App 组合根中的一次性字体配置。
    container: &mut Container,
    // 仅在兼容分支借用平台字体发现能力。
    system_info: &dyn ISystemInfo,
    // 返回后续所有窗口共享的唯一字体服务 owner。
) -> Result<FontService> {
    // 记录字体解析与服务组装耗时，不包含窗口或图形设备创建。
    let started_at = std::time::Instant::now();
    // 先建立正文与 fallback 的单一来源。
    let (mut font_service, deterministic) = initialize_text_font_service(container, system_info)?;
    // Icon 依赖仓库随包 Lucide PUA 字形，且不改变正文主字体句柄。
    crate::ui::widgets::icon::init_lucide_font(
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
mod tests {
    // 复用本模块私有组装函数。
    use super::*;
    // 引入系统信息返回值，构造不依赖 feature 的最小测试替身。
    use crate::native::capabilities::system::{MemoryInfo, OsInfo};
    // 使用内部可变计数器记录不可发生的平台字体查询。
    use std::cell::Cell;

    // 使用仓库内稳定字体 fixture 避免依赖宿主字体安装状态。
    const TEST_FONT: &[u8] = include_bytes!("../../../../assets/fonts/lucide.ttf");

    // 只记录字体发现调用，其余系统信息返回稳定占位值。
    struct CountingSystemInfo {
        // 调用次数必须保持为零，证明确定性分支未越过抽象边界。
        default_font_calls: Cell<usize>,
    }

    // 提供测试所需的最小平台系统信息契约实现。
    impl CountingSystemInfo {
        // 创建尚未发生任何平台字体查询的替身。
        fn new() -> Self {
            // 初始化唯一观察计数器。
            Self {
                // 起始状态必须为零。
                default_font_calls: Cell::new(0),
            }
        }
    }

    // 实现字体组装函数所依赖的窄平台契约。
    impl ISystemInfo for CountingSystemInfo {
        // 返回稳定测试系统身份。
        fn os_info(&self) -> Result<OsInfo> {
            // 字段值不参与当前测试判断。
            Ok(OsInfo {
                // 使用显式测试系统名称。
                name: "test".to_owned(),
                // 使用稳定版本占位值。
                version: "0".to_owned(),
                // 使用稳定构建占位值。
                build: "0".to_owned(),
                // 测试不依赖宿主位数。
                is_64bit: true,
            })
        }

        // 返回单核占位值。
        fn cpu_count(&self) -> Result<u32> {
            // 当前字体契约不会读取此值。
            Ok(1)
        }

        // 返回零内存占位值。
        fn memory_info(&self) -> Result<MemoryInfo> {
            // 当前字体契约不会读取这些字段。
            Ok(MemoryInfo {
                // 总内存保持稳定零值。
                total_bytes: 0,
                // 可用内存保持稳定零值。
                available_bytes: 0,
                // 工作集保持稳定零值。
                process_working_set: 0,
                // 私有内存保持稳定零值。
                process_private_bytes: 0,
            })
        }

        // 返回稳定主机名占位值。
        fn hostname(&self) -> Result<String> {
            // 当前字体契约不会读取此值。
            Ok("test".to_owned())
        }

        // 返回稳定用户名占位值。
        fn username(&self) -> Result<String> {
            // 当前字体契约不会读取此值。
            Ok("test".to_owned())
        }

        // 返回零运行时长占位值。
        fn up_time(&self) -> Result<u64> {
            // 当前字体契约不会读取此值。
            Ok(0)
        }

        // 记录任何越界的平台默认字体发现调用。
        fn default_font_paths(&self) -> Result<Vec<String>> {
            // 递增计数以让测试精确发现调用。
            self.default_font_calls
                // 使用饱和加法避免测试替身自身溢出。
                .set(self.default_font_calls.get().saturating_add(1));
            // 返回空列表，避免引入宿主字体状态。
            Ok(Vec::new())
        }
    }

    // 确认显式字体包完整隔离系统字体发现并在安装后释放配置。
    #[test]
    fn font_bundle_skips_platform_discovery_and_is_consumed() {
        // 创建只用于组合根配置的空容器。
        let mut container = Container::new();
        // 注册一份可由真实文本后端解析的确定性字体包。
        container.singleton(FontBundle::new("UIX Deterministic", TEST_FONT));
        // 创建记录 default_font_paths 调用次数的平台替身。
        let system_info = CountingSystemInfo::new();
        // 只组装正文字体，避免测试触碰全局图标字体 OnceLock。
        let result = initialize_text_font_service(&mut container, &system_info);
        // 有效随包字体必须成功建立 FontService。
        assert!(result.is_ok());
        // 从成功结果中取得服务和来源事实。
        let Ok((font_service, deterministic)) = result else {
            // 上方断言已经报告具体失败，此分支只满足无 unwrap 契约。
            return;
        };
        // 组装结果必须明确标记为确定性字体来源。
        assert!(deterministic);
        // 确定性分支不得调用任何平台默认字体查询。
        assert_eq!(system_info.default_font_calls.get(), 0);
        // 大字体配置在转交 FontService 后必须从多窗口容器移除。
        assert!(!container.has::<FontBundle>());
        // 主字体元数据必须来自应用字体包而非平台路径推断。
        assert_eq!(
            font_service.font_family(&font_service.loaded_font_handle),
            Some("UIX Deterministic")
        );
    }
}
