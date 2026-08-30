// 导入 macOS Platform System 根模块拥有的契约与错误类型。
use super::*;
// 导入纯逻辑过滤器解析端口。
use super::file_dialog_filters::parse_allowed_file_types;

// AppKit 确认按钮返回的模态响应码。
const NS_MODAL_RESPONSE_OK: isize = 1;
// AppKit 取消按钮返回的模态响应码。
const NS_MODAL_RESPONSE_CANCEL: isize = 0;
// Objective-C BOOL 与当前 64 位 AppKit ABI 保持 i8 布局。
type Bool = i8;
// Objective-C 真值常量。
const YES: Bool = 1;
// Objective-C 假值常量。
const NO: Bool = 0;

// 通过公开 Platform System 的窄 Adapter 打开多选文件面板。
pub(crate) fn choose_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    // AppKit 面板只能在进程主线程同步运行。
    ensure_appkit_thread("MacosFileDialog::open")?;
    // 通过当前 host Module 的窄函数适配器保留真实确认、取消和错误结果。
    // SAFETY: 主线程门禁已通过；面板及返回对象只在同步 runModal 期间使用。
    unsafe { open_files(title, filters) }
}

// 通过公开 Platform System 的窄 Adapter 打开保存面板。
pub(crate) fn choose_save_file(title: &str, filters: &str) -> Result<Option<String>> {
    // AppKit 面板只能在进程主线程同步运行。
    ensure_appkit_thread("MacosFileDialog::save")?;
    // 通过当前 host Module 的窄函数适配器保留真实确认、取消和错误结果。
    // SAFETY: 主线程门禁已通过；面板及返回对象只在同步 runModal 期间使用。
    unsafe { save_file(title, filters) }
}

// 通过公开 Platform System 的窄 Adapter 打开目录面板。
pub(crate) fn choose_folder(title: &str) -> Result<Option<String>> {
    // AppKit 面板只能在进程主线程同步运行。
    ensure_appkit_thread("MacosFileDialog::open_folder")?;
    // 通过当前 host Module 的窄函数适配器保留真实确认、取消和错误结果。
    // SAFETY: 主线程门禁已通过；面板及返回对象只在同步 runModal 期间使用。
    unsafe { open_directory(title) }
}

// 拒绝从非 AppKit 主线程启动模态面板。
fn ensure_appkit_thread(operation: &str) -> Result<()> {
    // SAFETY: pthread_main_np 无参数且只返回当前线程是否为进程主线程。
    let is_main_thread = unsafe { pthread_main_np() } != 0;
    // 非主线程不能安全驱动 AppKit 模态事件循环。
    if !is_main_thread {
        // 返回可观察的线程亲和错误而不是触发未定义的 AppKit 行为。
        return Err(Error::new(
            // 线程位置不满足组件生命周期前置条件。
            Errc::InvalidState,
            // 保留具体调用入口便于诊断。
            format!("{operation}: AppKit file dialogs require the process main thread"),
        ));
    }
    // 主线程满足同步模态面板前置条件。
    Ok(())
}

// 在 NSOpenPanel 中选择一个或多个文件。
unsafe fn open_files(title: &str, filters: &str) -> Result<Option<Vec<String>>> {
    // SAFETY: 本函数只在主线程调用，所有 Objective-C 对象均在同步消息期间保持存活。
    unsafe {
        // 获取 AppKit 管理的自动释放打开面板。
        let panel = create_panel("NSOpenPanel", "openPanel", "MacosFileDialog::open")?;
        // 设置调用方提供的面板标题。
        set_panel_title(panel, title)?;
        // 允许选择普通文件。
        msg_void_bool(panel, "setCanChooseFiles:", YES);
        // 禁止把目录本身作为结果。
        msg_void_bool(panel, "setCanChooseDirectories:", NO);
        // 文件打开契约允许返回多个路径。
        msg_void_bool(panel, "setAllowsMultipleSelection:", YES);
        // 解析并应用调用方的扩展名过滤器。
        apply_allowed_file_types(panel, filters)?;
        // 运行 AppKit 模态面板并区分确认、取消与异常响应。
        if !run_panel(panel, "MacosFileDialog::open")? {
            // 用户取消不是平台失败。
            return Ok(None);
        }
        // 读取所有已选 URL。
        let urls = msg_id(panel, "URLs");
        // 确认后缺失 URL 数组属于 AppKit 结果异常。
        if urls.is_null() {
            // 返回 typed 平台错误。
            return Err(platform_error(
                "MacosFileDialog::open: NSOpenPanel returned no URLs",
            ));
        }
        // 获取结果数量。
        let count = msg_usize(urls, "count");
        // 确认操作必须至少产生一个文件结果。
        if count == 0 {
            // 返回 typed 平台错误而不是空成功列表。
            return Err(platform_error(
                "MacosFileDialog::open: NSOpenPanel confirmed with an empty selection",
            ));
        }
        // 为全部结果预留稳定容量。
        let mut paths = Vec::with_capacity(count);
        // 按 AppKit 返回顺序转换 URL。
        for index in 0..count {
            // 取得当前 NSURL 对象。
            let url = msg_id_usize(urls, "objectAtIndex:", index);
            // 把 NSURL.path 复制为 Rust UTF-8 路径。
            paths.push(path_from_url(url, "MacosFileDialog::open")?);
        }
        // 返回非空多选结果。
        Ok(Some(paths))
    }
}

