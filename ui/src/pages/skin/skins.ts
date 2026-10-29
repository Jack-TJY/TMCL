// 皮肤页的纯逻辑：头像裁切与拖入文件筛选。

/** 用 CSS 背景从皮肤图片中裁出 8×8 的脸部与帽子层，作为皮肤库的缩略图。 */
export function headStyle(url: string, height: number): Record<string, string> {
  // 64×64 与旧版 64×32 皮肤的脸都在 (8, 8)，帽子层在 (40, 8)。
  const rows = height / 8;
  const y = `${(8 / (height - 8)) * 100}%`;
  return {
    backgroundImage: `url("${url}"), url("${url}")`,
    backgroundSize: `800% ${rows * 100}%`,
    backgroundPosition: `${(40 / 56) * 100}% ${y}, ${(8 / 56) * 100}% ${y}`,
  };
}

/** 拖入的路径中第一个 PNG；没有时为 `null`。 */
export function firstPng(paths: readonly string[]): string | null {
  return paths.find((p) => p.toLowerCase().endsWith(".png")) ?? null;
}

/** 路径中的文件名（不含扩展名），兼容 Windows 与 Unix 分隔符。 */
export function fileStem(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  return name.replace(/\.[^.]*$/, "");
}
