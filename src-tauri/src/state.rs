//! 应用共享状态。

use crate::providers::EmbeddingProvider;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

/// 由 Tauri 托管的应用状态
pub struct AppState {
    /// 共享数据库连接（WAL 模式下支持多读单写）
    pub conn: Arc<Mutex<Connection>>,
    /// 数据库文件路径（供备份/导出使用）
    pub db_path: PathBuf,
    /// 嵌入 Provider：P0 使用确定性 Mock，接入本地 ONNX 模型（ST-06/模型中台）后替换
    pub embedding: Arc<dyn EmbeddingProvider>,
}

impl AppState {
    pub fn new(
        conn: Arc<Mutex<Connection>>,
        db_path: PathBuf,
        embedding: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            conn,
            db_path,
            embedding,
        }
    }

    /// 获取连接锁；命令层直接将错误转为 String 返回给前端
    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|e| format!("数据库连接锁已中毒：{e}"))
    }
}
