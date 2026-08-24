// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 ImageGroup 集合、初始索引、变化事件与公共属性的完整生成契约。
#[test]
// 声明完整 ImageGroup 生成测试。
fn generates_image_group_contract() {
    // 生成覆盖拥有型集合、动态索引、Change 载荷和自动化标识的画廊。
    let snapshot = generate(
        // 使用 image-codecs capability 下的公开 images 构建器。
        r#"<ImageGroup images={gallery_images} startIndex={initial_index} @change="record_index($event)" width="320px" automationId="gallery" />"#,
    )
    // 合法画廊必须成功生成。
    .expect("文档属性应映射到公开 ImageGroup API");
    // 图片路径集合必须从调用方可迭代表达式取得所有权。
    assert!(snapshot.contains("IntoIterator :: into_iter ((gallery_images) . clone ())"));
    // 集合元素必须统一收集为运行时拥有的 Vec<String>。
    assert!(snapshot.contains("Vec < :: std :: string :: String >"));
    // 画廊必须从公开构造器和 images 构建器开始。
    assert!(snapshot.contains("ImageGroup :: new () . images"));
    // 动态初始索引必须进入运行时 start_index 构建器。
    assert!(snapshot.contains("start_index (initial_index)"));
    // 变化观察器必须复用公开 View Change 注册入口。
    assert!(snapshot.contains("on_change_fn"));
    // 变化处理器名称必须进入生成闭包。
    assert!(snapshot.contains("record_index"));
    // $event 必须投影为卫生的现有索引文本借用。
    assert!(snapshot.contains("__uix_image_group_change"));
    // 公共尺寸与自动化标识仍由统一属性层消费。
    assert!(snapshot.contains("width") && snapshot.contains("automation_id"));
    // 生成器必须进入 ImageGroup 自己的 UIX 根，不能直接构造运行时叶节点。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
}

// 验证 ImageGroup 最小声明保留运行时零索引与无观察器默认值。
#[test]
// 声明 ImageGroup 默认值测试。
fn generates_image_group_runtime_defaults() {
    // 生成只包含必需集合表达式的最小画廊。
    let snapshot = generate(r#"<ImageGroup images={gallery_images} />"#)
        // 最小合法画廊必须成功生成。
        .expect("缺省 ImageGroup 应保留运行时默认索引");
    // 图片集合必须进入公开 images 构建器。
    assert!(snapshot.contains("images"));
    // 省略初始索引时必须保留运行时 0 默认值。
    assert!(!snapshot.contains("start_index"));
    // 省略 Change 事件时不得注册额外观察器。
    assert!(!snapshot.contains("on_change_fn"));
    // 最小声明也必须经过 ImageGroup 的 UIX 根。
    assert!(!snapshot.contains("ViewNode :: leaf"), "{snapshot}");
}

// 验证 ImageGroup 必需集合、叶节点与静态索引边界。
#[test]
// 声明 ImageGroup 基础拒绝测试。
fn rejects_missing_literal_children_and_invalid_index() {
    // 缺失 images 时没有画廊内容来源。
    let missing = generate(r#"<ImageGroup />"#)
        // 缺失必需属性必须失败。
        .expect_err("缺少 images 必须被拒绝");
    // 诊断必须点名 images。
    assert!(missing.message.contains("images"));
    // 单个字符串字面量不能伪装为可迭代路径集合。
    let literal = generate(r#"<ImageGroup images="a.png" />"#)
        // 字面量集合必须失败。
        .expect_err("字面量 ImageGroup images 必须被拒绝");
    // 诊断必须说明可迭代表达式要求。
    assert!(literal.message.contains("可迭代字符串表达式"));
    // ImageGroup 自身绘制画廊，不能接受任意 View 子树。
    let child =
        generate(r#"<ImageGroup images={gallery_images}><Image src="a.png" /></ImageGroup>"#)
            // 嵌套元素必须失败。
            .expect_err("ImageGroup 子节点必须被拒绝");
    // 诊断必须点明叶组件边界。
    assert!(child.message.contains("不接受子节点"));
    // 负数不能映射到公开 usize 初始索引。
    let negative = generate(r#"<ImageGroup images={gallery_images} startIndex="-1" />"#)
        // 负静态索引必须失败。
        .expect_err("负 ImageGroup startIndex 必须被拒绝");
    // 诊断必须说明 usize 约束。
    assert!(negative.message.contains("usize"));
    // 小数不能映射到公开 usize 初始索引。
    let fractional = generate(r#"<ImageGroup images={gallery_images} startIndex="1.5" />"#)
        // 小数静态索引必须失败。
        .expect_err("小数 ImageGroup startIndex 必须被拒绝");
    // 诊断必须说明 usize 约束。
    assert!(fractional.message.contains("usize"));
}

// 验证 ImageGroup 未登记属性不能穿过公共映射。
#[test]
// 声明 ImageGroup 属性拒绝测试。
fn rejects_unknown_image_group_attribute() {
    // 未登记受控 current 属性不能复制运行时所有权。
    let unknown = generate(r#"<ImageGroup images={gallery_images} current={current_index} />"#)
        // 未登记属性必须失败。
        .expect_err("未知 ImageGroup 属性必须被拒绝");
    // 诊断必须包含具体属性名。
    assert!(unknown.message.contains("current"));
}
