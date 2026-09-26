//! 应用共享状态。

use crate::providers::EmbeddingProvider;
use crate::security::SecretStore;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

/// 由 Tauri 托管的应用状态
pub struct AppState {
    /// 共享数据库连接（WAL 模式下支持多读单写）
    pub conn: Arc<Mutex<Connection>>,
    /// 数据库文件路径（供备份/导出使用）
    pub db_path: PathBuf,
    /// 本地模型存放目录
    pub models_dir: PathBuf,
    /// 嵌入 Provider：P0 使用确定性 Mock，接入本地 ONNX 模型（ST-06/模型中台）后替换
    pub embedding: Arc<dyn EmbeddingProvider>,
    /// 密钥存储：系统钥匙链（测试环境可注入内存实现）
    pub secrets: Arc<dyn SecretStore>,
}

impl AppState {
    pub fn new(
        conn: Arc<Mutex<Connection>>,
        db_path: PathBuf,
        models_dir: PathBuf,
        embedding: Arc<dyn EmbeddingProvider>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            conn,
            db_path,
            models_dir,
            embedding,
            secrets,
        }
    }

    /// 获取连接锁；命令层直接将错误转为 String 返回给前端
    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|e| format!("数据库连接锁已中毒：{e}"))
    }
}
