// 账号接口（PLAN.md M1-08、M1-14）。M1-08 的命令完成前由 `__mocks__/account.ts` 提供替身；
// 届时把下列类型换成 `ui/src/bindings/` 中生成的同名类型、函数体换成 `invoke("account_*")`，
// 调用方（账号页、账号 store、“当前账号”组件）无需改动。
import type { AppError } from "../bindings/AppError";
import type { LaunchAccountArgs } from "../bindings/LaunchAccountArgs";
import {
  mockAccountAddAuthlib,
  mockAccountAddOffline,
  mockAccountList,
  mockAccountRemove,
  mockAccountSetDefault,
  mockAuthlibServerInfo,
  mockMsLoginStart,
} from "./__mocks__/account";
import { appInfo } from "./app";

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

export interface AuthlibServerArgs {
  /** 用户输入的皮肤站地址；后端按 authlib-injector 规范处理 API 地址指示（ALI）。 */
  api_url: string;
}

export interface AccountAddAuthlibArgs {
  /** `authlibServerInfo` 返回的 API 地址。 */
  api_url: string;
  /** 皮肤站的登录名（通常是邮箱）。 */
  username: string;
  password: string;
}

/** 微软设备码，即 `auth:device-code` 事件的负载（PLAN.md 第 3.3 节）。 */
export interface DeviceCode {
  user_code: string;
  verification_uri: string;
  /** 设备码有效期，单位秒。 */
  expires_in: number;
}

/** 一次进行中的微软登录。 */
export interface MsLogin {
  /** 拿到设备码时兑现。 */
  deviceCode: Promise<DeviceCode>;
  /** 登录成功（账号已加入并设为默认）时兑现，失败或取消时以 AppError 拒绝。 */
  result: Promise<Account>;
  /** 取消登录（取消对应任务）。 */
  cancel(): void;
}

/**
 * Minecraft 服务尚未批准本启动器的应用（`login_with_xbox` 返回 403），由 M0-10 / M1-08 新增；
 * 在其进入 `ErrorCode` 之前以字符串比较识别。
 */
export const AUTH_MC_APP_NOT_APPROVED = "AUTH_MC_APP_NOT_APPROVED";

/** 判断错误是否为“应用尚未获批”。 */
export function isAppNotApproved(error: AppError): boolean {
  return (error.code as string) === AUTH_MC_APP_NOT_APPROVED;
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

/** 构建时是否注入了微软登录的 Client ID；在浏览器中调试（无后端）时视为可用，以便走通替身流程。 */
export async function msLoginEnabled(): Promise<boolean> {
  try {
    return (await appInfo()).ms_login_enabled;
  } catch {
    return import.meta.env.DEV;
  }
}

/**
 * 开始微软设备码登录。真实实现：先订阅 `auth:device-code`，再调用 `auth_ms_begin` 拿到任务 ID，
 * 任务成功后重新读取账号列表，取消即 `task_cancel`。
 */
export function msLoginStart(): MsLogin {
  return mockMsLoginStart();
}

/**
 * 在系统浏览器中打开设备码验证页。界面层不直接访问网络（AGENTS.md §4），
 * M1-08 应提供打开网址的命令（或由 `auth_ms_begin` 自动打开）后替换这里的 `window.open`。
 */
export function openVerificationPage(uri: string) {
  window.open(uri, "_blank", "noopener");
}

/** 读取皮肤站元数据，返回规整后的 API 地址与皮肤站名称。 */
export function authlibServerInfo(args: AuthlibServerArgs): Promise<AuthlibServer> {
  return mockAuthlibServerInfo(args);
}

/** 用皮肤站账号登录，加入账号并设为默认。 */
export function accountAddAuthlib(args: AccountAddAuthlibArgs): Promise<Account> {
  return mockAccountAddAuthlib(args);
}

/**
 * 启动参数中的账号。`launch_start` 目前只接受离线账号（M1-07），M1-08 追加其他种类前，
 * 正版与外置账号用同名离线身份启动，只为让界面流程可走通。
 */
export function launchAccountArgs(account: Account): LaunchAccountArgs {
  return { kind: "offline", name: account.name };
}
