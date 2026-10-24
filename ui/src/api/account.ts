// `auth_*` 命令封装（PLAN.md M1-08、M1-14）。微软登录（M0-10）完成前由 `__mocks__/account.ts`
// 提供设备码流程的替身，只在没有后端的浏览器调试中可用。
import type { Account } from "../bindings/Account";
import type { AccountList } from "../bindings/AccountList";
import type { AppError } from "../bindings/AppError";
import type { AuthAddAuthlibArgs } from "../bindings/AuthAddAuthlibArgs";
import type { AuthAddOfflineArgs } from "../bindings/AuthAddOfflineArgs";
import type { AuthAuthlibServerArgs } from "../bindings/AuthAuthlibServerArgs";
import type { AuthIdArgs } from "../bindings/AuthIdArgs";
import type { AuthlibServer } from "../bindings/AuthlibServer";
import type { LaunchAccountArgs } from "../bindings/LaunchAccountArgs";
import { mockMsLoginStart } from "./__mocks__/account";
import { appInfo } from "./app";
import { invoke } from "./invoke";

export type { Account, AccountList, AuthlibServer };
export type { AccountKind } from "../bindings/AccountKind";

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
 * Minecraft 服务尚未批准本启动器的应用（`login_with_xbox` 返回 403），由 M0-10 新增；
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
  return invoke<AccountList>("auth_list");
}

/** 设为启动时使用的账号。 */
export function accountSetDefault(args: AuthIdArgs): Promise<void> {
  return invoke<void>("auth_set_default", { args });
}

/** 删除账号（同时删除系统密钥库中的令牌）；删除默认账号时改用列表中的第一个。 */
export function accountRemove(args: AuthIdArgs): Promise<void> {
  return invoke<void>("auth_remove", { args });
}

/** 添加离线账号并设为默认；同名账号已存在时返回该账号。 */
export function accountAddOffline(args: AuthAddOfflineArgs): Promise<Account> {
  return invoke<Account>("auth_add_offline", { args });
}

/** 读取皮肤站名称与规整后的 API 地址。 */
export function authlibServerInfo(args: AuthAuthlibServerArgs): Promise<AuthlibServer> {
  return invoke<AuthlibServer>("auth_authlib_server", { args });
}

/** 用皮肤站账号登录，加入账号并设为默认。 */
export function accountAddAuthlib(args: AuthAddAuthlibArgs): Promise<Account> {
  return invoke<Account>("auth_add_authlib", { args });
}

/** 微软登录的可用状态：可用、构建时没有 Client ID、功能尚未完成（M0-10）。 */
export type MsLoginStatus = "enabled" | "no_client_id" | "pending";

/** 微软登录是否可用。没有后端（浏览器调试）时可用，以便走通设备码弹窗的替身流程。 */
export async function msLoginStatus(): Promise<MsLoginStatus> {
  let enabled: boolean;
  try {
    enabled = (await appInfo()).ms_login_enabled;
  } catch {
    return import.meta.env.DEV ? "enabled" : "pending";
  }
  // `auth_ms_begin` 由 M0-10 提供，在此之前真实环境中不开放。
  return enabled ? "pending" : "no_client_id";
}

/**
 * 开始微软设备码登录（M0-10 前为替身）。真实实现：先订阅 `auth:device-code`，再调用
 * `auth_ms_begin` 拿到任务 ID，任务成功后重新读取账号列表，取消即 `task_cancel`。
 */
export function msLoginStart(): MsLogin {
  return mockMsLoginStart();
}

/**
 * 在系统浏览器中打开设备码验证页。界面层不直接访问网络（AGENTS.md §4），
 * M0-10 应提供打开网址的命令（或由 `auth_ms_begin` 自动打开）后替换这里的 `window.open`。
 */
export function openVerificationPage(uri: string) {
  window.open(uri, "_blank", "noopener");
}

/** 启动参数中的账号：按 ID 启动，外置登录账号的令牌刷新与 authlib-injector 由后端处理。 */
export function launchAccountArgs(account: Account): LaunchAccountArgs {
  return { kind: "account", id: account.id };
}
