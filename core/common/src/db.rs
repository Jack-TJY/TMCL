//! SQLite 数据库 `tmcl.db` 与迁移（PLAN.md 第 3.5 节）。
//!
//! 迁移文件放在 `core/common/migrations/`，命名 `NNNN_<模块>_<描述>.sql`，
//! 并在 [`MIGRATIONS`] 末尾追加一行登记。已发布的迁移不得修改。
//! 敏感数据（令牌）不进数据库，只存系统密钥库（第 3.6 节）。

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{AppError, AppResult, ErrorCode};

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(ErrorCode::CommonDb).with_detail(err.to_string())
    }
}

/// 一条迁移。
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// 版本号，等于文件名中的 `NNNN`，严格递增。
    pub version: i64,
    /// 文件名（不含扩展名）。
    pub name: &'static str,
    /// 迁移 SQL。
    pub sql: &'static str,
}

/// 所有迁移，按版本号递增排列，一行一条。
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "0001_common_init",
    sql: include_str!("../migrations/0001_common_init.sql"),
}];

/// 已打开并完成迁移的数据库连接。
#[derive(Debug)]
pub struct Db {
    conn: Connection,
}

impl Db {
    /// 打开（不存在则创建）数据库文件并执行未应用的迁移。
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| AppError::from(e).with_param("path", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    /// 打开内存数据库，供测试使用。
    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> AppResult<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// 底层连接。
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// 底层连接（可变，用于事务）。
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// 当前已应用的最高迁移版本，未应用任何迁移时为 0。
    pub fn schema_version(&self) -> AppResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    /// 读取 `kv` 表中的值。
    pub fn kv_get(&self, key: &str) -> AppResult<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM kv WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// 写入（覆盖）`kv` 表中的值。
    pub fn kv_set(&self, key: &str, value: &str) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO kv (key, value, updated_at) VALUES (?1, ?2, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value],
        )?;
        Ok(())
    }

    /// 删除 `kv` 表中的值，返回是否存在。
    pub fn kv_delete(&self, key: &str) -> AppResult<bool> {
        Ok(self.conn.execute("DELETE FROM kv WHERE key = ?1", [key])? > 0)
    }
}

/// 在事务中依次执行尚未应用的迁移；重复执行是安全的。
pub fn migrate(conn: &mut Connection, migrations: &[Migration]) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
             version    INTEGER PRIMARY KEY,
             name       TEXT    NOT NULL,
             applied_at INTEGER NOT NULL
         );",
    )?;
    let mut previous = 0;
    for migration in migrations {
        if migration.version <= previous {
            return Err(AppError::new(ErrorCode::CommonInternal)
                .with_detail(format!("迁移版本未严格递增：{}", migration.name)));
        }
        previous = migration.version;

        let tx = conn.transaction()?;
        let applied: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;
        if applied {
            continue;
        }
        tx.execute_batch(migration.sql)
            .map_err(|e| AppError::from(e).with_param("migration", migration.name))?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, unixepoch())",
            params![migration.version, migration.name],
        )?;
        tx.commit()?;
        tracing::info!(migration = migration.name, "已应用数据库迁移");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_applies_all_migrations() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(
            db.schema_version().unwrap(),
            MIGRATIONS.last().unwrap().version
        );
    }

    #[test]
    fn migrate_is_repeatable() {
        let mut db = Db::open_in_memory().unwrap();
        migrate(db.conn_mut(), MIGRATIONS).unwrap();
        migrate(db.conn_mut(), MIGRATIONS).unwrap();
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64);
    }

    #[test]
    fn reopening_file_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("tmcl.db");
        {
            let db = Db::open(&path).unwrap();
            db.kv_set("k", "v1").unwrap();
            db.kv_set("k", "v2").unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.kv_get("k").unwrap().as_deref(), Some("v2"));
        assert!(db.kv_delete("k").unwrap());
        assert_eq!(db.kv_get("k").unwrap(), None);
    }

    #[test]
    fn new_migration_applies_on_top() {
        const EXTRA: &[Migration] = &[
            MIGRATIONS[0],
            Migration {
                version: 2,
                name: "0002_test_extra",
                sql: "CREATE TABLE extra (id INTEGER PRIMARY KEY);",
            },
        ];
        let mut db = Db::open_in_memory().unwrap();
        migrate(db.conn_mut(), EXTRA).unwrap();
        migrate(db.conn_mut(), EXTRA).unwrap();
        assert_eq!(db.schema_version().unwrap(), 2);
        db.conn()
            .execute("INSERT INTO extra (id) VALUES (1)", [])
            .unwrap();
    }

    #[test]
    fn failed_migration_rolls_back() {
        const BROKEN: &[Migration] = &[
            MIGRATIONS[0],
            Migration {
                version: 2,
                name: "0002_test_broken",
                sql: "CREATE TABLE half (id INTEGER); THIS IS NOT SQL;",
            },
        ];
        let mut db = Db::open_in_memory().unwrap();
        let err = migrate(db.conn_mut(), BROKEN).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonDb);
        assert_eq!(db.schema_version().unwrap(), 1);
        let half: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'half'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(half, 0);
    }

    #[test]
    fn non_increasing_versions_rejected() {
        const BAD: &[Migration] = &[MIGRATIONS[0], MIGRATIONS[0]];
        let mut db = Db::open_in_memory().unwrap();
        let err = migrate(db.conn_mut(), BAD).unwrap_err();
        assert_eq!(err.code, ErrorCode::CommonInternal);
    }
}
