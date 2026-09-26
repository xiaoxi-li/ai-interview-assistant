//! 应用共享状态。

use crate::audio::CpalCapture;
use crate::providers::EmbeddingProvider;
use crate::security::SecretStore;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

/// 音频采集管理器：麦克风与系统音频各持一路
#[derive(Default)]
pub struct CaptureManager {
    pub mic: CpalCapture,
    pub system: CpalCapture,
    /// 当前正在采集的来源（None 表示未采集）
    pub running: Option<String>,
}

impl CaptureManager {
    /// 停止全部采集
    pub fn stop_all(&mut self) {
        let _ = crate::audio::AudioCapture::stop(&mut self.mic);
        let _ = crate::audio::AudioCapture::stop(&mut self.system);
        self.running = None;
    }
}

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
    /// 音频采集管理（内部流由专用线程持有，此处仅存跨线程句柄）
    pub capture: Mutex<CaptureManager>,
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
            capture: Mutex::new(CaptureManager::default()),
        }
    }

    /// 获取采集管理锁
    pub fn capture_lock(&self) -> Result<MutexGuard<'_, CaptureManager>, String> {
        self.capture
            .lock()
            .map_err(|e| format!("采集锁已中毒：{e}"))
    }

    /// 获取连接锁；命令层直接将错误转为 String 返回给前端
    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn
            .lock()
            .map_err(|e| format!("数据库连接锁已中毒：{e}"))
    }
}
