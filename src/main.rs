//! CapsLock 重映射工具(系统托盘常驻):
//! - 短按(默认 500ms 内松开)→ 模拟 Win+Space 切换输入法
//! - 长按(≥默认 500ms,选项面板可调)→ 合成 CapsLock 按下/释放,切换大小写
//!
//! 无窗口应用:隐藏消息窗口接收托盘回调,右键托盘图标弹出菜单可退出。
//! 基于 WH_KEYBOARD_LL 全局低级键盘钩子,吞噬原始 CapsLock 事件,
//! 自行判定长短按后注入合成事件。

#![windows_subsystem = "windows"]

mod caps;
mod config;

use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError, HWND, LPARAM,
    LRESULT, POINT, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    DEFAULT_GUI_FONT, GetStockObject, SetBkMode, SetTextColor, TRANSPARENT, WHITE_BRUSH, HBRUSH,
    HDC,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CAPITAL, VK_LWIN,
    VK_SPACE,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NOTIFYICONDATAW_0, NIF_ICON, NIF_MESSAGE, NIF_TIP,
    NIM_ADD, NIM_DELETE, NIM_SETVERSION, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BN_CLICKED, BS_CHECKBOX, BS_DEFPUSHBUTTON, CallNextHookEx,
    CreateIconFromResource, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, ES_NUMBER, GetCursorPos,
    GetDlgItem, GetMessageW, GetSystemMetrics, GetWindowTextW, KillTimer, LoadIconW, MessageBoxW,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowsHookExW, ShowWindow, SM_CXSCREEN, SM_CYSCREEN,
    SW_SHOW, TrackPopupMenu, WS_EX_CLIENTEDGE, GetDlgCtrlID, ICON_BIG, ICON_SMALL,
    WM_CTLCOLORBTN, WM_CTLCOLORSTATIC, WM_SETICON,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, HWND_MESSAGE, HICON,
    IDI_APPLICATION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MB_ICONERROR, MB_OK, MF_CHECKED,
    MF_SEPARATOR, MF_STRING, MSG, TPM_RETURNCMD, TPM_RIGHTBUTTON, WH_KEYBOARD_LL, WM_APP,
    WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONUP,
        WM_NULL, WM_RBUTTONUP, WM_SETFONT, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WS_CAPTION,
    WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WNDCLASSW,
};
/// BM_SETCHECK / BM_GETCHECK 消息(0.59 未导出控件 API,用消息字面量)
const BM_SETCHECK: u32 = 0x00F1;
const BM_GETCHECK: u32 = 0x00F0;
/// 按钮勾选状态值
const BST_CHECKED: usize = 1;

use caps::{Action, CapsWatcher};

/// SetTimer 定时器 id
const CAPS_TIMER_ID: usize = 1;
/// 托盘图标 uID
const TRAY_ICON_ID: u32 = 1;
/// 托盘回调消息(WM_APP + 1)
const WM_TRAYICON: u32 = WM_APP + 1;
/// 托盘菜单"退出"命令 ID
const ID_MENU_EXIT: usize = 1001;
/// 托盘菜单"开机启动"命令 ID
const ID_MENU_AUTOSTART: usize = 1002;
/// 托盘菜单"选项"命令 ID
const ID_MENU_OPTIONS: usize = 1003;
/// 隐藏窗口类名
const WINDOW_CLASS: &str = "CapIMESwitchTrayWindow";
/// 单实例互斥体名(跨进程唯一)
const SINGLE_INSTANCE_NAME: &str = "CapIMESwitch_SingleInstance";
/// 开机自启动注册表路径(HKCU 下)
const AUTOSTART_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// 自启动注册表值名
const AUTOSTART_VALUE_NAME: &str = "CapIMESwitch";
/// 选项面板窗口类名
const OPTIONS_WINDOW_CLASS: &str = "CapIMESwitchOptionsWindow";
/// 选项面板控件 ID
const ID_OPT_CHECK_AUTOSTART: usize = 2001;
const ID_OPT_EDIT_DELAY: usize = 2002;
const ID_OPT_SAVE: usize = 2003;
/// 说明文本控件 ID(用于 WM_CTLCOLORSTATIC 区分灰色说明文字)
const ID_OPT_HINT: usize = 2004;
/// 构建期生成的自绘 ICO(多尺寸 16/32/48)
static APP_ICON_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.ico"));
/// 自绘图标句柄缓存(惰性加载一次,存位模式以保持 static Sync)
static APP_ICON: AtomicUsize = AtomicUsize::new(0);

