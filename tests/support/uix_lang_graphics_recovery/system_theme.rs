// 导入同步系统主题验收锁。
use std::sync::{Mutex, MutexGuard};
// 导入有界窗口发现等待。
use std::thread;
// 导入窗口发现截止时间。
use std::time::{Duration, Instant};

// 导入 Win32 窗口句柄、消息参数与枚举上下文。
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
// 导入主题变化通知与目标窗口枚举能力。
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, SendMessageW,
    WM_SETTINGCHANGE,
};
// 导入 Win32 回调布尔返回值。
use windows::core::BOOL;

// 引入唯一主演示进程 fixture。
use super::process::DemoProcess;

// 全局主题偏好一次只能由一个当前进程测试修改。
static SYSTEM_THEME_LOCK: Mutex<()> = Mutex::new(());

// 保存当前用户注册表根键伪句柄。
const HKEY_CURRENT_USER: *mut std::ffi::c_void = 0x8000_0001usize as *mut std::ffi::c_void;
// 保存读取注册表值所需最小权限。
const KEY_QUERY_VALUE: u32 = 0x0001;
// 保存写入注册表值所需最小权限。
const KEY_SET_VALUE: u32 = 0x0002;
// 保存 DWORD 注册表类型常量。
const REG_DWORD: u32 = 4;
// 保存 Windows 应用主题偏好的精确键路径。
const PERSONALIZE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
// 保存 Windows 应用主题偏好的精确值名称。
const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

// 声明注册表读写所需的最小 Advapi32 入口。
#[link(name = "advapi32")]
// Rust 2024 要求显式标记不安全外部符号声明。
unsafe extern "system" {
    // 打开当前用户主题偏好键。
    fn RegOpenKeyExW(
        key: *mut std::ffi::c_void,
        sub_key: *const u16,
        options: u32,
        access: u32,
        result: *mut *mut std::ffi::c_void,
    ) -> i32;
    // 读取应用主题 DWORD 值。
    fn RegQueryValueExW(
        key: *mut std::ffi::c_void,
        value_name: *const u16,
        reserved: *mut u32,
        value_type: *mut u32,
        data: *mut u8,
        data_size: *mut u32,
    ) -> i32;
    // 写入应用主题 DWORD 值。
    fn RegSetValueExW(
        key: *mut std::ffi::c_void,
        value_name: *const u16,
        reserved: u32,
        value_type: u32,
        data: *const u8,
        data_size: u32,
    ) -> i32;
    // 关闭当前函数打开的主题偏好键。
    fn RegCloseKey(key: *mut std::ffi::c_void) -> i32;
}

// 保存可恢复的当前用户应用主题偏好事务。
pub(crate) struct SystemThemePreference {
    // 保存测试开始前的精确 DWORD 值。
    original: u32,
    // 记录显式恢复是否已经验证成功。
    restored: bool,
}

// 实现应用主题偏好的读取、切换与恢复事务。
impl SystemThemePreference {
    // 读取并保存当前用户原始偏好。
    pub(crate) fn capture() -> Result<Self, String> {
        // 只有成功读取原值后才允许建立可写事务。
        Ok(Self {
            // 保存精确原值供 Drop 兜底恢复。
            original: read_apps_use_light_theme()?,
            // 尚未执行显式恢复。
            restored: false,
        })
    }

    // 返回测试开始前的精确偏好值。
    pub(crate) fn original(&self) -> u32 {
        // 只读返回 DWORD，不转移恢复责任。
        self.original
    }

    // 把当前偏好切换到与原值相反的明暗模式。
    pub(crate) fn toggle(&self) -> Result<u32, String> {
        // 原值为暗色零时切到亮色一，否则切到暗色零。
        let next = u32::from(self.original == 0);
        // 只写入确切 AppsUseLightTheme 值。
        write_apps_use_light_theme(next)?;
        // 写后立即读回，避免把未生效操作当成验收前提。
        let actual = read_apps_use_light_theme()?;
        // 读回值必须精确命中目标。
        if actual != next {
            // 返回包含期望和实际值的定向错误。
            return Err(format!(
                "AppsUseLightTheme write was not observable: expected {next}, got {actual}"
            ));
        }
        // 返回已经验证生效的新值。
        Ok(next)
    }

    // 显式恢复并验证测试开始前的原始偏好。
    pub(crate) fn restore(&mut self) -> Result<(), String> {
        // 写回捕获的精确原值。
        write_apps_use_light_theme(self.original)?;
        // 立即读回恢复结果。
        let actual = read_apps_use_light_theme()?;
        // 恢复值必须精确一致。
        if actual != self.original {
            // 保持 restored 为 false 以便 Drop 再次兜底。
            return Err(format!(
                "AppsUseLightTheme restore was not observable: expected {}, got {actual}",
                self.original
            ));
        }
        // 标记本事务已经完成可验证恢复。
        self.restored = true;
        // 返回恢复成功。
        Ok(())
    }
}

// 确保 panic 或早退仍尽力恢复当前用户原始主题偏好。
impl Drop for SystemThemePreference {
    // 释放事务前检查显式恢复状态。
    fn drop(&mut self) {
        // 只有尚未恢复时才执行兜底写回。
        if !self.restored {
            // Drop 不能覆盖原测试失败，但仍尽力减少全局偏好影响。
            let _ = write_apps_use_light_theme(self.original);
        }
    }
}

