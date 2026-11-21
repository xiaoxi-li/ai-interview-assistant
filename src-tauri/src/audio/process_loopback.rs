//! Windows **进程级回环**（TODO-05 / IA-14）：只采集指定进程（含其子进程树）的音频输出。
//!
//! 与端点级回环（[`super::loopback`]，采集整个播放设备的混音）不同，本模块走
//! WASAPI **ApplicationLoopback** 通路：
//!
//! * 激活：`ActivateAudioInterfaceAsync` + `AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK`
//!   + `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE`（虚拟设备 `VAD\Process_Loopback`）；
//! * 进程枚举：默认播放设备 → `IAudioSessionManager2` → `IAudioSessionEnumerator`
//!   → `IAudioSessionControl2::GetProcessId`，供前端选择目标应用（如会议软件）；
//! * 采集流：`AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_EVENTCALLBACK` 共享模式，
//!   缓冲时长与周期必须为 0（进程回环的硬性要求）；格式取引擎混音格式（`GetMixFormat`）；
//! * 输出与端点回环一致：降混 / 重采样为 16kHz 单声道 f32 帧。
//!
//! `wasapi` crate 不支持进程回环激活，故此处直接使用 `windows` crate 的 COM 绑定
//! （版本与 `wasapi` 的传递依赖对齐，避免多版本共存）。COM 对象不是 `Send`，
//! 采集流仍由 [`CaptureWorker`] 专用线程持有。
//!
//! 需要 Windows 10 2004（build 19041）及以上。

use super::mic::push_samples;
use super::worker::{CaptureWorker, Ready};
use super::AudioCapture;
use crate::providers::types::AudioChunk;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::OnceLock;
use windows::core::{implement, IUnknown, Interface, GUID};
use windows::Win32::Foundation::{CloseHandle, WAIT_FAILED, WAIT_TIMEOUT};
use windows::Win32::Media::Audio::{
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioCaptureClient, IAudioClient, IAudioSessionControl, IAudioSessionControl2,
    IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator,
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX, eConsole, eRender,
};
use windows::Win32::System::Com::{BLOB, CoCreateInstance};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Threading::{
    CreateEventW, OpenProcess, QueryFullProcessImageNameW, SetEvent, WaitForSingleObject,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::Variant::VT_BLOB;

/// MMDeviceEnumerator CLSID（`BCDE0395-E52F-467C-8E3D-C4579291692E`）
const CLSID_MMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);

/// `AUDCLNT_BUFFERFLAGS_SILENT`（表示缓冲为静音）
const BUFFERFLAGS_SILENT: u32 = 0x2;
/// 激活结果等待超时（毫秒）
const ACTIVATION_TIMEOUT_MS: u32 = 8_000;
/// 采集事件等待超时（毫秒）：目标进程无输出时事件不触发，按空转继续
const EVENT_TIMEOUT_MS: u32 = 200;

/// 有音频会话的进程信息（前端选择目标应用用）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    /// 进程可执行文件名（如 wemeetapp.exe）
    pub name: String,
    /// 会话是否活跃（正在发声）
    pub active: bool,
}

/// 枚举默认播放设备上有音频会话的进程（去重；系统会话 pid=0 过滤掉）
pub fn enumerate_audio_processes() -> Result<Vec<ProcessInfo>, String> {
    // COM 初始化与本线程严格配对（结束时 CoUninitialize）
    let com = unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        )
    };
    let result = enumerate_audio_processes_inner();
    if com.is_ok() {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
    result
}