// 在 NSSavePanel 中选择单个保存路径。
unsafe fn save_file(title: &str, filters: &str) -> Result<Option<String>> {
    // SAFETY: 本函数只在主线程调用，所有 Objective-C 对象均在同步消息期间保持存活。
    unsafe {
        // 获取 AppKit 管理的自动释放保存面板。
        let panel = create_panel("NSSavePanel", "savePanel", "MacosFileDialog::save")?;
        // 设置调用方提供的面板标题。
        set_panel_title(panel, title)?;
        // 允许用户在保存面板内创建目录。
        msg_void_bool(panel, "setCanCreateDirectories:", YES);
        // 解析并应用调用方的扩展名过滤器。
        apply_allowed_file_types(panel, filters)?;
        // 运行 AppKit 模态面板并区分确认、取消与异常响应。
        if !run_panel(panel, "MacosFileDialog::save")? {
            // 用户取消不是平台失败。
            return Ok(None);
        }
        // 获取保存面板返回的单个 NSURL。
        let url = msg_id(panel, "URL");
        // 转换并返回规范路径。
        Ok(Some(path_from_url(url, "MacosFileDialog::save")?))
    }
}

// 在 NSOpenPanel 中选择单个目录。
unsafe fn open_directory(title: &str) -> Result<Option<String>> {
    // SAFETY: 本函数只在主线程调用，所有 Objective-C 对象均在同步消息期间保持存活。
    unsafe {
        // 获取 AppKit 管理的自动释放打开面板。
        let panel = create_panel("NSOpenPanel", "openPanel", "MacosFileDialog::open_folder")?;
        // 设置调用方提供的面板标题。
        set_panel_title(panel, title)?;
        // 禁止选择普通文件。
        msg_void_bool(panel, "setCanChooseFiles:", NO);
        // 允许选择目录。
        msg_void_bool(panel, "setCanChooseDirectories:", YES);
        // 目录选择契约只返回单一路径。
        msg_void_bool(panel, "setAllowsMultipleSelection:", NO);
        // 允许用户在面板内创建目录。
        msg_void_bool(panel, "setCanCreateDirectories:", YES);
        // 运行 AppKit 模态面板并区分确认、取消与异常响应。
        if !run_panel(panel, "MacosFileDialog::open_folder")? {
            // 用户取消不是平台失败。
            return Ok(None);
        }
        // 获取单选目录结果数组。
        let urls = msg_id(panel, "URLs");
        // 确认后缺失 URL 数组属于 AppKit 结果异常。
        if urls.is_null() || msg_usize(urls, "count") == 0 {
            // 返回 typed 平台错误而不是空成功路径。
            return Err(platform_error(
                "MacosFileDialog::open_folder: NSOpenPanel returned no directory URL",
            ));
        }
        // 读取唯一目录 URL。
        let url = msg_id_usize(urls, "objectAtIndex:", 0);
        // 转换并返回目录路径。
        Ok(Some(path_from_url(url, "MacosFileDialog::open_folder")?))
    }
}

