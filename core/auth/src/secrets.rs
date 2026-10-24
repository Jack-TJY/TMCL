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

/// 单个密钥库条目最多保存的字节数。Windows 凭据管理器的上限是 2560 字节
/// （`CRED_MAX_CREDENTIAL_BLOB_SIZE`），皮肤站的 JWT 令牌加上会话 JSON 可能超过它，
/// 所以较长的值拆成多个条目保存。
const CHUNK_BYTES: usize = 2048;
/// 拆分保存时主条目的内容前缀，后接分块数；分块的键名为 `<键名>#<序号>`（从 1 开始）。
const CHUNKED_PREFIX: &str = "tmcl-chunks:";

impl KeyringStore {
    fn entry(key: &str) -> AppResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, key).map_err(keyring_error)
    }

    fn read(key: &str) -> AppResult<Option<Vec<u8>>> {
        match Self::entry(key)?.get_secret() {
            Ok(bytes) => Ok(Some(bytes)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(keyring_error(err)),
        }
    }

    fn write(key: &str, bytes: &[u8]) -> AppResult<()> {
        Self::entry(key)?.set_secret(bytes).map_err(keyring_error)
    }

    fn remove(key: &str) -> AppResult<()> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(keyring_error(err)),
        }
    }

    /// 主条目内容是分块标记时返回分块数。
    fn chunk_count(main: &[u8]) -> Option<usize> {
        std::str::from_utf8(main)
            .ok()?
            .strip_prefix(CHUNKED_PREFIX)?
            .parse()
            .ok()
    }
}

fn chunk_key(key: &str, index: usize) -> String {
    format!("{key}#{index}")
}

fn corrupted(key: &str) -> AppError {
    AppError::new(ErrorCode::AuthSecretStore).with_detail(format!("{key}: 分块缺失或不是 UTF-8"))
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> AppResult<Option<String>> {
        let Some(main) = Self::read(key)? else {
            return Ok(None);
        };
        let bytes = match Self::chunk_count(&main) {
            Some(count) => {
                let mut joined = Vec::new();
                for i in 1..=count {
                    joined.extend(Self::read(&chunk_key(key, i))?.ok_or_else(|| corrupted(key))?);
                }
                joined
            }
            None => main,
        };
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| corrupted(key))
    }

    fn set(&self, key: &str, value: &str) -> AppResult<()> {
        // 先清掉旧值（包括可能更多的旧分块），避免残留。
        self.delete(key)?;
        let bytes = value.as_bytes();
        if bytes.len() <= CHUNK_BYTES {
            return Self::write(key, bytes);
        }
        let chunks: Vec<&[u8]> = bytes.chunks(CHUNK_BYTES).collect();
        for (i, chunk) in chunks.iter().enumerate() {
            if let Err(err) = Self::write(&chunk_key(key, i + 1), chunk) {
                let _ = self.delete(key);
                for j in 1..=i + 1 {
                    let _ = Self::remove(&chunk_key(key, j));
                }
                return Err(err);
            }
        }
        Self::write(key, format!("{CHUNKED_PREFIX}{}", chunks.len()).as_bytes())
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        if let Some(count) = Self::read(key)?.as_deref().and_then(Self::chunk_count) {
            for i in 1..=count {
                Self::remove(&chunk_key(key, i))?;
            }
        }
        Self::remove(key)
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
        // 超过单个条目上限的值（如皮肤站的 JWT 会话）拆成多块保存。
        let long: String = (0..9000)
            .map(|i| char::from(b'a' + (i % 26) as u8))
            .collect();
        store.set(&key, &long).unwrap();
        assert_eq!(store.get(&key).unwrap().as_deref(), Some(long.as_str()));
        // 改回短值后旧分块被清理。
        store.set(&key, "short").unwrap();
        assert_eq!(store.get(&key).unwrap().as_deref(), Some("short"));
        assert!(KeyringStore::read(&chunk_key(&key, 1)).unwrap().is_none());
        store.set(&key, &long).unwrap();
        store.delete(&key).unwrap();
        assert_eq!(store.get(&key).unwrap(), None);
        assert!(KeyringStore::read(&chunk_key(&key, 1)).unwrap().is_none());
    }
}
