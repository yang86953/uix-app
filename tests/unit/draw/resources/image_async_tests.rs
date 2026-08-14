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
