//! 应用共享状态。

use crate::audio::{AudioCapture, CpalCapture, SystemCapture};
use crate::providers::EmbeddingProvider;
use crate::security::SecretStore;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

/// 音频采集管理器：麦克风与系统音频各持一路，支持**并行采集**（IA-05 双通道）
///
/// 系统音频通道的实现随平台而定（IA-14：Windows 为 WASAPI 端点级回环，
/// 其他平台为名称匹配到的虚拟回环设备），故统一以 [`AudioCapture`] trait 对象暴露。
#[derive(Default)]
pub struct CaptureManager {
    pub mic: CpalCapture,
    pub system: SystemCapture,
}

impl CaptureManager {
    /// 停止全部采集
    pub fn stop_all(&mut self) {
        let _ = crate::audio::AudioCapture::stop(&mut self.mic);
        let _ = crate::audio::AudioCapture::stop(&mut self.system);
    }

    /// 指定来源是否正在采集
    pub fn is_running(&self, source: &str) -> bool {
        match source {
            "mic" => self.mic.is_running(),
            "system" => self.system.is_running(),
            _ => false,
        }
    }

    /// 当前正在采集的来源列表（mic / system）
    pub fn running_sources(&self) -> Vec<String> {
        let mut sources = Vec::new();
        if self.mic.is_running() {
            sources.push("mic".to_string());
        }
        if self.system.is_running() {
            sources.push("system".to_string());
        }
        sources
    }

    /// 是否有任一来源在采集
    pub fn any_running(&self) -> bool {
        self.mic.is_running() || self.system.is_running()
    }

    /// 按来源取采集句柄的可变引用（两路实现类型不同，故返回 trait 对象）
    pub fn capture_mut(&mut self, source: &str) -> Option<&mut dyn AudioCapture> {
        match source {
            "mic" => Some(&mut self.mic),
            "system" => Some(&mut self.system),
            _ => None,
        }
    }
}

/// 迷你模式进入前的主窗口几何快照（IA-13，就地迷你模式）
///
/// 尺寸与位置采用**物理像素**保存，退出时原样回填，避免逻辑/物理换算误差；
/// 置顶与最大化状态一并记录，保证「恢复」后与进入前完全一致。
#[derive(Debug, Clone, Copy)]
pub struct MiniWindowSnapshot {
    /// 窗口外部尺寸（宽，物理像素）
    pub width: u32,
    /// 窗口外部尺寸（高，物理像素）
    pub height: u32,
    /// 窗口左上角 X（物理像素）
    pub x: i32,
    /// 窗口左上角 Y（物理像素）
    pub y: i32,
    /// 进入迷你模式前的置顶状态（来自界面配置）
    pub always_on_top: bool,
    /// 进入迷你模式前是否处于最大化
    pub maximized: bool,
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
    /// ASR 运行时（同一时刻最多一个本地 ASR 会话）
    pub asr: Mutex<crate::asr::AsrRuntime>,
    /// 回答生成的「代数」：递增即取消上一轮流式生成（IA-08）
    pub answer_generation: Arc<std::sync::atomic::AtomicU64>,
    /// 采集暂停标记（IA-12）：置位后采集泵丢弃音频帧，但会话保持
    pub asr_paused: Arc<std::sync::atomic::AtomicBool>,
    /// 迷你模式窗口快照（IA-13）：`None` 表示当前不在迷你模式
    pub mini_snapshot: Mutex<Option<MiniWindowSnapshot>>,
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
            asr: Mutex::new(crate::asr::AsrRuntime::default()),
            answer_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            asr_paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            mini_snapshot: Mutex::new(None),
        }
    }

    /// 获取 ASR 运行时锁
    pub fn asr_lock(&self) -> Result<MutexGuard<'_, crate::asr::AsrRuntime>, String> {
        self.asr.lock().map_err(|e| format!("ASR 锁已中毒：{e}"))
    }

    /// 获取迷你模式窗口快照锁（IA-13）
    pub fn mini_snapshot_lock(&self) -> Result<MutexGuard<'_, Option<MiniWindowSnapshot>>, String> {
        self.mini_snapshot
            .lock()
            .map_err(|e| format!("迷你模式快照锁已中毒：{e}"))
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