// 取得当前测试进程内唯一系统主题变更许可。
pub(crate) fn lock() -> MutexGuard<'static, ()> {
    // 中毒锁仍恢复唯一互斥所有权。
    SYSTEM_THEME_LOCK
        // 等待同进程其它主题验收结束。
        .lock()
        // 保留中毒后的互斥守卫。
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// 向主演示真实 HWND 同步发送系统设置变化消息。
pub(crate) fn notify_demo(demo: &DemoProcess) {
    // 有界查找当前 fixture 最大可见顶层窗口。
    let window = wait_for_window(demo.process_id());
    // 消息不携带借用指针，只要求目标窗口重新读取系统主题。
    unsafe {
        // 同步投递确保返回时窗口过程已经观察设置变化。
        SendMessageW(window, WM_SETTINGCHANGE, Some(WPARAM(0)), Some(LPARAM(0)));
    }
}

// 保存枚举期间唯一进程窗口搜索状态。
struct WindowSearch {
    // 保存目标主演示进程 ID。
    process_id: u32,
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
    // 只接受目标进程中足够大的顶层窗口。
    if process_id == search.process_id && has_bounds && width >= 320 && height >= 240 {
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
fn wait_for_window(process_id: u32) -> HWND {
    // 设置窗口创建最长等待时间。
    let deadline = Instant::now() + Duration::from_secs(10);
    // 持续查询直到窗口出现或超时。
    loop {
        // 创建本轮枚举状态。
        let mut search = WindowSearch {
            // 锁定 fixture 子进程。
            process_id,
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
        assert!(
            Instant::now() < deadline,
            "timed out locating demo HWND for theme notification"
        );
        // 短暂等待避免忙轮询。
        thread::sleep(Duration::from_millis(25));
    }
}

// 把 Rust 字符串转换为带结尾空字符的 UTF-16。
fn wide(value: &str) -> Vec<u16> {
    // Win32 注册表入口要求零结尾宽字符串。
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

// 以指定最小权限打开当前用户主题偏好键。
fn open_personalize_key(access: u32) -> Result<*mut std::ffi::c_void, String> {
    // 构造同步调用期间有效的 UTF-16 子键路径。
    let sub_key = wide(PERSONALIZE_KEY);
    // 创建句柄输出槽。
    let mut key = std::ptr::null_mut();
    // 伪句柄、字符串与输出指针在同步调用期间有效。
    let status = unsafe {
        // 只请求调用方提供的读或写最小权限。
        RegOpenKeyExW(HKEY_CURRENT_USER, sub_key.as_ptr(), 0, access, &mut key)
    };
    // 失败或空句柄都不能继续访问。
    if status != 0 || key.is_null() {
        // 返回原生状态码供定向诊断。
        Err(format!("RegOpenKeyExW Personalize failed: {status}"))
    } else {
        // 返回由调用方负责关闭的有效句柄。
        Ok(key)
    }
}

// 读取当前用户 AppsUseLightTheme DWORD。
fn read_apps_use_light_theme() -> Result<u32, String> {
    // 只以查询权限打开精确键。
    let key = open_personalize_key(KEY_QUERY_VALUE)?;
    // 构造值名称宽字符串。
    let value_name = wide(APPS_USE_LIGHT_THEME);
    // 创建 DWORD 输出槽。
    let mut value = 0_u32;
    // 创建类型输出槽。
    let mut value_type = 0_u32;
    // 声明四字节输出缓冲大小。
    let mut data_size = std::mem::size_of::<u32>() as u32;
    // 键与全部输出缓冲在同步调用期间有效。
    let status = unsafe {
        // 读取唯一目标值。
        RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            (&mut value as *mut u32).cast::<u8>(),
            &mut data_size,
        )
    };
    // 关闭本函数成功打开的键。
    let _ = unsafe { RegCloseKey(key) };
    // 只接受成功、DWORD 与四字节精确形状。
    if status != 0 || value_type != REG_DWORD || data_size != 4 {
        // 返回完整形状诊断。
        Err(format!(
            "RegQueryValueExW AppsUseLightTheme failed: status={status}, type={value_type}, size={data_size}"
        ))
    } else {
        // 返回零暗色或一亮色原值。
        Ok(value)
    }
}

// 写入当前用户 AppsUseLightTheme DWORD。
fn write_apps_use_light_theme(value: u32) -> Result<(), String> {
    // 只以设置值权限打开精确键。
    let key = open_personalize_key(KEY_SET_VALUE)?;
    // 构造值名称宽字符串。
    let value_name = wide(APPS_USE_LIGHT_THEME);
    // 键、值名称与四字节数据在同步调用期间有效。
    let status = unsafe {
        // 只覆盖精确主题偏好值。
        RegSetValueExW(
            key,
            value_name.as_ptr(),
            0,
            REG_DWORD,
            (&value as *const u32).cast::<u8>(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    // 关闭本函数成功打开的键。
    let _ = unsafe { RegCloseKey(key) };
    // 成功状态直接返回。
    if status == 0 {
        // 写入已经由调用方后续读回验证。
        Ok(())
    } else {
        // 返回原生状态码供定向诊断。
        Err(format!("RegSetValueExW AppsUseLightTheme failed: {status}"))
    }
}