/// 全局状态机(单线程执行,锁仅作防御)
static WATCHER: Mutex<CapsWatcher> = Mutex::new(CapsWatcher::new());
/// 定时器是否武装中
static TIMER_ARMED: Mutex<bool> = Mutex::new(false);
/// 当前生效的长按判定阈值(毫秒):启动时从 config.toml 加载,保存设置后更新
static LONG_PRESS_MS: Mutex<u32> = Mutex::new(config::DEFAULT_LONG_PRESS_MS);
/// 选项面板是否打开(打开期间屏蔽托盘交互)
static OPTIONS_OPEN: AtomicBool = AtomicBool::new(false);
/// 隐藏窗口句柄(托盘回调路由),存位模式以保持 static Sync
static TRAY_HWND: AtomicUsize = AtomicUsize::new(0);
/// TaskbarCreated 动态消息号(资源管理器重启后重建图标)
static TASKBAR_CREATED_MSG: AtomicU32 = AtomicU32::new(0);

/// 从嵌入 ICO 中解析最大尺寸的图像数据(BITMAPINFOHEADER 起始偏移与长度)
///
/// ICO 文件头之后可能含多个尺寸(16/32/48),CreateIconFromResource
/// 需要单个图像目录条目对应的数据。返回尺寸最大的那个。
fn ico_largest_image(data: &[u8]) -> Option<(*const u8, u32)> {
    if data.len() < 6 {
        return None;
    }
    // ICONDIR: reserved(2) | type(2) | count(2)
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    let mut best: Option<(usize, usize, u32)> = None; // (尺寸, offset, length)
    for i in 0..count {
        let entry = 6 + i * 16;
        if entry + 16 > data.len() {
            break;
        }
        let width = data[entry] as usize; // 0 表示 256
        let height = data[entry + 1] as usize;
        let size = if width == 0 { 256 } else { width.max(height) };
        let len = u32::from_le_bytes([
            data[entry + 8],
            data[entry + 9],
            data[entry + 10],
            data[entry + 11],
        ]) as usize;
        let off = u32::from_le_bytes([
            data[entry + 12],
            data[entry + 13],
            data[entry + 14],
            data[entry + 15],
        ]) as usize;
        if off + len <= data.len() && best.is_none_or(|(bs, _, _)| size > bs) {
            best = Some((size, off, len as u32));
        }
    }
    best.map(|(_, off, len)| unsafe { (data.as_ptr().add(off), len) })
}

/// 从嵌入字节加载自绘图标(仅加载一次,失败返回 None 由调用方兜底)
fn load_app_icon() -> Option<HICON> {
    let cached = APP_ICON.load(Ordering::Relaxed);
    if cached != 0 {
        return Some(cached as HICON);
    }
    let icon = ico_largest_image(APP_ICON_BYTES).and_then(|(ptr, len)| unsafe {
        let icon = CreateIconFromResource(ptr, len, 1, 0x00030000);
        (!icon.is_null()).then_some(icon)
    });
    if let Some(h) = icon {
        APP_ICON.store(h as usize, Ordering::Relaxed);
    }
    icon
}