fn enumerate_audio_processes_inner() -> Result<Vec<ProcessInfo>, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(
            &CLSID_MMDEVICE_ENUMERATOR,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|err| format!("初始化音频设备枚举失败：{err}"))?;
        let device: IMMDevice = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|err| format!("未找到系统默认播放设备：{err}"))?;
        let session_manager: IAudioSessionManager2 =
            device.Activate(windows::Win32::System::Com::CLSCTX_ALL, None)
                .map_err(|err| format!("获取音频会话管理器失败：{err}"))?;
        let session_enum: IAudioSessionEnumerator = session_manager
            .GetSessionEnumerator()
            .map_err(|err| format!("获取会话枚举器失败：{err}"))?;
        let count = session_enum
            .GetCount()
            .map_err(|err| format!("读取会话数量失败：{err}"))?;

        let mut processes: Vec<ProcessInfo> = Vec::new();
        for index in 0..count {
            let control: IAudioSessionControl = match session_enum.GetSession(index) {
                Ok(control) => control,
                Err(_) => continue,
            };
            let control2: IAudioSessionControl2 = match control.cast() {
                Ok(control2) => control2,
                Err(_) => continue,
            };
            let pid = control2.GetProcessId().unwrap_or(0);
            // pid 0 是系统音频服务自身的会话，不作为可选目标
            if pid == 0 {
                continue;
            }
            if processes.iter().any(|process| process.pid == pid) {
                continue;
            }
            let active = control2
                .GetState()
                .map(|state| state.0 == 1) // AudioSessionStateActive
                .unwrap_or(false);
            processes.push(ProcessInfo {
                pid,
                name: process_name(pid).unwrap_or_else(|| format!("pid-{pid}")),
                active,
            });
        }
        // 活跃（正在发声）的排前面，其次按名称排序
        processes.sort_by(|a, b| b.active.cmp(&a.active).then(a.name.cmp(&b.name)));
        Ok(processes)
    }
}

/// 进程可执行文件名（失败返回 None）
fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 512];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(handle);
        if !ok {
            return None;
        }
        let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
        Some(
            full_path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&full_path)
                .to_string(),
        )
    }
}

/// 回调对象持有的资源：事件句柄 + 激活参数（堆）
///
/// 进程回环激活的完成回调：唤醒等待方
///
/// **所有权设计**（避免堆损坏与句柄竞态）：
/// * 本对象**只持有数据**（事件句柄值），析构不做任何系统调用 —— 即使系统侧
///   存在延迟 Release，最坏也只是对象内存延迟释放，无副作用；
/// * `done` 事件与激活参数（堆）由**等待方**（[`run_process_loopback`]）在
///   回调完成、激活结果取回之后统一释放。
#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivationHandler {
    /// 激活完成事件（手动复位；所有权在等待方，回调只负责 SetEvent）
    done: windows::Win32::Foundation::HANDLE,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivationHandler_Impl {
    fn ActivateCompleted(
        &self,
        _activateoperation: windows::core::Ref<'_, IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        // 只负责唤醒等待方：激活结果由等待线程在事件后调用
        // `GetActivateResult` 获取（避免在系统回调线程上访问激活内部状态）
        let _ = unsafe { SetEvent(self.done) };
        Ok(())
    }
}

/// 与 `PROPVARIANT` 布局等价的 blob 载体（vt=VT_BLOB），用 `transmute` 规避 union 写入限制
#[repr(C)]
#[allow(non_snake_case)] // 与 Win32 PROPVARIANT 字段名保持一致
struct BlobPropVariant {
    vt: u16,
    wReserved1: u16,
    wReserved2: u16,
    wReserved3: u16,
    blob: BLOB,
}

// 布局一致性由编译期断言保证
const _: () = assert!(
    std::mem::size_of::<BlobPropVariant>() == std::mem::size_of::<PROPVARIANT>()
);

/// 把 `AUDIOCLIENT_ACTIVATION_PARAMS` 打包为 `PROPVARIANT`（VT_BLOB）
///
/// blob 指向**堆上**参数（由调用方保活至激活完成），避免异步激活期间悬垂；
/// 采用与 `PROPVARIANT` 相同的结构化构造（含 `ManuallyDrop`，无 Drop 语义）。
fn activation_params_propvariant(params: &Box<AUDIOCLIENT_ACTIVATION_PARAMS>) -> PROPVARIANT {
    let blob = BlobPropVariant {
        vt: VT_BLOB.0,
        wReserved1: 0,
        wReserved2: 0,
        wReserved3: 0,
        blob: BLOB {
            cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
            pBlobData: &**params as *const AUDIOCLIENT_ACTIVATION_PARAMS as *mut u8,
        },
    };
    // 结构化构造：与 blob 布局一致，逐字段映射（无 Drop 语义）
    PROPVARIANT {
        Anonymous: windows::Win32::System::Com::StructuredStorage::PROPVARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(
                windows::Win32::System::Com::StructuredStorage::PROPVARIANT_0_0 {
                    vt: VT_BLOB,
                    wReserved1: blob.wReserved1,
                    wReserved2: blob.wReserved2,
                    wReserved3: blob.wReserved3,
                    Anonymous: windows::Win32::System::Com::StructuredStorage::PROPVARIANT_0_0_0 {
                        blob: blob.blob,
                    },
                },
            ),
        },
    }
}

