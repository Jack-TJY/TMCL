//! 启动 classpath：依赖库主构件 + 客户端 jar。

use std::collections::BTreeSet;
use std::path::PathBuf;

use tmcl_game::{GameFiles, Library, RuleEnv, VersionJson};

use crate::placeholders::classpath_separator;

/// `version`（已经过 [`tmcl_game::resolve`] 合并）在 `env` 下的 classpath，顺序与版本 JSON 一致，
/// 客户端 jar（按 [`VersionJson::jar_id`]）在最后。
///
/// 包括官方 `downloads.artifact` 与只给 Maven 坐标的加载器库（如 Fabric）；
/// 旧式 natives（`natives` + `classifiers`）只解压、不进 classpath。按路径去重。
pub fn classpath(files: &GameFiles, version: &VersionJson, env: &RuleEnv) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut entries: Vec<PathBuf> = version
        .libraries_for(env)
        .filter_map(Library::file)
        .map(|f| files.library(&f.path))
        .filter(|p| seen.insert(p.clone()))
        .collect();
    let jar = files.client_jar(version.jar_id());
    if seen.insert(jar.clone()) {
        entries.push(jar);
    }
    entries
}

/// 用 `env` 对应系统的分隔符拼接 classpath。
pub fn join_classpath(entries: &[PathBuf], env: &RuleEnv) -> String {
    entries
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(classpath_separator(env))
}
