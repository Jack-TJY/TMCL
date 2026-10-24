// 第三方许可证数据（scripts/gen-licenses.mjs 生成的 third-party-licenses.json）的类型与筛选。

/** 一个随安装包分发的第三方包。 */
export interface ThirdPartyPackage {
  name: string;
  /** 版本号；同名包有多个版本时用 `, ` 连接。 */
  version: string;
  /** `cargo`：Rust crate；`npm`：前端依赖。 */
  source: "cargo" | "npm";
  /** SPDX 许可证表达式。 */
  license: string;
  /** 仓库或主页地址。 */
  url: string;
  /** 许可证原文在 `LicenseData.texts` 中的下标；包没有附带原文时为空。 */
  texts: number[];
}

/** 生成文件的结构。 */
export interface LicenseData {
  /** 去重后的许可证原文。 */
  texts: string[];
  packages: ThirdPartyPackage[];
}

/** 按包名或许可证表达式筛选（不区分大小写，空查询返回全部）。 */
export function filterPackages(packages: ThirdPartyPackage[], query: string): ThirdPartyPackage[] {
  const q = query.trim().toLowerCase();
  if (!q) return packages;
  return packages.filter(
    (p) => p.name.toLowerCase().includes(q) || p.license.toLowerCase().includes(q),
  );
}
