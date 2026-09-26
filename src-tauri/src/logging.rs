//! 结构化日志（分级、脱敏、按天滚动写入文件）。
//!
//! * 级别：`RUST_LOG` 优先，否则 debug 构建用 `debug`、release 用 `info`；
//! * 输出：标准错误（开发时可见）+ `<应用数据目录>/logs/app.log.<日期>`（按天滚动）；
//! * 脱敏：所有日志在写出前经过 [`redact`]，避免 API Key / 凭证进入日志文件。

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// 敏感字段名（比较时忽略大小写与 `-`/`_` 差异）
const SENSITIVE_FIELDS: &[&str] = &[
    "apikey",
    "api_key",
    "key",
    "secret",
    "secretkey",
    "secret_key",
    "apisecret",
    "api_secret",
    "token",
    "access_token",
    "accesskey",
    "access_key",
    "refresh_token",
    "password",
    "passwd",
    "authorization",
    "appid",
    "app_id",
    "secretid",
    "secret_id",
    "bearer",
];

/// 需要整体遮蔽的密钥前缀
const SECRET_PREFIXES: &[&str] = &["sk-", "sk_", "xai-", "ghp_", "akia", "gho_"];

/// 形似随机密钥的最小长度（长串字母数字/符号）
const SUSPICIOUS_MIN_LEN: usize = 32;

/// 脱敏后的占位
const MASK: &str = "***";

/// 保留的明文字符数（便于排查「是哪一把 key」而不泄露内容）
const KEEP_PREFIX: usize = 4;

/// 是否为敏感字段名（忽略大小写、分隔符与包裹符号差异）
fn is_sensitive_field(name: &str) -> bool {
    let normalized: String = name
        .trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '{' | '}' | '[' | ']' | ':' | '=' | ',' | ';')
        })
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    SENSITIVE_FIELDS
        .iter()
        .any(|field| field.replace(['-', '_'], "") == normalized)
}

/// 遮蔽一个值：保留少量前缀便于定位，其余全部替换。
/// 会保留原有的引号包裹与尾部标点（JSON 场景），避免破坏日志结构。
fn mask_value(raw: &str) -> String {
    let trimmed = raw.trim();

    // 尾部标点（如 `,` `}` 属于 JSON 结构，不属于密钥）
    let mut core = trimmed;
    let mut tail = String::new();
    loop {
        let Some(last) = core.chars().last() else { break };
        if matches!(last, ',' | ';' | '}' | ']') {
            tail.insert(0, last);
            core = &core[..core.len() - last.len_utf8()];
        } else {
            break;
        }
    }

    // 引号包裹
    let bytes = core.as_bytes();
    let quoted = bytes.len() >= 2
        && (bytes[0] == b'"' || bytes[0] == b'\'')
        && bytes[bytes.len() - 1] == bytes[0];
    let inner = if quoted { &core[1..core.len() - 1] } else { core };

    let masked = if inner.chars().count() <= KEEP_PREFIX {
        MASK.to_string()
    } else {
        format!(
            "{}{MASK}",
            inner.chars().take(KEEP_PREFIX).collect::<String>()
        )
    };

    if quoted {
        let quote = core.chars().next().unwrap_or('"');
        format!("{quote}{masked}{quote}{tail}")
    } else {
        format!("{masked}{tail}")
    }
}

/// 形似随机密钥（长且由字母数字/常见符号组成）
fn looks_like_secret(token: &str) -> bool {
    if token.chars().count() < SUSPICIOUS_MIN_LEN {
        return false;
    }
    token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_' | '.'))
}

/// 是否以已知密钥前缀开头
fn has_secret_prefix(token: &str) -> bool {
    let lowered = token.to_ascii_lowercase();
    SECRET_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix) && token.chars().count() > prefix.len() + 3)
}

/// 分隔符（不参与「字段=值」识别，但会切分字段，保证 JSON 等无空格结构可被逐字段处理）
fn is_delimiter(ch: char) -> bool {
    matches!(ch, ',' | ';' | '{' | '}' | '[' | ']')
}

