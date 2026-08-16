// 引入宏生成代码承诺使用的公开 prelude。
use crate::prelude::*;

// 验证 Pagination 显示策略、跳转入口与尺寸选项只依赖公开运行时契约。
#[test]
fn pagination_configuration_compiles_against_public_uix_api() {
    // 创建调用方拥有的当前页状态。
    let current_page = State::new(1_usize);
    // 创建调用方拥有的每页条数状态。
    let page_size = State::new(20_usize);
    // 声明由 Rust 类型系统核对的动态总数显示策略。
    let show_total = true;
    // 声明由 Rust 类型系统核对的动态尺寸切换策略。
    let allow_size_change = true;
    // 声明拥有 usize 元素的尺寸选项集合。
    let page_size_options = [10_usize, 20_usize, 50_usize];
    // 展开完整 Pagination 配置形状。
    let _pagination: ViewNode = crate::uix!(
        // 使用布尔表达式、简写与拥有型可迭代尺寸集合。
        r#"<Pagination current={current_page} pageSize={page_size} total="120" pageSizeOptions={page_size_options} showTotal={show_total} showSizeChanger={allow_size_change} simple="false" showJumper />"#
    );
}
