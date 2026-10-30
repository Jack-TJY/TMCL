//! 实例的增删改查、索引对账与回收站。
//!
//! 磁盘上的 `instances/<id>/instance.json` 是唯一事实来源；SQLite 的
//! `instance_index` 表只是列表缓存，由 [`InstanceStore::reconcile`] 按
//! `instance.json` 的修改时间与大小刷新，启动时调用一次即可让列表与磁盘一致。
//! `instance.json` 损坏的实例在列表中带错误码，不会让整个列表失败。
//!
//! 所有方法都是同步的文件与数据库操作，调用方（`src-tauri`）应放在阻塞线程中执行。

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tmcl_common::{AppError, AppResult, DataPaths, Db, ErrorCode, write_atomic};
use ts_rs::TS;

use crate::model::{InstanceConfig, LoaderKind, LoaderSpec, normalize_name, now_rfc3339};

/// 回收站中的实例保留多久后被 [`InstanceStore::purge_trash`] 清理。
pub const TRASH_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// 实例列表中的一项（来自索引）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct InstanceSummary {
    /// 实例 ID（目录名）。
    pub id: String,
    /// 显示名称；损坏的实例为其 ID。
    pub name: String,
    /// 游戏版本 ID；损坏的实例为空字符串。
    pub game_version: String,
    /// 模组加载器，`null` 表示原版。
    pub loader: Option<LoaderSpec>,
    /// 分组名。
    pub group: Option<String>,
    /// 创建时间（RFC 3339）；损坏的实例为空字符串。
    pub created_at: String,
    /// 最近一次启动时间（RFC 3339）。
    pub last_played_at: Option<String>,
    /// `null` 表示正常；否则为损坏原因，如 `INSTANCE_CORRUPTED`。
    pub error: Option<ErrorCode>,
}

/// [`InstanceStore::reconcile`] 的结果统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// 新出现在磁盘上的实例数。
    pub added: usize,
    /// `instance.json` 有变化而重新读取的实例数。
    pub updated: usize,
    /// 目录已不存在而从索引移除的实例数。
    pub removed: usize,
    /// 对账后处于损坏状态的实例数（含未变化的）。
    pub corrupted: usize,
}

/// `instance.json` 的指纹：修改时间（unix 毫秒）与大小；文件不存在时为 `(-1, -1)`。
type Fingerprint = (i64, i64);

/// 实例存储，借用数据目录与数据库。
#[derive(Debug, Clone, Copy)]
pub struct InstanceStore<'a> {
    paths: &'a DataPaths,
    db: &'a Db,
}

impl<'a> InstanceStore<'a> {
    /// 以给定数据目录与数据库创建存储。
    pub fn new(paths: &'a DataPaths, db: &'a Db) -> Self {
        Self { paths, db }
    }

