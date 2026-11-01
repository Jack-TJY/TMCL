// `java_*` 命令封装。
import type { JavaList } from "../bindings/JavaList";
import { invoke } from "./invoke";

/** 探测本机 Java：启动器下载的运行时（`managed`）在前，其余按主版本从高到低。 */
export function javaList(): Promise<JavaList> {
  return invoke<JavaList>("java_list");
}
