// 引入临时 fixture 的文件系统能力。
use std::fs;
// 引入确定路径类型。
use std::path::PathBuf;
// 引入唯一目录时间来源。
use std::time::{SystemTime, UNIX_EPOCH};

// 引入被测导入 resolver 与内嵌拒绝 Gate。
use crate::uix_import::{reject_inline_imports, resolve_file};
// 引入纯文档生成与解析入口。
use crate::uix_lang::{Declaration, generate_document_app, parse_document};
// 引入完整 Compiler System 入口与公开诊断契约。
use crate::{CompileTarget, DiagnosticPhase, check_file, compile_file};

// 唯一拥有一组导入测试文件。
struct Fixture {
    // 保存本测试精确临时根。
    root: PathBuf,
}

// 提供安全创建与精确路径写入。
impl Fixture {
    // 创建本测试独占目录。
    fn new(label: &str) -> Self {
        // 使用高精度时间避免并行碰撞。
        let unique = SystemTime::now()
            // 取得 Unix epoch 后时长。
            .duration_since(UNIX_EPOCH)
            // 系统时钟错误应直接失败。
            .expect("system clock after Unix epoch")
            // 使用纳秒后缀。
            .as_nanos();
        // 组合进程、标签与时间形成精确目录。
        let root = std::env::temp_dir().join(format!(
            // 使用固定前缀便于诊断。
            "uix-import-{label}-{}-{unique}",
            // 隔离并行测试进程。
            std::process::id()
        ));
        // 创建唯一根目录。
        fs::create_dir(&root).expect("create import fixture root");
        // 返回目录所有者。
        Self { root }
    }

    // 写入一个相对 fixture 文件。
    fn write(&self, relative: &str, source: &str) -> PathBuf {
        // 解析精确目标。
        let path = self.root.join(relative);
        // 创建目标父目录。
        fs::create_dir_all(path.parent().expect("fixture file parent"))
            // 目录失败时保留目标诊断。
            .expect("create import fixture parent");
        // 写入 UTF-8 UIX 源码。
        fs::write(&path, source).expect("write import fixture file");
        // 返回目标路径。
        path
    }
}

// 测试结束后只删除 fixture 自己拥有的精确目录。
impl Drop for Fixture {
    // 执行幂等清理。
    fn drop(&mut self) {
        // 临时测试目录可在失败时安全回收。
        let _ = fs::remove_dir_all(&self.root);
    }
}

// 验证嵌套相对导入、依赖闭包与全部文件追踪。
#[test]
fn nested_imports_resolve_relative_to_each_importer_and_track_every_file() {
    // 创建独占文件图。
    let fixture = Fixture::new("nested");
    // 写入底层 helper 组件库。
    fixture.write(
        // 放在共享子目录。
        "shared/helper.uix",
        // 显式导出 Helper 并提供合法根。
        "@export('Helper')\n<Widget name=\"Helper\"><Text>共享</Text></Widget>\n<Helper />",
    );
    // 写入中层页面并相对自身导入 helper。
    fixture.write(
        // 放在 pages 子目录。
        "pages/page.uix",
        // Page 依赖 Helper，根只用于单文件语法完整性。
        "@import('../shared/helper.uix', 'Helper')\n@export('Page')\n<Widget name=\"Page\"><Helper /></Widget>\n<Page />",
    );
    // 写入 App 根文件。
    let root = fixture.write(
        // 保存根入口。
        "main.uix",
        // 导入全部 Page exports 并实例化。
        "@import('./pages/page.uix')\n<App><Page /></App>",
    );
    // 解析完整多文件编译单元。
    let resolved = resolve_file(&root).expect("nested imports should resolve");
    // 三个真实文件都必须进入 rustc 依赖追踪。
    assert_eq!(resolved.tracked_files.len(), 3);
    // 同一次读取必须形成根优先三节点与两条直接导入边。
    assert_eq!(resolved.source_graph.files().len(), 3);
    assert_eq!(resolved.source_graph.imports().len(), 2);
    assert_eq!(
        resolved
            .source_graph
            .file(resolved.source_graph.root())
            .expect("源码图根节点必须存在")
            .path,
        root.to_string_lossy()
    );
    assert_ne!(resolved.source_graph.dependency_hash(), 0);
    // 组件依赖闭包必须同时包含 Page 与 Helper。
    let components = component_names(&resolved.document.declarations);
    // 核对确定组件集合。
    assert_eq!(components, vec!["Helper", "Page"]);
    // 合并文档必须能直接进入既有纯 App codegen。
    let generated = generate_document_app(&resolved.document)
        // 生成失败表示导入没有形成完整文档。
        .expect("resolved app should generate")
        // 规范化令牌用于最小事实断言。
        .to_string();
    // 生成物必须展开 helper 内容。
    assert!(generated.contains("共享"));
}