    /// 数据目录。
    pub fn paths(&self) -> &'a DataPaths {
        self.paths
    }

    /// 新建实例：生成 ID，创建 `game/` 目录并写入 `instance.json`。
    pub fn create(
        &self,
        name: &str,
        game_version: &str,
        loader: Option<LoaderSpec>,
    ) -> AppResult<InstanceConfig> {
        let config = InstanceConfig::new(name, game_version, loader)?;
        fs::create_dir_all(self.paths.instance_game_dir(&config.id))?;
        self.write(&config)?;
        tracing::info!(id = %config.id, version = %config.game_version, "已创建实例");
        Ok(config)
    }

    /// 读取索引中的所有实例：最近启动的在前，从未启动的按创建时间倒序。
    pub fn list(&self) -> AppResult<Vec<InstanceSummary>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, game_version, loader_kind, loader_version, group_name,
                    created_at, last_played_at, error
             FROM instance_index
             ORDER BY last_played_at IS NULL, last_played_at DESC, created_at DESC, id",
        )?;
        let rows = stmt.query_map([], |row| {
            let loader_kind: Option<String> = row.get(3)?;
            let loader_version: Option<String> = row.get(4)?;
            let error: Option<String> = row.get(8)?;
            Ok(InstanceSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                game_version: row.get(2)?,
                loader: loader_kind
                    .as_deref()
                    .and_then(LoaderKind::parse)
                    .map(|kind| LoaderSpec {
                        kind,
                        version: loader_version.unwrap_or_default(),
                    }),
                group: row.get(5)?,
                created_at: row.get(6)?,
                last_played_at: row.get(7)?,
                error: error.map(|code| parse_error_code(&code)),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// 读取实例配置。目录不存在返回 `INSTANCE_NOT_FOUND`，`instance.json`
    /// 缺失或损坏返回 `INSTANCE_CORRUPTED` / `INSTANCE_SCHEMA_UNSUPPORTED`。
    pub fn get(&self, id: &str) -> AppResult<InstanceConfig> {
        self.require_dir(id)?;
        load(self.paths, id)
            .map(|(config, _)| config)
            .map_err(|e| e.with_param("id", id))
    }

    /// 保存修改后的配置（实例必须已存在）。名称会被校验并去掉首尾空白。
    pub fn save(&self, config: &InstanceConfig) -> AppResult<InstanceConfig> {
        self.require_dir(&config.id)?;
        let mut config = config.clone();
        config.name = normalize_name(&config.name)?;
        self.write(&config)?;
        Ok(config)
    }

    /// 重命名实例（只改显示名称，ID 与目录不变）。
    pub fn rename(&self, id: &str, name: &str) -> AppResult<InstanceConfig> {
        let mut config = self.get(id)?;
        config.name = normalize_name(name)?;
        self.write(&config)?;
        Ok(config)
    }

    /// 把最近启动时间设为当前时间。
    pub fn mark_played(&self, id: &str) -> AppResult<InstanceConfig> {
        let mut config = self.get(id)?;
        config.last_played_at = Some(now_rfc3339());
        self.write(&config)?;
        Ok(config)
    }

    /// 删除实例：把整个实例目录移到 `trash/<unix 秒>-<id>/`，并从索引移除。
    ///
    /// 损坏的实例同样可以删除。回收站内容在 [`TRASH_RETENTION`] 后由
    /// [`purge_trash`](Self::purge_trash) 清理。
    pub fn delete(&self, id: &str) -> AppResult<()> {
        let dir = self.require_dir(id)?;
        let trash = self.paths.trash_dir();
        fs::create_dir_all(&trash)?;
        let target = trash.join(format!("{}-{id}", unix_secs(SystemTime::now())));
        fs::rename(&dir, &target).map_err(|e| io_error(e, &dir))?;
        self.db
            .conn()
            .execute("DELETE FROM instance_index WHERE id = ?1", [id])?;
        tracing::info!(id, "已将实例移到回收站");
        Ok(())
    }

    /// 清理回收站中超过 [`TRASH_RETENTION`] 的实例，返回清理的数量。
    pub fn purge_trash(&self) -> AppResult<usize> {
        let cutoff = SystemTime::now()
            .checked_sub(TRASH_RETENTION)
            .unwrap_or(UNIX_EPOCH);
        self.purge_trash_before(unix_secs(cutoff))
    }

    /// 清理回收站中删除时间早于 `cutoff`（unix 秒）的实例。
    fn purge_trash_before(&self, cutoff: u64) -> AppResult<usize> {
        let trash = self.paths.trash_dir();
        let entries = match fs::read_dir(&trash) {
            Ok(entries) => entries,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(io_error(e, &trash)),
        };
        let mut removed = 0;
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let deleted_at = name
                .to_str()
                .and_then(|n| n.split_once('-'))
                .and_then(|(secs, _)| secs.parse::<u64>().ok());
            if deleted_at.is_some_and(|secs| secs < cutoff) {
                fs::remove_dir_all(entry.path()).map_err(|e| io_error(e, &entry.path()))?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// 让索引与 `instances/` 目录一致。
    ///
    /// - 新目录或 `instance.json` 修改时间 / 大小有变化的实例重新读取；
    /// - 读取失败的实例以错误码记入索引（列表中显示为“损坏”）；
    /// - `instance.json` 中的 `id` 与目录名不一致时（例如用户手动复制了目录）
    ///   以目录名为准并写回；
    /// - 目录已不存在的实例从索引移除。
    pub fn reconcile(&self) -> AppResult<ReconcileReport> {
        let instances = self.paths.instances_dir();
        fs::create_dir_all(&instances)?;

        let mut indexed: HashMap<String, (Fingerprint, bool)> = HashMap::new();
        {
            let mut stmt = self.db.conn().prepare(
                "SELECT id, file_mtime_ms, file_size, error IS NOT NULL FROM instance_index",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ((row.get(1)?, row.get(2)?), row.get(3)?),
                ))
            })?;
            for row in rows {
                let (id, entry) = row?;
                indexed.insert(id, entry);
            }
        }

        let mut report = ReconcileReport::default();
        let tx = self.db.conn().unchecked_transaction()?;
        for entry in fs::read_dir(&instances).map_err(|e| io_error(e, &instances))? {
            let entry = entry?;
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !entry.file_type()?.is_dir() || !is_valid_id(&id) {
                continue;
            }
            let fingerprint = fingerprint(&self.paths.instance_file(&id));
            match indexed.remove(&id) {
                Some((old, corrupted)) if old == fingerprint => {
                    report.corrupted += usize::from(corrupted);
                    continue;
                }
                Some(_) => report.updated += 1,
                None => report.added += 1,
            }
            match load(self.paths, &id) {
                Ok((config, false)) => index_ok(self.db, &config, fingerprint)?,
                Ok((config, true)) => self.write(&config)?,
                Err(err) => {
                    tracing::warn!(id, code = %err.code, "实例配置损坏");
                    report.corrupted += 1;
                    index_corrupted(self.db, &id, err.code, fingerprint)?;
                }
            }
        }
        for id in indexed.keys() {
            tx.execute("DELETE FROM instance_index WHERE id = ?1", [id])?;
            report.removed += 1;
        }
        tx.commit()?;
        if report != ReconcileReport::default() {
            tracing::info!(?report, "实例索引对账完成");
        }
        Ok(report)
    }

    /// 写 `instance.json` 并刷新索引。
    pub(crate) fn write(&self, config: &InstanceConfig) -> AppResult<()> {
        let file = self.paths.instance_file(&config.id);
        write_atomic(&file, config.to_json()?.as_bytes())?;
        index_ok(self.db, config, fingerprint(&file))
    }

    /// 校验 ID 并确认实例目录存在，返回目录路径。
    fn require_dir(&self, id: &str) -> AppResult<std::path::PathBuf> {
        let dir = self.paths.instance_dir(id);
        if is_valid_id(id) && dir.is_dir() {
            Ok(dir)
        } else {
            Err(AppError::new(ErrorCode::InstanceNotFound).with_param("id", id))
        }
    }
}

/// 实例 ID 只允许 ASCII 字母、数字、`-`、`_`，防止路径穿越；
/// 以 `.` 开头等其他目录名在对账时被忽略。
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// 读取并解析 `instances/<id>/instance.json`。
///
/// 返回的布尔值表示文件中的 `id` 与目录名不一致、已按目录名修正，需要写回。
fn load(paths: &DataPaths, id: &str) -> AppResult<(InstanceConfig, bool)> {
    let file = paths.instance_file(id);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(
                AppError::new(ErrorCode::InstanceCorrupted).with_detail("缺少 instance.json")
            );
        }
        Err(e) => return Err(io_error(e, &file)),
    };
    let mut config = InstanceConfig::from_json(&text)?;
    let mismatched = config.id != id;
    if mismatched {
        config.id = id.to_owned();
    }
    Ok((config, mismatched))
}