// 获取指定 AppKit 面板类的共享面板实例。
unsafe fn create_panel(class_name: &str, selector: &str, operation: &str) -> Result<cocoa::Id> {
    // SAFETY: class 与 selector 名称由本组件常量调用点提供，消息签名返回 Objective-C 对象。
    let panel = unsafe { msg_id(class(class_name), selector) };
    // 空面板表示 AppKit 无法创建请求对象。
    if panel.is_null() {
        // 返回 typed 平台错误并保留具体操作。
        return Err(platform_error(format!(
            "{operation}: AppKit returned a null panel"
        )));
    }
    // 返回由 AppKit 自动释放池管理的面板借用。
    Ok(panel)
}

// 设置面板标题并配平临时 NSString 所有权。
unsafe fn set_panel_title(panel: cocoa::Id, title: &str) -> Result<()> {
    // SAFETY: panel 在同步调用期间存活，临时 NSString 持有独立 +1 引用。
    unsafe {
        // 创建 UTF-8 标题对应的 NSString。
        let title = ns_string(title, "file dialog title")?;
        // AppKit setter 在返回前消费字符串内容。
        msg_void_id(panel, "setTitle:", title);
        // 配平 alloc/init 创建的临时字符串引用。
        msg_void(title, "release");
        // 标题已同步写入面板。
        Ok(())
    }
}

// 把扩展名过滤器写入 NSSavePanel/NSOpenPanel。
unsafe fn apply_allowed_file_types(panel: cocoa::Id, filters: &str) -> Result<()> {
    // 先在 Rust 侧完成纯逻辑规范化。
    let allowed = parse_allowed_file_types(filters);
    // 空列表表示允许所有文件类型。
    if allowed.is_empty() {
        // 不调用 deprecated allowedFileTypes setter即可保留 AppKit 默认行为。
        return Ok(());
    }
    // SAFETY: panel 在同步调用期间存活，NSMutableArray 由当前自动释放池管理。
    unsafe {
        // 创建足够容量的可变字符串数组。
        let array = msg_id_usize(class("NSMutableArray"), "arrayWithCapacity:", allowed.len());
        // 空数组对象表示 Objective-C 分配失败。
        if array.is_null() {
            // 返回 typed 平台错误。
            return Err(platform_error(
                "MacosFileDialog: NSMutableArray allocation failed",
            ));
        }
        // 逐项写入规范化扩展名。
        for file_type in allowed {
            // 创建扩展名 NSString。
            let value = ns_string(&file_type, "file dialog filter")?;
            // NSMutableArray 在加入对象后持有元素。
            msg_void_id(array, "addObject:", value);
            // 配平当前临时 NSString 的 +1 引用。
            msg_void(value, "release");
        }
        // 要求 AppKit 只允许数组声明的文件类型。
        msg_void_id(panel, "setAllowedFileTypes:", array);
        // 禁止绕过已声明过滤器选择其他类型。
        msg_void_bool(panel, "setAllowsOtherFileTypes:", NO);
        // 过滤器已同步应用。
        Ok(())
    }
}

// 运行面板并把 AppKit 响应规范化为确认布尔值。
unsafe fn run_panel(panel: cocoa::Id, operation: &str) -> Result<bool> {
    // SAFETY: panel 是存活的 NSSavePanel 或 NSOpenPanel，runModal 返回 NSInteger。
    let response = unsafe { msg_isize(panel, "runModal") };
    // 只接受 AppKit 文件面板契约定义的确认与取消响应。
    match response {
        // 确认表示调用方可以读取 URL 结果。
        NS_MODAL_RESPONSE_OK => Ok(true),
        // 取消保持非失败结果。
        NS_MODAL_RESPONSE_CANCEL => Ok(false),
        // 其他响应码不能伪装成用户取消。
        unexpected => Err(platform_error(format!(
            // 保留操作与原始响应码便于诊断。
            "{operation}: AppKit returned unexpected modal response {unexpected}"
        ))),
    }
}

