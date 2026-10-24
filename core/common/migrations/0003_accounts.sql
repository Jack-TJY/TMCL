-- 0003_accounts：tmcl-auth 的账号列表（PLAN.md M1-08）。
-- 只存账号元数据；访问令牌、刷新令牌在系统密钥库（键名 tmcl:account:<id>）。
-- 默认账号记在 kv 表的 auth.default_account。

CREATE TABLE IF NOT EXISTS accounts (
    id          TEXT    PRIMARY KEY NOT NULL,
    -- microsoft | offline | authlib
    kind        TEXT    NOT NULL,
    name        TEXT    NOT NULL,
    -- 玩家 UUID，带连字符
    uuid        TEXT    NOT NULL,
    -- 仅外置登录：Yggdrasil API 地址与皮肤站名称
    server_url  TEXT,
    server_name TEXT,
    created_at  INTEGER NOT NULL
);
