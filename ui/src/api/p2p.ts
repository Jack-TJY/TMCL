// `p2p_*` 命令封装。
import type { P2pInfo } from "../bindings/P2pInfo";
import { invoke } from "./invoke";

/** EasyTier 版本与内置默认公共节点。 */
export function p2pInfo(): Promise<P2pInfo> {
  return invoke<P2pInfo>("p2p_info");
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
