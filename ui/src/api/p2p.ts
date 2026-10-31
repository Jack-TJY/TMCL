// `p2p_*` 命令封装。
import type { LanWorldList } from "../bindings/LanWorldList";
import type { P2pHostArgs } from "../bindings/P2pHostArgs";
import type { P2pInfo } from "../bindings/P2pInfo";
import type { P2pJoinArgs } from "../bindings/P2pJoinArgs";
import type { P2pStatus } from "../bindings/P2pStatus";
import type { TaskStarted } from "../bindings/TaskStarted";
import { invoke } from "./invoke";

/** EasyTier 版本与内置默认公共节点。 */
export function p2pInfo(): Promise<P2pInfo> {
  return invoke<P2pInfo>("p2p_info");
}

/** 开房（任务种类 `p2p_host`）；之后轮询 `p2pStatus` 取房间码。 */
export function p2pHost(args: P2pHostArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("p2p_host", { args });
}

/** 加入房间（任务种类 `p2p_join`）；之后轮询 `p2pStatus` 取本机连接地址。 */
export function p2pJoin(args: P2pJoinArgs): Promise<TaskStarted> {
  return invoke<TaskStarted>("p2p_join", { args });
}

/** 本机已对局域网开放的世界（约 2 秒）。 */
export function p2pLanWorlds(): Promise<LanWorldList> {
  return invoke<LanWorldList>("p2p_lan_worlds");
}

/** 当前房间状态，可每 2–5 秒轮询。 */
export function p2pStatus(): Promise<P2pStatus> {
  return invoke<P2pStatus>("p2p_status");
}

/** 关闭或退出房间。 */
export function p2pStop(): Promise<void> {
  return invoke<void>("p2p_stop");
}

/** EasyTier 节点地址允许的协议，与后端 `tmcl_p2p::validate_node` 一致。 */
const NODE_SCHEMES = ["tcp", "udp", "ws", "wss", "quic", "wg", "http", "https", "txt", "srv"];

/** 前端预检节点地址 `协议://主机[:端口][/路径]`；保存时后端会再校验一次。 */
export function isValidNode(node: string): boolean {
  if (node !== node.trim() || /\s/.test(node)) return false;
  const match = /^([a-z]+):\/\/([^/?#]+)/.exec(node);
  if (!match || !NODE_SCHEMES.includes(match[1])) return false;
  const authority = match[2];
  if (authority.includes("@")) return false;
  const host = authority.startsWith("[")
    ? authority.slice(0, authority.indexOf("]") + 1)
    : authority.split(":")[0];
  return host.length > 0 && host !== "[]";
}
