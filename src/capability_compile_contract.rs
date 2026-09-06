//! capability feature 的公开编译边界合同。
//!
//! 本模块只在 `cfg(doctest)` 下参与 rustdoc 测试收集。每个产品 feature 都拥有
//! 一对互斥用例：启用时公开入口必须可编译，关闭时同一入口必须以指定错误码消失。
//! 因此默认能力、最小基础与非默认 opt-in 能力可以使用同一仓库测试入口验证，
//! 无需建立独立使用方 fixture、验证脚本或运行时开关。

// 最小基础不依赖任何可选 capability，负责证明无默认 feature 构建仍有稳定公开面。
/// # minimum-base enabled
///
/// ```
/// // 引入最小基础几何值与基础按钮组件。
/// use uix::prelude::{Button, Point};
/// // 基础几何构造不依赖任何可选 feature。
/// let _ = Point::new(1.0, 2.0);
/// // 基础组件类型在无默认 feature 构建中仍可从 prelude 解析。
/// let _ = std::any::TypeId::of::<Button>();
/// ```
pub struct MinimumBaseEnabledContract;

// d3d11 启用侧只验证公开选择入口，不绕过目标平台内部实现边界。
#[cfg(feature = "d3d11")]
/// # d3d11 enabled
///
/// ```
/// // 引入当前构建公开的图形后端选择枚举。
/// use uix::prelude::GraphicsBackend;
/// // feature 启用后 Direct3D 11 选择入口必须存在。
/// let _ = GraphicsBackend::Direct3D11;
/// ```
pub struct D3d11EnabledContract;

// d3d11 关闭侧要求对应枚举变体在编译期消失。
#[cfg(not(feature = "d3d11"))]
/// # d3d11 disabled
///
/// ```compile_fail,E0599
/// // 引入仍然存在的图形后端枚举类型。
/// use uix::prelude::GraphicsBackend;
/// // 未启用 d3d11 时不得保留 Direct3D 11 选择入口。
/// let _ = GraphicsBackend::Direct3D11;
/// ```
pub struct D3d11DisabledContract;

// d3d12 启用侧验证显式 opt-in 后的公开选择入口。
#[cfg(feature = "d3d12")]
/// # d3d12 enabled
///
/// ```
/// // 引入当前构建公开的图形后端选择枚举。
/// use uix::prelude::GraphicsBackend;
/// // feature 启用后 Direct3D 12 选择入口必须存在。
/// let _ = GraphicsBackend::Direct3D12;
/// ```
pub struct D3d12EnabledContract;

// d3d12 关闭侧要求对应枚举变体在编译期消失。
#[cfg(not(feature = "d3d12"))]
/// # d3d12 disabled
///
/// ```compile_fail,E0599
/// // 引入仍然存在的图形后端枚举类型。
/// use uix::prelude::GraphicsBackend;
/// // 未启用 d3d12 时不得保留 Direct3D 12 选择入口。
/// let _ = GraphicsBackend::Direct3D12;
/// ```
pub struct D3d12DisabledContract;

// opengles 启用侧验证 EGL/OpenGL ES 的公开选择入口。
#[cfg(feature = "opengles")]
/// # opengles enabled
///
/// ```
/// // 引入当前构建公开的图形后端选择枚举。
/// use uix::prelude::GraphicsBackend;
/// // feature 启用后 OpenGL ES 选择入口必须存在。
/// let _ = GraphicsBackend::OpenGlEs;
/// ```
pub struct OpenGlEsEnabledContract;

// opengles 关闭侧要求对应枚举变体在编译期消失。
#[cfg(not(feature = "opengles"))]
/// # opengles disabled
///
/// ```compile_fail,E0599
/// // 引入仍然存在的图形后端枚举类型。
/// use uix::prelude::GraphicsBackend;
/// // 未启用 opengles 时不得保留 OpenGL ES 选择入口。
/// let _ = GraphicsBackend::OpenGlEs;
/// ```
pub struct OpenGlEsDisabledContract;

// vulkan 启用侧验证显式 opt-in 后的公开选择入口。
#[cfg(feature = "vulkan")]
/// # vulkan enabled
///
/// ```
/// // 引入当前构建公开的图形后端选择枚举。
/// use uix::prelude::GraphicsBackend;
/// // feature 启用后 Vulkan 选择入口必须存在。
/// let _ = GraphicsBackend::Vulkan;
/// ```
pub struct VulkanEnabledContract;

