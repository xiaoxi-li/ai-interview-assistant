//! 采集工作线程脚手架：麦克风（cpal）与系统音频（WASAPI 回环）共用。
//!
//! 两类采集流有同一约束 —— `cpal::Stream` 与 WASAPI 的 COM 对象都**不是 `Send`**，
//! 必须在创建它们的线程内存活。因此统一由专用线程持有，对外只暴露可跨线程传递的
//! 句柄 [`CaptureWorker`]，上层只认「事件流 + 停止信号」。

use crate::providers::types::AudioChunk;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// 启动超时（等待设备打开）
pub const START_TIMEOUT: Duration = Duration::from_secs(10);

/// 采集流就绪通知：采集流成功打开后调用 [`Ready::ok`]；
/// 若在就绪前失败，直接让构建闭包返回 `Err` 即可，由 [`CaptureWorker::spawn`] 上报。
pub struct Ready(Sender<Result<(), String>>);

impl Ready {
    /// 声明采集流已成功打开（`spawn` 收到后立即返回）
    pub fn ok(self) {
        let _ = self.0.send(Ok(()));
    }
}

/// 采集工作线程句柄
pub struct CaptureWorker {
    stop_tx: Sender<()>,
    events: Option<Receiver<AudioChunk>>,
    join: Option<JoinHandle<()>>,
}

impl CaptureWorker {
    /// 在专用线程内建立采集流（线程在 `build` 返回后结束）
    ///
    /// `build` 的三个参数依次为：就绪通知、事件发送端、停止信号接收端。
    pub fn spawn<F>(build: F) -> Result<Self, String>
    where
        F: FnOnce(Ready, Sender<AudioChunk>, Receiver<()>) -> Result<(), String> + Send + 'static,
    {
        let (chunk_tx, chunk_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();

        let join = thread::spawn(move || {
            if let Err(err) = build(Ready(ready_tx.clone()), chunk_tx, stop_rx) {
                tracing::warn!("音频采集线程退出：{err}");
                // 就绪前失败：上报错误；已就绪时该消息无人接收，忽略即可
                let _ = ready_tx.send(Err(err));
            }
        });

        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                stop_tx,
                events: Some(chunk_rx),
                join: Some(join),
            }),
            Ok(Err(err)) => {
                let _ = join.join();
                Err(err)
            }
            // 超时或线程异常退出（发送端被丢弃）
            Err(_) => Err("启动音频采集超时或采集线程异常退出".into()),
        }
    }

    /// 取出事件流（每个实例仅可取出一次）
    pub fn take_events(&mut self) -> Result<Receiver<AudioChunk>, String> {
        self.events
            .take()
            .ok_or_else(|| "采集尚未启动或事件流已被取出".to_string())
    }

    /// 发送停止信号并等待采集线程退出
    pub fn stop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_reports_build_failure() {
        let failed = CaptureWorker::spawn(|_ready, _tx, _stop| Err("设备打开失败".to_string()));
        assert_eq!(failed.err(), Some("设备打开失败".to_string()));
    }

    #[test]
    fn spawn_succeeds_when_build_signals_ready() {
        let mut worker = CaptureWorker::spawn(|ready, _tx, stop_rx| {
            ready.ok();
            let _ = stop_rx.recv();
            Ok(())
        })
        .expect("应能启动");

        assert!(worker.take_events().is_ok());
        assert!(worker.take_events().is_err(), "事件流仅可取出一次");
        worker.stop();
    }

    #[test]
    fn stop_is_idempotent() {
        let mut worker =
            CaptureWorker::spawn(|ready, _tx, stop_rx| {
                ready.ok();
                let _ = stop_rx.recv();
                Ok(())
            })
            .expect("应能启动");
        worker.stop();
        worker.stop();
    }
}
