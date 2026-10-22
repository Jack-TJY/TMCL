-- 0001_common_init：tmcl-common 的通用键值表。
-- schema_migrations 表由迁移执行器自行创建，不在此处定义。

CREATE TABLE IF NOT EXISTS kv (
    key        TEXT    PRIMARY KEY NOT NULL,
    value      TEXT    NOT NULL,
    updated_at INTEGER NOT NULL
);
