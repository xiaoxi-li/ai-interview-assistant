//! ASR 子系统。
//!
//! **P0 决策**：ASR 只走本地通路（本地引擎 + VAD 分段），音频不出本机；
//! 云端多服务商（讯飞/阿里/腾讯）协议适配整体延后至 P1，配置项与凭证管理保留。

pub mod engine;
pub mod pipeline;
pub mod store;

pub use engine::{LocalAsrEngine, ReplayEngine, SherpaOnnxEngine, LOCAL_ASR_MODEL_ID};
pub use pipeline::{AsrPipeline, DEFAULT_PARTIAL_INTERVAL_MS};
pub use store::{SessionDto, TranscriptRepo, TranscriptSegmentDto};

use crate::providers::types::Speaker;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

/// 采集来源 → 说话人（麦克风流=我，系统音频流=面试官）
pub fn speaker_for_source(source: &str) -> Option<Speaker> {
    match source {
        "mic" => Some(Speaker::Me),
        "system" => Some(Speaker::Interviewer),
        _ => None,
    }
}

/// 采集泵控制指令
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrControl {
    /// 停止会话
    Stop,
    /// 手动触发一次问题识别（IA-06）
    TriggerQuestion,
}

/// 一次 ASR 会话的运行时句柄
pub struct AsrSession {
    pub id: String,
    pub source: String,
    pub speaker: Speaker,
    pub engine: String,
    pub started_at: i64,
    control_tx: Option<Sender<AsrControl>>,
    pump: Option<JoinHandle<()>>,
}

impl AsrSession {
    pub fn new(
        id: String,
        source: String,
        speaker: Speaker,
        engine: String,
        started_at: i64,
        control_tx: Sender<AsrControl>,
        pump: JoinHandle<()>,
    ) -> Self {
        Self {
            id,
            source,
            speaker,
            engine,
            started_at,
            control_tx: Some(control_tx),
            pump: Some(pump),
        }
    }

    /// 发送控制指令（会话已停止时返回 Err）
    pub fn send(&self, control: AsrControl) -> Result<(), String> {
        self.control_tx
            .as_ref()
            .ok_or_else(|| "ASR 会话已停止".to_string())?
            .send(control)
            .map_err(|_| "ASR 采集泵已退出，无法发送指令".to_string())
    }

    /// 停止会话并等待采集泵退出（幂等）
    pub fn stop(&mut self) {
        if let Some(tx) = self.control_tx.take() {
            let _ = tx.send(AsrControl::Stop);
        }
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
    }
}

/// ASR 运行时：同一时刻最多一个会话
#[derive(Default)]
pub struct AsrRuntime {
    pub session: Option<AsrSession>,
}

impl AsrRuntime {
    pub fn is_running(&self) -> bool {
        self.session.is_some()
    }

    pub fn session_id(&self) -> Option<String> {
        self.session.as_ref().map(|session| session.id.clone())
    }

    /// 停止当前会话（幂等）
    pub fn stop(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::capture::{rms_level, FRAME_SAMPLES};
    use crate::audio::vad::VadConfig;
    use crate::audio::{to_audio_chunk, TARGET_SAMPLE_RATE};
    use crate::providers::types::TranscriptEvent;
    use crate::storage::Db;

    #[test]
    fn speaker_mapping_matches_channels() {
        assert_eq!(speaker_for_source("mic"), Some(Speaker::Me));
        assert_eq!(speaker_for_source("system"), Some(Speaker::Interviewer));
        assert_eq!(speaker_for_source("other"), None);
    }

    #[test]
    fn runtime_stop_is_idempotent_when_idle() {
        let mut runtime = AsrRuntime::default();
        assert!(!runtime.is_running());
        assert_eq!(runtime.session_id(), None);
        runtime.stop();
        assert!(!runtime.is_running());
    }

    /// PCM 回放集成测试：合成「语音-静音-语音」音频 → 流水线 → 事件 → 落库。
    #[test]
    fn pcm_replay_end_to_end_produces_events_and_persists() {
        let db = Db::open_in_memory().expect("db");
        let repo = TranscriptRepo::new(db.conn());
        let session = repo.start_session("回放面试").expect("session");

        let engine = ReplayEngine::with_script(vec![
            "请介绍一下你最近的项目".into(),
            "这个项目的架构是怎样的".into(),
        ]);
        let mut pipeline = AsrPipeline::new(
            Box::new(engine),
            Speaker::Interviewer,
            VadConfig::default(),
            0.5,
            200,
        );

        // 合成 16kHz 单声道 PCM：两轮「600ms 语音 + 600ms 静音」
        let speech: Vec<f32> = (0..FRAME_SAMPLES * 30)
            .map(|i| if i % 2 == 0 { 0.8 } else { -0.8 })
            .collect();
        let silence: Vec<f32> = vec![0.0; FRAME_SAMPLES * 30];

        let mut events: Vec<TranscriptEvent> = Vec::new();
        let mut ts = 0i64;
        for round in 0..2 {
            for frame in speech.chunks(FRAME_SAMPLES) {
                ts += 20;
                assert!(rms_level(frame) > 0.5, "第 {round} 轮应为语音帧");
                // 转成 AudioChunk 再还原，覆盖 PCM 编解码链路
                let chunk = to_audio_chunk(frame, ts);
                assert_eq!(chunk.sample_rate, TARGET_SAMPLE_RATE);
                events.extend(pipeline.push_frame(frame, ts));
            }
            for frame in silence.chunks(FRAME_SAMPLES) {
                ts += 20;
                events.extend(pipeline.push_frame(frame, ts));
            }
        }
        events.extend(pipeline.flush());

        // 落库：final 全量落库，partial 也保留（崩溃恢复）
        for event in &events {
            match event {
                TranscriptEvent::Final { speaker, text } => {
                    repo.append(&session.id, *speaker, text, true, ts).expect("append final");
                }
                TranscriptEvent::Partial { speaker, text } => {
                    repo.append(&session.id, *speaker, text, false, ts).expect("append partial");
                }
                TranscriptEvent::Error { message } => panic!("不应有错误：{message}"),
            }
        }

        let finals = repo.list_finals(&session.id).expect("finals");
        let texts: Vec<&str> = finals.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(
            texts,
            vec!["请介绍一下你最近的项目", "这个项目的架构是怎样的"]
        );
        assert!(finals.iter().all(|s| s.speaker == "interviewer"));
        assert!(repo.count_segments(&session.id).expect("count") > finals.len(), "应包含 partial 记录");

        assert!(repo.end_session(&session.id).expect("end"));
    }
}
