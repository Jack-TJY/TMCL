//! 外置登录（authlib-injector）：登录并保存账号、启动前刷新令牌、
//! 下载并校验 `authlib-injector.jar`、生成启动用的 JVM 参数。

use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tmcl_common::{AppError, AppResult, Db, ErrorCode, TaskCtx, write_atomic};
use tmcl_net::HttpClient;
use uuid::Uuid;

use crate::accounts::{self, Account, AccountKind, AuthlibServer, NewAccount};
use crate::secrets::SecretStore;
use crate::yggdrasil::{Session, Yggdrasil, resolve_server};

/// authlib-injector 官方发布索引。
pub const ARTIFACT_URL: &str = "https://authlib-injector.yushi.moe/artifact/latest.json";
/// BMCLAPI 镜像，官方索引不可用时使用。
pub const ARTIFACT_MIRROR_URL: &str =
    "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json";

/// 发布索引中的一个版本。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Artifact {
    pub version: String,
    pub download_url: String,
    pub checksums: Checksums,
}

/// 发布文件的校验和。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Checksums {
    pub sha256: String,
}

/// 用户在外置登录弹窗中输入的内容。密码只用于这一次登录，不保存。
#[derive(Clone, Copy)]
pub struct LoginRequest<'a> {
    /// 皮肤站首页或 API 地址。
    pub server: &'a str,
    pub username: &'a str,
    pub password: &'a str,
}

/// 外置登录：读取皮肤站信息、登录、令牌写入密钥库，账号设为默认。
pub async fn login(
    db: &Db,
    secrets: &dyn SecretStore,
    http: &HttpClient,
    ygg: &Yggdrasil,
    request: LoginRequest<'_>,
    ctx: &TaskCtx,
) -> AppResult<Account> {
    let server = resolve_server(http, request.server, ctx).await?;
    let session = ygg
        .authenticate(&server.api_url, request.username, request.password, ctx)
        .await?;
    save(
        db,
        secrets,
        &session,
        AuthlibServer {
            api_url: server.api_url,
            name: server.name,
        },
    )
}

fn save(
    db: &Db,
    secrets: &dyn SecretStore,
    session: &Session,
    server: AuthlibServer,
) -> AppResult<Account> {
    let uuid = Uuid::parse_str(&session.profile.id).map_err(|_| {
        AppError::new(ErrorCode::AuthAuthlibServerInvalid).with_param("url", &server.api_url)
    })?;
    let secret = serde_json::to_string(session)
        .map_err(|e| AppError::new(ErrorCode::CommonJson).with_detail(e.to_string()))?;
    accounts::upsert(
        db,
        secrets,
        NewAccount {
            kind: AccountKind::Authlib,
            name: session.profile.name.clone(),
            uuid,
            server: Some(server),
        },
        Some(&secret),
    )
}

/// 启动外置登录账号所需的信息。
#[derive(Debug, Clone)]
pub struct AuthlibLaunch {
    /// 当前有效的会话（令牌已按需刷新并写回密钥库）。
    pub session: Session,
    /// 皮肤站元数据原文，用于 `prefetched` 参数。
    pub metadata: String,
}

/// 启动前准备：读取令牌，失效时刷新并写回，再读取最新的皮肤站元数据。
/// 密钥库中没有令牌或刷新失败时返回 `AUTH_YGGDRASIL_SESSION_EXPIRED`，需重新登录。
pub async fn prepare_launch(
    secrets: &dyn SecretStore,
    http: &HttpClient,
    ygg: &Yggdrasil,
    account: &Account,
    ctx: &TaskCtx,
) -> AppResult<AuthlibLaunch> {
    let expired =
        || AppError::new(ErrorCode::AuthYggdrasilSessionExpired).with_param("name", &account.name);
    let server = account.server.as_ref().ok_or_else(expired)?;
    let stored = accounts::secret(secrets, &account.id)?.ok_or_else(expired)?;
    let session: Session = serde_json::from_str(&stored).map_err(|_| expired())?;
    let (changed, session) = ygg.ensure_valid(&server.api_url, session, ctx).await?;
    if changed {
        let secret = serde_json::to_string(&session)
            .map_err(|e| AppError::new(ErrorCode::CommonJson).with_detail(e.to_string()))?;
        accounts::set_secret(secrets, &account.id, &secret)?;
    }
    let metadata = resolve_server(http, &server.api_url, ctx).await?.metadata;
    Ok(AuthlibLaunch { session, metadata })
}

