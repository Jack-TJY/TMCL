-- 0002_instance_index：tmcl-instance 的实例索引。
-- 磁盘上的 instances/<id>/instance.json 是唯一事实来源，本表只是缓存，
-- 启动时由对账（reconcile）按 instance.json 的修改时间与大小刷新。

CREATE TABLE IF NOT EXISTS instance_index (
    id             TEXT    PRIMARY KEY NOT NULL,
    name           TEXT    NOT NULL,
    game_version   TEXT    NOT NULL,
    loader_kind    TEXT,
    loader_version TEXT,
    group_name     TEXT,
    created_at     TEXT    NOT NULL,
    last_played_at TEXT,
    -- 损坏时为错误码（如 INSTANCE_CORRUPTED），正常时为 NULL
    error          TEXT,
    -- instance.json 的修改时间（unix 毫秒）与字节数，用于对账时跳过未变化的实例
    file_mtime_ms  INTEGER NOT NULL,
    file_size      INTEGER NOT NULL
);
