//! 密钥安全：API Key 与 ASR 服务商凭证存入系统钥匙链。
//!
//! * 生产环境使用 [`KeyringStore`]（macOS Keychain / Windows Credential Manager）；
//! * 单元测试与无钥匙链环境使用 [`MemorySecretStore`] 降级。

use std::collections::HashMap;
use std::sync::Mutex;

/// 钥匙链服务名（应用唯一标识）
pub const SERVICE_NAME: &str = "com.easyinterview.app";

/// 密钥存取抽象
pub trait SecretStore: Send + Sync {
    fn set(&self, key: &str, value: &str) -> Result<(), String>;
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    fn delete(&self, key: &str) -> Result<bool, String>;

    /// 返回掩码后的密钥（不泄露完整内容）
    fn masked(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.get(key)?.map(|value| mask_secret(&value)))
    }
}

/// AI 平台 API Key 的存储键名
pub fn api_key_ref(platform: &str) -> String {
    format!("ai-platform:{platform}")
}

/// ASR 服务商凭证的存储键名前缀
pub fn asr_credential_ref(provider: &str, field: &str) -> String {
    format!("asr:{provider}:{field}")
}

/// 密钥掩码：保留前 3 位与后 4 位，中间以 **** 代替
pub fn mask_secret(secret: &str) -> String {
    let chars: Vec<char> = secret.chars().collect();
    if chars.len() <= 8 {
        return "*".repeat(chars.len().max(4));
    }
    let head: String = chars.iter().take(3).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}****{tail}")
}

/// 系统钥匙链实现
pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, key: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(&self.service, key).map_err(|e| e.to_string())
    }
}

impl SecretStore for KeyringStore {
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.entry(key)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.to_string()),
        }
    }

    fn delete(&self, key: &str) -> Result<bool, String> {
        match self.entry(key)?.delete_credential() {
            Ok(()) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(err) => Err(err.to_string()),
        }
    }
}

/// 内存实现：单元测试与无钥匙链环境降级
#[derive(Default)]
pub struct MemorySecretStore {
    items: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let mut items = self.items.lock().map_err(|e| e.to_string())?;
        items.insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<String>, String> {
        let items = self.items.lock().map_err(|e| e.to_string())?;
        Ok(items.get(key).cloned())
    }

    fn delete(&self, key: &str) -> Result<bool, String> {
        let mut items = self.items.lock().map_err(|e| e.to_string())?;
        Ok(items.remove(key).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_keeps_head_and_tail_for_long_secret() {
        assert_eq!(mask_secret("sk-abcdefghijklmn"), "sk-****klmn");
    }

    #[test]
    fn mask_hides_short_secret_completely() {
        assert_eq!(mask_secret("1234"), "****");
        assert_eq!(mask_secret(""), "****");
        assert_eq!(mask_secret("abcdefgh"), "********");
    }

    #[test]
    fn memory_store_round_trip() {
        let store = MemorySecretStore::new();
        assert_eq!(store.get("k").expect("get"), None);

        store.set("k", "value").expect("set");
        assert_eq!(store.get("k").expect("get"), Some("value".to_string()));

        store.set("k", "new-value").expect("overwrite");
        assert_eq!(store.get("k").expect("get"), Some("new-value".to_string()));

        assert!(store.delete("k").expect("delete"));
        assert!(!store.delete("k").expect("delete twice"));
        assert_eq!(store.get("k").expect("get"), None);
    }

    #[test]
    fn memory_store_masked_never_returns_full_secret() {
        let store = MemorySecretStore::new();
        store.set("k", "sk-1234567890abcdef").expect("set");

        let masked = store.masked("k").expect("masked").expect("exists");
        assert!(masked.contains("****"));
        assert!(!masked.contains("1234567890"));
    }

    #[test]
    fn key_refs_are_namespaced() {
        assert_eq!(api_key_ref("deepseek"), "ai-platform:deepseek");
        assert_eq!(asr_credential_ref("xfyun", "app_id"), "asr:xfyun:app_id");
    }
}
