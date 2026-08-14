// 引入像素去重与可见性采样集合。
use std::collections::BTreeSet;
// 引入证据文件路径。
use std::path::{Path, PathBuf};
// 引入窗口出现的有界等待。
use std::thread;
// 引入等待预算。
use std::time::{Duration, Instant};

// 引入图像拼板与保存能力。
use image::{imageops, DynamicImage, GenericImage, RgbaImage};
// 引入窗口、坐标与矩形类型。
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
// 引入桌面像素复制所需 GDI 资源。
use windows::Win32::Graphics::Gdi::{
    BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
// 引入顶层窗口枚举、可见性与置顶能力。
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClientRect, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, IsZoomed, SetWindowPos, HWND_TOPMOST, SWP_NOMOVE,
    SWP_NOSIZE, SWP_SHOWWINDOW,
};
// 引入 Win32 回调布尔返回值。
use windows::core::BOOL;

// 引入唯一主演示进程 fixture。
use super::process::DemoProcess;

// 声明 DWM 提交刷新入口。
#[link(name = "dwmapi")]
// Rust 2024 要求显式标记不安全外部符号声明。
unsafe extern "system" {
    // 等待当前桌面组合批次完成。
    fn DwmFlush() -> i32;
}

// 保存枚举期间唯一进程窗口搜索状态。
struct WindowSearch {
    // 保存目标主演示进程 ID。
    process_id: u32,
    // 保存可选精确 UTF-16 标题筛选。
    title: Option<Vec<u16>>,
    // 保存当前最佳顶层窗口。
    window: Option<HWND>,
    // 保存可见性优先后的面积评分。
    score: i64,
}

// 为 EnumWindows 选择目标进程最大的可见窗口。
unsafe extern "system" fn find_window(window: HWND, context: LPARAM) -> BOOL {
    // 从同步调用上下文恢复搜索状态。
    let search = unsafe { &mut *(context.0 as *mut WindowSearch) };
    // 创建进程 ID 输出槽。
    let mut process_id = 0_u32;
    // 查询当前顶层窗口所属进程。
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    // 创建窗口矩形输出槽。
    let mut bounds = RECT::default();
    // 读取外框尺寸以排除辅助窗口。
    let has_bounds = unsafe { GetWindowRect(window, &mut bounds) }.is_ok();
    // 计算非负宽度。
    let width = bounds.right.saturating_sub(bounds.left);
    // 计算非负高度。
    let height = bounds.bottom.saturating_sub(bounds.top);
    // 读取候选窗口标题供次级窗口精确筛选。
    let title_matches = search.title.as_ref().is_none_or(|expected| {
        // 固定缓冲足以容纳本测试两个稳定短标题。
        let mut title = [0_u16; 256];
        // Win32 返回不含末尾空字符的 UTF-16 单元数。
        let length = unsafe { GetWindowTextW(window, &mut title) }.max(0) as usize;
        // 精确比较有效标题单元。
        title.get(..length).is_some_and(|actual| actual == expected)
    });
    // 只接受目标进程中足够大的顶层窗口。
    if process_id == search.process_id
        && title_matches
        && has_bounds
        && width >= 320
        && height >= 240
    {
        // 用高位保证可见窗口优先。
        let visible = i64::from(unsafe { IsWindowVisible(window) }.as_bool()) << 48;
        // 用面积在相同可见性中选择主演示。
        let score = visible.saturating_add(i64::from(width) * i64::from(height));
        // 只替换成更合适的候选。
        if score > search.score {
            // 保存候选窗口。
            search.window = Some(window);
            // 保存候选评分。
            search.score = score;
        }
    }
    // 继续枚举其余顶层窗口。
    BOOL(1)
}

// 在有界时间内定位主演示真实 HWND。
fn wait_for_window(process_id: u32, title: Option<&str>) -> HWND {
    // 设置窗口创建最长等待时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续查询直到窗口出现或超时。
    loop {
        // 创建本轮枚举状态。
        let mut search = WindowSearch {
            // 锁定 fixture 子进程。
            process_id,
            // 每轮拥有一份可供同步回调借用的 UTF-16 标题。
            title: title.map(|value| value.encode_utf16().collect()),
            // 本轮尚无候选。
            window: None,
            // 允许首个有效窗口胜出。
            score: -1,
        };
        // 同步枚举桌面顶层窗口。
        unsafe {
            // 回调只在本调用返回前借用栈状态。
            let _ = EnumWindows(
                // 使用当前模块的精确回调。
                Some(find_window),
                // 传递同步有效的搜索状态指针。
                LPARAM((&mut search as *mut WindowSearch) as isize),
            );
        }
        // 找到主演示窗口后立即返回。
        if let Some(window) = search.window {
            // 返回不拥有所有权的系统句柄。
            return window;
        }
        // 超时表示真实窗口没有建立。
        assert!(Instant::now() < deadline, "timed out locating demo HWND");
        // 短暂等待避免忙轮询。
        thread::sleep(Duration::from_millis(25));
    }
}

