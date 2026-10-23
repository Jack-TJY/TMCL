//! 测试用的临时数据目录与数据库。

use tmcl_common::{DataPaths, Db};

use crate::store::InstanceStore;

/// 临时数据目录 + 已迁移的数据库，离开作用域时删除。
pub(crate) struct Env {
    _dir: tempfile::TempDir,
    pub(crate) paths: DataPaths,
    pub(crate) db: Db,
}

impl Env {
    pub(crate) fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let paths = DataPaths::at(dir.path().join("tmcl-data"));
        paths.ensure_dirs().unwrap();
        let db = Db::open(&paths.db_file()).unwrap();
        Self {
            _dir: dir,
            paths,
            db,
        }
    }

    pub(crate) fn store(&self) -> InstanceStore<'_> {
        InstanceStore::new(&self.paths, &self.db)
    }
}
