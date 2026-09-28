//! 思考 / 非思考模式（ST-12）：把模式按平台差异写入请求体。
//!
//! 各平台的调用方式并不统一，因此这里集中维护映射关系，Provider 只负责调用：
//!
//! | 平台 | 参数 | 思考 | 非思考 |
//! |---|---|---|---|
//! | 通义千问 Qwen（百炼兼容模式） | 顶层 `enable_thinking` | `true` | `false` |
//! | 硅基流动 SiliconFlow | 顶层 `enable_thinking` | `true` | `false` |
//! | 智谱 GLM（`/api/paas/v4`） | `thinking.type` | `"enabled"` | `"disabled"` |
//! | DeepSeek | 无参数（`deepseek-chat` / `deepseek-reasoner` 用模型名区分） | 不下发 | 不下发 |
//! | OpenAI | 无等价布尔开关 | 不下发 | 不下发 |
//!
//! 只下发 `enable_thinking`，不下发 `thinking_budget`：流式场景下
//! `enable_thinking: false` 已足以关闭推理，多余参数只会增加 400 风险。

use crate::config::{AiPlatform, ThinkingMode};
use serde_json::{json, Value};

/// 平台用于控制思考模式的请求参数键；`None` 表示该平台没有开关参数
pub fn param_key(platform: AiPlatform) -> Option<&'static str> {
    match platform {
        AiPlatform::Qwen | AiPlatform::Siliconflow => Some("enable_thinking"),
        AiPlatform::Glm => Some("thinking"),
        AiPlatform::Deepseek | AiPlatform::Openai => None,
    }
}

/// 把思考模式写入请求体，返回实际写入的顶层键（供上游拒绝该参数时精确移除）
pub fn apply(body: &mut Value, platform: AiPlatform, mode: ThinkingMode) -> Vec<&'static str> {
    let Some(key) = param_key(platform) else {
        return Vec::new();
    };
    let Some(object) = body.as_object_mut() else {
        return Vec::new();
    };

    let enabled = mode.is_thinking();
    let value = match key {
        // 智谱 GLM 用嵌套对象表达开关
        "thinking" => json!({ "type": if enabled { "enabled" } else { "disabled" } }),
        _ => Value::Bool(enabled),
    };
    object.insert(key.to_string(), value);
    vec![key]
}

/// 移除此前写入的思考参数（上游不支持该参数时回退）
pub fn remove(body: &mut Value, keys: &[&'static str]) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    for key in keys {
        object.remove(*key);
    }
}

/// 判断上游错误信息是否由「思考模式参数不被支持」引起。
///
/// 仅作关键词判定：错误体必须提到 thinking，且同时出现「不支持 / 未知 / 无效」类措辞，
/// 避免把内容审核、余额不足等错误误判成参数问题而白白重试。
pub fn is_param_rejected(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    if !lower.contains("thinking") {
        return false;
    }
    [
        "unsupported",
        "unknown",
        "unrecognized",
        "invalid",
        "not support",
        "unexpected",
        "不支持",
        "无效",
        "未知",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> Value {
        json!({ "model": "qwen3.8-flash", "stream": true })
    }

    #[test]
    fn param_key_matches_platform_capability() {
        assert_eq!(param_key(AiPlatform::Qwen), Some("enable_thinking"));
        assert_eq!(param_key(AiPlatform::Siliconflow), Some("enable_thinking"));
        assert_eq!(param_key(AiPlatform::Glm), Some("thinking"));
        assert_eq!(param_key(AiPlatform::Deepseek), None);
        assert_eq!(param_key(AiPlatform::Openai), None);
    }

    #[test]
    fn qwen_gets_top_level_boolean() {
        let mut payload = body();
        let keys = apply(&mut payload, AiPlatform::Qwen, ThinkingMode::NonThinking);
        assert_eq!(keys, vec!["enable_thinking"]);
        assert_eq!(payload["enable_thinking"], false);

        let mut payload = body();
        apply(&mut payload, AiPlatform::Qwen, ThinkingMode::Thinking);
        assert_eq!(payload["enable_thinking"], true);
    }

    #[test]
    fn glm_gets_nested_enabled_flag() {
        let mut payload = body();
        apply(&mut payload, AiPlatform::Glm, ThinkingMode::NonThinking);
        assert_eq!(payload["thinking"]["type"], "disabled");
        assert_eq!(payload["thinking"]["enable_thinking"], Value::Null);

        let mut payload = body();
        apply(&mut payload, AiPlatform::Glm, ThinkingMode::Thinking);
        assert_eq!(payload["thinking"]["type"], "enabled");
    }

    #[test]
    fn platforms_without_param_leave_body_untouched() {
        for platform in [AiPlatform::Deepseek, AiPlatform::Openai] {
            let mut payload = body();
            let keys = apply(&mut payload, platform, ThinkingMode::Thinking);
            assert!(keys.is_empty(), "{platform:?} 不应下发思考参数");
            assert_eq!(payload, body());
        }
    }

    #[test]
    fn remove_drops_applied_keys_only() {
        let mut payload = body();
        let keys = apply(&mut payload, AiPlatform::Qwen, ThinkingMode::NonThinking);
        remove(&mut payload, &keys);
        assert_eq!(payload, body());
    }

    #[test]
    fn non_object_body_is_ignored() {
        let mut payload = json!("not an object");
        assert!(apply(&mut payload, AiPlatform::Qwen, ThinkingMode::Thinking).is_empty());
        remove(&mut payload, &["enable_thinking"]);
        assert_eq!(payload, json!("not an object"));
    }

    #[test]
    fn param_rejection_detection_is_narrow() {
        for message in [
            "Unsupported parameter: enable_thinking",
            "InvalidParameter: thinking is not supported",
            "未知参数 thinking",
            "thinking.type 无效",
        ] {
            assert!(is_param_rejected(message), "应判定为参数问题：{message}");
        }

        for message in [
            "Insufficient balance",
            "Rate limit exceeded",
            "invalid api key",
            "content filtered",
            "服务返回 HTTP 500",
        ] {
            assert!(!is_param_rejected(message), "不应判定为参数问题：{message}");
        }
    }
}
