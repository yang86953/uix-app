//! 自 src/lib.rs 迁出的文档测试样例；原检查语义保留，仅源仓 RUSTDOCFLAGS
//! --cfg uix_repo_doc_contract 下的 cargo test --doc 收集，不进发布包。


/// 迁出的文档样例 1（原 src/lib.rs:24）：
/// ```compile_fail
/// use uix_app::prelude::*;
/// let _: ViewNode = uix!("tests/fixtures/uix_lang/does_not_exist.uix");
/// ```
pub struct DocExample1;


/// 迁出的文档样例 2（原 src/lib.rs:29）：
/// ```compile_fail
/// use uix_app::prelude::*;
/// let _: ViewNode = uix!("<Column>\n<Text>x</Column>");
/// ```
pub struct DocExample2;


/// 迁出的文档样例 3（原 src/lib.rs:34）：
/// ```compile_fail
/// use uix_app::prelude::*;
/// let _: ViewNode = uix!("<Mystery />");
/// ```
pub struct DocExample3;


/// 迁出的文档样例 4（原 src/lib.rs:39）：
/// ```compile_fail
/// use uix_app::prelude::*;
/// let _: ViewNode = uix!(r#"<Input type="search" />"#);
/// ```
pub struct DocExample4;


/// 迁出的文档样例 5（原 src/lib.rs:51）：
/// ```compile_fail
/// // native 根不可达（SPI、实现与厂商对象一律不允许外部引用）。
/// use uix_app::native::factory::create_platform;
/// ```
pub struct DocExample5;


/// 迁出的文档样例 6（原 src/lib.rs:56）：
/// ```compile_fail
/// // 平台 backend SPI 不可达。
/// use uix_app::native::backends::windows::clipboard::WindowsClipboard;
/// ```
pub struct DocExample6;


/// 迁出的文档样例 7（原 src/lib.rs:61）：
/// ```compile_fail
/// // 图形 context / 厂商对象不可达（SMC-02 后位于 presentation Module）。
/// use uix_app::native::presentation::graphics::d3d11::D3d11Context;
/// ```
pub struct DocExample7;


/// 迁出的文档样例 8（原 src/lib.rs:66）：
/// ```compile_fail
/// // 平台共享事件源实现不可达（SMC-02 后位于 windowing Module）。
/// use uix_app::native::windowing::shared::event_loop::OsEventSource;
/// ```
pub struct DocExample8;


/// 迁出的文档样例 9（原 src/lib.rs:71）：
/// ```compile_fail
/// // native 私有 Module 边界不可达：capabilities / windowing / presentation。
/// use uix_app::native::capabilities;
/// ```
pub struct DocExample9;


/// 迁出的文档样例 10（原 src/lib.rs:76）：
/// ```compile_fail
/// use uix_app::platform::presentation::GraphicsRecipeContext;
/// ```
pub struct DocExample10;


/// 迁出的文档样例 11（原 src/lib.rs:80）：
/// ```compile_fail
/// // platform 私有 adapter 不可达（feature 启用与否都不允许外部引用）。
/// use uix_app::platform::adapters::transport::AgentEndpoint;
/// ```
pub struct DocExample11;


/// 迁出的文档样例 12（原 src/lib.rs:85）：
/// ```compile_fail
/// // 测试平台聚合不可达：native 测试替身已随平台 facade 重构移除。
/// use uix_app::native::test_harness::FakePlatform;
/// ```
pub struct DocExample12;


/// 迁出的文档样例 13（原 src/lib.rs:90）：
/// ```compile_fail
/// // 旧运行保障入口已被删除：`core::log` 不得复活。
/// use uix_app::core::log::info_fn;
/// ```
pub struct DocExample13;


/// 迁出的文档样例 14（原 src/lib.rs:95）：
/// ```compile_fail
/// // 旧运行保障入口已被删除：`core::diagnostic` 不得复活。
/// use uix_app::core::diagnostic::collector::Collector;
/// ```
pub struct DocExample14;


/// 迁出的文档样例 15（原 src/lib.rs:100）：
/// ```compile_fail
/// // 旧平台工厂入口不得通过 prelude 恢复。
/// use uix_app::prelude::create_platform;
/// ```
pub struct DocExample15;


/// 迁出的文档样例 16（原 src/lib.rs:107）：
/// ```compile_fail
/// // ui 旧运行时平铺路径归 component Module（SMC-04）。
/// use uix_app::ui::core::widget::WidgetTree;
/// ```
pub struct DocExample16;