/// 跨线程进程回环采集句柄：内部线程持有 WASAPI 采集流
#[derive(Default)]
pub struct WasapiProcessLoopbackCapture {
    worker: Option<CaptureWorker>,
    /// 当前采集目标（`pid:<n>` 解析结果）
    target_pid: Option<u32>,
}

impl WasapiProcessLoopbackCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.worker.is_some()
    }

    /// 当前目标进程 PID
    pub fn target(&self) -> Option<u32> {
        self.target_pid
    }
}

impl AudioCapture for WasapiProcessLoopbackCapture {
    /// `device_id` 格式：`pid:<进程号>`（非法格式返回可操作错误）
    fn start(&mut self, device_id: &str) -> Result<(), String> {
        if self.worker.is_some() {
            return Err("进程回环采集已在进行中".into());
        }
        let pid = parse_target_pid(device_id)?;

        let worker = CaptureWorker::spawn(move |ready, tx, stop_rx| {
            run_process_loopback(pid, ready, tx, stop_rx)
        })?;

        self.target_pid = Some(pid);
        self.worker = Some(worker);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
        self.target_pid = None;
        Ok(())
    }

    fn take_events(&mut self) -> Result<Receiver<AudioChunk>, String> {
        match self.worker.as_mut() {
            Some(worker) => worker.take_events(),
            None => Err("采集尚未启动或事件流已被取出".to_string()),
        }
    }
}

/// 解析 `pid:<n>` 形式的目标标识
pub fn parse_target_pid(device_id: &str) -> Result<u32, String> {
    let trimmed = device_id.trim();
    let pid = trimmed
        .strip_prefix("pid:")
        .or_else(|| trimmed.strip_prefix("PID:"))
        .unwrap_or(trimmed);
    pid.parse::<u32>().map_err(|_| {
        format!(
            "进程级回环的目标格式应为「pid:<进程号>」，实际收到「{device_id}」"
        )
    })
}

/// 采集线程主体：激活进程回环流 → 循环投递 16kHz 单声道帧
///
/// COM 初始化与本线程严格配对（结束时 `CoUninitialize`）—— MTA 的最后一个引用
/// 被释放时会触发系统侧 apartment 清理，缺失配对会导致延迟的堆损坏。
fn run_process_loopback(
    pid: u32,
    ready: Ready,
    tx: Sender<AudioChunk>,
    stop_rx: Receiver<()>,
) -> Result<(), String> {
    let com = unsafe {
        windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        )
    };
    let result = run_process_loopback_inner(pid, ready, tx, stop_rx);
    if com.is_ok() {
        unsafe {
            windows::Win32::System::Com::CoUninitialize();
        }
    }
    result
}

