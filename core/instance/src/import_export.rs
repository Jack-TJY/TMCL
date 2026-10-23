//! 复制实例，导出 / 导入 `.tmclpack`。
//!
//! `.tmclpack` 是一个 zip：根目录是 `instance.json`，`game/` 下是导出时选中的
//! `game/` 顶层子目录或文件。导入时总是分配新 ID 与创建时间，名称与设置沿用包内的
//! `instance.json`（旧 `schema` 会被迁移）。
//!
//! 这些操作可能很耗时，都接收 [`TaskCtx`]：逐个文件检查取消并上报进度
//! （阶段名 `copy` / `export` / `import`）。失败或取消时删除已写出的半成品。

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};

use tmcl_common::{AppError, AppResult, ErrorCode, ProgressUpdate, TaskCtx};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::model::InstanceConfig;
use crate::store::{InstanceStore, io_error};

/// `.tmclpack` 的扩展名（不含点）。
pub const PACK_EXTENSION: &str = "tmclpack";

/// 包内 `instance.json` 的路径。
const PACK_CONFIG: &str = "instance.json";

/// 包内游戏目录的前缀。
const PACK_GAME_DIR: &str = "game";

/// 存档所在的 `game/` 子目录名。
const SAVES_DIR: &str = "saves";

/// `game/` 下的一项（相对 `game/` 的路径）。
enum Entry {
    Dir(PathBuf),
    File(PathBuf, u64),
}