/// 迁出的文档样例 17（原 src/lib.rs:112）：
/// ```compile_fail
/// // foundation 目录已拆解：state 归 reactive、style 归 theme（SMC-04）。
/// use uix_app::ui::foundation::state::State;
/// ```
pub struct DocExample17;


/// 迁出的文档样例 18（原 src/lib.rs:117）：
/// ```compile_fail
/// // traits 目录已拆解：组件契约归 component（SMC-04）。
/// use uix_app::ui::traits::Widget;
/// ```
pub struct DocExample18;


/// 迁出的文档样例 19（原 src/lib.rs:122）：
/// ```compile_fail
/// // 表单组件归 form Module，不得经 widgets::input 路径恢复（SMC-04）。
/// use uix_app::ui::widgets::input::form::Form;
/// ```
pub struct DocExample19;


/// 迁出的文档样例 20（原 src/lib.rs:127）：
/// ```compile_fail
/// // view DSL 组合子归 widgets（view 不再构建具体组件，SMC-04）。
/// use uix_app::ui::view::combinators::button;
/// ```
pub struct DocExample20;


/// 迁出的文档样例 21（原 src/lib.rs:132）：
/// ```compile_fail
/// // 语义快照/覆盖归 accessibility Module（SMC-04）。
/// use uix_app::ui::semantic_snapshot::SemanticTarget;
/// ```
pub struct DocExample21;


/// 迁出的文档样例 22（原 src/lib.rs:137）：
/// ```compile_fail
/// // ViewAdapter 是 System 私有边界粘合（SMC-04）。
/// use uix_app::ui::adapter::ViewAdapter;
/// ```
pub struct DocExample22;


/// 迁出的文档样例 23（原 src/lib.rs:142）：
/// ```compile_fail
/// // 树-组件语义访问点为 System 私有边界（SMC-04）。
/// use uix_app::ui::tree_widget_hooks::modal_was_present;
/// ```
pub struct DocExample23;


/// 迁出的文档样例 24（原 src/lib.rs:147）：
/// ```compile_fail
/// // 旧 ui 根级模块路径不得复活：window_chrome 归 widgets（SMC-04）。
/// use uix_app::ui::window_chrome::WindowControl;
/// ```
pub struct DocExample24;


/// 迁出的文档样例 25（原 src/lib.rs:154）：
/// ```compile_fail
/// // data settings Module 为私有边界（SMC-05）。
/// use uix_app::data::settings::SettingsService;
/// ```
pub struct DocExample25;


/// 迁出的文档样例 26（原 src/lib.rs:161）：
/// ```compile_fail
/// // 旧 shell 目录已拆解为 application / event_loop / window / agent（SMC-06）。
/// use uix_app::app::shell::application::App;
/// ```
pub struct DocExample26;


/// 迁出的文档样例 27（原 src/lib.rs:166）：
/// ```compile_fail
/// // 组合根 session_runtime 是 System 私有边界（SMC-06）。
/// use uix_app::app::session_runtime::AppRuntime;
/// ```
pub struct DocExample27;


/// 迁出的文档样例 28（原 src/lib.rs:171）：
/// ```compile_fail
/// // 主循环调度队列是 System 私有边界（SMC-06）。
/// use uix_app::app::queues::app_timer::TimerHandle;
/// ```
pub struct DocExample28;


/// 迁出的文档样例 29（原 src/lib.rs:176）：
/// ```compile_fail
/// // 每窗口语义状态是 System 私有边界（SMC-06）。
/// use uix_app::app::window_semantics::WindowSemanticState;
/// ```
pub struct DocExample29;


/// 迁出的文档样例 30（原 src/lib.rs:181）：
/// ```compile_fail
/// // agent Module 为私有边界，自动化 IPC 不经公开面暴露（SMC-06）。
/// use uix_app::app::agent::agent_transport::AgentTransport;
/// ```
pub struct DocExample30;


/// 迁出的文档样例 31（原 src/lib.rs:186）：
/// ```compile_fail
/// // frame_scheduler 已归 window Module，旧 event_loop 路径不得复活（SMC-06）。
/// use uix_app::app::event_loop::frame_scheduler::FrameScheduler;
/// ```
pub struct DocExample31;


/// 迁出的文档样例 32（原 src/lib.rs:191）：
/// ```compile_fail
/// // window 内部驱动实现不可达（SMC-06）。
/// use uix_app::app::window::window_driver::WindowDriver;
/// ```
pub struct DocExample32;
