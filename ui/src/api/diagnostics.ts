// `diagnostics_*` 命令封装。
import type { DiagnosticsExportArgs } from "../bindings/DiagnosticsExportArgs";
import type { DiagnosticsExported } from "../bindings/DiagnosticsExported";
import { invoke } from "./invoke";

/** 收集诊断信息（日志、系统信息、实例列表），脱敏后写成 zip。 */
export function diagnosticsExport(args: DiagnosticsExportArgs): Promise<DiagnosticsExported> {
  return invoke<DiagnosticsExported>("diagnostics_export", { args });
}
