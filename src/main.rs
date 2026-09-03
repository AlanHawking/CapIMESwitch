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
mod i18n;

use i18n::Lang;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, GetLastError, HWND, LPARAM,
    LRESULT, POINT, RECT, SIZE, SYSTEMTIME, WPARAM,
};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::Graphics::Gdi::{
    DEFAULT_GUI_FONT, GetDC, GetStockObject, GetTextExtentPoint32W, ReleaseDC, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT, WHITE_BRUSH, BI_RGB, BITMAPINFO, BITMAPINFOHEADER,
    DIB_RGB_COLORS, CreateDIBSection, CreateBitmap, DeleteObject, HBRUSH, HBITMAP, HDC,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, OpenProcess, QueryFullProcessImageNameW,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, TrackMouseEvent, TRACKMOUSEEVENT, TME_LEAVE, INPUT, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, VK_CAPITAL, VK_LWIN, VK_SPACE,
};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NOTIFYICONDATAW_0, NIF_ICON, NIF_MESSAGE, NIF_TIP,
    NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, CallNextHookEx,
    GetForegroundWindow, GetWindowThreadProcessId,
    CreateIconFromResource, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, ICONINFO,
    DestroyMenu, DestroyWindow, DispatchMessageW, ES_AUTOHSCROLL, ES_MULTILINE, ES_NUMBER,
    ES_AUTOVSCROLL, ES_READONLY, ES_WANTRETURN, GetCursorPos, CB_ADDSTRING, CB_GETCURSEL,
    CB_SETCURSEL, CBS_DROPDOWNLIST, CBS_HASSTRINGS,
    GetClassLongPtrW, GetClientRect, GetDlgCtrlID, GetDlgItem, GetMessageW, GetSystemMetrics,
    GetParent, GetWindowTextW, GCLP_WNDPROC, GWLP_WNDPROC, KillTimer,
    LoadIconW, MessageBoxW, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SendMessageW, SetWindowLongPtrW, SetWindowTextW,
    SetForegroundWindow, SetTimer, SetWindowsHookExW, ShowWindow, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, SW_SHOW, TrackPopupMenu,
    CallWindowProcW, WS_EX_CLIENTEDGE, ICON_BIG,
    ICON_SMALL,
    WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_SETICON,
    TranslateMessage, UnhookWindowsHookEx, HC_ACTION, HHOOK, HMENU, HWND_MESSAGE, HICON,
    IDI_APPLICATION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MB_ICONERROR, MB_OK, MF_CHECKED,
    MF_POPUP, MF_SEPARATOR, MF_STRING, MIIM_BITMAP, MENUITEMINFOW, MSG, SetMenuItemInfoW,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WH_KEYBOARD_LL, WM_APP,
    WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONUP,
    WM_MOUSEMOVE,
        WM_NULL, WM_RBUTTONUP, WM_SETFONT, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TIMER, WS_CAPTION,
    WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
    WNDCLASSW,
};
/// BM_SETCHECK / BM_GETCHECK 消息(0.59 未导出控件 API,用消息字面量)
const BM_SETCHECK: u32 = 0x00F1;
const BM_GETCHECK: u32 = 0x00F0;
/// WM_MOUSELEAVE 未在 0.59 导出,用消息字面量
const WM_MOUSELEAVE: u32 = 0x02A3;
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
/// 托盘菜单"语言:简体中文"命令 ID
const ID_MENU_LANG_ZH: usize = 1004;
/// 托盘菜单"语言:English"命令 ID
const ID_MENU_LANG_EN: usize = 1005;
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
/// 排除程序输入框 ID
const ID_OPT_EDIT_EXCLUDE: usize = 2005;
/// 问号帮助图标 ID(逐行一个)
const ID_OPT_HELP_AUTOSTART: usize = 2008;
const ID_OPT_HELP_DELAY: usize = 2009;
const ID_OPT_HELP_EXCLUDE: usize = 2010;
/// 版本号文本 ID(底部灰色小字)
const ID_OPT_VERSION: usize = 2011;
/// 说明栏控件 ID(面板底部只读文本,点击/悬浮问号时显示说明)
const ID_OPT_HELP_BAR: usize = 2012;
/// 选项面板语言下拉框 ID
const ID_OPT_COMBO_LANGUAGE: usize = 2013;
/// 静态控件样式(0.59 未导出 SS_* 常量:SS_CENTER=0x1 水平居中,
/// SS_CENTERIMAGE=0x200 垂直居中,SS_NOTIFY=0x100 可收单击,SS_ICON=0x3 显示图标)
const SS_CENTER: u32 = 0x0001;
const SS_CENTERIMAGE: u32 = 0x0200;
const SS_NOTIFY: u32 = 0x0100;
const SS_ICON: u32 = 0x0003;
/// STM_SETICON 消息(0.59 未导出,用消息字面量)
const STM_SETICON: u32 = 0x0170;
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
/// 排除程序列表(小写 exe 文件名):启动时从 config.toml 加载,保存设置后更新
static EXCLUDE_PROCESSES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// 当前界面语言:启动时从 config.toml 解析(默认跟随系统),切换后即时更新
static LANGUAGE: Mutex<Lang> = Mutex::new(Lang::Zh);
/// 选项面板是否打开(打开期间屏蔽托盘交互)
static OPTIONS_OPEN: AtomicBool = AtomicBool::new(false);
/// 当前点击固定的问号控件 ID(说明栏保持显示);None 表示未固定
static HELP_PINNED: Mutex<Option<usize>> = Mutex::new(None);
/// Static 类原始窗口过程(问号图标的子类化链回目标)
static ORIG_STATIC_PROC: AtomicUsize = AtomicUsize::new(0);
/// 当前语言的全部用户可见字符串(锁在返回后立即释放,引用为 'static)
fn tr() -> &'static i18n::Strings {
    LANGUAGE.lock().strings()
}