/// 确保 `dir` 下有最新的 `authlib-injector-<版本>.jar` 并校验 SHA-256，返回其路径。
/// 依次尝试 `index_urls`（通常是 [`ARTIFACT_URL`]、[`ARTIFACT_MIRROR_URL`]）；
/// 全部失败但本地已有旧版本时使用旧版本。
pub async fn ensure_injector(
    http: &HttpClient,
    dir: &Path,
    index_urls: &[&str],
    ctx: &TaskCtx,
) -> AppResult<PathBuf> {
    let mut last_err = None;
    for index in index_urls {
        match fetch_injector(http, dir, index, ctx).await {
            Ok(path) => return Ok(path),
            Err(e) if e.is_cancelled() => return Err(e),
            Err(e) => last_err = Some(e),
        }
    }
    if let Some(existing) = newest_local(dir) {
        tracing::warn!(
            "authlib-injector 索引不可用，使用本地版本 {}",
            existing.display()
        );
        return Ok(existing);
    }
    Err(last_err.unwrap_or_else(|| AppError::new(ErrorCode::CommonInternal)))
}

async fn fetch_injector(
    http: &HttpClient,
    dir: &Path,
    index: &str,
    ctx: &TaskCtx,
) -> AppResult<PathBuf> {
    let artifact: Artifact = http.get_json(index, ctx).await?;
    let expected = artifact.checksums.sha256.to_ascii_lowercase();
    // 版本号只用于文件名，去掉可能的路径字符。
    let version: String = artifact
        .version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect();
    let path = dir.join(format!("authlib-injector-{version}.jar"));
    if let Ok(bytes) = std::fs::read(&path)
        && sha256_hex(&bytes) == expected
    {
        return Ok(path);
    }
    let bytes = http.get_bytes(&artifact.download_url, ctx).await?;
    if sha256_hex(&bytes) != expected {
        return Err(
            AppError::new(ErrorCode::NetHashMismatch).with_param("url", &artifact.download_url)
        );
    }
    std::fs::create_dir_all(dir)?;
    write_atomic(&path, &bytes)?;
    Ok(path)
}

fn newest_local(dir: &Path) -> Option<PathBuf> {
    let mut jars: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("authlib-injector-") && n.ends_with(".jar"))
        })
        .collect();
    jars.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    jars.pop()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// 启动参数：`-javaagent:<jar>=<api>` 与 `-Dauthlibinjector.yggdrasil.prefetched=<base64 元数据>`。
/// 需放在主类之前的 JVM 参数中。
pub fn jvm_args(jar: &Path, api_url: &str, metadata: &str) -> Vec<String> {
    vec![
        format!("-javaagent:{}={api_url}", jar.display()),
        format!(
            "-Dauthlibinjector.yggdrasil.prefetched={}",
            base64(metadata.as_bytes())
        ),
    ]
}

/// 标准 Base64（带填充）。
fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(base64(input.as_bytes()), expected, "{input}");
        }
    }

    #[test]
    fn jvm_args_point_agent_at_api() {
        let args = jvm_args(
            Path::new("/data/cache/authlib-injector-1.2.5.jar"),
            "https://a.test/api/yggdrasil",
            "{}",
        );
        assert_eq!(
            args,
            vec![
                format!(
                    "-javaagent:{}=https://a.test/api/yggdrasil",
                    Path::new("/data/cache/authlib-injector-1.2.5.jar").display()
                ),
                "-Dauthlibinjector.yggdrasil.prefetched=e30=".to_owned(),
            ]
        );
    }
}
