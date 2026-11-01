//! API 响应的 gzip 压缩。
//!
//! 工作区的 `reqwest` 没有启用 `gzip` feature（启用后对所有客户端生效，包括按原始字节校验
//! 大小与续传的下载引擎），所以 Modrinth / CurseForge 客户端自己请求 gzip 并解压。
//! Modrinth 整合包的版本列表未压缩时有几 MB（每个版本都带整包的依赖列表），压缩后约为十分之一。

use std::io::Read;

use reqwest::header::{CONTENT_ENCODING, HeaderMap};
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::HttpClient;

/// 请求头 `Accept-Encoding` 的值。
pub(crate) const ACCEPT: &str = "gzip";

/// 读完响应体；响应声明了 `Content-Encoding: gzip` 时解压。解压失败返回 `NET_REQUEST`。
pub(crate) async fn read_body(
    http: &HttpClient,
    response: reqwest::Response,
    url: &str,
    ctx: &TaskCtx,
) -> AppResult<Vec<u8>> {
    let gzip = is_gzip(response.headers());
    let bytes = http.body(response, url).bytes(ctx).await?;
    if gzip { decode(&bytes, url) } else { Ok(bytes) }
}

fn is_gzip(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("gzip"))
}

fn decode(bytes: &[u8], url: &str) -> AppResult<Vec<u8>> {
    let mut out = Vec::with_capacity(bytes.len() * 8);
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut out)
        .map_err(|e| {
            AppError::new(ErrorCode::NetRequest)
                .with_param("url", url)
                .with_detail(format!("解压 gzip 响应失败：{e}"))
        })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use reqwest::header::HeaderValue;

    use super::*;

    #[test]
    fn decodes_gzip_and_rejects_garbage() {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(b"[1,2,3]").unwrap();
        let packed = encoder.finish().unwrap();
        assert_eq!(decode(&packed, "u").unwrap(), b"[1,2,3]");
        let err = decode(b"not gzip", "u").unwrap_err();
        assert_eq!(err.code, ErrorCode::NetRequest);
    }

    #[test]
    fn only_gzip_encoding_is_decoded() {
        let mut headers = HeaderMap::new();
        assert!(!is_gzip(&headers));
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("GZIP"));
        assert!(is_gzip(&headers));
        headers.insert(CONTENT_ENCODING, HeaderValue::from_static("br"));
        assert!(!is_gzip(&headers));
    }
}