/// 用参数按序替换模板中的 `{}`(模板来自 i18n 表,属运行期值,不能直接用 format!)
fn tpl(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = template.to_string();
    for a in args {
        out = out.replacen("{}", &a.to_string(), 1);
    }
    out
}

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
        log_write(&format!(
            "CapIMESwitch v{} 启动 exe={:?}",
            env!("CARGO_PKG_VERSION"),
            std::env::current_exe().unwrap_or_default()
        ));
        // 尽早加载持久化设置(缺失或损坏时保持默认值),让后续错误提示使用正确语言
        if let Some(path) = config_path() {
            let cfg = config::load(&path);
            *LONG_PRESS_MS.lock() = cfg.long_press_ms;
            *EXCLUDE_PROCESSES.lock() = cfg.exclude_processes;
            *LANGUAGE.lock() = resolve_language(&cfg.language);
        }
        // 单实例:命名互斥体跨进程唯一(类名仅进程内有效,不能用于跨进程检测)
        let mutex_name = to_utf16(SINGLE_INSTANCE_NAME);
        let instance_mutex = CreateMutexW(std::ptr::null_mut(), 0, mutex_name.as_ptr());
        if instance_mutex.is_null() {
            let err = GetLastError();
            log_write(&format!("创建实例锁失败 GetLastError={err}"));
            show_error(tr().err_instance_lock);
            return;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            log_write("检测到已有实例,退出");
            show_error(tr().err_already_running);
            return;
        }
        // 句柄保持存活至进程退出(进程结束自动释放,无需 CloseHandle)

        let hmod = GetModuleHandleW(std::ptr::null::<u16>());

        if !register_window_class(hmod) {
            let err = GetLastError();
            log_write(&format!("注册主窗口类失败 GetLastError={err}"));
            show_error(&tpl(tr().err_register_class, &[&err]));
            return;
        }
        log_write("主窗口类注册成功");
        if !register_options_window_class(hmod) {
            let err = GetLastError();
            log_write(&format!("注册选项面板窗口类失败 GetLastError={err}"));
            show_error(&tpl(tr().err_register_options_class, &[&err]));
            return;
        }
        log_write("选项面板窗口类注册成功");

        let hwnd = create_hidden_window(hmod);
        if hwnd.is_null() {
            let err = GetLastError();
            log_write(&format!("创建托盘窗口失败 GetLastError={err}"));
            show_error(tr().err_create_tray_window);
            return;
        }
        TRAY_HWND.store(hwnd as usize, Ordering::Relaxed);
        log_write(&format!("托盘窗口创建成功 hwnd={hwnd:?}"));

        // 监听资源管理器重启(托盘图标丢失时自动重建)
        let class_name = to_utf16("TaskbarCreated");
        TASKBAR_CREATED_MSG.store(
            RegisterWindowMessageW(class_name.as_ptr()),
            Ordering::Relaxed,
        );

        if !add_tray_icon(hwnd) {
            let err = GetLastError();
            log_write(&format!("添加托盘图标失败 GetLastError={err}"));
            show_error(tr().err_add_tray_icon);
            return;
        }
        log_write("托盘图标添加成功");

        let hook: HHOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hmod, 0);
        if hook.is_null() {
            let err = GetLastError();
            log_write(&format!("安装键盘钩子失败 GetLastError={err}"));
            show_error(&tpl(tr().err_install_hook, &[&err]));
            return;
        }
        log_write("键盘钩子安装成功,进入消息循环");

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

        log_write("程序退出");
        KillTimer(hwnd, CAPS_TIMER_ID);
        remove_tray_icon(hwnd);
        destroy_menu_icon_bitmaps();
        destroy_panel_icons();
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
    set_utf16(&mut nid.szTip, tr().tray_tooltip);
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

/// 托盘右键菜单:开机启动(带勾选标记)+ 选项 + 语言子菜单 + 退出
unsafe fn show_tray_menu(hwnd: HWND) {
    SetForegroundWindow(hwnd);
    let menu = CreatePopupMenu();
    let strings = tr();

    // 开机启动:根据注册表当前状态显示勾选
    let auto_start = is_autostart_enabled();
    let autostart_label = to_utf16(strings.menu_autostart);
    let autostart_flags = MF_STRING | if auto_start { MF_CHECKED } else { 0 };
    AppendMenuW(menu, autostart_flags, ID_MENU_AUTOSTART, autostart_label.as_ptr());

    let options_label = to_utf16(strings.menu_options);
    AppendMenuW(menu, MF_STRING, ID_MENU_OPTIONS, options_label.as_ptr());

    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

    // 语言子菜单:各语言带互斥勾选,选项显示母语名
    let lang_menu = CreatePopupMenu();
    let cur_lang = *LANGUAGE.lock();
    let zh_label = to_utf16(strings.lang_zh);
    let en_label = to_utf16(strings.lang_en);
    AppendMenuW(
        lang_menu,
        MF_STRING | if cur_lang == Lang::Zh { MF_CHECKED } else { 0 },
        ID_MENU_LANG_ZH,
        zh_label.as_ptr(),
    );
    AppendMenuW(
        lang_menu,
        MF_STRING | if cur_lang == Lang::En { MF_CHECKED } else { 0 },
        ID_MENU_LANG_EN,
        en_label.as_ptr(),
    );
    let lang_label = to_utf16(strings.menu_language);
    AppendMenuW(menu, MF_POPUP, lang_menu as usize, lang_label.as_ptr());

    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());

    let exit_label = to_utf16(strings.menu_exit);
    AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, exit_label.as_ptr());

    // 给各菜单项挂左侧图标(语言地球/开机启动电源/选项滑杆/退出 X)
    set_menu_item_icon(menu, ID_MENU_AUTOSTART, menu_icon_bitmap(MenuIcon::Autostart));
    set_menu_item_icon(menu, ID_MENU_OPTIONS, menu_icon_bitmap(MenuIcon::Options));
    set_menu_item_icon(menu, lang_menu as usize, menu_icon_bitmap(MenuIcon::Language));
    set_menu_item_icon(menu, ID_MENU_EXIT, menu_icon_bitmap(MenuIcon::Exit));

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
                show_error(tr().err_toggle_autostart);
            }
        }
        ID_MENU_OPTIONS => open_options_panel(hwnd),
        ID_MENU_LANG_ZH => set_language(Lang::Zh),
        ID_MENU_LANG_EN => set_language(Lang::En),
        ID_MENU_EXIT => PostQuitMessage(0),
        _ => {}
    }
    // 让菜单正确关闭的标准做法
    PostMessageW(hwnd, WM_NULL, 0, 0);
}

// ---------- 语言(i18n) ----------

/// 解析配置中的语言值:zh/en 直接使用;auto 与未知值按系统 UI 语言解析
fn resolve_language(cfg_lang: &str) -> Lang {
    match Lang::parse(cfg_lang) {
        Some(l) => l,
        None => system_language(),
    }
}

