// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 将单个 UIX 元素生成成稳定令牌文本。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 生成公开 Rust API 调用并转成可断言文本。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Upload 全部首版属性、受控队列与类型化 Change 映射。
#[test]
fn generates_controlled_upload_contract() {
    // 生成覆盖静态过滤、动态布尔和整数、类型化事件与公共属性的 Upload。
    let snapshot = generate(
        // files 必须保留 State<Vec<UploadFile>> 句柄。
        r#"<Upload files={upload_files} accept=".png,*.JPG" multiple={allow_many} maxCount="6" maxSize={file_limit} drag @change="record_upload($event)" width="360px" automationId="upload" />"#,
    )
    // 合法首版契约必须生成成功。
    .expect("Upload 文档属性应映射到公开运行时 API");
    // 受控文件队列必须直接传入状态句柄。
    assert!(snapshot.contains("Upload :: new () . files (& (upload_files))"));
    // accept 字面量必须先验证再调用类型化运行时入口。
    assert!(snapshot.contains(". accept (\".png,*.JPG\") . expect"));
    // multiple 表达式必须传给单批多选配置。
    assert!(snapshot.contains(". multiple (allow_many)"));
    // maxCount 必须映射到 usize 运行时构造器。
    assert!(snapshot.contains(". max_count (6usize)"));
    // maxSize 表达式必须映射到单文件大小门禁。
    assert!(snapshot.contains(". max_size (file_limit)"));
    // drag 简写必须生成 true。
    assert!(snapshot.contains(". drag (true)"));
    // Change 必须在组件物化前接入类型化观察器。
    assert!(snapshot.contains(". on_change (move | __uix_upload_change |"));
    // 显式处理器名称必须保留在类型化闭包中。
    assert!(snapshot.contains("record_upload"));
    // $event 必须投影为卫生的 UploadChange 借用。
    assert!(snapshot.contains("__uix_upload_change"));
    // 公共宽度仍由统一 View 属性处理。
    assert!(snapshot.contains("width"));
    // 自动化标识必须保留。
    assert!(snapshot.contains(". automation_id (\"upload\")"));
}

// 验证 Upload 必需状态与所有权边界诊断。
#[test]
fn rejects_missing_or_uncontrolled_files_and_action() {
    // 缺失 files 必须失败。
    let missing = generate(r#"<Upload />"#)
        // 规划标签已经落地，失败原因应是必需属性。
        .expect_err("Upload 缺少 files 必须被拒绝");
    // 诊断必须点名 files。
    assert!(missing.message.contains("files"));

    // 字符串不能冒充状态句柄。
    let literal = generate(r#"<Upload files="demo" />"#)
        // 裸初值不能提供双向受控所有权。
        .expect_err("Upload files 字面量必须被拒绝");
    // 诊断必须点名公开状态类型。
    assert!(literal.message.contains("State<Vec<UploadFile>>"));

    // action 会把网络传输错误归属给 UI Widget。
    let action = generate(r#"<Upload files={upload_files} action="/api/upload" />"#)
        // 首版必须返回明确所有权诊断。
        .expect_err("Upload action 必须被拒绝");
    // 诊断必须点名 action 不属于契约。
    assert!(action.message.contains("action"));
}

// 验证 accept MIME、动态模式与非法扩展名都在编译期失败。
#[test]
fn rejects_unsupported_accept_shapes() {
    // MIME 模式当前没有统一内容推断能力。
    let mime = generate(r#"<Upload files={upload_files} accept="image/*" />"#)
        // MIME 字面量必须直接失败。
        .expect_err("Upload MIME accept 必须被拒绝");
    // 诊断必须说明 MIME 暂不支持。
    assert!(mime.suggestion.contains("MIME"));

    // 动态 UIX accept 无法在宏展开期验证。
    let dynamic = generate(r#"<Upload files={upload_files} accept={accept_pattern} />"#)
        // 动态模式应转到 Rust typed Result API。
        .expect_err("Upload 动态 accept 必须被拒绝");
    // 诊断必须要求字符串字面量。
    assert!(dynamic.message.contains("字符串字面量"));

    // 不带点的旧模糊语法不属于首版契约。
    let extension = generate(r#"<Upload files={upload_files} accept="png" />"#)
        // 非规范扩展名必须失败。
        .expect_err("Upload 不带点扩展名必须被拒绝");
    // 诊断必须列出 .ext 形状。
    assert!(extension.message.contains(".ext"));
}

// 验证 Upload 叶节点和未知属性拒绝路径。
#[test]
fn rejects_children_and_unknown_attributes() {
    // Upload 自行绘制文件列表，不能承载额外 UIX 子树。
    let child = generate(r#"<Upload files={upload_files}><Text>额外节点</Text></Upload>"#)
        // 子节点必须得到叶组件诊断。
        .expect_err("Upload 子节点必须被拒绝");
    // 诊断必须说明不接受子节点。
    assert!(child.message.contains("不接受子节点"));

    // showList 尚未进入首版文档契约。
    let unknown = generate(r#"<Upload files={upload_files} showList />"#)
        // 未登记属性不得静默忽略。
        .expect_err("Upload 未知属性必须被拒绝");
    // 诊断必须保留未知属性名。
    assert!(unknown.message.contains("showList"));
}