// 把 NSURL.path 复制为拥有所有权的 UTF-8 Rust 字符串。
unsafe fn path_from_url(url: cocoa::Id, operation: &str) -> Result<String> {
    // 空 URL 不能形成有效文件系统结果。
    if url.is_null() {
        // 返回 typed 平台错误。
        return Err(platform_error(format!(
            "{operation}: AppKit returned a null URL"
        )));
    }
    // SAFETY: url 在面板结果集合中保持存活，path 与 UTF8String 均只在同步读取期间借用。
    unsafe {
        // 获取 NSURL 的 NSString 路径。
        let path = msg_id(url, "path");
        // 缺失路径不能返回空成功值。
        if path.is_null() {
            // 返回 typed 平台错误。
            return Err(platform_error(format!(
                "{operation}: selected URL has no filesystem path"
            )));
        }
        // 获取 NSString 内部稳定到下一次变更前的 UTF-8 指针。
        let bytes = msg_const_char_ptr(path, "UTF8String");
        // 空指针表示字符串无法提供 UTF-8 表示。
        if bytes.is_null() {
            // 返回 typed 格式错误。
            return Err(Error::new(
                // 路径编码不满足公开 UTF-8 契约。
                Errc::FormatError,
                // 保留具体调用入口。
                format!("{operation}: selected path has no UTF-8 representation"),
            ));
        }
        // 复制 C 字符串并严格验证 UTF-8。
        let path = CStr::from_ptr(bytes)
            // 拒绝非 UTF-8 路径而不是损失替换字符。
            .to_str()
            // 转换为 UIX typed 格式错误。
            .map_err(|error| {
                Error::new(
                    Errc::FormatError,
                    format!("{operation}: selected path is not UTF-8: {error}"),
                )
            })?
            // 在 AppKit 对象生命周期之外持有路径。
            .to_owned();
        // 空路径同样属于异常 AppKit 结果。
        if path.is_empty() {
            // 返回 typed 平台错误。
            return Err(platform_error(format!(
                "{operation}: selected path is empty"
            )));
        }
        // 返回拥有所有权的 UTF-8 路径。
        Ok(path)
    }
}

// 创建由调用方拥有 +1 引用的 NSString。
unsafe fn ns_string(value: &str, field: &str) -> Result<cocoa::Id> {
    // 拒绝 Objective-C UTF-8 初始化器无法表示的内嵌 NUL。
    let value = CString::new(value).map_err(|error| {
        // 返回 typed 格式错误并指出输入字段。
        Error::new(
            Errc::FormatError,
            format!("MacosFileDialog: {field} contains NUL: {error}"),
        )
    })?;
    // SAFETY: CString 在同步 initWithUTF8String: 返回前保持有效。
    let string = unsafe {
        // 先取得 NSString 的 +1 alloc 对象。
        let allocated = msg_id(class("NSString"), "alloc");
        // 用 UTF-8 C 字符串初始化对象。
        msg_id_ptr(allocated, "initWithUTF8String:", value.as_ptr())
    };
    // 初始化失败时没有可供调用方使用的字符串对象。
    if string.is_null() {
        // 返回 typed 平台错误。
        return Err(platform_error(format!(
            "MacosFileDialog: failed to create NSString for {field}"
        )));
    }
    // 返回需由调用方 release 的 +1 字符串。
    Ok(string)
}

// 构造统一的 AppKit 平台错误。
fn platform_error(message: impl Into<String>) -> Error {
    // 保留 typed PlatformError 分类供上层诊断与恢复策略使用。
    Error::new(Errc::PlatformError, message)
}

// Objective-C 对象指针别名。
type Id = cocoa::Id;
// Objective-C selector 指针别名。
type Sel = *mut c_void;

// 读取 Objective-C 类对象。
unsafe fn class(name: &str) -> Id {
    // 类名来自本组件常量且不会包含 NUL。
    let name = CString::new(name).expect("Objective-C class name must not contain NUL");
    // SAFETY: C 字符串在 objc_getClass 同步调用期间保持有效。
    unsafe { objc_getClass(name.as_ptr()) }
}

// 注册并读取 Objective-C selector。
unsafe fn sel(name: &str) -> Sel {
    // selector 名称来自本组件常量且不会包含 NUL。
    let name = CString::new(name).expect("Objective-C selector name must not contain NUL");
    // SAFETY: C 字符串在 sel_registerName 同步调用期间保持有效。
    unsafe { sel_registerName(name.as_ptr()) }
}

// 发送无额外参数且返回对象的 Objective-C 消息。
unsafe fn msg_id(receiver: Id, selector: &str) -> Id {
    // 声明与目标选择器匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel) -> Id;
    // SAFETY: objc_msgSend 只按当前选择器的对象返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且响应 selector。
    unsafe { message(receiver, sel(selector)) }
}