/// 系统 UI 语言:按 LCID 主语言映射 zh/en,未知回退中文
fn system_language() -> Lang {
    lang_from_lcid(unsafe { GetUserDefaultUILanguage() })
}

/// 按 LCID 主语言(低 10 位)映射语言:0x04 中文系 / 0x09 英文系,其余回退中文
fn lang_from_lcid(lcid: u16) -> Lang {
    match lcid & 0x3FF {
        0x04 => Lang::Zh, // 中文系(zh-CN/HK/TW/SG/MO)
        0x09 => Lang::En, // 英文系
        _ => Lang::Zh,    // 未知语言回退中文
    }
}

/// 切换运行语言:更新全局状态、托盘 tooltip,并持久化到 config.toml(language 字段)
fn set_language(lang: Lang) {
    {
        let mut cur = LANGUAGE.lock();
        if *cur == lang {
            return;
        }
        *cur = lang;
    }
    log_write(&format!("切换语言: {}", lang.code()));
    unsafe { update_tray_tooltip() };
    if let Some(path) = config_path() {
        let mut cfg = config::load(&path);
        cfg.language = lang.code().to_string();
        if !config::save(&cfg, &path) {
            log_write("保存语言设置失败");
        }
    }
}

/// 刷新托盘 tooltip(语言切换后即时生效)
unsafe fn update_tray_tooltip() {
    let hwnd = TRAY_HWND.load(Ordering::Relaxed) as HWND;
    if hwnd.is_null() {
        return;
    }
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_TIP;
    set_utf16(&mut nid.szTip, tr().tray_tooltip);
    Shell_NotifyIconW(NIM_MODIFY, &nid);
}

// ---------- 托盘菜单图标 ----------

/// 菜单图标尺寸(像素)
const MENU_ICON_SIZE: i32 = 16;
/// 图标主色(品牌蓝,与 build.rs 图标一致)
const ICON_BLUE: [f32; 3] = [0.13, 0.55, 1.0];
/// 地球经纬线亮色
const ICON_LIGHT: [f32; 3] = [0.70, 0.86, 1.0];

/// 托盘菜单/选项面板图标种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuIcon {
    /// 语言:地球
    Language,
    /// 开机启动:电源按钮
    Autostart,
    /// 选项:滑杆
    Options,
    /// 退出:X
    Exit,
    /// 长按延迟:时钟
    Clock,
    /// 排除程序:禁止
    Blocked,
}

/// 直通 alpha 合成器
#[derive(Clone, Copy)]
struct Rgba {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl Rgba {
    fn transparent() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }

    /// 在自身之上叠加形状(标准 over 合成,直通 alpha)
    fn over(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let da = self.a;
        let oa = a + da * (1.0 - a);
        if oa <= 0.0 {
            self.r = r;
            self.g = g;
            self.b = b;
            self.a = a;
        } else {
            self.r = (r * a + self.r * da * (1.0 - a)) / oa;
            self.g = (g * a + self.g * da * (1.0 - a)) / oa;
            self.b = (b * a + self.b * da * (1.0 - a)) / oa;
            self.a = oa;
        }
    }

    fn to_bgra(self) -> (u8, u8, u8, u8) {
        (
            (self.b * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.r * 255.0).round() as u8,
            (self.a * 255.0).round() as u8,
        )
    }
}

/// 点到点距离
fn dist_pt(x: f32, y: f32, px: f32, py: f32) -> f32 {
    ((x - px) * (x - px) + (y - py) * (y - py)).sqrt()
}

/// 点到线段 (x1,y1)-(x2,y2) 的距离
fn dist_seg(x: f32, y: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let vx = x2 - x1;
    let vy = y2 - y1;
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= 0.0 {
        0.0
    } else {
        (((x - x1) * vx + (y - y1) * vy) / len2).clamp(0.0, 1.0)
    };
    dist_pt(x, y, x1 + vx * t, y1 + vy * t)
}

/// 平滑覆盖:距边缘 half 内全覆盖,half+1 外为零,之间线性
fn cover(d: f32, half: f32) -> f32 {
    (half + 0.5 - d).clamp(0.0, 1.0)
}

/// 实心圆
fn fill_circle(p: &mut Rgba, x: f32, y: f32, cx: f32, cy: f32, r: f32, color: [f32; 3]) {
    let a = cover(dist_pt(x, y, cx, cy), r);
    if a > 0.0 {
        p.over(color[0], color[1], color[2], a);
    }
}

/// 线段
fn fill_line(
    p: &mut Rgba,
    x: f32,
    y: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    half: f32,
    color: [f32; 3],
) {
    let a = cover(dist_seg(x, y, x1, y1, x2, y2), half);
    if a > 0.0 {
        p.over(color[0], color[1], color[2], a);
    }
}

/// 绘制地球:蓝圆 + 经纬线 + 左上高光
fn draw_globe(p: &mut Rgba, x: f32, y: f32) {
    const CX: f32 = 7.5;
    const CY: f32 = 7.5;
    const R: f32 = 6.0;
    fill_circle(p, x, y, CX, CY, R, ICON_BLUE);
    // 经纬线仅画在地球圆内
    if dist_pt(x, y, CX, CY) <= R {
        for mx in [5.6, 7.5, 9.4] {
            fill_line(p, x, y, mx, CY - 5.2, mx, CY + 5.2, 0.4, ICON_LIGHT);
        }
        for my in [4.8, 7.5, 10.2] {
            fill_line(p, x, y, CX - 5.2, my, CX + 5.2, my, 0.4, ICON_LIGHT);
        }
    }
    // 左上高光(半透明)
    let a = cover(dist_pt(x, y, 4.8, 4.0), 1.2) * 0.55;
    if a > 0.0 {
        p.over(1.0, 1.0, 1.0, a);
    }
}

/// 绘制电源按钮:顶部开口圆环 + 竖直短杆
fn draw_power(p: &mut Rgba, x: f32, y: f32) {
    const CX: f32 = 7.5;
    const CY: f32 = 8.0;
    const R: f32 = 4.8;
    // 竖直短杆(平头矩形,仅左右边缘抗锯齿,避免圆头呈菱形)
    if (2.0..=5.0).contains(&y) {
        let a = cover((x - 7.5).abs(), 0.7);
        if a > 0.0 {
            p.over(ICON_BLUE[0], ICON_BLUE[1], ICON_BLUE[2], a);
        }
    }
    // 圆环(顶部 -90° ± 35° 开口,避开竖杆区域)
    let d = dist_pt(x, y, CX, CY);
    let ang = (y - CY).atan2(x - CX);
    let in_gap = (ang + core::f32::consts::FRAC_PI_2).abs() < 35f32.to_radians();
    let a = cover((d - R).abs(), 0.6);
    if a > 0.0 && !in_gap {
        p.over(ICON_BLUE[0], ICON_BLUE[1], ICON_BLUE[2], a);
    }
}

