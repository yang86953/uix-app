// 引入当前图片资源服务。
use super::ImageService;

// 验证后台加载首轮不阻塞且最终登记固有尺寸。
#[test]
fn async_path_load_returns_pending_then_ready_with_intrinsic_size() {
    // 生成只属于当前测试进程和时刻的临时图片路径。
    let path = std::env::temp_dir().join(format!(
        // 文件名包含进程与单调时间片，避免并发冲突。
        "uix-image-async-{}-{}.png",
        // 加入进程号。
        std::process::id(),
        // 加入当前 UNIX 纳秒时间戳。
        std::time::SystemTime::now()
            // 测试环境必须晚于 UNIX epoch。
            .duration_since(std::time::UNIX_EPOCH)
            // 时钟异常直接暴露为测试失败。
            .expect("system clock should be after unix epoch")
            // 使用纳秒降低同进程重名概率。
            .as_nanos()
    ));
    // 构造二乘三像素的确定性 RGBA 图片。
    let image = image::RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 30, 255]));
    // 把测试图片编码到确切临时路径。
    image.save(&path).expect("temporary png should be written");
    // 创建没有现有路径缓存的图片服务。
    let service = ImageService::new();
    // 首次请求只启动后台任务，不应同步返回句柄。
    assert_eq!(
        service
            // 请求刚刚写入的本地图片。
            .poll_load_from_path(&path)
            // 线程创建必须成功。
            .expect("async image request should start"),
        // 首帧保持非阻塞 pending 状态。
        None
    );
    // 给后台读取与解码最多两秒完成。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    // 持续非阻塞轮询直到获得句柄。
    let handle = loop {
        // 查询当前后台任务状态。
        if let Some(handle) = service
            // 复用相同路径身份。
            .poll_load_from_path(&path)
            // 有效 PNG 不应产生 typed error。
            .expect("async image decode should succeed")
        {
            // 资源就绪后结束轮询。
            break handle;
        }
        // 超时意味着后台任务没有满足非阻塞完成契约。
        assert!(
            std::time::Instant::now() < deadline,
            "async image decode timed out"
        );
        // 短暂让出线程，避免测试忙等独占核心。
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    // 读取已登记槽位的固有像素尺寸。
    let intrinsic = service.with_slot(handle, |slot| (slot.width(), slot.height()));
    // 后台解码结果必须完整进入主线程资源槽位。
    assert_eq!(intrinsic, Some((2, 3)));
    // 删除本测试创建的确切临时文件。
    std::fs::remove_file(&path).expect("temporary png should be removable");
}

// 验证后台文件失败最终返回框架 typed error 而不是永久 pending。
#[test]
fn async_path_load_surfaces_typed_io_error() {
    // 构造当前进程专属且不会创建的缺失路径。
    let path = std::env::temp_dir().join(format!(
        // 使用进程号形成稳定缺失文件名。
        "uix-image-missing-{}.png",
        // 加入当前进程身份。
        std::process::id()
    ));
    // 创建空图片服务。
    let service = ImageService::new();
    // 首轮只启动后台读取。
    assert_eq!(
        service
            // 请求确切缺失路径。
            .poll_load_from_path(&path)
            // 线程创建本身应成功。
            .expect("missing path request should start"),
        // 文件 I/O 不得阻塞首轮。
        None
    );
    // 为后台文件失败留出最多两秒。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    // 轮询直到收到 typed error。
    let error = loop {
        // 查询当前失败任务状态。
        match service.poll_load_from_path(&path) {
            // 任务仍在后台时继续等待。
            Ok(None) => {}
            // 缺失文件不可能成功返回句柄。
            Ok(Some(_)) => panic!("missing image unexpectedly loaded"),
            // 收到 typed error 后结束轮询。
            Err(error) => break error,
        }
        // 超时意味着错误没有交付给所有者线程。
        assert!(
            std::time::Instant::now() < deadline,
            "async image error timed out"
        );
        // 短暂让出线程避免忙等。
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    // 文件失败必须保留统一 I/O 错误码。
    assert_eq!(error.code(), crate::core::error::Errc::IoError);
    // 错误消息应包含确切资源路径以便诊断。
    assert!(
        error
            .message()
            .contains(&path.to_string_lossy().into_owned())
    );
}

// 验证同步加载抢先填充缓存后会清理同路径后台接收器。
#[test]
fn cached_path_hit_discards_stale_async_receiver() {
    // 生成当前测试专属的临时图片路径。
    let path = std::env::temp_dir().join(format!(
        // 文件名包含进程与时间，避免并行测试重名。
        "uix-image-cache-race-{}-{}.png",
        // 加入当前进程号。
        std::process::id(),
        // 使用 UNIX 纳秒时间戳形成唯一后缀。
        std::time::SystemTime::now()
            // 测试环境必须拥有有效系统时钟。
            .duration_since(std::time::UNIX_EPOCH)
            // 时钟异常直接暴露为测试失败。
            .expect("system clock should be after unix epoch")
            // 纳秒精度降低重名概率。
            .as_nanos()
    ));
    // 构造一乘一像素的确定性测试图片。
    let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([30, 40, 50, 255]));
    // 把图片写入确切临时路径。
    image.save(&path).expect("temporary png should be written");
    // 创建没有现有路径缓存的图片服务。
    let service = ImageService::new();
    // 首次轮询启动后台解码并保留接收器。
    assert_eq!(
        service
            // 请求刚写入的本地图片。
            .poll_load_from_path(&path)
            // 后台线程必须成功启动。
            .expect("async image request should start"),
        // 首轮保持非阻塞 pending。
        None
    );
    // 后台任务身份必须已经登记。
    assert!(
        service
            .pending_path_decodes
            .borrow()
            .contains_key(&path.to_string_lossy().into_owned())
    );
    // 模拟同一帧其他所有者通过同步入口抢先填充路径缓存。
    let cached = service
        // 复用完全相同的本地路径。
        .load_from_path(&path)
        // 有效 PNG 必须同步加载成功。
        .expect("synchronous cache fill should succeed");
    // 后继轮询必须直接交付同步缓存句柄。
    assert_eq!(
        service
            // 再次请求同一路径。
            .poll_load_from_path(&path)
            // 缓存命中不应产生错误。
            .expect("cached image request should succeed"),
        // 返回同步入口登记的同一代际句柄。
        Some(cached)
    );
    // 已被同步结果取代的后台接收器必须立即清理。
    assert!(service.pending_path_decodes.borrow().is_empty());
    // 删除本测试创建的确切临时文件。
    std::fs::remove_file(&path).expect("temporary png should be removable");
}
