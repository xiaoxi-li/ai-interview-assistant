//! 本地 WebSocket mock 服务端（**仅测试**）。
//!
//! 作用：在 `127.0.0.1` 上按阿里云百炼实时识别的协议时序回放完整交互，使云端通路
//! 「run-task → task-started → 音频 → result-generated → finish-task → task-finished」
//! 与三类故障（鉴权失败 / 中途断连 / 连接超时）都能**离线自动化验证**，无需真实凭证。

use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

/// mock 服务端行为
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockBehavior {
    /// 正常时序（partial → final → task-finished）
    Normal,
    /// 握手阶段即回 `task-failed`
    AuthFailed,
    /// 收到第一段音频后直接断开连接
    DropAfterAudio,
}

/// mock 服务端配置
#[derive(Debug, Clone)]
pub struct MockConfig {
    pub behavior: MockBehavior,
    pub partial_text: String,
    pub final_text: String,
    /// 累计收到多少字节音频后回一次中间结果
    pub partial_after_bytes: usize,
}

impl MockConfig {
    pub fn normal() -> Self {
        Self {
            behavior: MockBehavior::Normal,
            partial_text: "面试官在问".to_string(),
            final_text: "面试官在问问题".to_string(),
            // 100ms @16kHz 单声道 16bit = 3200 字节
            partial_after_bytes: 3_200,
        }
    }

    pub fn auth_failed() -> Self {
        Self {
            behavior: MockBehavior::AuthFailed,
            ..Self::normal()
        }
    }

    pub fn drop_after_audio() -> Self {
        Self {
            behavior: MockBehavior::DropAfterAudio,
            ..Self::normal()
        }
    }
}

/// mock 服务端句柄：析构时通知后台线程退出
pub struct MockServer {
    port: u16,
    stop: Arc<AtomicBool>,
}

impl MockServer {
    /// 启动 mock 服务端（异步监听 → 同步返回端口）
    pub fn start(config: MockConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_handle = stop.clone();
        let (port_tx, port_rx) = std_mpsc::channel::<u16>();

        thread::Builder::new()
            .name("asr-cloud-mock".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("mock runtime");
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("mock bind");
                    let port = listener.local_addr().expect("mock addr").port();
                    let _ = port_tx.send(port);

                    loop {
                        if stop_handle.load(Ordering::SeqCst) {
                            break;
                        }
                        match tokio::time::timeout(
                            Duration::from_millis(50),
                            listener.accept(),
                        )
                        .await
                        {
                            Ok(Ok((stream, _))) => {
                                let _ = handle_connection(stream, &config).await;
                            }
                            Ok(Err(_)) => break,
                            Err(_) => continue,
                        }
                    }
                });
            })
            .expect("spawn mock server");

        let port = port_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("mock 服务端启动超时");
        Self { port, stop }
    }

    /// 供引擎使用的 WebSocket 地址
    pub fn websocket_url(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    config: &MockConfig,
) -> Result<(), String> {
    let mut ws = tokio_tungstenite::accept_async(stream)
        .await
        .map_err(|err| err.to_string())?;

    // 1) 读取 run-task，取回 task_id
    let task_id = match ws.next().await {
        Some(Ok(Message::Text(text))) => parse_task_id(text.as_str()),
        _ => return Ok(()),
    };

    // 2) 鉴权失败：直接回 task-failed
    if config.behavior == MockBehavior::AuthFailed {
        let _ = ws.send(Message::text(failed_event(&task_id))).await;
        let _ = ws.close(None).await;
        return Ok(());
    }

    // 3) 握手成功
    ws.send(Message::text(started_event(&task_id)))
        .await
        .map_err(|err| err.to_string())?;

    // 4) 双向循环：收音频回中间结果，收 finish-task 回最终结果并结束
    let mut audio_bytes = 0usize;
    let mut sent_partial = false;
    while let Some(message) = ws.next().await {
        match message {
            Ok(Message::Binary(data)) => {
                audio_bytes += data.len();
                if config.behavior == MockBehavior::DropAfterAudio {
                    // 模拟网络中断：不发关闭帧直接断开
                    return Ok(());
                }
                if !sent_partial && audio_bytes >= config.partial_after_bytes {
                    sent_partial = true;
                    let _ = ws
                        .send(Message::text(result_event(
                            &task_id,
                            &config.partial_text,
                            false,
                        )))
                        .await;
                }
            }
            Ok(Message::Text(text)) => {
                if text.as_str().contains("finish-task") {
                    let _ = ws
                        .send(Message::text(result_event(
                            &task_id,
                            &config.final_text,
                            true,
                        )))
                        .await;
                    let _ = ws.send(Message::text(finished_event(&task_id))).await;
                    let _ = ws.close(None).await;
                    return Ok(());
                }
            }
            Ok(Message::Close(_)) | Err(_) => return Ok(()),
            _ => {}
        }
    }

    Ok(())
}

fn parse_task_id(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .pointer("/header/task_id")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "mock_task".to_string())
}

fn started_event(task_id: &str) -> String {
    serde_json::json!({
        "header": { "task_id": task_id, "event": "task-started" },
        "payload": {}
    })
    .to_string()
}

fn finished_event(task_id: &str) -> String {
    serde_json::json!({
        "header": { "task_id": task_id, "event": "task-finished" },
        "payload": {}
    })
    .to_string()
}

fn failed_event(task_id: &str) -> String {
    serde_json::json!({
        "header": {
            "task_id": task_id,
            "event": "task-failed",
            "error_code": "InvalidApiKey",
            "error_message": "Invalid API-key provided."
        },
        "payload": {}
    })
    .to_string()
}

fn result_event(task_id: &str, text: &str, is_final: bool) -> String {
    serde_json::json!({
        "header": { "task_id": task_id, "event": "result-generated" },
        "payload": {
            "output": {
                "sentence": {
                    "text": text,
                    "sentence_end": is_final
                }
            }
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_events_expose_expected_protocol_fields() {
        let started: serde_json::Value =
            serde_json::from_str(&started_event("t1")).expect("json");
        assert_eq!(started["header"]["event"], "task-started");

        let result: serde_json::Value =
            serde_json::from_str(&result_event("t1", "你好", true)).expect("json");
        assert_eq!(result["header"]["event"], "result-generated");
        assert_eq!(result["payload"]["output"]["sentence"]["text"], "你好");
        assert_eq!(
            result["payload"]["output"]["sentence"]["sentence_end"],
            true
        );

        let failed: serde_json::Value =
            serde_json::from_str(&failed_event("t1")).expect("json");
        assert_eq!(failed["header"]["event"], "task-failed");
        assert_eq!(failed["header"]["error_code"], "InvalidApiKey");
    }

    #[test]
    fn task_id_is_extracted_from_run_task() {
        let run_task = serde_json::json!({
            "header": { "action": "run-task", "task_id": "ei_abc" }
        })
        .to_string();
        assert_eq!(parse_task_id(&run_task), "ei_abc");
        assert_eq!(parse_task_id("not json"), "mock_task");
    }
}