/// 绘制滑杆:三条轨道 + 三个圆形滑块
fn draw_sliders(p: &mut Rgba, x: f32, y: f32) {
    for ty in [4.5, 8.0, 11.5] {
        fill_line(p, x, y, 3.0, ty, 13.0, ty, 0.4, ICON_BLUE);
    }
    fill_circle(p, x, y, 10.5, 4.5, 1.9, ICON_BLUE);
    fill_circle(p, x, y, 5.5, 8.0, 1.9, ICON_BLUE);
    fill_circle(p, x, y, 11.5, 11.5, 1.9, ICON_BLUE);
}

/// 绘制 X:两条对角粗线
fn draw_exit(p: &mut Rgba, x: f32, y: f32) {
    fill_line(p, x, y, 4.0, 4.0, 12.0, 12.0, 0.9, ICON_BLUE);
    fill_line(p, x, y, 12.0, 4.0, 4.0, 12.0, 0.9, ICON_BLUE);
}

/// 绘制时钟:圆环 + 向上分针 + 向右时针
fn draw_clock(p: &mut Rgba, x: f32, y: f32) {
    const CX: f32 = 7.5;
    const CY: f32 = 7.5;
    const R: f32 = 5.5;
    let d = dist_pt(x, y, CX, CY);
    let a = cover((d - R).abs(), 0.6);
    if a > 0.0 {
        p.over(ICON_BLUE[0], ICON_BLUE[1], ICON_BLUE[2], a);
    }
    // 分针(向上)
    fill_line(p, x, y, CX, CY, CX, 3.5, 0.4, ICON_BLUE);
    // 时针(向右)
    fill_line(p, x, y, CX, CY, 11.0, CY, 0.5, ICON_BLUE);
}

/// 绘制禁止:圆环 + 对角斜杠
fn draw_blocked(p: &mut Rgba, x: f32, y: f32) {
    const CX: f32 = 7.5;
    const CY: f32 = 7.5;
    const R: f32 = 5.5;
    let d = dist_pt(x, y, CX, CY);
    let a = cover((d - R).abs(), 0.7);
    if a > 0.0 {
        p.over(ICON_BLUE[0], ICON_BLUE[1], ICON_BLUE[2], a);
    }
    // 斜杠:右上到左下
    fill_line(p, x, y, 12.0, 4.0, 4.0, 12.0, 0.6, ICON_BLUE);
}

/// 计算 16x16 图标某像素的 BGRA(自顶向下行序,像素中心在 +0.5)
fn menu_icon_pixel(kind: MenuIcon, x: f32, y: f32) -> (u8, u8, u8, u8) {
    let mut p = Rgba::transparent();
    match kind {
        MenuIcon::Language => draw_globe(&mut p, x, y),
        MenuIcon::Autostart => draw_power(&mut p, x, y),
        MenuIcon::Options => draw_sliders(&mut p, x, y),
        MenuIcon::Exit => draw_exit(&mut p, x, y),
        MenuIcon::Clock => draw_clock(&mut p, x, y),
        MenuIcon::Blocked => draw_blocked(&mut p, x, y),
    }
    p.to_bgra()
}

/// 绘制 16x16 菜单图标像素(BGRA,自顶向下)
fn draw_menu_icon_pixels(kind: MenuIcon) -> Vec<u8> {
    let n = (MENU_ICON_SIZE * MENU_ICON_SIZE * 4) as usize;
    let mut out = vec![0u8; n];
    for y in 0..MENU_ICON_SIZE {
        for x in 0..MENU_ICON_SIZE {
            let (b, g, r, a) = menu_icon_pixel(kind, x as f32 + 0.5, y as f32 + 0.5);
            let i = ((y * MENU_ICON_SIZE + x) * 4) as usize;
            out[i] = b;
            out[i + 1] = g;
            out[i + 2] = r;
            out[i + 3] = a;
        }
    }
    out
}

/// 菜单图标位图缓存(按 MenuIcon 顺序索引,0 = 未创建)
static MENU_ICON_BITMAPS: Mutex<[usize; 6]> = Mutex::new([0; 6]);

/// 惰性创建并缓存指定菜单图标位图
fn menu_icon_bitmap(kind: MenuIcon) -> HBITMAP {
    let mut cache = MENU_ICON_BITMAPS.lock();
    let slot = &mut cache[kind as usize];
    if *slot != 0 {
        return *slot as HBITMAP;
    }
    let hbmp = unsafe { create_menu_icon_bitmap(kind) };
    *slot = hbmp as usize;
    hbmp
}