// 发送接收一个指针参数且返回对象的 Objective-C 消息。
unsafe fn msg_id_ptr(receiver: Id, selector: &str, value: *const c_char) -> Id {
    // 声明与 NSString 初始化器匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel, *const c_char) -> Id;
    // SAFETY: objc_msgSend 只按当前选择器的对象返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 与指针参数在同步调用期间有效。
    unsafe { message(receiver, sel(selector), value) }
}

// 发送接收 NSUInteger 且返回对象的 Objective-C 消息。
unsafe fn msg_id_usize(receiver: Id, selector: &str, value: usize) -> Id {
    // 声明与数组容量或索引选择器匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel, usize) -> Id;
    // SAFETY: objc_msgSend 只按当前选择器的对象返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且 selector 接收 NSUInteger。
    unsafe { message(receiver, sel(selector), value) }
}

// 发送接收对象且无返回值的 Objective-C 消息。
unsafe fn msg_void_id(receiver: Id, selector: &str, value: Id) {
    // 声明与对象 setter 匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel, Id);
    // SAFETY: objc_msgSend 只按当前选择器的 void 返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 与 value 在同步调用期间有效。
    unsafe { message(receiver, sel(selector), value) };
}

// 发送接收 BOOL 且无返回值的 Objective-C 消息。
unsafe fn msg_void_bool(receiver: Id, selector: &str, value: Bool) {
    // 声明与布尔 setter 匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel, Bool);
    // SAFETY: objc_msgSend 只按当前选择器的 void 返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且 selector 接收 Objective-C BOOL。
    unsafe { message(receiver, sel(selector), value) };
}

// 发送无额外参数且无返回值的 Objective-C 消息。
unsafe fn msg_void(receiver: Id, selector: &str) {
    // 声明与 release 等选择器匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel);
    // SAFETY: objc_msgSend 只按当前选择器的 void 返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且响应 selector。
    unsafe { message(receiver, sel(selector)) };
}

// 发送无额外参数且返回 NSInteger 的 Objective-C 消息。
unsafe fn msg_isize(receiver: Id, selector: &str) -> isize {
    // 声明与 runModal 匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel) -> isize;
    // SAFETY: objc_msgSend 只按当前选择器的 NSInteger 返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且 selector 返回 NSInteger。
    unsafe { message(receiver, sel(selector)) }
}

// 发送无额外参数且返回 NSUInteger 的 Objective-C 消息。
unsafe fn msg_usize(receiver: Id, selector: &str) -> usize {
    // 声明与 NSArray count 匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel) -> usize;
    // SAFETY: objc_msgSend 只按当前选择器的 NSUInteger 返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且 selector 返回 NSUInteger。
    unsafe { message(receiver, sel(selector)) }
}

// 发送无额外参数且返回 UTF-8 C 字符串指针的 Objective-C 消息。
unsafe fn msg_const_char_ptr(receiver: Id, selector: &str) -> *const c_char {
    // 声明与 NSString UTF8String 匹配的函数签名。
    type Message = unsafe extern "C" fn(Id, Sel) -> *const c_char;
    // SAFETY: objc_msgSend 只按当前选择器的指针返回 ABI 转型。
    let message: Message = unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    // SAFETY: 调用方保证 receiver 存活且 selector 返回只读 C 字符串指针。
    unsafe { message(receiver, sel(selector)) }
}

#[link(name = "objc")]
// SAFETY: 声明严格对应 Objective-C runtime C ABI，消息发送只会按选择器签名转型后调用。
unsafe extern "C" {
    // 按 NUL 结尾类名查找 Objective-C 类。
    fn objc_getClass(name: *const c_char) -> Id;
    // 注册 NUL 结尾选择器名称。
    fn sel_registerName(name: *const c_char) -> Sel;
    // Objective-C 动态消息发送入口，具体签名由窄包装器确定。
    fn objc_msgSend();
}

#[link(name = "AppKit", kind = "framework")]
// SAFETY: 空链接块只要求装载 AppKit 框架，不声明额外符号。
unsafe extern "C" {}

#[link(name = "System")]
// SAFETY: pthread_main_np 无参数并按系统 C ABI 返回当前线程是否为进程主线程。
unsafe extern "C" {
    // 查询当前线程是否为进程主线程。
    fn pthread_main_np() -> i32;
}
