use serde::Serialize;

pub mod storage;

/// 应用基础信息（骨架阶段用于验证前后端 IPC 通路）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "AI 面试助手".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_contains_name_and_version() {
        let info = app_info();
        assert_eq!(info.name, "AI 面试助手");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn app_info_is_serializable() {
        let json = serde_json::to_string(&app_info()).expect("serialize app_info");
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"version\""));
    }
}
