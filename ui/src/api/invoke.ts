// 调用 Tauri 命令的唯一入口：失败时一律抛出结构化的 AppError（PLAN.md 第 3.1、3.2 节）。
import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";
import type { AppError } from "../bindings/AppError";

/** 判断一个值是否为后端返回的 AppError。 */
export function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const { code, params } = value as Record<string, unknown>;
  return typeof code === "string" && typeof params === "object" && params !== null;
}

/** 把任意失败值规整为 AppError；非结构化错误（如 IPC 层错误）记为 COMMON_INTERNAL。 */
export function toAppError(value: unknown): AppError {
  if (isAppError(value)) return value;
  const detail = value instanceof Error ? value.message : String(value);
  return { code: "COMMON_INTERNAL", params: {}, detail };
}

/** 调用命令；拒绝值保证是 AppError。 */
export async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (err) {
    throw toAppError(err);
  }
}
