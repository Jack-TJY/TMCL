// 账号接口（PLAN.md M1-08、M1-14）。M1-08 的命令完成前由 `__mocks__/account.ts` 提供替身；
// 届时把下列类型换成 `ui/src/bindings/` 中生成的同名类型、函数体换成 `invoke("account_*")`，
// 调用方（账号页、账号 store、“当前账号”组件）无需改动。
import type { LaunchAccountArgs } from "../bindings/LaunchAccountArgs";
import {
  mockAccountAddOffline,
  mockAccountList,
  mockAccountRemove,
  mockAccountSetDefault,
} from "./__mocks__/account";

/** 账号类型：微软正版、离线（仅限本地测试）、外置登录（authlib-injector）。 */
export type AccountKind = "microsoft" | "offline" | "authlib";

/** 外置登录的皮肤站。 */
export interface AuthlibServer {
  /** Yggdrasil API 地址，例如 `https://littleskin.cn/api/yggdrasil`。 */
  api_url: string;
  /** 皮肤站名称，取自 API 元数据的 `meta.serverName`。 */
  name: string;
}

/** 一个账号。令牌只在后端的系统密钥库中，不会出现在这里。 */
export interface Account {
  id: string;
  kind: AccountKind;
  /** 玩家名。 */
  name: string;
  /** 玩家 UUID（带连字符）。 */
  uuid: string;
  /** 仅外置登录账号有值。 */
  server: AuthlibServer | null;
}

export interface AccountList {
  accounts: Account[];
  /** 启动游戏时使用的账号；列表为空时为 `null`。 */
  default_id: string | null;
}

export interface AccountIdArgs {
  id: string;
}

export interface AccountAddOfflineArgs {
  /** 玩家名，须符合 `^[A-Za-z0-9_]{3,16}$`，否则返回 `AUTH_OFFLINE_INVALID_NAME`。 */
  name: string;
}

/** 离线账号用户名规则，与后端一致。 */
export const OFFLINE_NAME_PATTERN = /^[A-Za-z0-9_]{3,16}$/;

/** 读取账号列表与默认账号。 */
export function accountList(): Promise<AccountList> {
  return mockAccountList();
}

/** 设为启动时使用的账号。 */
export function accountSetDefault(args: AccountIdArgs): Promise<void> {
  return mockAccountSetDefault(args);
}

/** 删除账号（同时删除系统密钥库中的令牌）；删除默认账号时改用列表中的第一个。 */
export function accountRemove(args: AccountIdArgs): Promise<void> {
  return mockAccountRemove(args);
}

/** 添加离线账号并设为默认；同名账号已存在时返回该账号。 */
export function accountAddOffline(args: AccountAddOfflineArgs): Promise<Account> {
  return mockAccountAddOffline(args);
}

/**
 * 启动参数中的账号。`launch_start` 目前只接受离线账号（M1-07），M1-08 追加其他种类前，
 * 正版与外置账号用同名离线身份启动，只为让界面流程可走通。
 */
export function launchAccountArgs(account: Account): LaunchAccountArgs {
  return { kind: "offline", name: account.name };
}
