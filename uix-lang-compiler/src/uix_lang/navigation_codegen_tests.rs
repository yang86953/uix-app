// 引入文档解析与普通 View 生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 把 UIX 源码生成 Rust 令牌字符串快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 解析测试文档。
    let document = parse_document(source)?;
    // 生成根 View 并转成可断言字符串。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证 Navigation 生成受控 Menu 组合、调用方版本与选择事件。
#[test]
fn generates_controlled_navigation_contract() {
    // 生成覆盖首版全部专有属性的侧栏组合。
    let tokens = generate(
        "<Navigation title=\"应用\" items={[MenuItem('首页', 'home')]} activeKey={active} openKeys={open} collapsed={collapsed} version={build_version} @select=\"on_select($event)\" width=\"240px\" />",
    )
    // 合法契约必须成功生成。
    .expect("Navigation 应生成成功");
    // 快照必须包含组合构造、三个句柄、版本与稳定选择处理器。
    assert!(
        tokens.contains("Navigation :: controlled")
            && tokens.contains("active")
            && tokens.contains("open")
            && tokens.contains("collapsed")
            && tokens.contains("build_version")
            && tokens.contains("on_change_fn")
            && tokens.contains("on_select"),
        "{tokens}"
    );
    // 公共尺寸属性必须继续应用到组合根 View。
    assert!(tokens.contains("width (240.0)"), "{tokens}");
}

// 验证缺失任一受控状态时返回定位诊断。
#[test]
fn rejects_navigation_without_required_state_contract() {
    // activeKey 缺失时不能建立第二套内部选择事实。
    let missing_active = generate(
        "<Navigation title=\"应用\" items={items} openKeys={open} collapsed={collapsed} />",
    )
    // 生成必须失败。
    .expect_err("缺失 activeKey 必须失败");
    // 诊断必须点名缺失活动状态。
    assert!(
        missing_active.message.contains("activeKey"),
        "{missing_active:?}"
    );
    // collapsed 缺失时不能把整栏折叠藏在外壳内部。
    let missing_collapsed =
        generate("<Navigation title=\"应用\" items={items} activeKey={active} openKeys={open} />")
            // 生成必须失败。
            .expect_err("缺失 collapsed 必须失败");
    // 诊断必须点名缺失折叠状态。
    assert!(
        missing_collapsed.message.contains("collapsed"),
        "{missing_collapsed:?}"
    );
}

// 验证 Navigation 拒绝第二套子树和未知专有属性。
#[test]
fn rejects_navigation_children_and_unknown_attributes() {
    // items 是菜单树唯一来源，UIX 子节点不得被丢弃。
    let child = generate(
        "<Navigation title=\"应用\" items={items} activeKey={active} openKeys={open} collapsed={collapsed}><Text>非法</Text></Navigation>",
    )
    // 生成必须失败。
    .expect_err("Navigation 子节点必须失败");
    // 诊断必须明确叶形状边界。
    assert!(child.message.contains("不接受子节点"), "{child:?}");
    // 旧 nav_version 属性不得静默成为 locale 回退。
    let unknown = generate(
        "<Navigation title=\"应用\" items={items} activeKey={active} openKeys={open} collapsed={collapsed} nav_version=\"v1\" />",
    )
    // 生成必须失败。
    .expect_err("未知 Navigation 属性必须失败");
    // 公共映射必须报告未知属性。
    assert!(unknown.message.contains("nav_version"), "{unknown:?}");
}