/// 把文本切成「空白/分隔符段」与「词段」两类片段，便于原地替换
fn split_keep_whitespace(text: &str) -> Vec<(bool, &str)> {
    let mut pieces: Vec<(bool, &str)> = Vec::new();
    let mut start = 0usize;
    let mut current_is_space: Option<bool> = None;

    for (index, ch) in text.char_indices() {
        let is_space = ch.is_whitespace() || is_delimiter(ch);
        match current_is_space {
            Some(state) if state == is_space => {}
            Some(state) => {
                pieces.push((!state, &text[start..index]));
                start = index;
                current_is_space = Some(is_space);
            }
            None => {
                start = 0;
                current_is_space = Some(is_space);
            }
        }
    }
    if let Some(state) = current_is_space {
        pieces.push((!state, &text[start..]));
    }
    pieces
}

/// 脱敏：遮蔽 API Key、Token、密码等敏感值
pub fn redact(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut mask_next = false;

    for (is_word, piece) in split_keep_whitespace(text) {
        if !is_word {
            output.push_str(piece);
            continue;
        }

        // 形如 key=value / key: value 的字段
        if let Some(separator) = piece.find(['=', ':']) {
            let (head, rest) = piece.split_at(separator);
            let value = &rest[1..];
            if is_sensitive_field(head) && !value.is_empty() {
                output.push_str(head);
                output.push_str(&rest[..1]);
                output.push_str(&mask_value(value));
                mask_next = false;
                continue;
            }
        }

        if is_sensitive_field(piece) {
            // 单独的敏感字段名（如 `Authorization:` / `Bearer` / `token`）：
            // 原样输出，并遮蔽紧随其后的值（空白片段已由外层保留）
            output.push_str(piece);
            mask_next = true;
            continue;
        }

        if mask_next {
            output.push_str(&mask_value(piece));
            mask_next = false;
            continue;
        }

        if has_secret_prefix(piece) || looks_like_secret(piece) {
            output.push_str(&mask_value(piece));
            continue;
        }

        output.push_str(piece);
    }

    output
}

/// 时间戳（Unix 秒.毫秒）——避免引入日期库
fn timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{}.{:03}", duration.as_secs(), duration.subsec_millis()),
        Err(_) => "0.000".to_string(),
    }
}

/// 收集事件字段为字符串（随后统一脱敏）
struct FieldCollector(String);

impl tracing::field::Visit for FieldCollector {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let rendered = format!("{value:?}");
            self.0
                .push_str(rendered.trim_matches('"'));
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            self.0.push_str(&format!("{}={value:?}", field.name()));
        }
    }
}

/// 自定义事件格式：`时间 [级别] 目标: 消息`（消息经脱敏处理）
struct RedactingFormat;

impl<S, N> FormatEvent<S, N> for RedactingFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let metadata = event.metadata();
        let mut visitor = FieldCollector(String::new());
        event.record(&mut visitor);

        let target = metadata.target();
        let scope = ctx
            .event_scope()
            .and_then(|scope| scope.into_iter().next().map(|span| span.name().to_string()));

        match scope {
            Some(span) => write!(
                writer,
                "{} [{}] {} ({}): {}",
                timestamp(),
                metadata.level(),
                target,
                span,
                redact(&visitor.0)
            ),
            None => write!(
                writer,
                "{} [{}] {}: {}",
                timestamp(),
                metadata.level(),
                target,
                redact(&visitor.0)
            ),
        }
    }
}

/// 日志文件写入线程句柄（需长期持有，否则日志会丢失）
static GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// 日志目录（`<应用数据目录>/logs`）
pub fn log_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("logs")
}