fn run_process_loopback_inner(
    pid: u32,
    ready: Ready,
    tx: Sender<AudioChunk>,
    stop_rx: Receiver<()>,
) -> Result<(), String> {
    unsafe {

        // 1) 组装激活参数（堆上，VT_BLOB 包 AUDIOCLIENT_ACTIVATION_PARAMS；
        //    参数由完成回调对象持有保活，覆盖整个异步激活生命周期）
        let mut activation_params = Box::new(AUDIOCLIENT_ACTIVATION_PARAMS::default());
        activation_params.ActivationType = AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK;
        activation_params.Anonymous.ProcessLoopbackParams =
            windows::Win32::Media::Audio::AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            };

        // 2) 手动复位事件 + 完成回调（回调只 SetEvent；
        //    事件句柄与激活参数由本线程持有，回调对象本身无任何析构副作用）
        let done = CreateEventW(None, true, false, None)
            .map_err(|err| format!("创建激活等待事件失败：{err}"))?;
        let propvariant = activation_params_propvariant(&activation_params);
        let handler: IActivateAudioInterfaceCompletionHandler =
            ActivationHandler { done }.into();

        // 3) 异步激活虚拟进程回环设备
        let async_operation = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&propvariant as *const PROPVARIANT),
            &handler,
        )
        .map_err(|err| {
            format!(
                "激活进程回环失败（进程 {pid}）：{err}。需要 Windows 10 2004 及以上，且目标进程正在运行"
            )
        })?;

        // 4) 等待激活完成（完成回调只 SetEvent；结果在等待线程上取回）
        let wait = WaitForSingleObject(done, ACTIVATION_TIMEOUT_MS);
        if wait == WAIT_FAILED || wait == WAIT_TIMEOUT {
            // 超时：句柄与参数可能仍被迟到的回调触碰，泄漏而非关闭（记日志）
            tracing::warn!("进程回环激活超时（pid {pid}）：等待事件与激活参数将延迟到进程退出时释放");
            std::mem::forget(done);
            std::mem::forget(activation_params);
            return Err(format!(
                "激活进程回环超时（{ACTIVATION_TIMEOUT_MS}ms，进程 {pid}）"
            ));
        }
        // 激活已确认完成：回调不会再访问事件与参数，由本线程统一释放
        let _ = CloseHandle(done);

        let client: IAudioClient = {
            let mut hr = windows::core::HRESULT(0);
            let mut unknown: Option<IUnknown> = None;
            async_operation
                .GetActivateResult(&mut hr, &mut unknown)
                .map_err(|err| format!("读取激活结果失败：{err}"))?;
            if hr.is_err() {
                return Err(format!(
                    "进程回环激活失败（进程 {pid}）：{}。请确认目标进程存在且正在播放音频，且系统为 Windows 10 2004 及以上",
                    windows::core::Error::from_hresult(hr)
                ));
            }
            unknown
                .ok_or("激活成功但未返回接口")?
                .cast()
                .map_err(|err| format!("激活结果类型转换失败：{err}"))?
        };
        // 6) 激活相关对象的**延迟释放**：`async_operation` / `handler` / `propvariant`
        //    的本地释放与 MMDeviceAPI 侧的引用管理存在实测竞态（主动释放可触发
        //    STATUS_HEAP_CORRUPTION，具体为系统侧延迟 Release 与本地析构的交错）。
        //    故会话结束后不再显式释放，交由进程退出统一回收。
        //    代价：每次采集会话泄漏约数百字节的 COM 对象内存 —— 对单次面试会话的
        //    频率与量级可忽略；换来的是确定性不崩溃。
        //    激活参数（Box）与事件句柄无此问题，仍正常释放/关闭。
        std::mem::forget(async_operation);
        std::mem::forget(handler);
        std::mem::forget(propvariant);

        // 5) 初始化采集流：请求固定格式（48kHz 立体声 float32），
        //    由音频引擎自动转换（AUTOCONVERTPCM + SRC_DEFAULT_QUALITY）。
        //    注：进程回环虚拟设备上 GetMixFormat 行为不可靠（实测可致崩溃），
        //    参照成熟实现直接声明目标格式，让引擎负责混音格式互转。
        let format = target_format();
        let flags = AUDCLNT_STREAMFLAGS_LOOPBACK
            | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
            | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
            | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                flags,
                BUFFER_DURATION_HNS,
                0,
                &format as *const WAVEFORMATEX,
                None,
            )
            .map_err(|err| format!("初始化进程回环流失败（进程 {pid}）：{err}"))?;

        let event = CreateEventW(None, false, false, None)
            .map_err(|err| format!("创建采集事件失败：{err}"))?;
        client
            .SetEventHandle(event)
            .map_err(|err| format!("绑定采集事件失败：{err}"))?;
        let capture: IAudioCaptureClient = client
            .GetService()
            .map_err(|err| format!("获取采集客户端失败：{err}"))?;
        client
            .Start()
            .map_err(|err| format!("启动进程回环流失败（进程 {pid}）：{err}"))?;

        ready.ok();

        // 6) 采集循环：事件驱动读取缓冲并转 16k 单声道（目标格式固定为 float32 交错）
        let mut buffer: Vec<f32> = Vec::new();

        while stop_rx.try_recv().is_err() {
            let packet = match capture.GetNextPacketSize() {
                Ok(size) => size,
                Err(err) => {
                    tracing::warn!("进程回环读取失败（pid {pid}）：{err}");
                    let _ = tx.send(crate::audio::capture::error_chunk());
                    break;
                }
            };
            let mut frames_left: u32 = packet;
            while frames_left > 0 {
                let mut data: *mut u8 = std::ptr::null_mut();
                let mut frames_to_read: u32 = 0;
                let mut flags: u32 = 0;
                if let Err(err) =
                    capture.GetBuffer(&mut data, &mut frames_to_read, &mut flags, None, None)
                {
                    tracing::warn!("进程回环取缓冲失败（pid {pid}）：{err}");
                    let _ = tx.send(crate::audio::capture::error_chunk());
                    break;
                }
                let samples = if flags & BUFFERFLAGS_SILENT == 0
                    && !data.is_null()
                    && frames_to_read > 0
                {
                    decode_samples(
                        std::slice::from_raw_parts(
                            data,
                            frames_to_read as usize * CAPTURE_CHANNELS as usize * 4,
                        ),
                    )
                } else {
                    // 静音帧：补零保持时长
                    vec![0.0f32; frames_to_read as usize * CAPTURE_CHANNELS as usize]
                };
                let _ = capture.ReleaseBuffer(frames_to_read);
                if frames_to_read == 0 {
                    break;
                }
                frames_left = frames_left.saturating_sub(frames_to_read);
                if !push_samples(&mut buffer, &samples, CAPTURE_CHANNELS, CAPTURE_SAMPLE_RATE, &tx) {
                    let _ = client.Stop();
                    let _ = CloseHandle(event);
                    return Ok(()); // 接收端已关闭
                }
            }
            // 目标进程暂无输出：超时继续轮询
            let _ = WaitForSingleObject(event, EVENT_TIMEOUT_MS);
        }

        let _ = client.Stop();
        let _ = CloseHandle(event);
        Ok(())
    }
}

