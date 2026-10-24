//! 账号列表（PLAN.md M1-08）：SQLite 只存账号 ID、类型、名称、UUID 与皮肤站，
//! 令牌经 [`SecretStore`] 存系统密钥库。默认账号（启动时使用）记在 `kv` 表。

use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, Db, ErrorCode};
use ts_rs::TS;
use uuid::Uuid;

use crate::offline::OfflineAccount;
use crate::secrets::{SecretStore, secret_key};

/// `kv` 表中保存默认账号 ID 的键。
const DEFAULT_KEY: &str = "auth.default_account";

/// 账号类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    /// 微软正版。
    Microsoft,
    /// 离线，仅限本地测试。
    Offline,
    /// 外置登录（authlib-injector）。
    Authlib,
}

impl AccountKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Microsoft => "microsoft",
            Self::Offline => "offline",
            Self::Authlib => "authlib",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "microsoft" => Some(Self::Microsoft),
            "offline" => Some(Self::Offline),
            "authlib" => Some(Self::Authlib),
            _ => None,
        }
    }
}

/// 外置登录的皮肤站。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AuthlibServer {
    /// Yggdrasil API 地址（不带末尾 `/`），例如 `https://littleskin.cn/api/yggdrasil`。
    pub api_url: String,
    /// 皮肤站名称，取自元数据的 `meta.serverName`。
    pub name: String,
}

/// 一个账号。令牌不在其中。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Account {
    /// 启动器内部 ID（UUID v4）。
    pub id: String,
    pub kind: AccountKind,
    /// 玩家名。
    pub name: String,
    /// 玩家 UUID（带连字符）。
    pub uuid: String,
    /// 仅外置登录账号有值。
    pub server: Option<AuthlibServer>,
}

/// 账号列表与默认账号。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct AccountList {
    /// 按添加顺序排列。
    pub accounts: Vec<Account>,
    /// 启动游戏时使用的账号；列表为空时为 `null`。
    pub default_id: Option<String>,
}

/// 新增或更新账号时的内容；`(kind, uuid, 皮肤站)` 相同视为同一个账号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAccount {
    pub kind: AccountKind,
    pub name: String,
    pub uuid: Uuid,
    pub server: Option<AuthlibServer>,
}

fn not_found(id: &str) -> AppError {
    AppError::new(ErrorCode::AuthAccountNotFound).with_param("id", id)
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<Option<Account>> {
    let kind: String = row.get("kind")?;
    let server_url: Option<String> = row.get("server_url")?;
    let server_name: Option<String> = row.get("server_name")?;
    // 未知类型（来自更新版本的数据库）跳过而不报错。
    Ok(AccountKind::parse(&kind).map(|kind| Account {
        id: row.get("id").unwrap_or_default(),
        kind,
        name: row.get("name").unwrap_or_default(),
        uuid: row.get("uuid").unwrap_or_default(),
        server: server_url.map(|api_url| AuthlibServer {
            api_url,
            name: server_name.unwrap_or_default(),
        }),
    }))
}

const COLUMNS: &str = "id, kind, name, uuid, server_url, server_name";

/// 按 ID 读取账号。
pub fn get(db: &Db, id: &str) -> AppResult<Option<Account>> {
    let sql = format!("SELECT {COLUMNS} FROM accounts WHERE id = ?1");
    Ok(db
        .conn()
        .query_row(&sql, [id], from_row)
        .optional()?
        .flatten())
}

/// 读取账号列表；默认账号缺失或已被删除时改用第一个账号。
pub fn list(db: &Db) -> AppResult<AccountList> {
    let sql = format!("SELECT {COLUMNS} FROM accounts ORDER BY created_at, rowid");
    let mut stmt = db.conn().prepare(&sql)?;
    let accounts: Vec<Account> = stmt
        .query_map([], from_row)?
        .filter_map(|row| row.transpose())
        .collect::<rusqlite::Result<_>>()?;
    let saved = db.kv_get(DEFAULT_KEY)?;
    let default_id = saved
        .filter(|id| accounts.iter().any(|a| &a.id == id))
        .or_else(|| accounts.first().map(|a| a.id.clone()));
    Ok(AccountList {
        accounts,
        default_id,
    })
}

/// 启动游戏时使用的账号。
pub fn default_account(db: &Db) -> AppResult<Option<Account>> {
    let list = list(db)?;
    Ok(list
        .default_id
        .and_then(|id| list.accounts.into_iter().find(|a| a.id == id)))
}

/// 设为默认账号；账号不存在时返回 `AUTH_ACCOUNT_NOT_FOUND`。
pub fn set_default(db: &Db, id: &str) -> AppResult<()> {
    if get(db, id)?.is_none() {
        return Err(not_found(id));
    }
    db.kv_set(DEFAULT_KEY, id)
}

/// 新增账号或更新已有账号（玩家名、皮肤站名称），并设为默认。
/// `secret` 为 `Some` 时写入密钥库；写入失败则不保存账号。
pub fn upsert(
    db: &Db,
    secrets: &dyn SecretStore,
    account: NewAccount,
    secret: Option<&str>,
) -> AppResult<Account> {
    let uuid = account.uuid.hyphenated().to_string();
    let server_url = account.server.as_ref().map(|s| s.api_url.as_str());
    let existing: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM accounts
             WHERE kind = ?1 AND uuid = ?2 AND COALESCE(server_url, '') = COALESCE(?3, '')",
            params![account.kind.as_str(), uuid, server_url],
            |row| row.get(0),
        )
        .optional()?;
    let id = existing
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if let Some(secret) = secret {
        secrets.set(&secret_key(&id), secret)?;
    }
    let server_name = account.server.as_ref().map(|s| s.name.as_str());
    let result = if existing.is_some() {
        db.conn().execute(
            "UPDATE accounts SET name = ?2, server_name = ?3 WHERE id = ?1",
            params![id, account.name, server_name],
        )
    } else {
        db.conn().execute(
            "INSERT INTO accounts (id, kind, name, uuid, server_url, server_name, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, unixepoch())",
            params![
                id,
                account.kind.as_str(),
                account.name,
                uuid,
                server_url,
                server_name
            ],
        )
    };
    if let Err(err) = result {
        // 新账号写库失败时清理刚写入的令牌，避免密钥库残留。
        if existing.is_none() && secret.is_some() {
            let _ = secrets.delete(&secret_key(&id));
        }
        return Err(err.into());
    }
    db.kv_set(DEFAULT_KEY, &id)?;
    Ok(Account {
        id,
        kind: account.kind,
        name: account.name,
        uuid,
        server: account.server,
    })
}

