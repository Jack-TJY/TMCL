//! 令牌存储：访问令牌、刷新令牌只存系统密钥库（PLAN.md 第 3.6 节），不进数据库和日志。
//!
//! 键名 `tmcl:account:<account-id>`，值由各账号类型自行序列化（通常是 JSON）。

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use tmcl_common::{AppError, AppResult, ErrorCode};

/// 密钥库中的服务名。
const SERVICE: &str = "TMCL";

/// 账号令牌在密钥库中的键名。
pub fn secret_key(account_id: &str) -> String {
    format!("tmcl:account:{account_id}")
}

/// 令牌存储。生产环境用 [`KeyringStore`]，测试用 [`MemorySecretStore`]。
pub trait SecretStore: Send + Sync + fmt::Debug {
    /// 读取；不存在时返回 `None`。
    fn get(&self, key: &str) -> AppResult<Option<String>>;
    /// 写入（覆盖）。
    fn set(&self, key: &str, value: &str) -> AppResult<()>;
    /// 删除；不存在也视为成功。
    fn delete(&self, key: &str) -> AppResult<()>;
}

/// 系统密钥库：Windows 凭据管理器、macOS 钥匙串、Linux Secret Service。
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringStore;

/// 密钥库错误统一映射为 `AUTH_SECRET_STORE`；详情只含错误类型，不含值。
fn keyring_error(err: keyring::Error) -> AppError {
    AppError::new(ErrorCode::AuthSecretStore).with_detail(err.to_string())
}

impl KeyringStore {
    fn entry(key: &str) -> AppResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, key).map_err(keyring_error)
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        match Self::entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(keyring_error(err)),
        }
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        Self::entry(key)?.set_password(value).map_err(keyring_error)
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keyring_error(err)),
        }
    }
}

/// 内存中的令牌存储，供测试使用。
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    fn values(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.values.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// 当前保存的键，按字母排序。
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.values().keys().cloned().collect();
        keys.sort();
        keys
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self.values().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        self.values().insert(key.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        self.values().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemorySecretStore::default();
        let key = secret_key("a1");
        assert_eq!(key, "tmcl:account:a1");
        assert_eq!(store.get(&key).unwrap(), None);
        store.set(&key, "test-token").unwrap();
        assert_eq!(store.get(&key).unwrap().as_deref(), Some("test-token"));
        store.delete(&key).unwrap();
        store.delete(&key).unwrap();
        assert!(store.keys().is_empty());
    }

    /// 真实密钥库需要桌面会话（Linux CI 没有 Secret Service），默认不跑：
    /// `cargo test -p tmcl-auth -- --ignored keyring_round_trip`。
    #[test]
    #[ignore]
    fn keyring_round_trip() {
        let store = KeyringStore;
        let key = secret_key("test-00000000-0000-0000-0000-000000000000");
        store.set(&key, "test-token").unwrap();
        assert_eq!(store.get(&key).unwrap().as_deref(), Some("test-token"));
        store.delete(&key).unwrap();
        assert_eq!(store.get(&key).unwrap(), None);
    }
}