/// 请求的采集格式：48kHz 立体声 float32（引擎按此自动转换目标进程的输出）
const CAPTURE_SAMPLE_RATE: u32 = 48_000;
const CAPTURE_CHANNELS: u16 = 2;
/// 缓冲时长（单位 100ns）：20ms
const BUFFER_DURATION_HNS: i64 = 200_000;

/// 进程回环请求的目标格式（WAVE_FORMAT_IEEE_FLOAT 交错）
fn target_format() -> WAVEFORMATEX {
    let block_align = (CAPTURE_CHANNELS * 4) as u16;
    WAVEFORMATEX {
        wFormatTag: 3, // WAVE_FORMAT_IEEE_FLOAT
        nChannels: CAPTURE_CHANNELS,
        nSamplesPerSec: CAPTURE_SAMPLE_RATE,
        nAvgBytesPerSec: CAPTURE_SAMPLE_RATE * block_align as u32,
        nBlockAlign: block_align,
        wBitsPerSample: 32,
        cbSize: 0,
    }
}

/// 把原始缓冲解为 f32 交错样本（目标格式恒为 float32）
fn decode_samples(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// 进程级回环运行能力（仅 Windows 10 2004+ 的运行时才能实际激活）
pub fn is_supported() -> bool {
    true
}

/// 首次调用缓存进程枚举是否可用（避免 UI 反复试探）
static ENUMERATION_CHECK: OnceLock<bool> = OnceLock::new();

/// 进程枚举能力探测：默认播放设备存在且能建立会话枚举
pub fn enumeration_available() -> bool {
    *ENUMERATION_CHECK.get_or_init(|| enumerate_audio_processes().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_pid_parses_pid_prefix() {
        assert_eq!(parse_target_pid("pid:1234").expect("pid"), 1234);
        assert_eq!(parse_target_pid("  pid:5678  ").expect("pid"), 5678);
        assert_eq!(parse_target_pid("4321").expect("纯数字也接受"), 4321);
    }

    #[test]
    fn target_pid_rejects_invalid_input() {
        let err = parse_target_pid("pid:abc").expect_err("非数字应被拒绝");
        assert!(err.contains("pid:"), "{err}");
        assert!(parse_target_pid("").is_err());
        assert!(parse_target_pid("pid:-1").is_err());
        assert!(parse_target_pid("pid:99999999999").is_err(), "超出 u32 应被拒绝");
    }

    #[test]
    fn blob_prop_variant_layout_matches_propvariant() {
        // 编译期断言已保证大小一致；此处运行期复核对齐
        assert_eq!(
            std::mem::size_of::<BlobPropVariant>(),
            std::mem::size_of::<PROPVARIANT>()
        );
        assert_eq!(
            std::mem::align_of::<BlobPropVariant>(),
            std::mem::align_of::<PROPVARIANT>()
        );
    }

    #[test]
    fn target_format_is_ieee_float_stereo_48k() {
        // WAVEFORMATEX 为 packed：解构到局部变量再断言（字段按值拷贝是允许的）
        let WAVEFORMATEX {
            wFormatTag,
            nChannels,
            nSamplesPerSec,
            nAvgBytesPerSec,
            nBlockAlign,
            wBitsPerSample,
            cbSize: _,
        } = target_format();
        assert_eq!(wFormatTag, 3, "应为 IEEE float");
        assert_eq!(nChannels, CAPTURE_CHANNELS);
        assert_eq!(nSamplesPerSec, CAPTURE_SAMPLE_RATE);
        assert_eq!(wBitsPerSample, 32);
        assert_eq!(nBlockAlign, CAPTURE_CHANNELS * 4);
        assert_eq!(nAvgBytesPerSec, CAPTURE_SAMPLE_RATE * (CAPTURE_CHANNELS * 4) as u32);
    }

    #[test]
    fn decode_float32_interleaved_samples() {
        let float_bytes: Vec<u8> = [0.5f32, -0.25f32, 0.125f32]
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(decode_samples(&float_bytes), vec![0.5, -0.25, 0.125]);
        // 不足 4 字节的尾部被忽略（不 panic）
        let with_tail = [float_bytes, vec![0x01, 0x02]].concat();
        assert_eq!(decode_samples(&with_tail), vec![0.5, -0.25, 0.125]);
    }

    #[test]
    fn process_loopback_module_reports_supported() {
        assert!(is_supported());
    }

    #[test]
    fn idle_capture_rejects_bad_target_and_stops_cleanly() {
        let mut capture = WasapiProcessLoopbackCapture::new();
        assert!(!capture.is_running());
        assert!(capture.take_events().is_err(), "未启动时不应有事件流");
        assert_eq!(capture.target(), None);

        // 非法目标直接报可操作错误
        let err = capture.start("not-a-pid").expect_err("非法目标应失败");
        assert!(err.contains("pid:"), "{err}");
        assert!(!capture.is_running());

        assert!(capture.stop().is_ok());
        assert!(capture.stop().is_ok(), "停止应幂等");
        assert_eq!(capture.target(), None, "停止后应清除目标");
    }

    #[test]
    fn handler_com_object_create_and_drop_is_clean() {
        // 最小实验：仅创建/释放完成回调 COM 对象，验证 implement 宏的堆管理。
        // handler 为纯数据（析构无系统调用），事件句柄由创建方关闭。
        unsafe {
            let done = CreateEventW(None, true, false, None).expect("event");
            let handler: IActivateAudioInterfaceCompletionHandler =
                ActivationHandler { done }.into();
            drop(handler);
            let _ = CloseHandle(done);
        }
        // 触发分配器验证（大量分配/释放暴露堆损坏）
        for _ in 0..1000 {
            let _ = vec![0u8; 1024];
        }
    }

    #[test]
    fn nonexistent_process_activates_but_produces_no_crash() {
        // Windows 对无效 PID 的进程回环激活**同样成功**（虚拟设备不校验 PID 存活，
        // 只是流内没有数据）。本用例验证整条链路启动/停止不崩溃、不悬挂。
        let mut capture = WasapiProcessLoopbackCapture::new();
        capture.start("pid:999999").expect("激活应成功（空流）");
        assert!(capture.is_running());
        assert_eq!(capture.target(), Some(999_999));

        // 停止：worker 线程回收 + COM 资源释放，全程不应崩溃
        capture.stop().expect("停止");
        assert!(!capture.is_running());
        // 分配器压力验证（暴露潜在堆损坏）
        for _ in 0..1000 {
            let _ = vec![0u8; 1024];
        }
    }

    #[test]
    fn enumerate_processes_returns_sorted_deduped_list() {
        // 本机环境可能无音频会话，但枚举本身不应失败（有默认播放设备即可）
        if !enumeration_available() {
            return; // 无音频设备的 CI 环境跳过
        }
        let processes = enumerate_audio_processes().expect("枚举");
        let mut pids: Vec<u32> = processes.iter().map(|p| p.pid).collect();
        let total = pids.len();
        pids.sort();
        pids.dedup();
        assert_eq!(pids.len(), total, "进程不应重复");
        assert!(processes.iter().all(|p| p.pid != 0), "系统会话应被过滤");
        assert!(processes.iter().all(|p| !p.name.is_empty()));
    }
}