// vulkan 关闭侧要求对应枚举变体在编译期消失。
#[cfg(not(feature = "vulkan"))]
/// # vulkan disabled
///
/// ```compile_fail,E0599
/// // 引入仍然存在的图形后端枚举类型。
/// use uix::prelude::GraphicsBackend;
/// // 未启用 vulkan 时不得保留 Vulkan 选择入口。
/// let _ = GraphicsBackend::Vulkan;
/// ```
pub struct VulkanDisabledContract;

// metal 启用侧验证显式 opt-in 后的公开选择入口。
#[cfg(feature = "metal")]
/// # metal enabled
///
/// ```
/// // 引入当前构建公开的图形后端选择枚举。
/// use uix::prelude::GraphicsBackend;
/// // feature 启用后 Metal 选择入口必须存在。
/// let _ = GraphicsBackend::Metal;
/// ```
pub struct MetalEnabledContract;

// metal 关闭侧要求对应枚举变体在编译期消失。
#[cfg(not(feature = "metal"))]
/// # metal disabled
///
/// ```compile_fail,E0599
/// // 引入仍然存在的图形后端枚举类型。
/// use uix::prelude::GraphicsBackend;
/// // 未启用 metal 时不得保留 Metal 选择入口。
/// let _ = GraphicsBackend::Metal;
/// ```
pub struct MetalDisabledContract;

// 图片编解码启用侧验证专属字节解码方法可见。
#[cfg(feature = "image-codecs")]
/// # image-codecs enabled
///
/// ```
/// // 引入始终存在的图片服务公开类型。
/// use uix::prelude::ImageService;
/// // 编译专属字节解码调用而不要求测试执行无效图片数据。
/// fn accepts_image_codecs(service: &ImageService) {
///     // feature 启用后字节解码入口必须存在。
///     let _ = service.load_from_bytes(&[]);
///     // 结束只用于编译入口的辅助函数。
/// }
/// ```
pub struct ImageCodecsEnabledContract;

// 图片编解码关闭侧要求专属方法从基础图片服务上消失。
#[cfg(not(feature = "image-codecs"))]
/// # image-codecs disabled
///
/// ```compile_fail,E0599
/// // 引入仍然支持原始 RGBA 槽位的基础图片服务。
/// use uix::prelude::ImageService;
/// // 编译调用以确认关闭 feature 后不存在解码入口。
/// fn rejects_image_codecs(service: &ImageService) {
///     // 未启用 image-codecs 时不得保留字节解码方法。
///     let _ = service.load_from_bytes(&[]);
///     // 结束预期编译失败的辅助函数。
/// }
/// ```
pub struct ImageCodecsDisabledContract;

// 二维码启用侧验证专属组件从 prelude 可见。
#[cfg(feature = "qrcode")]
/// # qrcode enabled
///
/// ```
/// // 引入二维码 capability 的公开组件类型。
/// use uix::prelude::QRCode;
/// // feature 启用后二维码组件类型必须可解析。
/// let _ = std::any::TypeId::of::<QRCode>();
/// ```
pub struct QrCodeEnabledContract;

// 二维码关闭侧要求专属 prelude 导入在编译期失败。
#[cfg(not(feature = "qrcode"))]
/// # qrcode disabled
///
/// ```compile_fail,E0432
/// // 未启用 qrcode 时公开门面不得导出二维码组件。
/// use uix::prelude::QRCode;
/// ```
pub struct QrCodeDisabledContract;

// 表单 pattern 启用侧验证专属 builder 方法可见。
#[cfg(feature = "form-pattern")]
/// # form-pattern enabled
///
/// ```
/// // 引入始终存在的基础表单组件。
/// use uix::prelude::Form;
/// // feature 启用后正则 pattern builder 方法必须可编译。
/// let _ = Form::new()
///     // 建立一个公开字段构建器。
///     .field("code", "Code")
///     // 添加 feature 专属的正则规则。
///     .validate_pattern("^[a-z]+$", "invalid");
/// ```
pub struct FormPatternEnabledContract;

// 表单 pattern 关闭侧要求专属 builder 方法消失而基础表单保留。
#[cfg(not(feature = "form-pattern"))]
/// # form-pattern disabled
///
/// ```compile_fail,E0599
/// // 引入仍然存在的基础表单组件。
/// use uix::prelude::Form;
/// // 先建立关闭 feature 后仍然合法的字段构建器。
/// let field = Form::new().field("code", "Code");
/// // 未启用 form-pattern 时不得保留正则规则方法。
/// let _ = field.validate_pattern("^[a-z]+$", "invalid");
/// ```
pub struct FormPatternDisabledContract;

