// 任务中心的数值格式化与错误详情。
import type { AppError } from "../../bindings/AppError";
import type { TaskProgress } from "../../bindings/TaskProgress";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** 字节数按 1024 进制格式化，例如 `1.5 MB`。 */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

/** 进度百分比（0–100）；总量未知时为 `null`，界面显示不确定进度。 */
export function progressPercent(progress: TaskProgress | null): number | null {
  if (!progress) return null;
  if (progress.bytes_total > 0) {
    return Math.min(100, (progress.bytes_done / progress.bytes_total) * 100);
  }
  if (progress.total > 0) return Math.min(100, (progress.done / progress.total) * 100);
  return null;
}

/** “复制详情”的文本：本地化文案在前，随后是机器可读的错误码、参数与详情。 */
export function errorDetails(message: string, error: AppError): string {
  const lines = [message, `code: ${error.code}`];
  if (Object.keys(error.params).length > 0) lines.push(`params: ${JSON.stringify(error.params)}`);
  if (error.detail) lines.push(`detail: ${error.detail}`);
  return lines.join("\n");
}