fn main() {
    unsafe {
        // 单实例:命名互斥体跨进程唯一(类名仅进程内有效,不能用于跨进程检测)
        let mutex_name = to_utf16(SINGLE_INSTANCE_NAME);
        let instance_mutex = CreateMutexW(std::ptr::null_mut(), 0, mutex_name.as_ptr());
        if instance_mutex.is_null() {
            show_error("创建实例锁失败");
            return;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            show_error("CapIMESwitch 已在运行中,请从系统托盘操作。");
            return;
        }
        // 句柄保持存活至进程退出(进程结束自动释放,无需 CloseHandle)

        let hmod = GetModuleHandleW(std::ptr::null::<u16>());

        if !register_window_class(hmod) {
            show_error(&format!("注册窗口类失败, GetLastError={}", GetLastError()));
            return;
        }
        if !register_options_window_class(hmod) {
            show_error(&format!("注册选项窗口类失败, GetLastError={}", GetLastError()));
            return;
        }

        let hwnd = create_hidden_window(hmod);
        if hwnd.is_null() {
            show_error("创建托盘窗口失败");
            return;
        }
        TRAY_HWND.store(hwnd as usize, Ordering::Relaxed);

        // 监听资源管理器重启(托盘图标丢失时自动重建)
        let class_name = to_utf16("TaskbarCreated");
        TASKBAR_CREATED_MSG.store(
            RegisterWindowMessageW(class_name.as_ptr()),
            Ordering::Relaxed,
        );

        if !add_tray_icon(hwnd) {
            show_error("添加托盘图标失败");
            return;
        }

        // 加载持久化的长按阈值(缺失或损坏时保持默认值)
        if let Some(path) = config_path() {
            *LONG_PRESS_MS.lock() = config::load(&path).long_press_ms;
        }

        let hook: HHOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hmod, 0);
        if hook.is_null() {
            show_error(&format!("安装键盘钩子失败, GetLastError={}", GetLastError()));
            return;
        }

        // 消息泵:派发托盘回调、WM_TIMER 与 WM_DESTROY 给窗口过程
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            match ret {
                -1 => break,
                0 => break, // WM_QUIT
                _ => {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        KillTimer(hwnd, CAPS_TIMER_ID);
        remove_tray_icon(hwnd);
        UnhookWindowsHookEx(hook);
        if let Some(icon) = load_app_icon() {
            DestroyIcon(icon);
            APP_ICON.store(0, Ordering::Relaxed);
        }
    }
}

// ---------- 窗口与托盘 ----------

/// 注册隐藏窗口类(托盘回调需要接收消息的窗口)
unsafe fn register_window_class(hmod: *mut core::ffi::c_void) -> bool {
    let class_name = to_utf16(WINDOW_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hmod,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    RegisterClassW(&wc) != 0
}

/// 创建隐藏消息窗口(不显示、不在任务栏)
unsafe fn create_hidden_window(hmod: *mut core::ffi::c_void) -> HWND {
    let class_name = to_utf16(WINDOW_CLASS);
    let title = to_utf16("CapIMESwitch");
    CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE, // 消息窗口父级,窗口完全不可见
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    )
}

/// 窗口过程:处理托盘回调、菜单命令、定时器
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            // 选项面板打开期间屏蔽托盘交互,避免面板与菜单状态不同步
            if OPTIONS_OPEN.load(Ordering::Relaxed) {
                return 0;
            }
            let evt = (lparam as usize) & 0xffff;
            match evt as u32 {
                WM_RBUTTONUP | WM_CONTEXTMENU | WM_LBUTTONUP => show_tray_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_TIMER => {
            if wparam == CAPS_TIMER_ID as _ {
                on_timer_tick();
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        m if m == TASKBAR_CREATED_MSG.load(Ordering::Relaxed) => {
            // Explorer 重启:重建托盘图标
            add_tray_icon(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 添加系统托盘图标
unsafe fn add_tray_icon(hwnd: HWND) -> bool {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = load_app_icon().unwrap_or_else(|| unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) });
    set_utf16(&mut nid.szTip, "CapIMESwitch - CapsLock 智能切换");
    let ok = Shell_NotifyIconW(NIM_ADD, &nid) != 0;
    if ok {
        nid.Anonymous = NOTIFYICONDATAW_0 {
            uVersion: NOTIFYICON_VERSION_4,
        };
        Shell_NotifyIconW(NIM_SETVERSION, &nid);
    }
    ok
}

/// 移除系统托盘图标(退出前)
unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

/// 托盘右键菜单:开机启动(带勾选标记)+ 退出
unsafe fn show_tray_menu(hwnd: HWND) {
    SetForegroundWindow(hwnd);
    let menu = CreatePopupMenu();

    // 开机启动:根据注册表当前状态显示勾选
    let auto_start = is_autostart_enabled();
    let autostart_label = to_utf16("开机启动");
    let autostart_flags = MF_STRING | if auto_start { MF_CHECKED } else { 0 };
    AppendMenuW(menu, autostart_flags, ID_MENU_AUTOSTART, autostart_label.as_ptr());

    let options_label = to_utf16("选项");
    AppendMenuW(menu, MF_STRING, ID_MENU_OPTIONS, options_label.as_ptr());

    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

    let exit_label = to_utf16("退出");
    AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, exit_label.as_ptr());

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    // TPM_RETURNCMD:返回值即选中的命令 ID
    let cmd = TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON | TPM_RETURNCMD,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    DestroyMenu(menu);

    match cmd as usize {
        ID_MENU_AUTOSTART => {
            if !toggle_autostart() {
                show_error("修改开机自启动设置失败");
            }
        }
        ID_MENU_OPTIONS => open_options_panel(hwnd),
        ID_MENU_EXIT => PostQuitMessage(0),
        _ => {}
    }
    // 让菜单正确关闭的标准做法
    PostMessageW(hwnd, WM_NULL, 0, 0);
}

// ---------- 注册表通用读写(参数化,便于测试) ----------

/// 读取 REG_SZ 值,不存在或非 REG_SZ 返回 None
fn reg_read_value(root: HKEY, subkey: &str, name: &str) -> Option<String> {
    let subkey_w = to_utf16(subkey);
    let name_w = to_utf16(name);
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(root, subkey_w.as_ptr(), 0, KEY_QUERY_VALUE, &mut key);
        if status != ERROR_SUCCESS || key.is_null() {
            return None;
        }
        let mut size: u32 = 0;
        let status = RegQueryValueExW(
            key,
            name_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut size,
        );
        if status != ERROR_SUCCESS {
            RegCloseKey(key);
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let mut value_type: REG_VALUE_TYPE = 0;
        let status = RegQueryValueExW(
            key,
            name_w.as_ptr(),
            std::ptr::null(),
            &mut value_type,
            buf.as_mut_ptr(),
            &mut size,
        );
        RegCloseKey(key);
        if status != ERROR_SUCCESS || value_type != REG_SZ {
            return None;
        }
        let stored = String::from_utf16_lossy(
            buf.as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<_>>()
                .as_slice(),
        );
        Some(stored.trim_end_matches('\0').to_string())
    }
}

/// 写入 REG_SZ 值(打开或创建子键),返回是否成功
fn reg_write_value(root: HKEY, subkey: &str, name: &str, value: &str) -> bool {
    let subkey_w = to_utf16(subkey);
    let name_w = to_utf16(name);
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegCreateKeyExW(
            root,
            subkey_w.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if status != ERROR_SUCCESS || key.is_null() {
            return false;
        }
        let mut wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        let written = RegSetValueExW(
            key,
            name_w.as_ptr(),
            0,
            REG_SZ,
            wide.as_mut_ptr() as *const u8,
            (wide.len() * 2) as u32,
        );
        RegCloseKey(key);
        written == ERROR_SUCCESS
    }
}

/// 删除值,值不存在也视作成功
fn reg_delete_value(root: HKEY, subkey: &str, name: &str) -> bool {
    let subkey_w = to_utf16(subkey);
    let name_w = to_utf16(name);
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(root, subkey_w.as_ptr(), 0, KEY_SET_VALUE, &mut key);
        if status != ERROR_SUCCESS || key.is_null() {
            return false;
        }
        let status = RegDeleteValueW(key, name_w.as_ptr());
        RegCloseKey(key);
        status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND
    }
}

// ---------- 开机自启动 ----------

/// 查询当前 exe 路径(注册表值内容)
fn current_exe_quoted() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    format!("\"{}\"", exe.display())
}

/// 自启动是否已开启:Run 键下存在且指向当前 exe
fn is_autostart_enabled() -> bool {
    reg_read_value(HKEY_CURRENT_USER, AUTOSTART_KEY, AUTOSTART_VALUE_NAME)
        .as_deref()
        == Some(current_exe_quoted().as_str())
}

/// 切换自启动状态,返回是否成功
fn toggle_autostart() -> bool {
    if is_autostart_enabled() {
        reg_delete_value(HKEY_CURRENT_USER, AUTOSTART_KEY, AUTOSTART_VALUE_NAME)
    } else {
        reg_write_value(
            HKEY_CURRENT_USER,
            AUTOSTART_KEY,
            AUTOSTART_VALUE_NAME,
            &current_exe_quoted(),
        )
    }
}

// ---------- 长按阈值设置(config.toml 持久化) ----------

/// 解析长按阈值输入:100-5000 范围内的整数(允许首尾空白),非法返回 None
fn parse_long_press_ms(s: &str) -> Option<u32> {
    let v = s.trim().parse::<u32>().ok()?;
    (config::MIN_LONG_PRESS_MS..=config::MAX_LONG_PRESS_MS)
        .contains(&v)
        .then_some(v)
}

/// 配置文件路径:exe 所在目录下的 config.toml(portable,安装目录用户可写)
fn config_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join("config.toml"))
}