fn fingerprint(file: &Path) -> Fingerprint {
    let Ok(meta) = fs::metadata(file) else {
        return (-1, -1);
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    (mtime, i64::try_from(meta.len()).unwrap_or(i64::MAX))
}

fn index_ok(db: &Db, config: &InstanceConfig, (mtime, size): Fingerprint) -> AppResult<()> {
    db.conn().execute(
        "INSERT OR REPLACE INTO instance_index
             (id, name, game_version, loader_kind, loader_version, group_name,
              created_at, last_played_at, error, file_mtime_ms, file_size)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10)",
        params![
            config.id,
            config.name,
            config.game_version,
            config.loader.as_ref().map(|l| l.kind.as_str()),
            config.loader.as_ref().map(|l| l.version.as_str()),
            config.group,
            config.created_at,
            config.last_played_at,
            mtime,
            size,
        ],
    )?;
    Ok(())
}

fn index_corrupted(
    db: &Db,
    id: &str,
    code: ErrorCode,
    (mtime, size): Fingerprint,
) -> AppResult<()> {
    db.conn().execute(
        "INSERT OR REPLACE INTO instance_index
             (id, name, game_version, created_at, error, file_mtime_ms, file_size)
         VALUES (?1, ?1, '', '', ?2, ?3, ?4)",
        params![id, code.as_str(), mtime, size],
    )?;
    Ok(())
}