// 验证具名导入只带入所选公开组件。
#[test]
fn named_import_selects_one_export() {
    // 创建独占文件图。
    let fixture = Fixture::new("named");
    // 写入两个互不依赖的公开组件。
    fixture.write(
        // 保存组件库。
        "library.uix",
        // 同时导出 Alpha 与 Beta。
        "@export('Alpha', 'Beta')\n<Widget name=\"Alpha\"><Text>A</Text></Widget>\n<Widget name=\"Beta\"><Text>B</Text></Widget>\n<Alpha />",
    );
    // 根文件只选择 Beta。
    let root = fixture.write(
        // 保存根入口。
        "main.uix",
        // 使用具名 import。
        "@import('./library.uix', 'Beta')\n<App><Beta /></App>",
    );
    // 解析选择性导入。
    let resolved = resolve_file(&root).expect("named import should resolve");
    // 只应出现 Beta。
    assert_eq!(
        component_names(&resolved.document.declarations),
        vec!["Beta"]
    );
}

// 验证被选组件依赖的 Visual 支持声明进入 Items 生成并保留真实来源。
#[test]
fn imported_visual_declarations_generate_items_from_their_source_file() {
    let fixture = Fixture::new("visual-items");
    let library = fixture.write(
        "library.uix",
        "@export('IconView')\n<Visual name=\"ICON_VISUAL\" type=\"IconVisual\" size={24.0} />\n<Widget name=\"IconView\"><Text>图标</Text></Widget>\n<IconView />",
    );
    let root = fixture.write(
        "main.uix",
        "@import('./library.uix', 'IconView')\n<IconView />",
    );

    let output =
        compile_file(&root, CompileTarget::Items).expect("被导入 Visual 必须进入模块项目标生成");
    let tokens = output.tokens.to_string();
    assert!(
        tokens.contains("pub (crate) const ICON_VISUAL : IconVisual"),
        "{tokens}"
    );
    let visual = output
        .ir
        .declarations()
        .iter()
        .find(|declaration| declaration.name == "ICON_VISUAL")
        .expect("TypedUiIr 必须保留 Visual 声明");
    assert_eq!(
        visual.span.source_id,
        crate::source_graph::SourceId::from_source_name(&library.to_string_lossy())
    );
}

// 验证没有 export 或导入未公开组件时立即失败。
#[test]
fn imports_require_explicit_existing_exports() {
    // 创建独占文件图。
    let fixture = Fixture::new("exports");
    // 写入没有 export 的组件文件。
    fixture.write(
        // 保存私有组件库。
        "private.uix",
        // 只定义组件。
        "<Widget name=\"Private\"><Text>P</Text></Widget>\n<Private />",
    );
    // 根文件尝试导入私有文件。
    let no_export = fixture.write(
        // 保存第一入口。
        "no_export.uix",
        // 导入全部导出。
        "@import('./private.uix')\n<App><Private /></App>",
    );
    // 缺少 export 必须被 resolver 拒绝。
    let error = resolve_file(&no_export).expect_err("file without exports must fail");
    // 诊断必须明确边界原因。
    assert!(error.diagnostic.message.contains("没有 @export"));

    // 写入只导出 Public 的组件库。
    fixture.write(
        // 保存公开组件库。
        "public.uix",
        // 明确只公开 Public。
        "@export('Public')\n<Widget name=\"Public\"><Text>P</Text></Widget>\n<Public />",
    );
    // 根文件请求不存在的 export。
    let missing = fixture.write(
        // 保存第二入口。
        "missing_export.uix",
        // 使用错误具名选择器。
        "@import('./public.uix', 'Missing')\n<App><Text>x</Text></App>",
    );
    // 未公开名称必须失败。
    let error = resolve_file(&missing).expect_err("missing export must fail");
    // 诊断必须包含具体名称。
    assert!(error.diagnostic.message.contains("Missing"));
}

// 验证跨文件组件冲突不会被 BTreeMap 静默覆盖。
#[test]
fn duplicate_component_names_are_rejected() {
    // 创建独占文件图。
    let fixture = Fixture::new("duplicate");
    // 写入导出 Page 的组件库。
    fixture.write(
        // 保存组件库。
        "page.uix",
        // 定义公开 Page。
        "@export('Page')\n<Widget name=\"Page\"><Text>导入</Text></Widget>\n<Page />",
    );
    // 根文件同时本地定义同名 Page。
    let root = fixture.write(
        // 保存冲突入口。
        "main.uix",
        // import 后的本地同名声明必须失败。
        "@import('./page.uix')\n<Widget name=\"Page\"><Text>本地</Text></Widget>\n<App><Page /></App>",
    );
    // 解析必须返回重复组件诊断。
    let error = resolve_file(&root).expect_err("duplicate components must fail");
    // 诊断必须指出名称与重复事实。
    assert!(error.diagnostic.message.contains("组件 Page 重复声明"));
}