/// 创建 16x16 32bpp 菜单图标位图(自顶向下行序),失败返回 null
unsafe fn create_menu_icon_bitmap(kind: MenuIcon) -> HBITMAP {
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = MENU_ICON_SIZE;
    bmi.bmiHeader.biHeight = -MENU_ICON_SIZE; // 负数 = 自顶向下行序
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let hbmp = CreateDIBSection(
        std::ptr::null_mut(),
        &bmi,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    if hbmp.is_null() || bits.is_null() {
        return std::ptr::null_mut();
    }
    let pixels = draw_menu_icon_pixels(kind);
    std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
    hbmp
}

/// 销毁全部菜单图标位图(进程退出时调用)
fn destroy_menu_icon_bitmaps() {
    let mut cache = MENU_ICON_BITMAPS.lock();
    for slot in cache.iter_mut() {
        if *slot != 0 {
            unsafe { DeleteObject(*slot as HBITMAP) };
            *slot = 0;
        }
    }
}

/// 选项面板标签图标 HICON 缓存(按 MenuIcon 顺序索引,0 = 未创建)
static PANEL_ICON_ICONS: Mutex<[usize; 6]> = Mutex::new([0; 6]);

/// 惰性创建并缓存面板标签图标 HICON
fn panel_icon_hicon(kind: MenuIcon) -> HICON {
    let mut cache = PANEL_ICON_ICONS.lock();
    let slot = &mut cache[kind as usize];
    if *slot != 0 {
        return *slot as HICON;
    }
    let icon = unsafe { create_panel_icon(kind) };
    *slot = icon as usize;
    icon
}

/// 从图标像素创建 HICON(32bpp 颜色位图 + 全零单色掩码,alpha 控制透明)
unsafe fn create_panel_icon(kind: MenuIcon) -> HICON {
    let color = create_menu_icon_bitmap(kind);
    if color.is_null() {
        return std::ptr::null_mut();
    }
    // 全零单色掩码:16x16 @1bpp,每行按 4 字节对齐
    let zero_bits = [0u8; 16 * 4];
    let mask = CreateBitmap(
        MENU_ICON_SIZE,
        MENU_ICON_SIZE,
        1,
        1,
        zero_bits.as_ptr() as *const core::ffi::c_void,
    );
    if mask.is_null() {
        DeleteObject(color);
        return std::ptr::null_mut();
    }
    let mut ii: ICONINFO = std::mem::zeroed();
    ii.fIcon = 1;
    ii.hbmColor = color;
    ii.hbmMask = mask;
    let icon = CreateIconIndirect(&ii);
    // CreateIconIndirect 复制位图,可安全释放源位图
    DeleteObject(color);
    DeleteObject(mask);
    icon
}

/// 销毁全部面板标签图标(进程退出时调用)
fn destroy_panel_icons() {
    let mut cache = PANEL_ICON_ICONS.lock();
    for slot in cache.iter_mut() {
        if *slot != 0 {
            unsafe { DestroyIcon(*slot as HICON) };
            *slot = 0;
        }
    }
}

/// 给菜单项设置左侧图标(MIIM_BITMAP,图标槽独立于勾选标记)
unsafe fn set_menu_item_icon(menu: HMENU, cmd: usize, hbmp: HBITMAP) {
    if hbmp.is_null() {
        return;
    }
    let mut mii: MENUITEMINFOW = std::mem::zeroed();
    mii.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
    mii.fMask = MIIM_BITMAP;
    mii.hbmpItem = hbmp;
    SetMenuItemInfoW(menu, cmd as u32, 0, &mii);
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

// ---------- 日志 ----------

/// 日志文件路径:exe 同目录 cap-ime-switch.log
fn log_path() -> Option<std::path::PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.join("cap-ime-switch.log"))
}

/// 本地时间戳字符串(用于日志行前缀)
fn log_timestamp() -> String {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
        )
    }
}

/// 追加写日志(exe 同目录 cap-ime-switch.log):超过 1MB 时清空重写,失败静默
fn log_write(msg: &str) {
    use std::io::Write;
    let Some(path) = log_path() else { return };
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > 1024 * 1024 {
            let _ = std::fs::write(&path, b"");
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{} {}", log_timestamp(), msg);
    }
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
    let title = to_utf16(tr().panel_title);
    const W: i32 = 440;
    const H: i32 = 330;
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
        show_error(&tpl(tr().err_create_options_window, &[&GetLastError()]));
        return;
    }
    OPTIONS_OPEN.store(true, Ordering::Relaxed);
    log_write(&format!("打开选项面板 hwnd={hwnd:?}"));
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