/// 把索引中存的错误码字符串还原；未知值（来自更新版本的程序）按损坏处理。
fn parse_error_code(code: &str) -> ErrorCode {
    serde_json::from_value(serde_json::Value::from(code)).unwrap_or(ErrorCode::InstanceCorrupted)
}

fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub(crate) fn io_error(err: std::io::Error, path: &Path) -> AppError {
    AppError::from(err).with_param("path", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::Env;

    fn fabric() -> Option<LoaderSpec> {
        Some(LoaderSpec {
            kind: LoaderKind::Fabric,
            version: "0.16.5".into(),
        })
    }

    #[test]
    fn create_get_list() {
        let env = Env::new();
        let store = env.store();
        let a = store.create(" 生存 ", "1.20.1", fabric()).unwrap();
        assert_eq!(a.name, "生存");
        assert!(env.paths.instance_game_dir(&a.id).is_dir());
        assert_eq!(store.get(&a.id).unwrap(), a);

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, a.id);
        assert_eq!(list[0].loader, fabric());
        assert_eq!(list[0].error, None);
    }

    #[test]
    fn list_orders_recently_played_first() {
        let env = Env::new();
        let store = env.store();
        let a = store.create("a", "1.20.1", None).unwrap();
        let b = store.create("b", "1.12.2", None).unwrap();
        store.mark_played(&a.id).unwrap();
        let ids: Vec<_> = store.list().unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, [a.id, b.id]);
    }

    #[test]
    fn rename_and_save_validate_name() {
        let env = Env::new();
        let store = env.store();
        let a = store.create("a", "1.20.1", None).unwrap();
        assert_eq!(store.rename(&a.id, "新名字").unwrap().name, "新名字");
        assert_eq!(store.list().unwrap()[0].name, "新名字");

        let err = store.rename(&a.id, "  ").unwrap_err();
        assert_eq!(err.code, ErrorCode::InstanceInvalidName);

        let mut edited = store.get(&a.id).unwrap();
        edited.memory.max_mb = 8192;
        store.save(&edited).unwrap();
        assert_eq!(store.get(&a.id).unwrap().memory.max_mb, 8192);
    }

    #[test]
    fn unknown_or_unsafe_id_is_not_found() {
        let env = Env::new();
        let store = env.store();
        for id in ["missing", "../tmcl-data", "", "a/b"] {
            assert_eq!(store.get(id).unwrap_err().code, ErrorCode::InstanceNotFound);
            assert_eq!(
                store.delete(id).unwrap_err().code,
                ErrorCode::InstanceNotFound
            );
        }
    }

    #[test]
    fn delete_moves_to_trash_and_purge_cleans_old() {
        let env = Env::new();
        let store = env.store();
        let a = store.create("a", "1.20.1", None).unwrap();
        fs::write(env.paths.instance_game_dir(&a.id).join("options.txt"), "x").unwrap();
        store.delete(&a.id).unwrap();

        assert!(!env.paths.instance_dir(&a.id).exists());
        assert!(store.list().unwrap().is_empty());
        let trashed: Vec<_> = fs::read_dir(env.paths.trash_dir())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(trashed.len(), 1);
        assert!(trashed[0].join("game/options.txt").is_file());

        // 刚删除的不会被清理；把截止时间放到未来则会。
        assert_eq!(store.purge_trash().unwrap(), 0);
        let future = unix_secs(SystemTime::now()) + 10;
        fs::create_dir(env.paths.trash_dir().join("not-a-trash-entry")).unwrap();
        assert_eq!(store.purge_trash_before(future).unwrap(), 1);
        assert!(env.paths.trash_dir().join("not-a-trash-entry").exists());
    }

    #[test]
    fn reconcile_follows_manual_changes_on_disk() {
        let env = Env::new();
        let a_id;
        let b_id;
        {
            let store = env.store();
            a_id = store.create("a", "1.20.1", None).unwrap().id;
            b_id = store.create("b", "1.12.2", None).unwrap().id;
            assert_eq!(store.reconcile().unwrap(), ReconcileReport::default());
        }

        // 模拟启动器关闭期间用户手动改动目录。
        fs::remove_dir_all(env.paths.instance_dir(&a_id)).unwrap();
        let b_file = env.paths.instance_file(&b_id);
        let text = fs::read_to_string(&b_file).unwrap();
        fs::write(&b_file, text.replace("\"b\"", "\"b 改过的名字\"")).unwrap();
        copy_dir(
            &env.paths.instance_dir(&b_id),
            &env.paths.instance_dir("copied"),
        );
        fs::create_dir(env.paths.instance_dir("broken")).unwrap();
        fs::write(env.paths.instance_file("broken"), "{ oops").unwrap();
        fs::create_dir(env.paths.instance_dir("no-json")).unwrap();
        fs::create_dir(env.paths.instance_dir(".hidden")).unwrap();
        fs::write(env.paths.instance_dir("stray-file"), "").unwrap();

        // “重启”：新的数据库连接与存储。
        let db = Db::open(&env.paths.db_file()).unwrap();
        let store = InstanceStore::new(&env.paths, &db);
        let report = store.reconcile().unwrap();
        assert_eq!(
            report,
            ReconcileReport {
                added: 3,
                updated: 1,
                removed: 1,
                corrupted: 2,
            }
        );

        let list = store.list().unwrap();
        let by_id: HashMap<_, _> = list.iter().map(|s| (s.id.as_str(), s)).collect();
        assert_eq!(by_id.len(), 4);
        assert_eq!(by_id[b_id.as_str()].name, "b 改过的名字");
        assert_eq!(by_id["broken"].error, Some(ErrorCode::InstanceCorrupted));
        assert_eq!(by_id["no-json"].error, Some(ErrorCode::InstanceCorrupted));
        assert_eq!(by_id["copied"].error, None);
        // 复制出的目录以目录名为 ID 并写回。
        assert_eq!(store.get("copied").unwrap().id, "copied");
        assert_eq!(
            store.get("broken").unwrap_err().code,
            ErrorCode::InstanceCorrupted
        );

        // 再次对账：没有变化，损坏数仍被统计。
        let again = store.reconcile().unwrap();
        assert_eq!(
            again,
            ReconcileReport {
                corrupted: 2,
                ..ReconcileReport::default()
            }
        );

        // 损坏的实例可以删除。
        store.delete("broken").unwrap();
        assert_eq!(store.list().unwrap().len(), 3);
    }

    #[test]
    fn unsupported_schema_is_listed_with_its_code() {
        let env = Env::new();
        let store = env.store();
        let a = store.create("a", "1.20.1", None).unwrap();
        let file = env.paths.instance_file(&a.id);
        let text = fs::read_to_string(&file).unwrap();
        fs::write(
            &file,
            text.replace(
                &format!("\"schema\": {}", crate::INSTANCE_SCHEMA),
                "\"schema\": 99",
            ),
        )
        .unwrap();
        store.reconcile().unwrap();
        assert_eq!(
            store.list().unwrap()[0].error,
            Some(ErrorCode::InstanceSchemaUnsupported)
        );
    }

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).unwrap();
            }
        }
    }
}
