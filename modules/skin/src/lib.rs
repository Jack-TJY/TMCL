//! tmcl-skin：皮肤与披风（PLAN.md M2-07）。
//!
//! 功能模块 crate，只依赖 `core/*`，不依赖 `tauri`，见 `docs/PLAN.md` 第 2 节。
//!
//! - [`mojang`]：正版账号的皮肤与披风（`api.minecraftservices.com`）。
//! - [`yggdrasil`]：外置登录账号的皮肤站地址。
//! - [`library`]：本地皮肤库 `<data_root>/skins/` 与 PNG 校验。
//! - [`texture`]：下载档案中的皮肤与披风图片（`textures.minecraft.net`）。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub mod library;
pub mod mojang;
pub mod texture;
pub mod yggdrasil;

pub use library::{SkinEntry, SkinLibrary, validate_png};
pub use mojang::{MojangClient, ProfileCape, ProfileSkin, SkinProfile};

/// 皮肤模型：`classic`（Steve，4 像素手臂）或 `slim`（Alex，3 像素手臂）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SkinVariant {
    Classic,
    Slim,
}

impl SkinVariant {
    /// 上传皮肤时 `variant` 字段的值。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Slim => "slim",
        }
    }
}
