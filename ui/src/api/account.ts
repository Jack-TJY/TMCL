// `auth_*` 命令封装（PLAN.md M0-10、M1-08、M1-14）。
import type { Account } from "../bindings/Account";
import type { AccountList } from "../bindings/AccountList";
import type { AppError } from "../bindings/AppError";
import type { AuthAddAuthlibArgs } from "../bindings/AuthAddAuthlibArgs";
import type { AuthAddOfflineArgs } from "../bindings/AuthAddOfflineArgs";
import type { AuthAuthlibServerArgs } from "../bindings/AuthAuthlibServerArgs";
import type { AuthIdArgs } from "../bindings/AuthIdArgs";
import type { AuthlibServer } from "../bindings/AuthlibServer";
import type { DeviceCode as DeviceCodeEvent } from "../bindings/DeviceCode";
import type { LaunchAccountArgs } from "../bindings/LaunchAccountArgs";
import type { TaskFinished } from "../bindings/TaskFinished";
import type { TaskStarted } from "../bindings/TaskStarted";
import { appInfo } from "./app";
import { onDeviceCode, onTaskFinished } from "./events";
import { invoke, toAppError } from "./invoke";
import { taskCancel } from "./task";

export type { Account, AccountList, AuthlibServer };
export type { AccountKind } from "../bindings/AccountKind";

/** 微软设备码：`auth:device-code` 事件的负载去掉任务 ID。 */
export type DeviceCode = Omit<DeviceCodeEvent, "task_id">;

/** 一次进行中的微软登录。 */
export interface MsLogin {
  /** 拿到设备码时兑现。 */
  deviceCode: Promise<DeviceCode>;
  /** 登录成功（账号已加入并设为默认）时兑现，失败或取消时以 AppError 拒绝。 */
  result: Promise<Account>;
  /** 取消登录（取消对应任务）。 */
  cancel(): void;
}

/** 判断错误是否为“Minecraft 服务拒绝本启动器的应用”（`login_with_xbox` 返回 403）。 */
export function isAppNotApproved(error: AppError): boolean {
  return error.code === "AUTH_MC_APP_NOT_APPROVED";
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

/** 微软登录的可用状态：可用、构建时没有 Client ID。 */
export type MsLoginStatus = "enabled" | "no_client_id";

/** 微软登录是否可用，即构建时是否注入了 Client ID；读不到应用信息时视为不可用。 */
export async function msLoginStatus(): Promise<MsLoginStatus> {
  try {
    return (await appInfo()).ms_login_enabled ? "enabled" : "no_client_id";
  } catch {
    return "no_client_id";
  }
}

const CANCELLED: AppError = { code: "COMMON_CANCELLED", params: {}, detail: null };

/**
 * 开始微软设备码登录：先订阅 `auth:device-code` 与 `task:finished`，再调用 `auth_ms_begin`
 * 拿到任务 ID（任务 ID 返回前收到的事件先缓存）；任务成功后账号已是默认账号，
 * 重新读取账号列表取得它。取消即 `task_cancel`。
 */
export function msLoginStart(): MsLogin {
  let taskId: string | null = null;
  let cancelled = false;
  let settleCode!: { resolve: (code: DeviceCode) => void; reject: (error: AppError) => void };
  const deviceCode = new Promise<DeviceCode>((resolve, reject) => {
    settleCode = { resolve, reject };
  });
  // 只关心结果的调用方不必处理设备码的拒绝。
  deviceCode.catch(() => undefined);

  const result = new Promise<Account>((resolve, reject) => {
    const early: Array<() => void> = [];
    const unlisten: Array<() => void> = [];
    const fail = (error: AppError) => {
      unlisten.forEach((stop) => stop());
      settleCode.reject(error);
      reject(error);
    };
    const handleCode = (event: DeviceCodeEvent) => {
      if (taskId === null) return early.push(() => handleCode(event));
      if (event.task_id !== taskId) return;
      settleCode.resolve({
        user_code: event.user_code,
        verification_uri: event.verification_uri,
        expires_in: event.expires_in,
      });
    };
    const handleFinished = (finished: TaskFinished) => {
      if (taskId === null) return early.push(() => handleFinished(finished));
      if (finished.task_id !== taskId) return;
      if (finished.state !== "succeeded") {
        fail(finished.state === "cancelled" ? CANCELLED : (finished.error ?? CANCELLED));
        return;
      }
      unlisten.forEach((stop) => stop());
      accountList().then((list) => {
        const account = list.accounts.find((a) => a.id === list.default_id);
        if (account) resolve(account);
        else reject(toAppError("signed-in account missing from the list"));
      }, reject);
    };

    Promise.all([onDeviceCode(handleCode), onTaskFinished(handleFinished)])
      .then((stops) => {
        unlisten.push(...stops);
        if (cancelled) throw CANCELLED;
        return invoke<TaskStarted>("auth_ms_begin");
      })
      .then((started) => {
        taskId = started.task_id;
        if (cancelled) void taskCancel({ task_id: taskId });
        early.splice(0).forEach((replay) => replay());
      })
      .catch((err) => fail(toAppError(err)));
  });

  return {
    deviceCode,
    result,
    cancel() {
      cancelled = true;
      if (taskId !== null) void taskCancel({ task_id: taskId });
    },
  };
}

/** 在系统浏览器中打开设备码验证页（只接受微软域名下的 https 地址）。 */
export function openVerificationPage(uri: string): Promise<void> {
  return invoke<void>("auth_ms_open_verification", { args: { uri } });
}

/** 启动参数中的账号：按 ID 启动，外置登录账号的令牌刷新与 authlib-injector 由后端处理。 */
export function launchAccountArgs(account: Account): LaunchAccountArgs {
  return { kind: "account", id: account.id };
}
