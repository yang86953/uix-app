// 引入上传组件的公开队列契约。
use super::{
    Upload, UploadChange, UploadFile, UploadFileId, UploadRejectReason, UploadStatus,
    UploadUpdateError,
};
// 引入响应式状态作为受控队列唯一真值。
use crate::ui::reactive::state::State;
// 引入测试观察器的单线程可变记录容器。
use std::cell::RefCell;
// 引入测试观察器的共享所有权句柄。
use std::rc::Rc;

// 构造带显式稳定身份的测试文件项。
fn file(id: &str, name: &str) -> UploadFile {
    // 测试身份均为非空静态文本。
    let id = UploadFileId::new(id).expect("测试 UploadFileId 应合法");
    // 返回等待处理的最小文件项。
    UploadFile::with_id(id, name, 8)
}

// 验证稳定身份与 accept 首版语法都返回类型化格式结果。
#[test]
fn upload_validates_stable_ids_and_accept_patterns() {
    // 空白身份不得进入公开文件项。
    assert!(UploadFileId::new("   ").is_err());
    // 扩展名列表比较语义允许大小写混合文本。
    assert!(Upload::validate_accept(".png, *.JPG").is_ok());
    // 全量通配继续合法。
    assert!(Upload::validate_accept("*/*").is_ok());
    // MIME 模式必须返回类型化错误而非静默不匹配。
    let mime = Upload::validate_accept("image/*").expect_err("MIME 模式应失败");
    // 错误应保留原始模式供应用诊断。
    assert_eq!(mime.pattern(), "image/*");
    // 不带点的旧模糊扩展名语法不属于首版契约。
    assert!(Upload::validate_accept("png").is_err());
}

// 验证受控删除先写回 State，再发布带稳定身份的不可变事实。
#[test]
fn upload_controlled_remove_commits_state_before_typed_change() {
    // 两个重名项只能通过稳定身份区分。
    let first = file("first", "同名,文件.png");
    // 保存第二项用于最终状态断言。
    let second = file("second", "同名,文件.png");
    // 调用方状态是唯一队列真值。
    let state = State::new(vec![first.clone(), second.clone()]);
    // 记录处理器观察到的类型化事实。
    let changes = Rc::new(RefCell::new(Vec::<UploadChange>::new()));
    // 克隆状态句柄供处理器验证发布顺序。
    let observed_state = state.clone();
    // 克隆记录容器供处理器写入。
    let observed_changes = changes.clone();
    // 构造受控组件并登记类型化观察器。
    let mut upload = Upload::new().files(&state).on_change(move |change| {
        // 回调执行时 State 必须已经等于事实中的更新后快照。
        assert_eq!(observed_state.get(), change.files());
        // 保存完整事实供事件种类断言。
        observed_changes.borrow_mut().push(change.clone());
    });

    // 按稳定身份移除首个重名项。
    let removed = upload
        // 使用 id 避免名称和索引歧义。
        .remove_file_by_id(&first.id)
        // 目标身份必须存在。
        .expect("首个稳定身份应可移除");
    // 返回项必须保持被移除的稳定身份。
    assert_eq!(removed.id, first.id);
    // 外部唯一真值只保留第二项。
    assert_eq!(state.get(), vec![second.clone()]);
    // 只发布一次结构变化事实。
    let recorded = changes.borrow();
    // 本次应为 Removed 事实并携带首项身份。
    assert!(matches!(
        // 借用唯一事实。
        recorded.as_slice(),
        // 稳定身份集合必须精确且无名称编码。
        [UploadChange::Removed { ids, files }]
            // 身份与更新后快照均需匹配。
            if ids == &vec![first.id.clone()] && files == &vec![second]
    ));
}

