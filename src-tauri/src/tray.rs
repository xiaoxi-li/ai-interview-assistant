//! 系统托盘（IA-11）：显示主窗口、切换迷你浮窗、隐藏窗口与退出。

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager,
};

/// 主窗口标签
const MAIN_WINDOW: &str = "main";

/// 显示并聚焦主窗口
fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 创建托盘图标与菜单；托盘创建失败不应阻断应用启动
pub fn create(app: &App) -> Result<(), String> {
    let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let mini_item = MenuItem::with_id(app, "mini", "迷你模式", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let hide_item = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let separator = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| e.to_string())?;

    let menu = Menu::with_items(
        app,
        &[&show_item, &mini_item, &hide_item, &separator, &quit_item],
    )
    .map_err(|e| e.to_string())?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        // 左键单击直接唤出窗口，不弹菜单（更符合面试中快速切换的习惯）
        .show_menu_on_left_click(false)
        .tooltip("AI 面试助手");

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "mini" => {
                show_main(app);
                if let Err(err) = crate::commands::open_mini_window(app.clone()) {
                    eprintln!("从托盘打开迷你浮窗失败：{err}");
                }
            }
            "hide" => {
                if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                    let _ = window.hide();
                }
            }
            "quit" => app.exit(0),
            other => eprintln!("未处理的托盘菜单项：{other}"),
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|e| e.to_string())?;

    Ok(())
}
