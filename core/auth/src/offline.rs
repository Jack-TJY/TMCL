//! 离线账号：用户名校验与原版服务端一致的离线 UUID。
//!
//! 离线账号仅限本地测试，界面需明确标注（AGENTS.md §15）。

use md5::{Digest, Md5};
use tmcl_common::{AppError, AppResult, ErrorCode};
use uuid::Uuid;

/// 离线账号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineAccount {
    /// 用户名。
    pub name: String,
    /// 离线 UUID，与 `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)` 一致。
    pub uuid: Uuid,
}

impl OfflineAccount {
    /// 校验用户名并计算 UUID；用户名不合法时返回 `AUTH_OFFLINE_INVALID_NAME`。
    pub fn new(name: &str) -> AppResult<Self> {
        if !is_valid_name(name) {
            return Err(AppError::new(ErrorCode::AuthOfflineInvalidName).with_param("name", name));
        }
        Ok(Self {
            name: name.to_owned(),
            uuid: offline_uuid(name),
        })
    }
}

/// 用户名是否符合 `^[A-Za-z0-9_]{3,16}$`。
pub fn is_valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// 原版服务端的离线 UUID：`"OfflinePlayer:" + name` 的 MD5，按 UUID v3 设置版本与变体位。
pub fn offline_uuid(name: &str) -> Uuid {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);
    uuid::Builder::from_md5_bytes(bytes).into_uuid()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_matches_vanilla_server() {
        // 样本由 Java 的 UUID.nameUUIDFromBytes 算法得出。
        assert_eq!(
            offline_uuid("Notch").to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert_eq!(
            offline_uuid("Dev").to_string(),
            "380df991-f603-344c-a090-369bad2a924a"
        );
        assert_eq!(
            offline_uuid("jeb_").to_string(),
            "a762f560-4fce-3236-812a-b80efff0b62b"
        );
    }

    #[test]
    fn name_validation() {
        for ok in ["abc", "Dev", "jeb_", "A_1234567890_bcd"] {
            assert!(is_valid_name(ok), "{ok}");
        }
        for bad in [
            "",
            "ab",
            "A_1234567890_bcde",
            "has space",
            "中文名字",
            "a-b",
        ] {
            assert!(!is_valid_name(bad), "{bad}");
        }
        let err = OfflineAccount::new("ab").unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthOfflineInvalidName);
        assert_eq!(err.params["name"], "ab");
        assert_eq!(OfflineAccount::new("Notch").unwrap().name, "Notch");
    }
}
