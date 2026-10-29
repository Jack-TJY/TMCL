//! 读取 `textures.minecraft.net` 上的皮肤与披风图片，供界面预览。
//!
//! 界面不直接访问网络（AGENTS.md §4），档案里的图片地址经这里下载。只接受
//! `http(s)://textures.minecraft.net/texture/<十六进制>` 形式的地址，一律改用 HTTPS 请求，
//! 不会被当作任意地址的代理。

use reqwest::Url;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;

use crate::library::{MAX_SKIN_BYTES, PNG_SIGNATURE};

/// 材质服务器的根地址。
pub const TEXTURES_BASE: &str = "https://textures.minecraft.net";
/// 材质服务器的主机名。
const TEXTURES_HOST: &str = "textures.minecraft.net";

/// 从档案中的图片地址取出材质哈希；不是材质服务器的地址时返回 `COMMON_INTERNAL`（`params.url`）。
pub fn texture_hash(url: &str) -> AppResult<String> {
    let invalid = || AppError::new(ErrorCode::CommonInternal).with_param("url", url);
    let parsed = Url::parse(url).map_err(|_| invalid())?;
    let valid_origin = matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str() == Some(TEXTURES_HOST)
        && parsed.port().is_none()
        && parsed.query().is_none();
    let hash = parsed
        .path()
        .strip_prefix("/texture/")
        .filter(|h| !h.is_empty() && h.len() <= 128 && h.bytes().all(|b| b.is_ascii_hexdigit()));
    match hash {
        Some(hash) if valid_origin => Ok(hash.to_ascii_lowercase()),
        _ => Err(invalid()),
    }
}

/// 下载材质图片：`base` 为材质服务器根地址（测试中指向本地服务器）。
///
/// 地址不合法返回 `COMMON_INTERNAL`；内容不是 PNG 或超过 1 MB 返回 `SKIN_INVALID_IMAGE`；
/// 其余 `NET_*`。
pub async fn fetch_texture(
    http: &HttpClient,
    base: &str,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<u8>> {
    let hash = texture_hash(url)?;
    let target = format!("{}/texture/{hash}", base.trim_end_matches('/'));
    let bytes = http.get_bytes(&target, ctx).await?;
    if bytes.len() > MAX_SKIN_BYTES || !bytes.starts_with(PNG_SIGNATURE) {
        return Err(AppError::new(ErrorCode::SkinInvalidImage).with_param("url", url));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "1a4af718455d4aab528e7a61f86fa25e6a369d1768dcb13f7df319a713eb810b";

    #[test]
    fn accepts_profile_texture_urls() {
        for url in [
            format!("http://textures.minecraft.net/texture/{HASH}"),
            format!(
                "https://textures.minecraft.net/texture/{}",
                HASH.to_uppercase()
            ),
        ] {
            assert_eq!(texture_hash(&url).unwrap(), HASH);
        }
    }

    #[test]
    fn rejects_other_urls() {
        for url in [
            "not a url".to_owned(),
            format!("https://evil.example/texture/{HASH}"),
            format!("https://textures.minecraft.net.evil.example/texture/{HASH}"),
            format!("https://textures.minecraft.net:8443/texture/{HASH}"),
            format!("file://textures.minecraft.net/texture/{HASH}"),
            format!("https://textures.minecraft.net/texture/{HASH}?x=1"),
            format!("https://textures.minecraft.net/other/{HASH}"),
            "https://textures.minecraft.net/texture/".to_owned(),
            "https://textures.minecraft.net/texture/../../etc".to_owned(),
            "https://textures.minecraft.net/texture/abc/def".to_owned(),
        ] {
            let err = texture_hash(&url).unwrap_err();
            assert_eq!(err.code, ErrorCode::CommonInternal, "{url}");
        }
    }
}
