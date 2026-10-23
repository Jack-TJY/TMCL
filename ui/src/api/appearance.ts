// `appearance_*` 命令封装：导入与读取自定义背景图片。
import type { AppearanceImported } from "../bindings/AppearanceImported";
import { invoke } from "./invoke";

/** 背景图片支持的格式，供文件对话框筛选。 */
export const BACKGROUND_EXTENSIONS = ["png", "jpg", "jpeg", "webp"];

/** 把用户选中的图片复制到数据目录，返回写入 `appearance.background.image` 的文件名。 */
export function appearanceImportBackground(path: string): Promise<AppearanceImported> {
  return invoke<AppearanceImported>("appearance_import_background", { args: { path } });
}

/** 读取背景图片内容。 */
export function appearanceReadBackground(image: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("appearance_read_background", { args: { image } });
}