// 有界等待主演示进入或退出原生最大化状态。
pub(crate) fn wait_for_maximized(demo: &DemoProcess, expected: bool) {
    // 定位本 fixture 面积最大的真实顶层窗口。
    let window = wait_for_window(demo.process_id(), None);
    // 设置原生窗口状态变化最长等待时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续查询直到状态匹配或超时。
    loop {
        // IsZoomed 只读取当前 HWND 最大化事实。
        let maximized = unsafe { IsZoomed(window) }.as_bool();
        // 精确匹配期望状态时完成等待。
        if maximized == expected {
            // 原生窗口状态已经收敛。
            return;
        }
        // 超时表示产品窗口动作没有到达 Win32 owner。
        assert!(
            Instant::now() < deadline,
            "timed out waiting for maximized={expected}"
        );
        // 短暂等待避免忙轮询。
        thread::sleep(Duration::from_millis(25));
    }
}

// 唯一拥有一次 GDI 捕获的原生资源。
struct CaptureResources {
    // 保存桌面 DC。
    screen: HDC,
    // 保存内存 DC。
    memory: HDC,
    // 保存 DIB 位图。
    bitmap: Option<HBITMAP>,
    // 保存内存 DC 原先选入对象。
    previous: Option<HGDIOBJ>,
}

// 创建桌面与内存 DC。
impl CaptureResources {
    // 分配一组捕获资源。
    fn new() -> Self {
        // 获取桌面设备上下文。
        let screen = unsafe { GetDC(None) };
        // 无效桌面 DC 不能继续捕获。
        assert!(!screen.is_invalid(), "GetDC desktop failed");
        // 创建与桌面兼容的内存 DC。
        let memory = unsafe { CreateCompatibleDC(Some(screen)) };
        // 创建失败时先释放已经取得的桌面 DC。
        if memory.is_invalid() {
            // 对称释放桌面 DC。
            let _ = unsafe { ReleaseDC(None, screen) };
            // 报告原生资源失败。
            panic!("CreateCompatibleDC failed");
        }
        // 返回唯一资源所有者。
        Self {
            // 保存桌面 DC。
            screen,
            // 保存内存 DC。
            memory,
            // 位图稍后按窗口尺寸创建。
            bitmap: None,
            // 原对象稍后由 SelectObject 返回。
            previous: None,
        }
    }
}

// 按逆序确定释放全部 GDI 资源。
impl Drop for CaptureResources {
    // 释放本对象唯一拥有的原生句柄。
    fn drop(&mut self) {
        // 每个句柄都只在本对象中释放一次。
        unsafe {
            // 先恢复内存 DC 原对象。
            if let Some(previous) = self.previous.take() {
                // 恢复失败不覆盖原测试结论。
                let _ = SelectObject(self.memory, previous);
            }
            // 再删除不再选入 DC 的位图。
            if let Some(bitmap) = self.bitmap.take() {
                // 位图由本对象创建并唯一拥有。
                let _ = DeleteObject(bitmap.into());
            }
            // 删除内存 DC。
            let _ = DeleteDC(self.memory);
            // 最后释放桌面 DC。
            let _ = ReleaseDC(None, self.screen);
        }
    }
}

// 捕获主演示客户区并保存无覆盖 PNG。
pub(crate) fn capture_png(demo: &DemoProcess, path: &Path) {
    // 未指定标题时保持选择最大主演示窗口的既有行为。
    capture_png_for_title(demo, path, None);
}

// 捕获指定稳定标题的顶层窗口客户区。
pub(crate) fn capture_png_by_title(demo: &DemoProcess, path: &Path, title: &str) {
    // 精确标题用于区分同进程内的次级窗口。
    capture_png_for_title(demo, path, Some(title));
}

