// 声明本文件只消费 uix-lang 公开宏的文件入口，不启动窗口、平台后端或事件循环。
#![allow(dead_code)]

// 引入公开宏与 ViewNode 包装辅助。
use uix_app::prelude::*;

// 定义 OnlyVisual 的 Rust 字段形状；`.uix` 是取值的唯一事实源。
struct OnlyVisual {
    value: f32,
}

// 把顶层 Visual 声明生成为同模块只读静态项与借用常量。
uix_items!("tests/fixtures/uix_lang/items/visual_only.uix");

// 展开 root.uix 及其 pages / shared 递归导入链为 ViewNode。
fn consume_recursive_import_view() {
    // 文件入口以 CARGO_MANIFEST_DIR 为基准解析递归导入闭包。
    let _view = embed(uix!("tests/fixtures/uix_lang/imports/root.uix"));
}

// 展开 `<App>` 根文档并取得尚未运行的现有 App builder。
fn consume_app_root_file_entry() {
    // builder 未调用 run() 前不创建窗口；Rust 侧继续持有生命周期扩展点。
    let _app =
        uix_app!("tests/fixtures/uix_lang/imports/app-root.uix").title("uix-lang 公开宏消费者测试");
}

// 展开 number prop 到 ProgressBar 的类型边界，确保调用方无需手工降为 f32。
fn consume_number_prop_progress() {
    // 自定义 Widget 的 number 公开为 f64，内层 ProgressBar 应自行适配 fraction 类型。
    let _view = uix!(
        r#"
        <Widget name="NumberProgress" props="value: number">
          <ProgressBar progress={value} />
        </Widget>
        <NumberProgress value="0.5" />
        "#
    );
}

// 确认 Visual 文件入口只生成一份静态视觉事实，借用常量直接指向它。
#[test]
fn visual_item_compiles_with_single_shared_address() {
    // 语言面声明的值就是运行期唯一事实。
    assert_eq!(ONLY_VISUAL.value, 1.0);
    // 借用常量不得产生第二份指针存储。
    assert!(std::ptr::eq(ONLY_VISUAL_REF, &ONLY_VISUAL));
}

// 让 Cargo 显式链接并执行两个文件入口消费者的完整编译产物。
#[test]
fn file_entries_compile_as_external_consumers() {
    // 先展开递归导入的视图根。
    consume_recursive_import_view();
    // 再展开 App 组合根；builder 到此为止，不进入事件循环。
    consume_app_root_file_entry();
    // 最后编译自定义 number prop 到 ProgressBar 的直接传递路径。
    consume_number_prop_progress();
}