// 富文本启用侧验证组件与内容模型入口。
#[cfg(feature = "rich-text")]
/// # rich-text enabled
///
/// ```
/// // 引入富文本 capability 的公开组件类型。
/// use uix::prelude::RichText;
/// // feature 启用后富文本组件类型必须可解析。
/// let _ = std::any::TypeId::of::<RichText>();
/// ```
pub struct RichTextEnabledContract;

// 富文本关闭侧要求专属 prelude 导入在编译期失败。
#[cfg(not(feature = "rich-text"))]
/// # rich-text disabled
///
/// ```compile_fail,E0432
/// // 未启用 rich-text 时公开门面不得导出富文本组件。
/// use uix::prelude::RichText;
/// ```
pub struct RichTextDisabledContract;

// 图表启用侧验证完整组件族的代表公开类型。
#[cfg(feature = "charts")]
/// # charts enabled
///
/// ```
/// // 引入图表 capability 的代表组件类型。
/// use uix::prelude::AreaChart;
/// // feature 启用后图表组件类型必须可解析。
/// let _ = std::any::TypeId::of::<AreaChart>();
/// ```
pub struct ChartsEnabledContract;

// 图表关闭侧要求代表组件从 prelude 消失。
#[cfg(not(feature = "charts"))]
/// # charts disabled
///
/// ```compile_fail,E0432
/// // 未启用 charts 时公开门面不得导出图表组件。
/// use uix::prelude::AreaChart;
/// ```
pub struct ChartsDisabledContract;

// 表格启用侧验证组件、构建器与数据模型的代表入口。
#[cfg(feature = "table")]
/// # table enabled
///
/// ```
/// // 引入表格 capability 的代表组件类型。
/// use uix::prelude::Table;
/// // feature 启用后表格组件类型必须可解析。
/// let _ = std::any::TypeId::of::<Table>();
/// ```
pub struct TableEnabledContract;

// 表格关闭侧要求代表组件从 prelude 消失。
#[cfg(not(feature = "table"))]
/// # table disabled
///
/// ```compile_fail,E0432
/// // 未启用 table 时公开门面不得导出表格组件。
/// use uix::prelude::Table;
/// ```
pub struct TableDisabledContract;

// 导航启用侧验证完整组件族的代表公开类型。
#[cfg(feature = "navigation")]
/// # navigation enabled
///
/// ```
/// // 引入导航 capability 的代表组件类型。
/// use uix::prelude::Tabs;
/// // feature 启用后导航组件类型必须可解析。
/// let _ = std::any::TypeId::of::<Tabs>();
/// ```
pub struct NavigationEnabledContract;

// 导航关闭侧要求代表组件从 prelude 消失。
#[cfg(not(feature = "navigation"))]
/// # navigation disabled
///
/// ```compile_fail,E0432
/// // 未启用 navigation 时公开门面不得导出导航组件。
/// use uix::prelude::Tabs;
/// ```
pub struct NavigationDisabledContract;

// 反馈启用侧验证完整组件族的代表公开类型。
#[cfg(feature = "feedback")]
/// # feedback enabled
///
/// ```
/// // 引入反馈 capability 的代表组件类型。
/// use uix::prelude::Alert;
/// // feature 启用后反馈组件类型必须可解析。
/// let _ = std::any::TypeId::of::<Alert>();
/// ```
pub struct FeedbackEnabledContract;

// 反馈关闭侧要求代表组件从 prelude 消失。
#[cfg(not(feature = "feedback"))]
/// # feedback disabled
///
/// ```compile_fail,E0432
/// // 未启用 feedback 时公开门面不得导出反馈组件。
/// use uix::prelude::Alert;
/// ```
pub struct FeedbackDisabledContract;

// 树组件启用侧验证展示树与树选择器共享的 capability 门面。
#[cfg(feature = "tree-widgets")]
/// # tree-widgets enabled
///
/// ```
/// // 引入树组件 capability 的代表公开类型。
/// use uix::prelude::Tree;
/// // feature 启用后展示树组件类型必须可解析。
/// let _ = std::any::TypeId::of::<Tree>();
/// ```
pub struct TreeWidgetsEnabledContract;

// 树组件关闭侧要求代表组件从 prelude 消失。
#[cfg(not(feature = "tree-widgets"))]
/// # tree-widgets disabled
///
/// ```compile_fail,E0432
/// // 未启用 tree-widgets 时公开门面不得导出树组件。
/// use uix::prelude::Tree;
/// ```
pub struct TreeWidgetsDisabledContract;