// 捕获主演示指定窗口客户区并保存无覆盖 PNG。
fn capture_png_for_title(demo: &DemoProcess, path: &Path, title: Option<&str>) {
    // 定位本 fixture 的真实顶层窗口。
    let window = wait_for_window(demo.process_id(), title);
    // 把主演示放到桌面最上层以避免其他窗口覆盖像素证据。
    unsafe {
        // 不移动也不缩放，只显示并置顶目标窗口。
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        )
    }
    // 置顶失败应阻止产生不可信截图。
    .expect("raise demo window for capture");
    // 请求窗口进入 Z 序前端。
    let _ = unsafe { BringWindowToTop(window) };
    // 等待 DWM 完成本轮桌面组合。
    let flushed = unsafe { DwmFlush() };
    // 负 HRESULT 表示截图时点不可信。
    assert!(flushed >= 0, "DwmFlush failed with {flushed:#x}");

    // 读取客户区尺寸。
    let mut client = RECT::default();
    // 输出矩形在同步调用期间有效。
    unsafe { GetClientRect(window, &mut client) }.expect("GetClientRect");
    // 至少保留一个像素避免零大小 DIB。
    let width = (client.right - client.left).max(1);
    // 至少保留一个像素避免零大小 DIB。
    let height = (client.bottom - client.top).max(1);
    // 创建客户区屏幕原点。
    let mut origin = POINT::default();
    // 把客户区零点转换为桌面坐标。
    assert!(unsafe { ClientToScreen(window, &mut origin) }.as_bool());
    // 创建唯一资源所有者。
    let mut resources = CaptureResources::new();
    // 保存 DIB 映射地址。
    let mut bits = std::ptr::null_mut();
    // 定义 top-down 32 位无压缩位图。
    let info = BITMAPINFO {
        // 设置 DIB 头。
        bmiHeader: BITMAPINFOHEADER {
            // 声明头结构长度。
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            // 使用客户区宽度。
            biWidth: width,
            // 负高度表示 top-down 行序。
            biHeight: -height,
            // DIB 固定单平面。
            biPlanes: 1,
            // 使用 BGRA 四字节像素。
            biBitCount: 32,
            // 使用无压缩 RGB。
            biCompression: BI_RGB.0,
            // 其余字段保持零默认值。
            ..Default::default()
        },
        // 调色板保持默认空值。
        ..Default::default()
    };
    // 创建映射 DIB 位图。
    let bitmap = unsafe {
        // 传入有效桌面 DC、头与输出地址。
        CreateDIBSection(
            Some(resources.screen),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
    }
    // 创建失败直接报告。
    .expect("CreateDIBSection");
    // 映射地址必须存在。
    assert!(!bits.is_null(), "CreateDIBSection returned null pixels");
    // 把位图所有权交给资源对象。
    resources.bitmap = Some(bitmap);
    // 选入位图并取得原对象。
    let previous = unsafe { SelectObject(resources.memory, bitmap.into()) };
    // 无效原对象表示选入失败。
    assert!(!previous.is_invalid(), "SelectObject bitmap failed");
    // 保存原对象供 Drop 恢复。
    resources.previous = Some(previous);
    // 从桌面复制客户区像素到内存 DIB。
    unsafe {
        // 两个 DC、坐标与尺寸在同步调用期间有效。
        BitBlt(
            resources.memory,
            0,
            0,
            width,
            height,
            Some(resources.screen),
            origin.x,
            origin.y,
            SRCCOPY | CAPTUREBLT,
        )
    }
    // 像素复制失败时不写证据。
    .expect("BitBlt demo client");
    // 计算映射像素数量。
    let pixel_count = (width as usize).saturating_mul(height as usize);
    // 在位图释放前复制全部 BGRA 像素。
    let pixels = unsafe { std::slice::from_raw_parts(bits.cast::<u32>(), pixel_count) };
    // 采样颜色集合以拒绝黑屏或单色截图。
    let step = (pixels.len() / 4096).max(1);
    // 收集 RGB 唯一值。
    let unique = pixels
        // 遍历映射像素。
        .iter()
        // 有界采样大图。
        .step_by(step)
        // 丢弃未使用 alpha。
        .map(|pixel| pixel & 0x00FF_FFFF)
        // 使用确定集合去重。
        .collect::<BTreeSet<_>>();
    // 可见 UI 应至少提供多种颜色。
    assert!(
        unique.len() >= 8,
        "capture contains only {} colors",
        unique.len()
    );
    // 创建 RGBA 输出缓冲。
    let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
    // 把 Windows BGRA 转换为图像库 RGBA。
    for pixel in pixels {
        // 拆出小端 BGRA 字节。
        let [blue, green, red, _] = pixel.to_le_bytes();
        // 写入不透明 RGBA。
        rgba.extend_from_slice(&[red, green, blue, 255]);
    }
    // 创建证据父目录。
    if let Some(parent) = path.parent() {
        // 只创建指定证据目录链。
        std::fs::create_dir_all(parent).expect("create visual evidence directory");
    }
    // 保存无损 PNG。
    image::save_buffer_with_format(
        path,
        &rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    // 写入失败表示验收证据不完整。
    .expect("save visual evidence PNG");
}

// 断言同一页面的浅色与深色证据具有显著平均亮度差。
pub(crate) fn assert_theme_luminance_delta(light: &Path, dark: &Path, minimum_delta: f64) {
    // 计算浅色证据平均亮度。
    let light_luminance = average_luminance(light);
    // 计算深色证据平均亮度。
    let dark_luminance = average_luminance(dark);
    // 深色主题必须明显更暗，避免只切换图标却保留浅色界面。
    assert!(
        light_luminance - dark_luminance >= minimum_delta,
        "theme evidence luminance delta too small: light={light_luminance:.2}, dark={dark_luminance:.2}, minimum={minimum_delta:.2}"
    );
}

// 计算两张同尺寸真实窗口证据中 RGB 发生变化的像素比例。
pub(crate) fn changed_pixel_ratio(left: &Path, right: &Path) -> f64 {
    // 解码左侧无损证据并统一为 RGB。
    let left = image::open(left)
        // 缺失或损坏必须阻止系统主题验收。
        .expect("open left visual evidence")
        // 丢弃恒定 alpha 通道。
        .to_rgb8();
    // 解码右侧无损证据并统一为 RGB。
    let right = image::open(right)
        // 缺失或损坏必须阻止系统主题验收。
        .expect("open right visual evidence")
        // 丢弃恒定 alpha 通道。
        .to_rgb8();
    // 系统主题切换不能同时改变窗口客户区尺寸。
    assert_eq!(
        left.dimensions(),
        right.dimensions(),
        "theme captures must keep the same client extent"
    );
    // 统计任一颜色通道发生变化的像素。
    let changed = left
        // 按相同坐标遍历左图像素。
        .pixels()
        // 与右图同位置像素配对。
        .zip(right.pixels())
        // 只保留 RGB 不完全相同的像素。
        .filter(|(left, right)| left != right)
        // 计算变化像素数。
        .count();
    // 非空客户区已由捕获入口保证。
    let total = u64::from(left.width()) * u64::from(left.height());
    // 返回零到一范围的变化比例。
    changed as f64 / total as f64
}

// 计算同尺寸截图中一个逻辑矩形映射后的 RGB 变化比例。
pub(crate) fn changed_pixel_ratio_in_logical_region(
    // 接收左侧证据路径。
    left: &Path,
    // 接收右侧证据路径。
    right: &Path,
    // 接收语义根逻辑宽度。
    root_width: f64,
    // 接收语义根逻辑高度。
    root_height: f64,
    // 接收目标逻辑矩形 x、y、w、h。
    region: (f64, f64, f64, f64),
) -> f64 {
    // 解码左侧无损证据并统一为 RGB。
    let left = image::open(left)
        // 缺失或损坏必须阻止焦点视觉验收。
        .expect("open left focus evidence")
        // 丢弃恒定 alpha 通道。
        .to_rgb8();
    // 解码右侧无损证据并统一为 RGB。
    let right = image::open(right)
        // 缺失或损坏必须阻止焦点视觉验收。
        .expect("open right focus evidence")
        // 丢弃恒定 alpha 通道。
        .to_rgb8();
    // 两张焦点证据必须保持同一客户区尺寸。
    assert_eq!(
        left.dimensions(),
        right.dimensions(),
        "focus captures must keep the same client extent"
    );
    // 逻辑根必须非空才能建立 DPI 映射。
    assert!(root_width > 0.0 && root_height > 0.0);
    // 计算逻辑坐标到截图物理像素的水平比例。
    let scale_x = f64::from(left.width()) / root_width;
    // 计算逻辑坐标到截图物理像素的垂直比例。
    let scale_y = f64::from(left.height()) / root_height;
    // 把目标左边界映射并裁剪到图像范围。
    let x0 = (region.0 * scale_x).floor().max(0.0) as u32;
    // 把目标上边界映射并裁剪到图像范围。
    let y0 = (region.1 * scale_y).floor().max(0.0) as u32;
    // 把目标右边界映射并裁剪到图像范围。
    let x1 = ((region.0 + region.2) * scale_x)
        // 覆盖边界像素。
        .ceil()
        // 拒绝负坐标。
        .max(0.0)
        // 裁剪到图像宽度。
        .min(f64::from(left.width())) as u32;
    // 把目标下边界映射并裁剪到图像范围。
    let y1 = ((region.1 + region.3) * scale_y)
        // 覆盖边界像素。
        .ceil()
        // 拒绝负坐标。
        .max(0.0)
        // 裁剪到图像高度。
        .min(f64::from(left.height())) as u32;
    // 目标必须在当前客户区具有非空物理范围。
    assert!(x0 < x1 && y0 < y1, "focus region must be visible");
    // 保存变化像素数。
    let mut changed = 0_u64;
    // 按物理行遍历目标区域。
    for y in y0..y1 {
        // 按物理列遍历目标区域。
        for x in x0..x1 {
            // RGB 任一通道变化即计入本像素。
            if left.get_pixel(x, y) != right.get_pixel(x, y) {
                // 饱和推进变化计数。
                changed = changed.saturating_add(1);
            }
        }
    }
    // 计算目标物理像素总数。
    let total = u64::from(x1 - x0) * u64::from(y1 - y0);
    // 返回零到一范围的区域变化比例。
    changed as f64 / total as f64
}

// 读取 PNG 并计算不含透明度的平均感知亮度。
fn average_luminance(path: &Path) -> f64 {
    // 解码本测试刚写出的无损证据。
    let image = image::open(path)
        // 文件缺失或损坏必须阻止验收通过。
        .unwrap_or_else(|error| panic!("open visual evidence {}: {error}", path.display()))
        // 统一为八位 RGB 像素。
        .to_rgb8();
    // 使用 Rec. 709 权重累加全部像素亮度。
    let total = image.pixels().fold(0.0, |sum, pixel| {
        // 读取红色通道。
        let red = f64::from(pixel[0]);
        // 读取绿色通道。
        let green = f64::from(pixel[1]);
        // 读取蓝色通道。
        let blue = f64::from(pixel[2]);
        // 返回包含当前像素的感知亮度总和。
        sum + red * 0.2126 + green * 0.7152 + blue * 0.0722
    });
    // 非空客户区已经由捕获入口保证。
    let pixel_count = f64::from(image.width()) * f64::from(image.height());
    // 返回零到二百五十五范围的平均亮度。
    total / pixel_count
}

// 把多张页面截图缩略排成固定列数的审阅板。
pub(crate) fn write_contact_sheet(paths: &[PathBuf], output: &Path, columns: u32) {
    // 审阅板必须至少包含一张证据图。
    assert!(!paths.is_empty(), "contact sheet requires images");
    // 列数必须为正。
    assert!(columns > 0, "contact sheet columns must be positive");
    // 固定每格宽度便于并排审阅。
    let cell_width = 360_u32;
    // 固定每格高度便于并排审阅。
    let cell_height = 240_u32;
    // 向上取整计算行数。
    let rows = (paths.len() as u32).div_ceil(columns);
    // 创建白色不透明画布。
    let mut sheet = RgbaImage::from_pixel(
        columns * cell_width,
        rows * cell_height,
        image::Rgba([255, 255, 255, 255]),
    );
    // 按调用方顺序放置每张证据图。
    for (index, path) in paths.iter().enumerate() {
        // 解码已经保存的无损 PNG。
        let source = image::open(path).expect("open visual evidence PNG");
        // 保持比例缩放到单元格范围。
        let thumbnail = imageops::thumbnail(&source, cell_width, cell_height);
        // 计算零基列坐标。
        let column = index as u32 % columns;
        // 计算零基行坐标。
        let row = index as u32 / columns;
        // 把缩略图复制进确定位置。
        sheet
            // 使用完整缩略图像素。
            .copy_from(&thumbnail, column * cell_width, row * cell_height)
            // 单元格尺寸保证复制不会越界。
            .expect("compose contact sheet cell");
    }
    // 创建审阅板父目录。
    if let Some(parent) = output.parent() {
        // 只创建指定证据目录链。
        std::fs::create_dir_all(parent).expect("create contact sheet directory");
    }
    // 以 PNG 保存审阅板。
    DynamicImage::ImageRgba8(sheet)
        // 使用输出扩展名选择编码器。
        .save(output)
        // 写入失败表示视觉审阅材料不完整。
        .expect("save visual contact sheet");
}