// 验证循环依赖在形成回边的 import 处被拒绝。
#[test]
fn circular_imports_are_rejected_with_dependency_chain() {
    // 创建独占文件图。
    let fixture = Fixture::new("cycle");
    // 写入 A 到 B 的边。
    let root = fixture.write(
        // 保存 A 文件。
        "a.uix",
        // A 导入 B 并导出 AView。
        "@import('./b.uix')\n@export('AView')\n<Widget name=\"AView\"><Text>A</Text></Widget>\n<AView />",
    );
    // 写入 B 到 A 的回边。
    fixture.write(
        // 保存 B 文件。
        "b.uix",
        // B 导入 A 并导出 BView。
        "@import('./a.uix')\n@export('BView')\n<Widget name=\"BView\"><Text>B</Text></Widget>\n<BView />",
    );
    // 解析循环图必须失败。
    let error = resolve_file(&root).expect_err("cycle must fail");
    // 诊断必须明确循环并展示闭合箭头。
    assert!(error.diagnostic.message.contains("形成循环"));
    // 依赖链必须可审计。
    assert!(error.diagnostic.message.contains("->"));
}

// 验证内嵌入口不会继续静默忽略 import。
#[test]
fn inline_documents_reject_imports_without_a_file_base() {
    // 解析仍保持纯语法能力。
    let document = parse_document("@import('./page.uix')\n<App><Text>x</Text></App>")
        // 语法本身合法。
        .expect("inline import syntax should parse");
    // 文件边界 Gate 必须拒绝该文档。
    let error = reject_inline_imports(&document).expect_err("inline import must fail");
    // 诊断必须提供文件入口修复动作。
    assert!(error.message.contains("内嵌 UIX 源码不能使用 @import"));
    // 建议必须同时覆盖三个公开宏。
    assert!(error.suggestion.contains("uix_app!"));
}

// 验证生成期错误保留被导入组件的真实文件与精确跨度。
#[test]
fn imported_codegen_diagnostics_keep_their_source_identity() {
    // 创建包含一个根文件和一个组件库的独占文件图。
    let fixture = Fixture::new("codegen-source");
    // 保存可直接核对诊断字节范围的组件源码。
    let helper_source = "@export('Helper')\n<Widget name=\"Helper\">\n<Text mystery=\"x\">共享</Text>\n</Widget>\n<Helper />";
    // 未登记属性会通过解析与类型化 Gate，并在 Rust lowering 阶段失败。
    let helper = fixture.write("helper.uix", helper_source);
    // 根文件只负责导入并调用组件，不能成为组件体错误的归属。
    let root = fixture.write(
        "main.uix",
        "@import('./helper.uix', 'Helper')\n<App><Helper /></App>",
    );
    // AOT 与 check 必须走同一 lowering Gate 并返回同一来源事实。
    let errors = [
        compile_file(&root, CompileTarget::App).expect_err("AOT 必须拒绝未登记属性"),
        check_file(&root, CompileTarget::App).expect_err("check 必须拒绝未登记属性"),
    ];
    // 属性跨度应精确覆盖被导入文件中的非法声明。
    let expected_start = helper_source
        .find("mystery")
        .expect("fixture 必须包含非法属性");
    let expected_end = expected_start + "mystery=\"x\"".len();
    for error in errors {
        // 保持稳定阶段与代码，便于 CLI、LSP 和宏入口统一消费。
        assert_eq!(error.phase, DiagnosticPhase::Semantic);
        assert_eq!(error.code, "UIX2000");
        // 文件名与身份都必须指向 helper，而不是根 main.uix。
        assert_eq!(error.source_name, helper.to_string_lossy());
        assert_eq!(
            error.source_id,
            crate::source_graph::SourceId::from_source_name(&helper.to_string_lossy())
        );
        // 行列与字节范围必须能直接高亮真实非法属性。
        assert_eq!(error.start, expected_start);
        assert_eq!(error.end, expected_end);
        assert_eq!((error.line, error.column), (3, 7));
        assert_eq!(&helper_source[error.start..error.end], "mystery=\"x\"");
    }
}

// 收集文档声明中的组件名称并排序。
fn component_names(declarations: &[Declaration]) -> Vec<&str> {
    // 创建拥有声明顺序的名称集合。
    let mut names = declarations
        // 遍历全部声明。
        .iter()
        // 只提取 Component。
        .filter_map(|declaration| match declaration {
            // 返回组件名借用。
            Declaration::Widget(component) => Some(component.name.as_str()),
            // 其他声明不计入。
            _ => None,
        })
        // 收集名称。
        .collect::<Vec<_>>();
    // 按名称排序便于确定断言。
    names.sort_unstable();
    // 返回排序结果。
    names
}
