// 本项目的 GitHub Releases 尚无后端命令（AGENTS.md §6.3）。界面不能直接联网，
// 需要在命令层追加一个读取 Releases 的命令后替换调用方并删除本文件；在此之前返回空列表。

/** 一个发布版本。 */
export interface ReleaseNote {
  /** 标签，例如 `v0.1.0-alpha.1`。 */
  tag: string;
  /** 标题。 */
  name: string;
  /** 发布时间（RFC 3339）。 */
  published_at: string;
  /** 是否为预发布。 */
  prerelease: boolean;
}

/** 最近的发布版本，新的在前。 */
export async function mockReleaseList(): Promise<ReleaseNote[]> {
  return [];
}
