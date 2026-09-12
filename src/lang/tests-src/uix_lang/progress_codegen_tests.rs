// 引入文档解析与普通 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 把 UIX 源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析测试文档。
    let document = parse_document(source)?;
    // 生成根 View 并转成可断言字符串。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证动态 fraction、模式、圆形与公共属性完整映射。
#[test]
fn generates_progress_bar_contract() {
    // 生成动态 ProgressBar 完整契约。
    let tokens = generate(
        "<ProgressBar progress={upload_progress} indeterminate={waiting} type=\"circle\" width=\"96px\" automationId=\"upload-progress\" />",
    )
    // 合法契约必须生成成功。
    .expect("ProgressBar 应生成成功");
    // 运行时必须接收动态 fraction、显式模式与圆形选择。
    assert!(
        tokens.contains("ProgressBar :: new")
            && tokens.contains("progress ((upload_progress) as f32)")
            && tokens.contains("indeterminate_when (waiting)")
            && tokens.contains("circle ()"),
        "{tokens}"
    );
    // 公共尺寸与自动化属性必须继续应用到 View。
    assert!(
        tokens.contains("width (96.0)") && tokens.contains("automation_id"),
        "{tokens}"
    );
    // 生成器必须进入 ProgressBar 自己的 View/UIX 声明边界。
    assert!(!tokens.contains("ViewNode :: leaf"), "{tokens}");
}

// 验证默认值与静态 fraction 边界。
#[test]
fn generates_progress_bar_defaults_and_static_boundaries() {
    // 最小标签必须保留运行时默认值。
    let defaults = generate("<ProgressBar />")
        // 默认契约必须生成成功。
        .expect("ProgressBar 默认值应生成成功");
    // 缺省不需要额外 progress、模式或圆形调用。
    assert!(
        defaults.contains("ProgressBar :: new")
            && !defaults.contains("indeterminate_when")
            && !defaults.contains("circle ()"),
        "{defaults}"
    );
    // fraction 下界必须合法。
    let zero = generate("<ProgressBar progress=\"0\" />")
        // 零值必须生成成功。
        .expect("progress=0 应合法");
    // fraction 上界必须合法。
    let one = generate("<ProgressBar progress=\"1\" indeterminate=\"false\" />")
        // 一值必须生成成功。
        .expect("progress=1 应合法");
    // 两个边界都必须进入公开 progress 构建器。
    assert!(zero.contains("progress (0.0)") && one.contains("progress (1.0)"));
}

// 验证静态越界与非有限 fraction 在编译期失败。
#[test]
fn rejects_invalid_static_progress_values() {
    // 下界越界必须被拒绝。
    let below = generate("<ProgressBar progress=\"-0.1\" />")
        // 静态越界不能留到运行时。
        .expect_err("负 fraction 必须失败");
    // 诊断必须说明统一范围。
    assert!(below.message.contains("0.0..=1.0"), "{below:?}");
    // 上界越界必须被拒绝。
    let above = generate("<ProgressBar progress=\"1.1\" />")
        // 静态越界不能留到运行时。
        .expect_err("大于一的 fraction 必须失败");
    // 诊断必须说明统一范围。
    assert!(above.message.contains("0.0..=1.0"), "{above:?}");
    // NaN 字面量必须被拒绝。
    let non_finite = generate("<ProgressBar progress=\"NaN\" />")
        // 静态非有限值不能进入运行时。
        .expect_err("NaN fraction 必须失败");
    // 诊断必须说明有限性。
    assert!(non_finite.message.contains("有限"), "{non_finite:?}");
}

// 验证旧属性、非法形态与内容子树不会被静默降级。
#[test]
fn rejects_unregistered_progress_bar_contracts() {
    // 旧 percent 单位不得被隐式缩放。
    let percent = generate("<ProgressBar percent=\"50\" />")
        // 未登记属性必须失败。
        .expect_err("旧 percent 必须失败");
    // 诊断必须点名旧属性。
    assert!(percent.message.contains("percent"), "{percent:?}");
    // 未落地 status 不得只靠颜色伪装。
    let status = generate("<ProgressBar status=\"success\" />")
        // 未登记属性必须失败。
        .expect_err("status 必须失败");
    // 诊断必须点名 status。
    assert!(status.message.contains("status"), "{status:?}");
    // 未登记形态必须被拒绝。
    let kind = generate("<ProgressBar type=\"dashboard\" />")
        // dashboard 不在首版形态集合。
        .expect_err("dashboard 必须失败");
    // 诊断必须列出 line 与 circle。
    assert!(kind.message.contains("line") && kind.message.contains("circle"));
    // 数据内容子树不得被丢弃。
    let child = generate("<ProgressBar><Text>非法</Text></ProgressBar>")
        // 叶组件子树必须失败。
        .expect_err("ProgressBar 子节点必须失败");
    // 诊断必须明确叶组件边界。
    assert!(child.message.contains("不接受子节点"), "{child:?}");
}