impl InstanceStore<'_> {
    /// 列出实例 `game/` 下的顶层条目名（按名称排序），供界面选择导出内容。
    pub fn game_entries(&self, id: &str) -> AppResult<Vec<String>> {
        let config = self.get(id)?;
        let game = self.paths().instance_game_dir(&config.id);
        let mut names: Vec<String> = read_dir_names(&game)?;
        names.sort();
        Ok(names)
    }

    /// 复制实例为新实例：新 ID、新名称，复制 `game/`；`include_saves` 为 `false`
    /// 时跳过 `game/saves/`。新实例的最近启动时间为空。
    pub fn duplicate(
        &self,
        id: &str,
        name: &str,
        include_saves: bool,
        ctx: &TaskCtx,
    ) -> AppResult<InstanceConfig> {
        let source = self.get(id)?;
        let fresh = InstanceConfig::new(name, &source.game_version, None)?;
        let config = InstanceConfig {
            id: fresh.id,
            name: fresh.name,
            created_at: fresh.created_at,
            last_played_at: None,
            ..source.clone()
        };

        let from = self.paths().instance_game_dir(&source.id);
        let skip: &[&str] = if include_saves { &[] } else { &[SAVES_DIR] };
        let entries = collect(&from, &read_dir_names(&from)?, skip)?;
        let new_dir = self.paths().instance_dir(&config.id);
        let result = (|| {
            copy_entries(
                &from,
                &self.paths().instance_game_dir(&config.id),
                &entries,
                ctx,
            )?;
            self.write(&config)
        })();
        finish(result, &new_dir)?;
        tracing::info!(from = %source.id, to = %config.id, include_saves, "已复制实例");
        Ok(config)
    }

    /// 把实例导出为 `.tmclpack`，`include` 为要包含的 `game/` 顶层条目名
    /// （见 [`game_entries`](Self::game_entries)），不存在的名字被忽略。
    ///
    /// 先写 `<dest>.part`，完成后再改名为 `dest`。
    pub fn export_pack(
        &self,
        id: &str,
        dest: &Path,
        include: &[String],
        ctx: &TaskCtx,
    ) -> AppResult<()> {
        let config = self.get(id)?;
        let game = self.paths().instance_game_dir(&config.id);
        let available = read_dir_names(&game)?;
        let selected: Vec<String> = available
            .into_iter()
            .filter(|name| include.contains(name))
            .collect();
        let entries = collect(&game, &selected, &[])?;

        let mut part_name = dest.file_name().unwrap_or_default().to_os_string();
        part_name.push(".part");
        let part = dest.with_file_name(part_name);
        let result = (|| {
            write_pack(&part, &config, &game, &entries, ctx)?;
            fs::rename(&part, dest).map_err(|e| io_error(e, dest))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        result?;
        tracing::info!(id = %config.id, entries = entries.len(), "已导出实例");
        Ok(())
    }

    /// 从 `.tmclpack` 导入为新实例。
    ///
    /// 不是 zip、缺少或损坏 `instance.json` 返回 `INSTANCE_PACK_INVALID`；
    /// 包的 `schema` 高于当前程序支持的版本返回 `INSTANCE_SCHEMA_UNSUPPORTED`。
    /// 只解压 `game/` 下的条目，并拒绝试图逃出目标目录的路径。
    pub fn import_pack(&self, pack: &Path, ctx: &TaskCtx) -> AppResult<InstanceConfig> {
        let file = File::open(pack).map_err(|e| io_error(e, pack))?;
        let mut archive = ZipArchive::new(BufReader::new(file)).map_err(pack_invalid)?;

        let mut text = String::new();
        archive
            .by_name(PACK_CONFIG)
            .map_err(pack_invalid)?
            .read_to_string(&mut text)
            .map_err(|e| pack_invalid(e.into()))?;
        let packed = InstanceConfig::from_json(&text).map_err(|e| match e.code {
            ErrorCode::InstanceSchemaUnsupported => e,
            _ => AppError::new(ErrorCode::InstancePackInvalid).with_detail(e.to_string()),
        })?;
        let fresh = InstanceConfig::new(&packed.name, &packed.game_version, None)?;
        let config = InstanceConfig {
            id: fresh.id,
            name: fresh.name,
            created_at: fresh.created_at,
            last_played_at: None,
            ..packed
        };

        let new_dir = self.paths().instance_dir(&config.id);
        let result = (|| {
            extract_game(&mut archive, &new_dir, ctx)?;
            self.write(&config)
        })();
        finish(result, &new_dir)?;
        tracing::info!(id = %config.id, "已导入实例");
        Ok(config)
    }
}

/// 列出目录下的条目名；目录不存在时返回空列表。
fn read_dir_names(dir: &Path) -> AppResult<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(e, dir)),
    };
    let mut names = Vec::new();
    for entry in entries {
        if let Some(name) = entry?.file_name().to_str() {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

/// 收集 `root` 下顶层名为 `names`（排除 `skip`）的所有目录与文件。符号链接被跳过。
fn collect(root: &Path, names: &[String], skip: &[&str]) -> AppResult<Vec<Entry>> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = names
        .iter()
        .filter(|n| !skip.contains(&n.as_str()))
        .map(PathBuf::from)
        .collect();
    while let Some(rel) = stack.pop() {
        let path = root.join(&rel);
        let meta = fs::symlink_metadata(&path).map_err(|e| io_error(e, &path))?;
        if meta.is_dir() {
            for name in read_dir_names(&path)? {
                stack.push(rel.join(name));
            }
            out.push(Entry::Dir(rel));
        } else if meta.is_file() {
            out.push(Entry::File(rel, meta.len()));
        }
    }
    Ok(out)
}

fn totals(entries: &[Entry]) -> (u64, u64) {
    entries.iter().fold((0, 0), |(n, bytes), e| match e {
        Entry::File(_, size) => (n + 1, bytes + size),
        Entry::Dir(_) => (n, bytes),
    })
}

/// 把 `entries` 从 `from` 复制到 `to`。
fn copy_entries(from: &Path, to: &Path, entries: &[Entry], ctx: &TaskCtx) -> AppResult<()> {
    let (total, bytes_total) = totals(entries);
    let (mut done, mut bytes_done) = (0, 0);
    fs::create_dir_all(to).map_err(|e| io_error(e, to))?;
    ctx.report(
        ProgressUpdate::new("copy")
            .items(0, total)
            .bytes(0, bytes_total),
    );
    for entry in entries {
        ctx.check_cancelled()?;
        match entry {
            Entry::Dir(rel) => {
                let dir = to.join(rel);
                fs::create_dir_all(&dir).map_err(|e| io_error(e, &dir))?;
            }
            Entry::File(rel, size) => {
                let target = to.join(rel);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| io_error(e, parent))?;
                }
                fs::copy(from.join(rel), &target).map_err(|e| io_error(e, &target))?;
                done += 1;
                bytes_done += size;
                ctx.report(
                    ProgressUpdate::new("copy")
                        .items(done, total)
                        .bytes(bytes_done, bytes_total),
                );
            }
        }
    }
    Ok(())
}

