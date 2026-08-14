// 导入多窗口控制器的线程安全所有权基元。
use std::sync::{Arc, Mutex};

// 导入应用、多窗口、主题与公开 View 组合能力。
use uix::prelude::*;

// 保存主演示初始主题状态文本。
const LIGHT_THEME_STATUS: &str = "当前主题：亮色";
// 保存暗色主题状态文本。
const DARK_THEME_STATUS: &str = "当前主题：暗色";
// 保存由 App System 跟随系统主题的策略文本。
const SYSTEM_THEME_STATUS: &str = "主题策略：跟随系统";
// 保存尚未打开子窗口的状态文本。
const WINDOW_READY_STATUS: &str = "主题联动窗口：等待打开";
// 保存子窗口已成功创建的状态文本。
const WINDOW_OPEN_STATUS: &str = "主题联动窗口：已打开";

// 持有应用级主题状态与 on_start 交付的 Application System 句柄。
pub(super) struct MultiWindowController {
    // 保存主窗与子窗共同观察的主题状态。
    theme_status: State<String>,
    // 保存主窗可观察的子窗口创建状态。
    window_status: State<String>,
    // 只保存 App System 交付的句柄，不持有窗口私有 session 或 surface。
    handle: Mutex<Option<AppHandle>>,
}

// 实现多窗口与跨窗主题的狭窄应用边界。
impl MultiWindowController {
    // 创建尚未取得 AppHandle 的控制器。
    pub(super) fn new(
        // 接收组合根是否启用系统主题跟随。
        follow_system_theme: bool,
    ) -> Self {
        // 返回全部状态都处于可观察初值的控制器。
        Self {
            // 主演示初始主题来自语言面 light 配置。
            theme_status: State::new(
                // 跟随模式不伪报某个可能立即变化的静态主题。
                if follow_system_theme {
                    SYSTEM_THEME_STATUS
                } else {
                    LIGHT_THEME_STATUS
                }
                // State 持有拥有所有权的策略文本。
                .to_string(),
            ),
            // 子窗口按需创建而不预建。
            window_status: State::new(WINDOW_READY_STATUS.to_string()),
            // AppHandle 只能稍后由 on_start 安装。
            handle: Mutex::new(None),
        }
    }

    // 克隆供声明视图订阅的主题状态句柄。
    pub(super) fn theme_status(&self) -> State<String> {
        // State 克隆共享同一唯一值槽。
        self.theme_status.clone()
    }

    // 克隆供声明视图订阅的窗口状态句柄。
    pub(super) fn window_status(&self) -> State<String> {
        // State 克隆共享同一唯一值槽。
        self.window_status.clone()
    }

    // 安装当前应用主窗口的公开句柄。
    pub(super) fn attach_handle(&self, handle: AppHandle) {
        // 中毒锁仍恢复唯一可选句柄值。
        let mut slot = self
            // 锁定短生命周期句柄槽。
            .handle
            // 等待当前安装或读取事务完成。
            .lock()
            // 保留中毒后的唯一内部值。
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // 用 App System 交付的句柄覆盖空槽。
        *slot = Some(handle);
    }

    // 通过公开 AppHandle 在应用范围切换主题。
    pub(super) fn set_dark(&self, dark: bool) {
        // 在锁内只克隆轻量公开句柄。
        let handle = self.handle();
        // 尚未进入 on_start 时不能伪造主题切换。
        let Some(handle) = handle else {
            // 保持当前可观察状态不变。
            return;
        };
        // 按调用意图选择完整公开主题值。
        let theme = if dark {
            // 使用内建暗色主题。
            Theme::antd_dark()
        } else {
            // 使用内建亮色主题。
            Theme::antd_light()
        };
        // 只有 Application System 接受主题后才更新共享状态。
        if handle.set_theme(theme).is_ok() {
            // 主窗与子窗订阅同一状态事实。
            self.theme_status.set(
                // 投影为稳定可访问文本。
                if dark {
                    DARK_THEME_STATUS
                } else {
                    LIGHT_THEME_STATUS
                }
                // State 拥有字符串所有权。
                .to_string(),
            );
        }
    }