/// 添加离线账号（UUID 由玩家名决定，同名只保留一个）并设为默认。
pub fn add_offline(db: &Db, secrets: &dyn SecretStore, name: &str) -> AppResult<Account> {
    let offline = OfflineAccount::new(name)?;
    upsert(
        db,
        secrets,
        NewAccount {
            kind: AccountKind::Offline,
            name: offline.name,
            uuid: offline.uuid,
            server: None,
        },
        None,
    )
}

/// 删除账号及其在密钥库中的令牌；删除默认账号时 [`list`] 会改用第一个账号。
pub fn remove(db: &Db, secrets: &dyn SecretStore, id: &str) -> AppResult<()> {
    let account = get(db, id)?.ok_or_else(|| not_found(id))?;
    if account.kind != AccountKind::Offline {
        secrets.delete(&secret_key(id))?;
    }
    db.conn()
        .execute("DELETE FROM accounts WHERE id = ?1", [id])?;
    if db.kv_get(DEFAULT_KEY)?.as_deref() == Some(id) {
        db.kv_delete(DEFAULT_KEY)?;
    }
    Ok(())
}

/// 读取账号的令牌；没有时返回 `None`。
pub fn secret(secrets: &dyn SecretStore, id: &str) -> AppResult<Option<String>> {
    secrets.get(&secret_key(id))
}