fn write_pack(
    part: &Path,
    config: &InstanceConfig,
    game: &Path,
    entries: &[Entry],
    ctx: &TaskCtx,
) -> AppResult<()> {
    let (total, bytes_total) = totals(entries);
    let (mut done, mut bytes_done) = (0, 0);
    let file = File::create(part).map_err(|e| io_error(e, part))?;
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    zip.start_file(PACK_CONFIG, options).map_err(zip_io)?;
    io::Write::write_all(&mut zip, config.to_json()?.as_bytes())?;
    ctx.report(
        ProgressUpdate::new("export")
            .items(0, total)
            .bytes(0, bytes_total),
    );
    for entry in entries {
        ctx.check_cancelled()?;
        match entry {
            Entry::Dir(rel) => {
                zip.add_directory(pack_path(rel), options).map_err(zip_io)?;
            }
            Entry::File(rel, size) => {
                let options = options.large_file(*size > u64::from(u32::MAX));
                zip.start_file(pack_path(rel), options).map_err(zip_io)?;
                let path = game.join(rel);
                let mut src = File::open(&path).map_err(|e| io_error(e, &path))?;
                io::copy(&mut src, &mut zip).map_err(|e| io_error(e, &path))?;
                done += 1;
                bytes_done += size;
                ctx.report(
                    ProgressUpdate::new("export")
                        .items(done, total)
                        .bytes(bytes_done, bytes_total),
                );
            }
        }
    }
    let mut writer = zip.finish().map_err(zip_io)?;
    io::Write::flush(&mut writer)?;
    writer
        .into_inner()
        .map_err(|e| AppError::from(e.into_error()))?
        .sync_all()?;
    Ok(())
}

/// 把包内 `game/` 下的条目解压到 `<instance_dir>/game/`。
fn extract_game<R: io::Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    instance_dir: &Path,
    ctx: &TaskCtx,
) -> AppResult<()> {
    let mut targets = Vec::new();
    let mut bytes_total = 0;
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(pack_invalid)?;
        let Some(rel) = file.enclosed_name() else {
            continue;
        };
        if !rel.starts_with(PACK_GAME_DIR) || file.is_symlink() {
            continue;
        }
        bytes_total += file.size();
        targets.push((index, rel, file.is_dir()));
    }
    let total = targets.iter().filter(|(_, _, is_dir)| !is_dir).count() as u64;
    let (mut done, mut bytes_done) = (0, 0);

    fs::create_dir_all(instance_dir.join(PACK_GAME_DIR))?;
    ctx.report(
        ProgressUpdate::new("import")
            .items(0, total)
            .bytes(0, bytes_total),
    );
    for (index, rel, is_dir) in targets {
        ctx.check_cancelled()?;
        let target = instance_dir.join(&rel);
        if is_dir {
            fs::create_dir_all(&target).map_err(|e| io_error(e, &target))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(e, parent))?;
        }
        let mut file = archive.by_index(index).map_err(pack_invalid)?;
        let mut out = File::create(&target).map_err(|e| io_error(e, &target))?;
        let written = io::copy(&mut file, &mut out).map_err(|e| {
            AppError::new(ErrorCode::InstancePackInvalid).with_detail(e.to_string())
        })?;
        done += 1;
        bytes_done += written;
        ctx.report(
            ProgressUpdate::new("import")
                .items(done, total)
                .bytes(bytes_done, bytes_total),
        );
    }
    Ok(())
}