/// 创建问号帮助图标(16x16 "?" 文本居中,不依赖图标 API,任何环境必然可见)
unsafe fn create_help_icon(
    parent: HWND,
    hmod: *mut core::ffi::c_void,
    ctrl_id: usize,
    x: i32,
    y: i32,
) -> HWND {
    let class = to_utf16("Static");
    let text = to_utf16("?");
    let help = CreateWindowExW(
        0,
        class.as_ptr(),
        text.as_ptr(),
        WS_CHILD | WS_VISIBLE | SS_CENTER | SS_CENTERIMAGE | SS_NOTIFY,
        x,
        y,
        16,
        16,
        parent,
        (ctrl_id as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !help.is_null() {
        set_control_font(help);
    }
    help
}

/// 创建设置行左侧标签图标(16x16 SS_ICON Static 显示面板图标 HICON)
unsafe fn create_label_icon(
    parent: HWND,
    hmod: *mut core::ffi::c_void,
    kind: MenuIcon,
    x: i32,
    y: i32,
) -> HWND {
    let class = to_utf16("Static");
    let hwnd = CreateWindowExW(
        0,
        class.as_ptr(),
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE | SS_ICON,
        x,
        y,
        MENU_ICON_SIZE,
        MENU_ICON_SIZE,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    let icon = panel_icon_hicon(kind);
    if !hwnd.is_null() && !icon.is_null() {
        SendMessageW(hwnd, STM_SETICON, icon as WPARAM, 0 as LPARAM);
    }
    hwnd
}

/// 在说明栏显示指定问号的说明文本(其他问号文本互斥,直接覆盖)
unsafe fn show_help_bar(parent: HWND, ctrl_id: usize) {
    let strings = tr();
    let text = match ctrl_id {
        ID_OPT_HELP_AUTOSTART => strings.help_autostart,
        ID_OPT_HELP_DELAY => strings.help_delay,
        ID_OPT_HELP_EXCLUDE => strings.help_exclude,
        _ => return,
    };
    let bar = GetDlgItem(parent, ID_OPT_HELP_BAR as i32);
    let w = to_utf16(text);
    SetWindowTextW(bar, w.as_ptr());
    ShowWindow(bar, SW_SHOW);
}

/// 隐藏说明栏
unsafe fn hide_help_bar(parent: HWND) {
    let bar = GetDlgItem(parent, ID_OPT_HELP_BAR as i32);
    ShowWindow(bar, SW_HIDE);
}

/// 点击问号:固定显示对应说明,再次点击(或点击其他问号)时切换隐藏
unsafe fn on_help_icon_click(parent: HWND, ctrl_id: usize) {
    {
        let mut pinned = HELP_PINNED.lock();
        if *pinned == Some(ctrl_id) {
            *pinned = None;
            drop(pinned);
            hide_help_bar(parent);
            log_write(&format!("帮助说明已隐藏: ctrl_id={ctrl_id}"));
            return;
        }
        *pinned = Some(ctrl_id);
    }
    show_help_bar(parent, ctrl_id);
    log_write(&format!("帮助说明固定显示: ctrl_id={ctrl_id}"));
}

/// 子类化问号图标控件:悬浮(WM_MOUSEMOVE)显示说明栏,移开(WM_MOUSELEAVE)隐藏,
/// 点击固定期间保持显示。
unsafe fn subclass_help_icon(help: HWND) {
    if ORIG_STATIC_PROC.load(Ordering::Relaxed) == 0 {
        let cls = GetClassLongPtrW(help, GCLP_WNDPROC) as usize;
        ORIG_STATIC_PROC.store(cls, Ordering::Relaxed);
    }
    SetWindowLongPtrW(help, GWLP_WNDPROC, help_icon_subproc as *const () as isize);
}

/// 问号图标子类化窗口过程
unsafe extern "system" fn help_icon_subproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let original = ORIG_STATIC_PROC.load(Ordering::Relaxed) as isize;
    match msg {
        WM_MOUSEMOVE => {
            let parent = GetParent(hwnd);
            if !parent.is_null() {
                show_help_bar(parent, GetDlgCtrlID(hwnd) as usize);
                // 一次性登记鼠标离开通知,每次 move 需重新登记
                let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
                tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
                tme.dwFlags = TME_LEAVE;
                tme.hwndTrack = hwnd;
                TrackMouseEvent(&mut tme);
            }
            0
        }
        WM_MOUSELEAVE => {
            let pinned_now = *HELP_PINNED.lock();
            if pinned_now != Some(GetDlgCtrlID(hwnd) as usize) {
                hide_help_bar(GetParent(hwnd));
            }
            0
        }
        _ => {
            if original == 0 {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            CallWindowProcW(std::mem::transmute(original), hwnd, msg, wparam, lparam)
        }
    }
}

/// 创建选项面板子控件。
/// 布局:左侧固定宽度名称列(左对齐)+ 问号帮助图标 + 右侧占满剩余空间的输入列;
/// 说明文字默认隐藏,悬浮/点击问号时通过工具提示显示;版本号位于底部左侧。
unsafe fn create_options_controls(parent: HWND, hmod: *mut core::ffi::c_void) {
    let button_class = to_utf16("Button");
    let edit_class = to_utf16("Edit");
    let static_class = to_utf16("Static");
    let combo_class = to_utf16("ComboBox");
    let strings = tr();

    // 运行时测量最宽标题的像素宽度(按实际字体/DPI),左侧列宽随内容而定
    let dc = GetDC(parent);
    let old_font = SelectObject(dc, GetStockObject(DEFAULT_GUI_FONT));
    let mut text_size: SIZE = std::mem::zeroed();
    let mut max_label = 0i32;
    for title in [
        strings.label_autostart,
        strings.label_delay,
        strings.label_exclude,
        strings.label_language,
    ] {
        let w = to_utf16(title);
        if GetTextExtentPoint32W(
            dc,
            w.as_ptr(),
            title.encode_utf16().count() as i32,
            &mut text_size,
        ) != 0
        {
            max_label = max_label.max(text_size.cx);
        }
    }
    // 全部测量失败时才用固定兜底宽度(避免兜底值污染真实测量)
    if max_label <= 0 {
        max_label = 130;
    }
    SelectObject(dc, old_font);
    ReleaseDC(parent, dc);
    // 布局基准为窗口客户区宽度(440 是含边框的外框宽,客户区小约 16px,
    // 若不修正,右侧控件会贴着客户区右缘,看起来没有边距)
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(parent, &mut client);
    let client_w = client.right;
    // 图标 16..32,标签 38..38+max_label,问号 16x16(+4 间距),输入列(+12 间距)占满至右缘 16
    const ICON_W: i32 = 16;
    const ICON_GAP: i32 = 6;
    const HELP_W: i32 = 16;
    const HELP_GAP: i32 = 4;
    const COL_GAP: i32 = 12;
    let label_x = 16 + ICON_W + ICON_GAP;
    let help_x = label_x + max_label + HELP_GAP;
    let right_x = help_x + HELP_W + COL_GAP;
    let right_w = client_w - 16 - right_x;
    log_write(&format!(
        "选项面板布局: 客户区宽={client_w} 标题宽={max_label} 问号x={help_x} 输入列x={right_x} 宽={right_w}"
    ));

    // 行 1:名称(左对齐)+ 问号 + 无字勾选框
    create_label_icon(parent, hmod, MenuIcon::Autostart, 16, 22);
    let check_label = to_utf16(strings.label_autostart);
    let label1 = CreateWindowExW(
        0,
        static_class.as_ptr(),
        check_label.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        label_x,
        22,
        max_label,
        20,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if !label1.is_null() {
        set_control_font(label1);
    }

    let help1 = create_help_icon(parent, hmod, ID_OPT_HELP_AUTOSTART, help_x, 22);
    if !help1.is_null() {
        subclass_help_icon(help1);
    }

    let check = CreateWindowExW(
        0,
        button_class.as_ptr(),
        std::ptr::null(),
        // BS_AUTOCHECKBOX:点击自动切换勾选;BS_CHECKBOX 需父窗口手动切换,会导致点击无反应
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        right_x,
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

    // 行 2:名称(左对齐)+ 问号 + 延迟输入框
    create_label_icon(parent, hmod, MenuIcon::Clock, 16, 58);
    let label_text = to_utf16(strings.label_delay);
    let label = CreateWindowExW(
        0,
        static_class.as_ptr(),
        label_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        label_x,
        58,
        max_label,
        20,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if !label.is_null() {
        set_control_font(label);
    }

    let help2 = create_help_icon(parent, hmod, ID_OPT_HELP_DELAY, help_x, 58);
    if !help2.is_null() {
        subclass_help_icon(help2);
    }

    let cur_ms = *LONG_PRESS_MS.lock();
    let cur_text = to_utf16(&cur_ms.to_string());
    let edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        edit_class.as_ptr(),
        cur_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_NUMBER as u32 | ES_AUTOHSCROLL as u32,
        right_x,
        56,
        right_w,
        24,
        parent,
        (ID_OPT_EDIT_DELAY as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !edit.is_null() {
        set_control_font(edit);
    }

    // 行 3:名称(左对齐)+ 问号 + 排除程序多行输入框
    create_label_icon(parent, hmod, MenuIcon::Blocked, 16, 94);
    let exclude_label_text = to_utf16(strings.label_exclude);
    let label3 = CreateWindowExW(
        0,
        static_class.as_ptr(),
        exclude_label_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        label_x,
        94,
        max_label,
        20,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if !label3.is_null() {
        set_control_font(label3);
    }

    let help3 = create_help_icon(parent, hmod, ID_OPT_HELP_EXCLUDE, help_x, 94);
    if !help3.is_null() {
        subclass_help_icon(help3);
    }

    let cur_exclude = EXCLUDE_PROCESSES.lock().join("\r\n");
    let cur_exclude_text = to_utf16(&cur_exclude);
    let edit2 = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        edit_class.as_ptr(),
        cur_exclude_text.as_ptr(),
        WS_CHILD
            | WS_VISIBLE
            | WS_TABSTOP
            | WS_VSCROLL as u32
            | ES_MULTILINE as u32
            | ES_AUTOVSCROLL as u32
            | ES_WANTRETURN as u32,
        right_x,
        92,
        right_w,
        68,
        parent,
        (ID_OPT_EDIT_EXCLUDE as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !edit2.is_null() {
        set_control_font(edit2);
    }

    // 行 4:名称(左对齐)+ 语言下拉框(选项显示母语名,选中项即当前语言)
    create_label_icon(parent, hmod, MenuIcon::Language, 16, 166);
    let lang_label_text = to_utf16(strings.label_language);
    let label4 = CreateWindowExW(
        0,
        static_class.as_ptr(),
        lang_label_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        label_x,
        166,
        max_label,
        20,
        parent,
        std::ptr::null_mut(),
        hmod,
        std::ptr::null_mut(),
    );
    if !label4.is_null() {
        set_control_font(label4);
    }

    let combo = CreateWindowExW(
        0,
        combo_class.as_ptr(),
        std::ptr::null(),
        // CBS_DROPDOWNLIST + CBS_HASSTRINGS:下拉列表由控件自存字符串并绘制
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32
            | WS_VSCROLL as u32,
        right_x,
        164,
        right_w,
        200,
        parent,
        (ID_OPT_COMBO_LANGUAGE as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !combo.is_null() {
        set_control_font(combo);
        let cur_lang = *LANGUAGE.lock();
        let mut select_idx = 0usize;
        for (i, lang) in Lang::all().iter().enumerate() {
            let item = to_utf16(lang.display_name());
            SendMessageW(combo, CB_ADDSTRING, i as WPARAM, item.as_ptr() as LPARAM);
            if *lang == cur_lang {
                select_idx = i;
            }
        }
        SendMessageW(combo, CB_SETCURSEL, select_idx as WPARAM, 0 as LPARAM);
    }

    // 说明栏:只读多行文本,点击/悬浮问号时在此显示对应说明,默认隐藏
    let bar = CreateWindowExW(
        0,
        edit_class.as_ptr(),
        std::ptr::null(),
        WS_CHILD | ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32,
        16,
        212,
        408,
        40,
        parent,
        (ID_OPT_HELP_BAR as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !bar.is_null() {
        set_control_font(bar);
        ShowWindow(bar, SW_HIDE);
    }

    // 版本号:底部左侧灰色小字(env! 与 Cargo.toml 同步)
    let ver_text = to_utf16(&tpl(strings.version_template, &[&env!("CARGO_PKG_VERSION")]));
    let ver = CreateWindowExW(
        0,
        static_class.as_ptr(),
        ver_text.as_ptr(),
        WS_CHILD | WS_VISIBLE,
        16,
        268,
        200,
        20,
        parent,
        (ID_OPT_VERSION as usize) as *mut core::ffi::c_void,
        hmod,
        std::ptr::null_mut(),
    );
    if !ver.is_null() {
        set_control_font(ver);
    }

    // 保存按钮(右下角,右/下边距 24)
    let save_label = to_utf16(strings.save);
    let save = CreateWindowExW(
        0,
        button_class.as_ptr(),
        save_label.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
        client_w - 24 - 80,
        262,
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
            } else if code as u32 == BN_CLICKED
                && (id == ID_OPT_HELP_AUTOSTART
                    || id == ID_OPT_HELP_DELAY
                    || id == ID_OPT_HELP_EXCLUDE)
            {
                // 问号图标点击:固定显示/隐藏说明(静态控件通知码 STN_CLICKED=0 与 BN_CLICKED 相同)
                on_help_icon_click(hwnd, id);
            }
            0
        }
        // 标签/说明文字绘制:白色背景融入面板,说明文字用灰色与主标签区分
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT => {
            let hdc = wparam as HDC;
            SetBkMode(hdc, TRANSPARENT as i32);
            let ctrl_id = GetDlgCtrlID(lparam as HWND);
            if msg == WM_CTLCOLORSTATIC && ctrl_id == ID_OPT_VERSION as i32 {
                SetTextColor(hdc, 0x00808080); // 版本号灰色,与主标签区分
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
            HELP_PINNED.lock().take();
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
            show_error(&tpl(
                tr().err_delay_range,
                &[&config::MIN_LONG_PRESS_MS, &config::MAX_LONG_PRESS_MS],
            ));
            return;
        }
    };
    let path = match config_path() {
        Some(p) => p,
        None => {
            show_error(tr().err_config_path);
            return;
        }
    };
    let exclude_text = {
        let edit2 = GetDlgItem(hwnd, ID_OPT_EDIT_EXCLUDE as i32);
        let mut buf = [0u16; 2048];
        let len = GetWindowTextW(edit2, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..len.max(0) as usize])
    };
    let exclude = config::parse_exclude_list(&exclude_text);
    // 语言下拉框:按 Lang::all() 顺序映射选中索引
    let combo = GetDlgItem(hwnd, ID_OPT_COMBO_LANGUAGE as i32);
    let sel = SendMessageW(combo, CB_GETCURSEL, 0, 0) as usize;
    let lang = Lang::all().get(sel).copied().unwrap_or(Lang::Zh);
    let cfg = config::Config {
        long_press_ms: ms,
        exclude_processes: exclude.clone(),
        language: lang.code().to_string(),
    };
    if !config::save(&cfg, &path) {
        log_write("保存 config.toml 失败");
        show_error(tr().err_save_failed);
        return;
    }
    log_write(&format!(
        "保存设置: 延迟={ms}ms 排除={exclude:?} 语言={}",
        lang.code()
    ));
    *LONG_PRESS_MS.lock() = ms;
    *EXCLUDE_PROCESSES.lock() = exclude;
    *LANGUAGE.lock() = lang;
    update_tray_tooltip();

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
        log_write("保存开机自启动设置失败");
        show_error(tr().err_save_autostart);
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

/// 排除程序列表是否命中当前前景进程;列表为空时直接返回 false(不查窗口)
fn is_excluded_process() -> bool {
    let list = EXCLUDE_PROCESSES.lock();
    if list.is_empty() {
        return false;
    }
    foreground_exe_name().is_some_and(|exe| config::is_excluded(&exe, &list))
}

/// 前景窗口所属进程的 exe 文件名(小写),查询失败返回 None。
/// 键盘输入目标即前景窗口,仅用于钩子回调判断按键归属进程。
fn foreground_exe_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(process);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        std::path::Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
    }
}

/// 低级键盘钩子回调。返回 1 表示吞掉该事件。
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);

        // 我们注入的事件带 LLKHF_INJECTED,直接放行,避免递归
        if kb.flags & LLKHF_INJECTED == 0 {
            let is_caps = kb.vkCode == VK_CAPITAL as u32;
            // 排除程序:CapsLock 恢复原始行为,不进入状态机、不吞事件
            let excluded = is_caps && is_excluded_process();
            if !excluded {
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

#[cfg(test)]
mod lang_tests {
    use super::*;

    #[test]
    fn lcid_maps_chinese_variants_to_zh() {
        // zh-CN / zh-TW / zh-HK / zh-SG / zh-MO 主语言均为 0x04
        for lcid in [0x0804u16, 0x0404, 0x0C04, 0x1004, 0x1404] {
            assert_eq!(lang_from_lcid(lcid), Lang::Zh, "lcid=0x{lcid:04X}");
        }
    }

    #[test]
    fn lcid_maps_english_variants_to_en() {
        // en-US / en-GB / en-CA / en-AU / en-NZ 主语言均为 0x09
        for lcid in [0x0409u16, 0x0809, 0x1009, 0x0C09, 0x1409] {
            assert_eq!(lang_from_lcid(lcid), Lang::En, "lcid=0x{lcid:04X}");
        }
    }

    #[test]
    fn unknown_lcid_falls_back_to_zh() {
        // 日语/法语/德语等未知语言回退中文
        assert_eq!(lang_from_lcid(0x0411), Lang::Zh); // ja-JP
        assert_eq!(lang_from_lcid(0x040C), Lang::Zh); // fr-FR
        assert_eq!(lang_from_lcid(0x0407), Lang::Zh); // de-DE
    }
}

#[cfg(test)]
mod menu_icon_tests {
    use super::*;

    /// 读取 16x16 BGRA 位图中 (x,y) 处的像素
    fn px(bitmap: &[u8], x: usize, y: usize) -> (u8, u8, u8, u8) {
        let i = (y * 16 + x) * 4;
        (bitmap[i], bitmap[i + 1], bitmap[i + 2], bitmap[i + 3])
    }

    #[test]
    fn each_icon_bitmap_is_16x16_32bpp() {
        for kind in [MenuIcon::Language, MenuIcon::Autostart, MenuIcon::Options, MenuIcon::Exit] {
            let px = draw_menu_icon_pixels(kind);
            assert_eq!(px.len(), 16 * 16 * 4, "{kind:?} 尺寸应为 16x16x4");
        }
    }

    #[test]
    fn globe_center_is_opaque_blue_and_corners_transparent() {
        let bmp = draw_menu_icon_pixels(MenuIcon::Language);
        let (b, _g, r, a) = px(&bmp, 8, 8);
        assert!(a > 200, "地球中心应不透明, alpha={a}");
        assert!(b > 150 && b > r, "地球应为蓝色调 b={b} r={r}");
        for (x, y) in [(0, 0), (15, 0), (0, 15), (15, 15)] {
            let (_, _, _, a) = px(&bmp, x, y);
            assert_eq!(a, 0, "({x},{y}) 角应透明");
        }
    }

    #[test]
    fn power_icon_has_top_gap_and_vertical_bar() {
        let bmp = draw_menu_icon_pixels(MenuIcon::Autostart);
        // 顶部中心缺口处(圆环开口)应无像素
        let (_, _, _, a_gap) = px(&bmp, 8, 3);
        // 竖杆:顶部中心
        let (_, _, _, a_bar) = px(&bmp, 7, 4);
        assert!(a_gap < 60, "电源符号顶部应有开口 alpha={a_gap}");
        assert!(a_bar > 150, "电源竖杆应存在 alpha={a_bar}");
    }

    #[test]
    fn options_sliders_have_three_knobs() {
        let bmp = draw_menu_icon_pixels(MenuIcon::Options);
        for (x, y) in [(10, 4), (5, 8), (11, 11)] {
            let (_, _, _, a) = px(&bmp, x, y);
            assert!(a > 150, "滑块 ({x},{y}) 应不透明 alpha={a}");
        }
    }

    #[test]
    fn exit_x_has_crossing_strokes() {
        let bmp = draw_menu_icon_pixels(MenuIcon::Exit);
        let (_, _, _, a) = px(&bmp, 8, 8);
        assert!(a > 150, "X 中心应不透明 alpha={a}");
        let (_, _, _, a2) = px(&bmp, 7, 7);
        assert!(a2 > 100, "X 左上臂应存在 alpha={a2}");
    }

    #[test]
    fn clock_icon_has_ring_and_hands() {
        let bmp = draw_menu_icon_pixels(MenuIcon::Clock);
        let (_, _, _, a_ring) = px(&bmp, 8, 2);
        assert!(a_ring > 100, "钟面圆环应存在 alpha={a_ring}");
        let (_, _, _, a_min) = px(&bmp, 7, 4);
        assert!(a_min > 150, "分针(向上)应存在 alpha={a_min}");
        let (_, _, _, a_hour) = px(&bmp, 10, 7);
        assert!(a_hour > 150, "时针(向右)应存在 alpha={a_hour}");
    }

    #[test]
    fn blocked_icon_has_ring_and_slash() {
        let bmp = draw_menu_icon_pixels(MenuIcon::Blocked);
        let (_, _, _, a_ring) = px(&bmp, 8, 2);
        assert!(a_ring > 100, "禁止圆环应存在 alpha={a_ring}");
        let (_, _, _, a_slash) = px(&bmp, 7, 8);
        assert!(a_slash > 150, "禁止斜杠应存在 alpha={a_slash}");
    }
}