// ---------- 选项面板 ----------

/// 注册选项面板窗口类(独立 WndProc 处理面板消息)
unsafe fn register_options_window_class(hmod: *mut core::ffi::c_void) -> bool {
    let class_name = to_utf16(OPTIONS_WINDOW_CLASS);
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(options_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hmod,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        // 白色客户区背景,与控件底色一致(控件背景画刷见 WM_CTLCOLORSTATIC)
        hbrBackground: GetStockObject(WHITE_BRUSH) as HBRUSH,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    RegisterClassW(&wc) != 0
}

/// 打开选项面板:加载当前设置到控件,创建并居中显示窗口
unsafe fn open_options_panel(owner: HWND) {
    if OPTIONS_OPEN.load(Ordering::Relaxed) {
        return;
    }
    let hmod = GetModuleHandleW(std::ptr::null::<u16>());
    let class_name = to_utf16(OPTIONS_WINDOW_CLASS);
    let title = to_utf16("CapIMESwitch 选项");
    const W: i32 = 440;
    const H: i32 = 220;
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title.as_ptr(),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
        (sw - W) / 2,
        (sh - H) / 2,
        W,
        H,
        owner,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if hwnd.is_null() {
        show_error(&format!("创建选项窗口失败, GetLastError={}", GetLastError()));
        return;
    }
    OPTIONS_OPEN.store(true, Ordering::Relaxed);
    create_options_controls(hwnd, hmod);
    // 标题栏图标与程序托盘图标一致
    if let Some(icon) = load_app_icon() {
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as WPARAM, icon as LPARAM);
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, icon as LPARAM);
    }
    ShowWindow(hwnd, SW_SHOW);
}