// 终端启用侧验证终端组件 capability 的代表公开类型。
#[cfg(feature = "terminal")]
/// # terminal enabled
///
/// ```
/// // 引入终端 capability 的代表公开类型。
/// use uix::prelude::Terminal;
/// // feature 启用后终端组件类型必须可解析。
/// let _ = std::any::TypeId::of::<Terminal>();
/// ```
pub struct TerminalEnabledContract;

// 终端关闭侧要求代表组件从 prelude 消失。
#[cfg(not(feature = "terminal"))]
/// # terminal disabled
///
/// ```compile_fail,E0432
/// // 未启用 terminal 时公开门面不得导出终端组件。
/// use uix::prelude::Terminal;
/// ```
pub struct TerminalDisabledContract;

// settings-serde 启用侧验证结构化设置 codec 的公开方法。
#[cfg(feature = "settings-serde")]
/// # settings-serde enabled
///
/// ```
/// // 引入始终存在的设置服务公开类型。
/// use uix::prelude::SettingsService;
/// // 建立不执行文件 IO 的设置服务。
/// let settings = SettingsService::new();
/// // feature 启用后结构化序列化入口必须可编译。
/// let _ = settings.set_struct("answer", &42_u32);
/// ```
pub struct SettingsSerdeEnabledContract;

// settings-serde 关闭侧要求结构化方法消失而字符串设置服务保留。
#[cfg(not(feature = "settings-serde"))]
/// # settings-serde disabled
///
/// ```compile_fail,E0599
/// // 引入关闭 codec 后仍然存在的字符串设置服务。
/// use uix::prelude::SettingsService;
/// // 建立基础设置服务。
/// let settings = SettingsService::new();
/// // 未启用 settings-serde 时不得保留结构化序列化入口。
/// let _ = settings.set_struct("answer", &42_u32);
/// ```
pub struct SettingsSerdeDisabledContract;

// agent-control 启用侧验证高权限控制面的公开确认入口。
#[cfg(feature = "agent-control")]
/// # agent-control enabled
///
/// ```
/// // 引入 Agent 确认入口所属句柄与稳定窗口身份。
/// use uix::prelude::{AppHandle, WindowId};
/// // 只编译公开方法调用，不建立真实 Agent transport。
/// fn accepts_agent_control(handle: &AppHandle, window_id: WindowId) {
///     // feature 启用后确认决定入口必须存在。
///     let _ = handle.resolve_agent_confirmation(window_id, 1, false);
///     // 结束只用于编译入口的辅助函数。
/// }
/// ```
pub struct AgentControlEnabledContract;

// agent-control 关闭侧要求高权限方法从基础 AppHandle 上消失。
#[cfg(not(feature = "agent-control"))]
/// # agent-control disabled
///
/// ```compile_fail,E0599
/// // 引入关闭 Agent transport 后仍然存在的应用句柄与窗口身份。
/// use uix::prelude::{AppHandle, WindowId};
/// // 编译调用以确认关闭 feature 后不存在高权限入口。
/// fn rejects_agent_control(handle: &AppHandle, window_id: WindowId) {
///     // 未启用 agent-control 时不得保留确认决定方法。
///     let _ = handle.resolve_agent_confirmation(window_id, 1, false);
///     // 结束预期编译失败的辅助函数。
/// }
/// ```
pub struct AgentControlDisabledContract;

// extensions 启用侧验证软件动态扩展宿主的公开入口。
#[cfg(feature = "extensions")]
/// # extensions enabled
///
/// ```
/// // 引入运行时扩展宿主与跨边界值。
/// use uix::app::extensions::{ExtensionHost, ExtensionValue};
/// // feature 启用后扩展宿主必须可显式构造并接受类型化调用入口。
/// let host = ExtensionHost::new();
/// let _ = host.list();
/// let _ = ExtensionValue::Null;
/// ```
pub struct ExtensionsEnabledContract;

// extensions 关闭侧要求扩展宿主从公开面消失。
#[cfg(not(feature = "extensions"))]
/// # extensions disabled
///
/// ```compile_fail,E0432
/// // 未启用 extensions 时不得保留解释器与扩展宿主公开面。
/// use uix::app::extensions::ExtensionHost;
/// // 结束预期编译失败的引用。
/// let _ = ExtensionHost::new;
/// ```
pub struct ExtensionsDisabledContract;
