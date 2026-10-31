// `sakura_*` 命令封装（PLAN.md M2-14、M2-15）。访问密钥只经 `sakuraSaveKey` 传给后端，任何返回值都不含密钥。
import type { SakuraNodeInfo } from "../bindings/SakuraNodeInfo";
import type { SakuraNodeList } from "../bindings/SakuraNodeList";
import type { SakuraPing } from "../bindings/SakuraPing";
import type { SakuraPolicy } from "../bindings/SakuraPolicy";
import type { SakuraStatus } from "../bindings/SakuraStatus";
import type { SakuraTunnel } from "../bindings/SakuraTunnel";
import type { SakuraTunnelList } from "../bindings/SakuraTunnelList";
import type { SakuraUser } from "../bindings/SakuraUser";
import type { StartRequest } from "../bindings/StartRequest";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

export type {
  SakuraNodeInfo,
  SakuraPing,
  SakuraPolicy,
  SakuraStatus,
  SakuraTunnel,
  SakuraUser,
  StartRequest,
};
export type { SakuraRoom } from "../bindings/SakuraRoom";

/** 开始联机任务的种类名，与后端 `SAKURA_START_KIND` 一致。 */
export const SAKURA_START_KIND = "sakura_start";

/** 是否已保存密钥、当前房间与 frpc 状态；不访问网络，可以频繁调用。 */
export function sakuraStatus(): Promise<SakuraStatus> {
  return invoke<SakuraStatus>("sakura_status");
}

/** 校验并保存访问密钥，返回账号信息。 */
export function sakuraSaveKey(key: string): Promise<SakuraUser> {
  return invoke<SakuraUser>("sakura_save_key", { args: { key } });
}

/** 删除保存的访问密钥；正在联机时先停止。 */
export function sakuraClearKey(): Promise<void> {
  return invoke<void>("sakura_clear_key");
}

/** 用保存的密钥读取账号信息。 */
export function sakuraUser(): Promise<SakuraUser> {
  return invoke<SakuraUser>("sakura_user");
}

/** 节点列表，按 ID 排序。 */
export async function sakuraNodes(): Promise<SakuraNodeInfo[]> {
  return (await invoke<SakuraNodeList>("sakura_nodes")).nodes;
}

/** SakuraFrp 服务条款。 */
export function sakuraTerms(): Promise<SakuraPolicy> {
  return invoke<SakuraPolicy>("sakura_terms");
}

/** 开始联机（后台任务）；结束后用 `sakuraStatus` 取联机地址。 */
export function sakuraStart(args: StartRequest): Promise<TaskStarted> {
  return invoke<TaskStarted>("sakura_start", { args });
}

/** 停止联机并删除临时隧道。 */
export function sakuraStop(): Promise<void> {
  return invoke<void>("sakura_stop");
}

/** 到联机地址（穿透节点）的延迟，以及地址后面的游戏是否回应。 */
export function sakuraPing(address: string): Promise<SakuraPing> {
  return invoke<SakuraPing>("sakura_ping", { args: { address } });
}

/** 账号下的全部隧道。 */
export async function sakuraTunnels(): Promise<SakuraTunnel[]> {
  return (await invoke<SakuraTunnelList>("sakura_tunnels")).tunnels;
}

/** 删除一条隧道；是当前房间的隧道时等同于停止联机。 */
export function sakuraDeleteTunnel(id: number): Promise<void> {
  return invoke<void>("sakura_delete_tunnel", { args: { id } });
}

/** TMCL 临时隧道的备注，与后端 `TEMP_NOTE` 一致。 */
export const SAKURA_TEMP_NOTE = "TMCL temporary tunnel";