/// 初始化日志（全局仅生效一次；重复调用安全）
pub fn init_once(app_data_dir: &Path) -> Result<PathBuf, String> {
    let dir = log_dir(app_data_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建日志目录失败：{e}"))?;

    if GUARD.get().is_some() {
        return Ok(dir);
    }

    let level = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    });
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));

    let file_appender = tracing_appender::rolling::daily(&dir, "app.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let stderr_layer = tracing_subscriber::fmt::layer()
        .event_format(RedactingFormat)
        .with_writer(std::io::stderr);
    let file_layer = tracing_subscriber::fmt::layer()
        .event_format(RedactingFormat)
        .with_ansi(false)
        .with_writer(file_writer);

    // try_init：重复初始化时返回 Err，忽略即可（不 panic）
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    let _ = GUARD.set(guard);
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "aia-log-{tag}-{}-{}",
            std::process::id(),
            crate::knowledge::store::now_ms()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn masks_openai_style_keys() {
        assert_eq!(
            redact("调用失败 key=sk-abcdefghijklmnop1234567890"),
            "调用失败 key=sk-a***"
        );
        assert_eq!(redact("Bearer sk-abcdefghijklmnop"), "Bearer sk-a***");
    }

    #[test]
    fn masks_sensitive_field_values() {
        assert_eq!(redact("api_key=1234567890abcdef"), "api_key=1234***");
        assert_eq!(redact("apiKey: \"abcdef123456\""), "apiKey: \"abcd***\"");
        assert_eq!(
            redact(r#"{"apiKey":"abcdef123456","model":"deepseek"}"#),
            r#"{"apiKey":"abcd***","model":"deepseek"}"#
        );
        assert_eq!(redact("secret_key=topsecretvalue"), "secret_key=tops***");
        assert_eq!(redact("password=hunter2hunter2"), "password=hunt***");
    }

    #[test]
    fn masks_standalone_secrets_after_keyword() {
        assert_eq!(
            redact("Authorization: Bearer abcdef1234567890"),
            "Authorization: Bearer abcd***"
        );
        assert_eq!(redact("token abcdef1234567890"), "token abcd***");
    }

    #[test]
    fn masks_long_random_tokens() {
        let token = "a".repeat(48);
        // 独立的长随机串（前面不是敏感字段名）
        let redacted = redact(&format!("响应体无法解析，原始内容 {token}"));
        assert!(redacted.contains("aaaa***"), "{redacted}");
        assert!(!redacted.contains(&token), "长随机串应被遮蔽");

        // 紧跟敏感字段名时同样被遮蔽
        let redacted = redact(&format!("token {token}"));
        assert!(!redacted.contains(&token), "{redacted}");
    }

    #[test]
    fn keeps_ordinary_text_intact() {
        let text = "模型 deepseek-chat 返回 HTTP 200，耗时 812ms，命中 3 条笔记";
        assert_eq!(redact(text), text);
        assert_eq!(redact("中文内容与常规 ID：s-1 / q-2"), "中文内容与常规 ID：s-1 / q-2");
    }

    #[test]
    fn short_values_are_fully_masked() {
        assert_eq!(redact("key=abc"), "key=***");
        assert_eq!(redact("api_key=ab"), "api_key=***");
        assert_eq!(redact("token=abc"), "token=***");
    }

    #[test]
    fn empty_and_whitespace_are_safe() {
        assert_eq!(redact(""), "");
        assert_eq!(redact("   "), "   ");
    }

    #[test]
    fn split_keep_whitespace_preserves_layout() {
        let pieces = split_keep_whitespace("a  b\tc");
        let rebuilt: String = pieces.iter().map(|(_, piece)| *piece).collect();
        assert_eq!(rebuilt, "a  b\tc");
        assert_eq!(pieces.len(), 5);
    }

    #[test]
    fn sensitive_field_names_ignore_case_and_separators() {
        for name in ["api_key", "API-KEY", "ApiKey", "apikey", "secret_id"] {
            assert!(is_sensitive_field(name), "{name} 应识别为敏感字段");
        }
        for name in ["model", "base_url", "temperature", "platform"] {
            assert!(!is_sensitive_field(name), "{name} 不应识别为敏感字段");
        }
    }

    #[test]
    fn file_layer_writes_redacted_lines() {
        let dir = temp_dir("file");
        let appender = tracing_appender::rolling::never(&dir, "test.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);

        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .event_format(RedactingFormat)
                .with_ansi(false)
                .with_writer(writer),
        );

        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!("调用失败 api_key=sk-abcdefghijklmnop123456");
        });
        drop(guard);

        let content = std::fs::read_to_string(dir.join("test.log")).expect("读取日志文件");
        assert!(content.contains("[WARN]"), "应写入级别：{content}");
        assert!(content.contains("api_key=sk-a***"), "应写入脱敏后的消息：{content}");
        assert!(
            !content.contains("abcdefghijklmnop"),
            "完整密钥不应进入日志：{content}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn init_creates_log_directory_and_is_idempotent() {
        let app_dir = temp_dir("init");
        let dir = init_once(&app_dir).expect("init");
        assert!(dir.exists(), "日志目录应被创建");
        assert_eq!(dir, app_dir.join("logs"));

        // 重复初始化不应 panic
        let again = init_once(&app_dir).expect("init again");
        assert_eq!(again, dir);

        std::fs::remove_dir_all(&app_dir).ok();
    }
}
