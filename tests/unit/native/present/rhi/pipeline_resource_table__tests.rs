// 引入被测资源表。
use super::RhiPipelineResourceTable;
// 引入统一错误码以验证失败分类。
use crate::core::Errc;
// 引入共享 pipeline 身份与语义。
use crate::platform::presentation::rhi::{PipelineBinding, PipelineKind};

// 验证插入时绑定语义来自资源表保存的 kind。
#[test]
fn insert_derives_binding_and_gets_resource() {
    // 创建空的测试资源表。
    let mut table = RhiPipelineResourceTable::new();
    // 由表登记资源并签发绑定。
    let binding = table.insert(PipelineKind::BlurPass, 7_u32);
    // 绑定 kind 必须等于插入时的共享语义。
    assert_eq!(binding.kind(), PipelineKind::BlurPass);
    // 正确绑定必须取得原始资源。
    assert_eq!(
        *table.get(binding).expect("matching binding should resolve"),
        7
    );
}

// 验证伪造 kind 被拒绝且资源仍可由正确绑定取得。
#[test]
fn rejects_forged_kind_and_preserves_resource() {
    // 创建并登记一个真实 blur pipeline。
    let mut table = RhiPipelineResourceTable::new();
    // 保存由资源表签发的正确身份。
    let binding = table.insert(PipelineKind::BlurPass, 9_u32);
    // 测试入口只用于构造错误 kind 的同句柄绑定。
    let forged = PipelineBinding::for_test(binding.handle(), PipelineKind::SolidMesh);
    // 错配必须返回共享 InvalidArgument。
    let error = table.get(forged).expect_err("forged kind must fail");
    // 错配统一归类为无效参数。
    assert_eq!(error.code(), Errc::InvalidArgument);
    // 错配查询不得破坏真实资源。
    assert_eq!(*table.get(binding).expect("resource must remain"), 9);
}

// 验证正确 take 后旧绑定失效且错误仍来自共享表。
#[test]
fn takes_matching_resource_and_rejects_stale_binding() {
    // 创建并登记一个真实 pipeline 资源。
    let mut table = RhiPipelineResourceTable::new();
    // 保存资源表签发的完整身份。
    let binding = table.insert(PipelineKind::SolidMesh, 11_u32);
    // 第一次 take 必须移交资源所有权。
    assert_eq!(table.take(binding).expect("take should succeed"), 11);
    // 资源移交后同一绑定必须稳定失败。
    let error = table.get(binding).expect_err("stale binding must fail");
    // 销毁后的身份统一归类为无效参数。
    assert_eq!(error.code(), Errc::InvalidArgument);
}

// 验证 owner 关闭时只从共享表逆序取得仍存活资源。
#[test]
fn drains_live_resources_in_reverse_order() {
    // 创建带三个资源的表，并在释放前销毁中间资源。
    let mut table = RhiPipelineResourceTable::new();
    // 第一个资源保持存活。
    table.insert(PipelineKind::SolidMesh, 1_u32);
    // 保存中间资源身份以模拟显式销毁。
    let removed = table.insert(PipelineKind::TexturedQuad, 2_u32);
    // 最后一个资源保持存活。
    table.insert(PipelineKind::BlurPass, 3_u32);
    // 显式销毁不得让 release 再次取得同一资源。
    assert_eq!(table.take(removed).expect("take should succeed"), 2);
    // release 只应按签发逆序取得仍存活资源。
    assert_eq!(table.drain_reverse().collect::<Vec<_>>(), vec![3, 1]);
}
