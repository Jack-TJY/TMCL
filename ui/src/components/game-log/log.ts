// 日志窗口的纯函数：筛选、导出文本与文件名。
import type { LogLine } from "./sessions";

/** 按关键字筛选日志行（不区分大小写）；关键字为空时返回原数组。 */
export function filterLines(lines: LogLine[], query: string): LogLine[] {
  const q = query.trim().toLowerCase();
  return q ? lines.filter((l) => l.text.toLowerCase().includes(q)) : lines;
}

/** 导出或复制用的纯文本，每行一条。 */
export function logText(lines: LogLine[]): string {
  return lines.map((l) => l.text).join("\n") + (lines.length ? "\n" : "");
}

/** 导出文件名：`<实例名>-<本地时间 yyyyMMdd-HHmmss>.log`，去掉文件名中不允许的字符。 */
export function logFileName(name: string, at: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp =
    `${at.getFullYear()}${pad(at.getMonth() + 1)}${pad(at.getDate())}-` +
    `${pad(at.getHours())}${pad(at.getMinutes())}${pad(at.getSeconds())}`;
  const safe = name.replace(/[\\/:*?"<>|]/g, "_").trim() || "game";
  return `${safe}-${stamp}.log`;
}