// 验证 max_count 不截断外部状态，进度与结果始终按稳定身份写回。
#[test]
fn upload_preserves_external_queue_and_updates_by_id() {
    // 构造超过后续新增上限的外部队列。
    let first = file("progress", "a.png");
    // 构造第二个外部完成项。
    let second = file("result", "b.png");
    // 外部状态拥有两个现有项。
    let state = State::new(vec![first.clone(), second.clone()]);
    // max_count 为一时仍必须完整采用外部队列。
    let mut upload = Upload::new().max_count(1).files(&state);
    // 外部队列不得被静默截断。
    assert_eq!(upload.file_count(), 2);

    // 应用按稳定身份写回有限进度。
    upload
        // 更新首项而非索引位置。
        .update_progress(&first.id, 0.5)
        // 有效进度应成功。
        .expect("有限进度应可写回");
    // 读取唯一状态快照。
    let progress_files = state.get();
    // 首项进入上传中并保存进度。
    assert_eq!(progress_files[0].status, UploadStatus::Uploading);
    // 进度应保持调用方有限值。
    assert_eq!(progress_files[0].progress, 0.5);
    // 第二项保持原状态。
    assert_eq!(progress_files[1], second);

    // 非有限进度必须显式失败。
    let error = upload
        // 传入 NaN 验证错误路径。
        .update_progress(&first.id, f32::NAN)
        // 不能静默归零。
        .expect_err("非有限进度应失败");
    // 错误种类必须稳定。
    assert_eq!(error, UploadUpdateError::NonFiniteProgress);
    // 失败不得覆盖先前有效进度。
    assert_eq!(state.get()[0].progress, 0.5);

    // 应用按第二项稳定身份写回完成结果。
    upload
        // 不依赖首项是否存在或排序。
        .complete_file(&second.id, true)
        // 有效身份应成功。
        .expect("完成结果应可写回");
    // 第二项必须进入完成状态。
    assert_eq!(state.get()[1].status, UploadStatus::Done);
}

// 验证用户批次只执行一次状态提交，拒绝项不产生额外事实。
#[test]
fn upload_queues_one_file_per_single_batch_and_reports_rejections() {
    // 为测试文件创建进程唯一目录名。
    let directory = std::env::temp_dir().join(format!(
        // 使用自动身份避免并行测试冲突。
        "uix-upload-test-{}",
        // 稳定身份文本可安全用于目录名。
        UploadFileId::generated().as_str()
    ));
    // 创建精确测试目录。
    std::fs::create_dir(&directory).expect("应可创建 Upload 测试目录");
    // 构造第一个受支持文件路径。
    let first_path = directory.join("first.PNG");
    // 构造同批第二个文件路径。
    let second_path = directory.join("second.png");
    // 写入第一个最小文件。
    std::fs::write(&first_path, b"first").expect("应可写入第一个测试文件");
    // 写入第二个最小文件。
    std::fs::write(&second_path, b"second").expect("应可写入第二个测试文件");

    // 调用方初始队列为空。
    let state = State::new(Vec::<UploadFile>::new());
    // 记录批量新增事实。
    let changes = Rc::new(RefCell::new(Vec::<UploadChange>::new()));
    // 克隆事实容器供回调使用。
    let observed = changes.clone();
    // 构造单选批次和静态扩展名过滤。
    let mut upload = Upload::new()
        // 绑定唯一队列状态。
        .files(&state)
        // 配置合法扩展名模式。
        .accept(".png")
        // 静态合法配置不应失败。
        .expect(".png accept 应合法")
        // 单批仅接收一个候选。
        .multiple(false)
        // 记录类型化新增事实。
        .on_change(move |change| observed.borrow_mut().push(change.clone()));
    // 以平台路径文本提交两个候选。
    let result = upload.queue_files(&[
        // 第一个候选使用真实路径。
        first_path.to_string_lossy().into_owned(),
        // 第二个候选同属本批。
        second_path.to_string_lossy().into_owned(),
    ]);
    // single batch 只加入第一个候选。
    assert_eq!(result.added_ids.len(), 1);
    // 未处理的第二候选不属于拒绝，因为它超出单批选择范围。
    assert!(result.rejected.is_empty());
    // 外部唯一状态只包含一个文件。
    assert_eq!(state.get().len(), 1);
    // 整批只发布一次新增事实。
    assert_eq!(changes.borrow().len(), 1);

    // 不匹配扩展名应返回类型化拒绝且不改变状态。
    let rejected = upload.try_add_file(directory.join("missing.txt").to_string_lossy().as_ref());
    // 扩展名门禁先于文件元数据读取。
    assert!(matches!(
        // 检查公开拒绝结果。
        rejected,
        // 必须明确为扩展名不匹配。
        Err(super::UploadRejection {
            reason: UploadRejectReason::ExtensionMismatch,
            ..
        })
    ));
    // 拒绝项不得发布额外事实。
    assert_eq!(changes.borrow().len(), 1);

    // 清理精确测试文件目录及其中两个已知文件。
    std::fs::remove_dir_all(&directory).expect("应可清理 Upload 测试目录");
}