/// 给控件设为系统默认 GUI 字体(否则中文可能以旧字体渲染)
unsafe fn set_control_font(hwnd: HWND) {
    let font = GetStockObject(DEFAULT_GUI_FONT);
    SendMessageW(hwnd, WM_SETFONT, font as usize as WPARAM, 1 as LPARAM);
}

/// 创建选项面板子控件。
/// 布局:左侧标签列(右对齐、统一宽度)+ 右侧控件列(勾选框/输入框,统一起始 x),
/// 两行结构完全对齐;说明文字为灰色独立一行;保存按钮位于右下角。
unsafe fn create_options_controls(parent: HWND, hmod: *mut core::ffi::c_void) {
    let button_class = to_utf16("Button");
    let edit_class = to_utf16("Edit");
    let static_class = to_utf16("Static");

    // 行 1:标签(右对齐)+ 无字勾选框(控件列起始 x=212)
    let check_label = to_utf16("开机启动:");
    let label1 = CreateWindowExW(
        0,
        static_class.as_ptr(),
        check_label.as_ptr(),
        WS_CHILD | WS_VISIBLE | 0x0002, // SS_RIGHT
        16,
        22,
        190,
        20,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if !label1.is_null() {
        set_control_font(label1);
    }

    let check = CreateWindowExW(
        0,
        button_class.as_ptr(),
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_CHECKBOX as u32,
        212,
        20,
        24,
        24,
        parent,
        (ID_OPT_CHECK_AUTOSTART as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !check.is_null() {
        set_control_font(check);
        // 初始勾选状态与注册表一致(BM_SETCHECK)
        if is_autostart_enabled() {
            SendMessageW(check, BM_SETCHECK, BST_CHECKED as WPARAM, 0 as LPARAM);
        }
    }

    // 行 2:标签(右对齐)+ 延迟输入框,与行 1 控件列齐平
    let label_text = to_utf16("大写锁切换延迟(毫秒):");
    let label = CreateWindowExW(
        0,
        static_class.as_ptr(),
        label_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | 0x0002, // SS_RIGHT
        16,
        58,
        190,
        20,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if !label.is_null() {
        set_control_font(label);
    }

    let cur_ms = *LONG_PRESS_MS.lock();
    let cur_text = to_utf16(&cur_ms.to_string());
    let edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        edit_class.as_ptr(),
        cur_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_NUMBER as u32 | ES_AUTOHSCROLL as u32,
        212,
        56,
        112,
        24,
        parent,
        (ID_OPT_EDIT_DELAY as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !edit.is_null() {
        set_control_font(edit);
    }

    // 行为说明:灰色小字独立一行(颜色由 options_wnd_proc 的 WM_CTLCOLORSTATIC 处理)
    let hint_text = to_utf16("长按 CapsLock 达到该毫秒数后切换大小写，否则短按切换输入法");
    let hint = CreateWindowExW(
        0,
        static_class.as_ptr(),
        hint_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        16,
        98,
        408,
        20,
        parent,
        (ID_OPT_HINT as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !hint.is_null() {
        set_control_font(hint);
    }

    // 保存按钮(右下角,右/下边距 24)
    let save_label = to_utf16("保存");
    let save = CreateWindowExW(
        0,
        button_class.as_ptr(),
        save_label.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
        336,
        137,
        80,
        28,
        parent,
        (ID_OPT_SAVE as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !save.is_null() {
        set_control_font(save);
    }
}

/// 选项面板窗口过程:保存按钮、关闭处理
unsafe extern "system" fn options_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam as usize) & 0xffff;
            let code = ((wparam as usize) >> 16) & 0xffff;
            if id == ID_OPT_SAVE && code as u32 == BN_CLICKED {
                save_options(hwnd);
            }
            0
        }
        // 标签/说明文字绘制:白色背景融入面板,说明文字用灰色与主标签区分
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let hdc = wparam as HDC;
            SetBkMode(hdc, TRANSPARENT as i32);
            if msg == WM_CTLCOLORSTATIC && GetDlgCtrlID(lparam as HWND) == ID_OPT_HINT as i32 {
                SetTextColor(hdc, 0x00808080); // 灰色说明,与黑色标签区分
            } else {
                SetTextColor(hdc, 0x00000000); // 黑色主标签
            }
            GetStockObject(WHITE_BRUSH) as LRESULT
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        // 不 PostQuitMessage:主消息泵由托盘窗口的 WM_DESTROY 终止
        WM_DESTROY => {
            OPTIONS_OPEN.store(false, Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 保存面板设置:校验延迟 → 写注册表并更新运行时值 → 同步自启动 → 关闭面板。
/// 任一环节失败弹错误框且不关闭,用户修正后可重试。
unsafe fn save_options(hwnd: HWND) {
    let edit = GetDlgItem(hwnd, ID_OPT_EDIT_DELAY as i32);
    let mut buf = [0u16; 32];
    let len = GetWindowTextW(edit, buf.as_mut_ptr(), buf.len() as i32);
    let text = String::from_utf16_lossy(&buf[..len.max(0) as usize]);
    let ms = match parse_long_press_ms(&text) {
        Some(v) => v,
        None => {
            show_error(&format!(
                "请输入 {} - {} 之间的毫秒数",
                config::MIN_LONG_PRESS_MS, config::MAX_LONG_PRESS_MS
            ));
            return;
        }
    };
    let path = match config_path() {
        Some(p) => p,
        None => {
            show_error("无法确定配置文件位置");
            return;
        }
    };
    let cfg = config::Config { long_press_ms: ms };
    if !config::save(&cfg, &path) {
        show_error("保存设置失败");
        return;
    }
    *LONG_PRESS_MS.lock() = ms;

    // 开机启动:勾选与当前注册表一致则跳过,否则写入或删除
    let check = GetDlgItem(hwnd, ID_OPT_CHECK_AUTOSTART as i32);
    let want_enabled = SendMessageW(check, BM_GETCHECK, 0, 0) != 0;
    let need_write = want_enabled != is_autostart_enabled();
    let ok = !need_write
        || if want_enabled {
            reg_write_value(
                HKEY_CURRENT_USER,
                AUTOSTART_KEY,
                AUTOSTART_VALUE_NAME,
                &current_exe_quoted(),
            )
        } else {
            reg_delete_value(HKEY_CURRENT_USER, AUTOSTART_KEY, AUTOSTART_VALUE_NAME)
        };
    if !ok {
        show_error("保存开机自启动设置失败");
        return;
    }
    DestroyWindow(hwnd);
}

/// 以消息框展示致命错误
fn show_error(message: &str) {
    unsafe {
        let text = to_utf16(message);
        let title = to_utf16("CapIMESwitch");
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

// ---------- 键盘钩子 ----------

/// 低级键盘钩子回调。返回 1 表示吞掉该事件。
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);

        // 我们注入的事件带 LLKHF_INJECTED,直接放行,避免递归
        if kb.flags & LLKHF_INJECTED == 0 {
            let is_caps = kb.vkCode == VK_CAPITAL as u32;
            let action = match wparam as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => on_key_down(kb.vkCode),
                WM_KEYUP | WM_SYSKEYUP => on_key_up(kb.vkCode),
                _ => None,
            };

            if is_caps {
                // 吞噬 CapsLock 的原始事件,防止系统默认切换大小写
                if let Some(act) = action {
                    execute(act);
                }
                return 1;
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

/// 任意键按下(不含 CapsLock 本身)
fn on_key_down(vk: u32) -> Option<Action> {
    if vk == VK_CAPITAL as u32 {
        let mut w = WATCHER.lock();
        if w.on_caps_down() {
            arm_timer();
        }
        None
    } else {
        let mut w = WATCHER.lock();
        if w.on_other_key() {
            disarm_timer();
        }
        None
    }
}

fn on_key_up(vk: u32) -> Option<Action> {
    if vk != VK_CAPITAL as u32 {
        return None;
    }
    disarm_timer();
    let mut w = WATCHER.lock();
    match w.on_caps_up() {
        Action::SwitchIme => Some(Action::SwitchIme),
        _ => None,
    }
}

/// 500ms 定时器到期
fn on_timer_tick() {
    let mut w = WATCHER.lock();
    if w.on_timer() == Action::ToggleCapsLock {
        disarm_timer();
        execute(Action::ToggleCapsLock);
    }
}

/// 执行动作:注入合成按键事件
fn execute(action: Action) {
    match action {
        Action::SwitchIme => send_win_space(),
        Action::ToggleCapsLock => send_caps_toggle(),
        Action::None => {}
    }
}

// ---------- 按键注入 ----------

fn make_key_input(vk: u16, keyup: bool) -> INPUT {
    unsafe {
        let mut input: INPUT = std::mem::zeroed();
        input.r#type = INPUT_KEYBOARD;
        input.Anonymous.ki = KEYBDINPUT {
            wVk: vk,
            wScan: 0,
            dwFlags: if keyup { KEYEVENTF_KEYUP } else { 0 },
            time: 0,
            dwExtraInfo: 0,
        };
        input
    }
}

/// 模拟 Win+Space:切换输入法
fn send_win_space() {
    let inputs = [
        make_key_input(VK_LWIN, false),
        make_key_input(VK_SPACE, false),
        make_key_input(VK_SPACE, true),
        make_key_input(VK_LWIN, true),
    ];
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// 合成 CapsLock 按下+释放:切换大小写状态(LED 同步)
fn send_caps_toggle() {
    let inputs = [
        make_key_input(VK_CAPITAL, false),
        make_key_input(VK_CAPITAL, true),
    ];
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

// ---------- 定时器 ----------

fn arm_timer() {
    let mut armed = TIMER_ARMED.lock();
    if !*armed {
        let hwnd = TRAY_HWND.load(Ordering::Relaxed) as HWND;
        let ms = *LONG_PRESS_MS.lock();
        unsafe {
            SetTimer(hwnd, CAPS_TIMER_ID, ms, None);
        }
        *armed = true;
    }
}

fn disarm_timer() {
    let mut armed = TIMER_ARMED.lock();
    if *armed {
        let hwnd = TRAY_HWND.load(Ordering::Relaxed) as HWND;
        unsafe {
            KillTimer(hwnd, CAPS_TIMER_ID);
        }
        *armed = false;
    }
}

// ---------- UTF-16 工具 ----------

/// &str → 以 NUL 结尾的 UTF-16 序列
fn to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 将字符串写入定长 UTF-16 数组(直到数组末尾或字符串结尾,末尾自动补 0)
fn set_utf16(dst: &mut [u16], s: &str) {
    let mut it = s.encode_utf16();
    for slot in dst.iter_mut() {
        *slot = match it.next() {
            Some(c) => c,
            None => break,
        };
    }
}

#[cfg(test)]
mod icon_tests {
    use super::*;

    #[test]
    fn ico_parses_largest_image() {
        // 构建期 ICO 已知布局:3 条目,16/32/48(最大 48x48:offset=5446,len=9640)
        let (ptr, len) = ico_largest_image(APP_ICON_BYTES).expect("应能解析 ico");
        let off = unsafe { ptr.offset_from(APP_ICON_BYTES.as_ptr()) } as usize;
        assert_eq!(off, 5446);
        assert_eq!(len, 9640);
        assert!(off + len as usize <= APP_ICON_BYTES.len());
    }

    #[test]
    fn ico_picks_largest_entry() {
        // 构造内存 ICO:16x16 在前,32x32 在后
        let mut ico = Vec::new();
        ico.extend_from_slice(&[0, 0, 1, 0, 2, 0]);
        let mut body_offsets = Vec::new();
        for &(w, body_len) in &[(16u32, 100usize), (32, 200)] {
            let entry = 6 + 16 * body_offsets.len();
            ico.push(w as u8);
            ico.push(w as u8);
            ico.extend_from_slice(&[0, 0]);
            ico.extend_from_slice(&1u16.to_le_bytes());
            ico.extend_from_slice(&32u16.to_le_bytes());
            ico.extend_from_slice(&(body_len as u32).to_le_bytes());
            ico.extend_from_slice(&((22 + entry) as u32).to_le_bytes());
            body_offsets.push(22 + entry);
        }
        ico.extend(std::iter::repeat_n(0u8, body_offsets[1] + 200));
        let (ptr, len) = ico_largest_image(&ico).expect("应能解析");
        let off = unsafe { ptr.offset_from(ico.as_ptr()) } as usize;
        assert_eq!(off, body_offsets[1], "应选中 32x32");
        assert_eq!(len, 200);
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use windows_sys::Win32::System::Registry::RegDeleteTreeW;

    /// 测试专用临时键(避免写真实 Run 键),测试结束删除。
    /// 每个测试用独立子键,防止并行执行时互相清理。
    const TEST_KEY_A: &str = r"Software\CapIMESwitch_Test_A";
    const TEST_KEY_B: &str = r"Software\CapIMESwitch_Test_B";

    fn cleanup(key: &str) {
        let key_w = to_utf16(key);
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, key_w.as_ptr());
        }
    }

    #[test]
    fn write_read_delete_roundtrip() {
        cleanup(TEST_KEY_A);
        // 初始不存在
        assert_eq!(reg_read_value(HKEY_CURRENT_USER, TEST_KEY_A, "v"), None);
        // 写入并读回
        assert!(reg_write_value(
            HKEY_CURRENT_USER,
            TEST_KEY_A,
            "v",
            r#""C:\Program Files\cap-ime-switch.exe""#
        ));
        assert_eq!(
            reg_read_value(HKEY_CURRENT_USER, TEST_KEY_A, "v").as_deref(),
            Some(r#""C:\Program Files\cap-ime-switch.exe""#)
        );
        // 覆盖写入
        assert!(reg_write_value(HKEY_CURRENT_USER, TEST_KEY_A, "v", "second"));
        assert_eq!(
            reg_read_value(HKEY_CURRENT_USER, TEST_KEY_A, "v").as_deref(),
            Some("second")
        );
        // 删除后不存在
        assert!(reg_delete_value(HKEY_CURRENT_USER, TEST_KEY_A, "v"));
        assert_eq!(reg_read_value(HKEY_CURRENT_USER, TEST_KEY_A, "v"), None);
        // 删除不存在的值也视为成功(幂等)
        assert!(reg_delete_value(HKEY_CURRENT_USER, TEST_KEY_A, "v"));
        cleanup(TEST_KEY_A);
    }

    #[test]
    fn unicode_value_roundtrip() {
        cleanup(TEST_KEY_B);
        assert!(reg_write_value(
            HKEY_CURRENT_USER,
            TEST_KEY_B,
            "v",
            "含中文\"路径\""
        ));
        assert_eq!(
            reg_read_value(HKEY_CURRENT_USER, TEST_KEY_B, "v").as_deref(),
            Some("含中文\"路径\"")
        );
        cleanup(TEST_KEY_B);
    }

    #[test]
    fn parse_long_press_ms_validation() {
        // 合法值:范围内且容忍首尾空白
        assert_eq!(parse_long_press_ms("500"), Some(500));
        assert_eq!(parse_long_press_ms(" 500 "), Some(500));
        assert_eq!(parse_long_press_ms("100"), Some(100), "下边界");
        assert_eq!(parse_long_press_ms("5000"), Some(5000), "上边界");
        // 非法值:越界 / 非数字 / 空 / 负数 / 小数
        assert_eq!(parse_long_press_ms("99"), None, "低于下边界");
        assert_eq!(parse_long_press_ms("5001"), None, "高于上边界");
        assert_eq!(parse_long_press_ms(""), None);
        assert_eq!(parse_long_press_ms("abc"), None);
        assert_eq!(parse_long_press_ms("-1"), None);
        assert_eq!(parse_long_press_ms("0"), None);
        assert_eq!(parse_long_press_ms("500.5"), None);
    }
}