/// 操作失败或取消时删除写了一半的新实例目录。
fn finish(result: AppResult<()>, new_dir: &Path) -> AppResult<()> {
    if result.is_err() {
        let _ = fs::remove_dir_all(new_dir);
    }
    result
}

/// `game/` 下相对路径 → 包内路径（总是用 `/` 分隔）。
fn pack_path(rel: &Path) -> String {
    let mut out = String::from(PACK_GAME_DIR);
    for part in rel.components() {
        out.push('/');
        out.push_str(&part.as_os_str().to_string_lossy());
    }
    out
}

fn pack_invalid(err: zip::result::ZipError) -> AppError {
    AppError::new(ErrorCode::InstancePackInvalid).with_detail(err.to_string())
}

fn zip_io(err: zip::result::ZipError) -> AppError {
    match err {
        zip::result::ZipError::Io(e) => AppError::from(e),
        other => AppError::new(ErrorCode::CommonInternal).with_detail(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tmcl_common::VecSink;

    use super::*;
    use crate::test_util::Env;

    /// 建一个带 mods、saves、options.txt 的实例。
    fn sample(store: &InstanceStore<'_>) -> InstanceConfig {
        let mut config = store.create("源实例", "1.20.1", None).unwrap();
        config.jvm_args = vec!["-XX:+UseG1GC".into()];
        store.save(&config).unwrap();
        let game = store.paths().instance_game_dir(&config.id);
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/a.jar"), b"jar").unwrap();
        fs::create_dir_all(game.join("saves/world/region")).unwrap();
        fs::write(game.join("saves/world/level.dat"), b"level").unwrap();
        fs::create_dir_all(game.join("config/empty")).unwrap();
        fs::write(game.join("options.txt"), b"lang:zh_cn").unwrap();
        store.mark_played(&config.id).unwrap()
    }

    #[test]
    fn game_entries_are_sorted() {
        let env = Env::new();
        let store = env.store();
        let a = sample(&store);
        assert_eq!(
            store.game_entries(&a.id).unwrap(),
            ["config", "mods", "options.txt", "saves"]
        );
    }

    #[test]
    fn duplicate_with_and_without_saves() {
        let env = Env::new();
        let store = env.store();
        let a = sample(&store);

        let with = store
            .duplicate(&a.id, "副本", true, &TaskCtx::noop("copy"))
            .unwrap();
        assert_ne!(with.id, a.id);
        assert_eq!(with.name, "副本");
        assert_eq!(with.jvm_args, a.jvm_args);
        assert_eq!(with.last_played_at, None);
        let game = env.paths.instance_game_dir(&with.id);
        assert!(game.join("saves/world/level.dat").is_file());
        assert!(game.join("saves/world/region").is_dir());
        assert!(game.join("config/empty").is_dir());

        let without = store
            .duplicate(&a.id, "无存档", false, &TaskCtx::noop("copy"))
            .unwrap();
        let game = env.paths.instance_game_dir(&without.id);
        assert!(game.join("mods/a.jar").is_file());
        assert!(!game.join("saves").exists());
        assert_eq!(store.list().unwrap().len(), 3);
    }

    #[test]
    fn cancelled_duplicate_leaves_nothing() {
        let env = Env::new();
        let store = env.store();
        let a = sample(&store);
        let ctx = TaskCtx::noop("copy");
        ctx.cancel.cancel();
        let err = store.duplicate(&a.id, "副本", true, &ctx).unwrap_err();
        assert!(err.is_cancelled());
        assert_eq!(fs::read_dir(env.paths.instances_dir()).unwrap().count(), 1);
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn export_then_import_round_trips() {
        let env = Env::new();
        let store = env.store();
        let a = sample(&store);
        let pack = env.paths.root().join("out.tmclpack");
        let sink = Arc::new(VecSink::new());
        let ctx = TaskCtx::new("instance_export", sink.clone());
        store
            .export_pack(
                &a.id,
                &pack,
                &["mods".into(), "options.txt".into(), "nope".into()],
                &ctx,
            )
            .unwrap();
        assert!(pack.is_file());
        assert!(!env.paths.root().join("out.tmclpack.part").exists());
        let last = sink.events().pop().unwrap();
        assert_eq!(
            (last.stage.as_str(), last.done, last.total),
            ("export", 2, 2)
        );

        let imported = store.import_pack(&pack, &TaskCtx::noop("import")).unwrap();
        assert_ne!(imported.id, a.id);
        assert_eq!(imported.name, a.name);
        assert_eq!(imported.jvm_args, a.jvm_args);
        assert_eq!(imported.last_played_at, None);
        let game = env.paths.instance_game_dir(&imported.id);
        assert_eq!(fs::read(game.join("mods/a.jar")).unwrap(), b"jar");
        assert_eq!(fs::read(game.join("options.txt")).unwrap(), b"lang:zh_cn");
        assert!(!game.join("saves").exists());
        assert_eq!(store.get(&imported.id).unwrap(), imported);
    }

    #[test]
    fn cancelled_export_leaves_no_file() {
        let env = Env::new();
        let store = env.store();
        let a = sample(&store);
        let pack = env.paths.root().join("out.tmclpack");
        let ctx = TaskCtx::noop("export");
        ctx.cancel.cancel();
        let err = store
            .export_pack(&a.id, &pack, &["mods".into()], &ctx)
            .unwrap_err();
        assert!(err.is_cancelled());
        assert!(!pack.exists());
        assert!(!env.paths.root().join("out.tmclpack.part").exists());
    }

    /// 手工构造一个包，条目为 (路径, 内容)。
    fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let mut zip = ZipWriter::new(File::create(path).unwrap());
        for (name, data) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            io::Write::write_all(&mut zip, data).unwrap();
        }
        zip.finish().unwrap();
    }

    fn config_json(schema: u32) -> Vec<u8> {
        let config = InstanceConfig::new("包", "1.12.2", None).unwrap();
        let mut value = serde_json::to_value(&config).unwrap();
        value["schema"] = schema.into();
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn import_skips_entries_outside_game_dir() {
        let env = Env::new();
        let store = env.store();
        let pack = env.paths.root().join("evil.tmclpack");
        let json = config_json(1);
        write_zip(
            &pack,
            &[
                ("instance.json", &json),
                ("game/../../escape.txt", b"x"),
                ("../outside.txt", b"x"),
                ("readme.txt", b"x"),
                ("game/ok.txt", b"ok"),
            ],
        );
        let imported = store.import_pack(&pack, &TaskCtx::noop("import")).unwrap();
        let dir = env.paths.instance_dir(&imported.id);
        assert_eq!(fs::read(dir.join("game/ok.txt")).unwrap(), b"ok");
        assert!(!dir.join("readme.txt").exists());
        assert!(!env.paths.root().join("escape.txt").exists());
        assert!(!env.paths.instances_dir().join("escape.txt").exists());
    }

    #[test]
    fn invalid_packs_are_rejected_without_leftovers() {
        let env = Env::new();
        let store = env.store();
        let root = env.paths.root();

        let not_zip = root.join("a.tmclpack");
        fs::write(&not_zip, b"not a zip").unwrap();
        let no_config = root.join("b.tmclpack");
        write_zip(&no_config, &[("game/x.txt", b"x")]);
        let bad_config = root.join("c.tmclpack");
        write_zip(&bad_config, &[("instance.json", b"{ oops")]);
        for pack in [&not_zip, &no_config, &bad_config] {
            let err = store
                .import_pack(pack, &TaskCtx::noop("import"))
                .unwrap_err();
            assert_eq!(err.code, ErrorCode::InstancePackInvalid, "{pack:?}");
        }

        let future = root.join("d.tmclpack");
        write_zip(&future, &[("instance.json", &config_json(99))]);
        let err = store
            .import_pack(&future, &TaskCtx::noop("import"))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InstanceSchemaUnsupported);

        assert_eq!(fs::read_dir(env.paths.instances_dir()).unwrap().count(), 0);
        assert!(store.list().unwrap().is_empty());
    }
}