    // 按需请求创建一个共享应用主题的次级窗口。
    pub(super) fn open_theme_window(self: &Arc<Self>) {
        // 在公开调用前取得独立 AppHandle 克隆。
        let handle = self.handle();
        // on_start 尚未交付句柄时报告确定状态。
        let Some(handle) = handle else {
            // 错误只投影到声明视图，不建立旁路日志状态。
            self.window_status
                .set("主题联动窗口：AppHandle 尚未就绪".to_string());
            // 缺少应用句柄不能继续创建窗口。
            return;
        };
        // 克隆控制器进入可重复执行的子窗口根工厂。
        let controller = self.clone();
        // 使用公开 WindowConfig 请求 Application System 创建次级窗口。
        let result = handle.open_window(
            // 窗口配置只拥有标题、尺寸、标题栏选择与 View 工厂。
            WindowConfig::new("UIX Theme Window", 640, 420, move || {
                // 每次协调继续使用同一控制器和单一 View 管线。
                build_theme_window(controller.clone())
            })
            // 子窗口使用由 View 提供的原生等价标题栏。
            .custom_title_bar(true),
        );
        // 把创建结果投影为主窗口可观察状态。
        match result {
            // 成功保留 Application System 对新窗口的所有权。
            Ok(_) => self.window_status.set(WINDOW_OPEN_STATUS.to_string()),
            // 失败保留类型化错误摘要。
            Err(error) => self
                // 更新唯一窗口状态槽。
                .window_status
                // 明确记录创建失败而不伪装窗口存在。
                .set(format!("主题联动窗口：创建失败：{error}")),
        }
    }

    // 克隆当前已经安装的公开应用句柄。
    fn handle(&self) -> Option<AppHandle> {
        // 锁定唯一句柄槽。
        self.handle
            // 等待短读取事务完成。
            .lock()
            // 保留中毒后的唯一内部值。
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            // 克隆公开句柄而不借用锁内值。
            .clone()
    }
}

// 构造次级主题窗口的单一公开 View 树。
fn build_theme_window(controller: Arc<MultiWindowController>) -> ViewNode {
    // 读取共享主题状态并登记当前子树的响应式依赖。
    let theme_status = controller.theme_status.get();
    // 克隆控制器供暗色按钮独占闭包。
    let dark_controller = controller.clone();
    // 克隆控制器供亮色按钮独占闭包。
    let light_controller = controller;
    // 构造支持拖动与关闭的自定义标题栏。
    let title_bar = row((
        // 非交互标题内容独占拖拽区域。
        window_drag_region(label("UIX Theme Window").padding(12.0))
            // 拖拽区域填满关闭控件之外的宽度。
            .flex_grow(1.0)
            // 提供稳定语义定位。
            .automation_id("theme-window-titlebar"),
        // 关闭控件只发布当前窗口动作，不借用主窗句柄。
        window_control_named(WindowControl::Close, "关闭主题联动窗口", label("×"))
            // 使用标准标题栏尺寸。
            .width(48.0)
            // 使用标准标题栏高度。
            .height(48.0)
            // 提供稳定 Agent 目标。
            .automation_id("theme-window-close"),
    ))
    // 固定标题栏高度。
    .height(48.0);
    // 构造共享主题的业务内容。
    let content = column((
        // 显示窗口用途标题。
        label("跨窗口主题联动").automation_id("theme-window-heading"),
        // 显示主窗与子窗共享的唯一主题状态。
        label(theme_status).automation_id("theme-window-theme-state"),
        // 排列两个明确的应用主题请求。
        row((
            // 通过主 AppHandle 广播暗色主题。
            button("暗色主题")
                // 事件只调用控制器的公开应用边界。
                .on_click_fn(move || dark_controller.set_dark(true))
                // 提供稳定 Agent 目标。
                .automation_id("theme-window-dark"),
            // 通过主 AppHandle 广播亮色主题。
            button("亮色主题")
                // 事件只调用控制器的公开应用边界。
                .on_click_fn(move || light_controller.set_dark(false))
                // 提供稳定 Agent 目标。
                .automation_id("theme-window-light"),
        ))
        // 设置主题按钮间距。
        .gap(12.0),
        // 说明跨窗口边界。
        label("业务 State 与应用主题跨窗共享；WindowSession、焦点与 swapchain 逐窗隔离。"),
    ))
    // 设置内容间距。
    .gap(18.0)
    // 设置内容内边距。
    .padding(28.0)
    // 填满标题栏之外的剩余空间。
    .flex_grow(1.0);
    // 使用当前主题的布局背景承载标题栏与内容。
    column((title_bar, content))
        // 采用运行时主题令牌而非固定颜色。
        .bg(ColorValue::neutral(NeutralRole::BgLayout))
        // 填满窗口客户区。
        .flex_grow(1.0)
}
