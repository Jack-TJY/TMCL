// `skin_*` 命令封装（PLAN.md M2-07、M2-08）。图片一律经后端读取，界面不直接访问文件系统和网络。
import type { SkinAccountArgs } from "../bindings/SkinAccountArgs";
import type { SkinEntry } from "../bindings/SkinEntry";
import type { SkinFileArgs } from "../bindings/SkinFileArgs";
import type { SkinIdArgs } from "../bindings/SkinIdArgs";
import type { SkinImportArgs } from "../bindings/SkinImportArgs";
import type { SkinLibraryList } from "../bindings/SkinLibraryList";
import type { SkinProfile } from "../bindings/SkinProfile";
import type { SkinSetCapeArgs } from "../bindings/SkinSetCapeArgs";
import type { SkinSite } from "../bindings/SkinSite";
import type { SkinTextureArgs } from "../bindings/SkinTextureArgs";
import type { SkinUpdateArgs } from "../bindings/SkinUpdateArgs";
import type { SkinUploadArgs } from "../bindings/SkinUploadArgs";
import { invoke } from "./invoke";

export type { SkinEntry, SkinProfile };
export type { ProfileCape } from "../bindings/ProfileCape";
export type { ProfileSkin } from "../bindings/ProfileSkin";
export type { SkinVariant } from "../bindings/SkinVariant";

/** 本地皮肤库，最近加入的在前。 */
export async function skinLibraryList(): Promise<SkinEntry[]> {
  return (await invoke<SkinLibraryList>("skin_library_list")).skins;
}

/** 把选中的 PNG 加入皮肤库；同一张图片已在库中时返回已有条目。 */
export function skinLibraryImport(args: SkinImportArgs): Promise<SkinEntry> {
  return invoke<SkinEntry>("skin_library_import", { args });
}

/** 读取皮肤库中的图片。 */
export function skinLibraryRead(args: SkinIdArgs): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("skin_library_read", { args });
}

/** 修改名称与模型。 */
export function skinLibraryUpdate(args: SkinUpdateArgs): Promise<SkinEntry> {
  return invoke<SkinEntry>("skin_library_update", { args });
}

/** 从皮肤库删除。 */
export function skinLibraryRemove(args: SkinIdArgs): Promise<void> {
  return invoke<void>("skin_library_remove", { args });
}

/** 读取拖入的 PNG 供预览，不加入皮肤库。 */
export function skinFileRead(args: SkinFileArgs): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("skin_file_read", { args });
}

/** 下载档案中的皮肤或披风图片（`textures.minecraft.net`）。 */
export function skinTexture(args: SkinTextureArgs): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("skin_texture", { args });
}

/** 正版账号档案中的皮肤与披风。 */
export function skinProfile(args: SkinAccountArgs): Promise<SkinProfile> {
  return invoke<SkinProfile>("skin_profile", { args });
}

/** 把皮肤库中的皮肤上传到正版账号，返回新档案。 */
export function skinUpload(args: SkinUploadArgs): Promise<SkinProfile> {
  return invoke<SkinProfile>("skin_upload", { args });
}

/** 切换披风；省略 `cape_id` 时隐藏披风。返回新档案。 */
export function skinSetCape(args: SkinSetCapeArgs): Promise<SkinProfile> {
  return invoke<SkinProfile>("skin_set_cape", { args });
}

/** 在系统浏览器中打开外置登录账号的皮肤站。 */
export function skinOpenSite(args: SkinAccountArgs): Promise<SkinSite> {
  return invoke<SkinSite>("skin_open_site", { args });
}

/** 把后端返回的 PNG 转成供 3D 预览使用的 object URL；不用时调用 `URL.revokeObjectURL`。 */
export function pngObjectUrl(bytes: ArrayBuffer): string {
  return URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
}