/// 更新账号的令牌（如刷新后）。
pub fn set_secret(secrets: &dyn SecretStore, id: &str, value: &str) -> AppResult<()> {
    secrets.set(&secret_key(id), value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;

    fn setup() -> (Db, MemorySecretStore) {
        (Db::open_in_memory().unwrap(), MemorySecretStore::default())
    }

    fn online(kind: AccountKind, name: &str, server: Option<&str>) -> NewAccount {
        NewAccount {
            kind,
            name: name.into(),
            uuid: crate::offline::offline_uuid(name),
            server: server.map(|url| AuthlibServer {
                api_url: url.into(),
                name: "Test Skin".into(),
            }),
        }
    }

    #[test]
    fn empty_list_has_no_default() {
        let (db, _) = setup();
        assert_eq!(
            list(&db).unwrap(),
            AccountList {
                accounts: vec![],
                default_id: None
            }
        );
        assert_eq!(default_account(&db).unwrap(), None);
    }

    #[test]
    fn offline_accounts_use_vanilla_uuid_and_dedupe_by_name() {
        let (db, secrets) = setup();
        let dev = add_offline(&db, &secrets, "Dev").unwrap();
        assert_eq!(dev.uuid, "380df991-f603-344c-a090-369bad2a924a");
        assert_eq!(dev.kind, AccountKind::Offline);
        let notch = add_offline(&db, &secrets, "Notch").unwrap();
        assert_eq!(list(&db).unwrap().default_id.as_deref(), Some(&*notch.id));
        let again = add_offline(&db, &secrets, "Dev").unwrap();
        assert_eq!(again.id, dev.id);
        let all = list(&db).unwrap();
        assert_eq!(all.accounts.len(), 2);
        assert_eq!(all.default_id.as_deref(), Some(&*dev.id));
        assert!(secrets.keys().is_empty());
    }

    #[test]
    fn invalid_offline_name_is_rejected() {
        let (db, secrets) = setup();
        let err = add_offline(&db, &secrets, "a b").unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthOfflineInvalidName);
        assert!(list(&db).unwrap().accounts.is_empty());
    }

    #[test]
    fn set_default_checks_existence() {
        let (db, secrets) = setup();
        let a = add_offline(&db, &secrets, "Alpha").unwrap();
        add_offline(&db, &secrets, "Bravo").unwrap();
        set_default(&db, &a.id).unwrap();
        assert_eq!(default_account(&db).unwrap().unwrap().name, "Alpha");
        let err = set_default(&db, "missing").unwrap_err();
        assert_eq!(err.code, ErrorCode::AuthAccountNotFound);
        assert_eq!(err.params["id"], "missing");
    }

    #[test]
    fn online_accounts_keep_tokens_in_secret_store() {
        let (db, secrets) = setup();
        let ms = upsert(
            &db,
            &secrets,
            online(AccountKind::Microsoft, "Steve", None),
            Some("{\"refresh_token\":\"test\"}"),
        )
        .unwrap();
        assert_eq!(secrets.keys(), vec![secret_key(&ms.id)]);
        assert_eq!(
            secret(&secrets, &ms.id).unwrap().as_deref(),
            Some("{\"refresh_token\":\"test\"}")
        );
        // 同一个账号再次登录：更新名称与令牌，不新增。
        let renamed = upsert(
            &db,
            &secrets,
            NewAccount {
                name: "Steve2".into(),
                ..online(AccountKind::Microsoft, "Steve", None)
            },
            Some("new"),
        )
        .unwrap();
        assert_eq!(renamed.id, ms.id);
        assert_eq!(list(&db).unwrap().accounts[0].name, "Steve2");
        assert_eq!(secret(&secrets, &ms.id).unwrap().as_deref(), Some("new"));
    }

    #[test]
    fn same_profile_on_different_skin_sites_are_separate() {
        let (db, secrets) = setup();
        let a = upsert(
            &db,
            &secrets,
            online(
                AccountKind::Authlib,
                "Steve",
                Some("https://a.test/api/yggdrasil"),
            ),
            Some("t1"),
        )
        .unwrap();
        let b = upsert(
            &db,
            &secrets,
            online(
                AccountKind::Authlib,
                "Steve",
                Some("https://b.test/api/yggdrasil"),
            ),
            Some("t2"),
        )
        .unwrap();
        assert_ne!(a.id, b.id);
        let all = list(&db).unwrap();
        assert_eq!(all.accounts.len(), 2);
        assert_eq!(
            all.accounts[0].server.as_ref().unwrap().api_url,
            "https://a.test/api/yggdrasil"
        );
    }

    #[test]
    fn remove_deletes_tokens_and_moves_default() {
        let (db, secrets) = setup();
        let offline = add_offline(&db, &secrets, "Alpha").unwrap();
        let ms = upsert(
            &db,
            &secrets,
            online(AccountKind::Microsoft, "Steve", None),
            Some("token"),
        )
        .unwrap();
        remove(&db, &secrets, &ms.id).unwrap();
        assert!(secrets.keys().is_empty());
        let all = list(&db).unwrap();
        assert_eq!(all.accounts, vec![offline.clone()]);
        assert_eq!(all.default_id, Some(offline.id.clone()));
        assert_eq!(
            remove(&db, &secrets, &ms.id).unwrap_err().code,
            ErrorCode::AuthAccountNotFound
        );
        remove(&db, &secrets, &offline.id).unwrap();
        assert_eq!(list(&db).unwrap().default_id, None);
    }
}
