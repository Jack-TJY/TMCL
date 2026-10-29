//! CurseForge API 客户端：`x-api-key`、`User-Agent`、重试、取消与错误码映射。
//!
//! 按 CurseForge 第三方 API 条款，响应只在内存中使用，不写入磁盘缓存。

use reqwest::header::{CONTENT_TYPE, HeaderValue, RETRY_AFTER};
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::{HttpClient, RetryPolicy};

use super::models::{
    ClassId, File, FingerprintMatches, MINECRAFT_GAME_ID, Mod, ModLoaderType, Page, SortField,
};
use crate::modrinth::USER_AGENT;
use crate::modrinth::client::{map_reqwest_error, parse_json, status_error, timeout_error};

/// 单个对象的响应：`{ data }`。
#[derive(serde::Deserialize)]
struct Data<T> {
    data: T,
}

/// CurseForge API v1 的地址。
pub const API_BASE: &str = "https://api.curseforge.com/v1";

/// 搜索每页最多 50 条（API 限制）。
const MAX_PAGE_SIZE: u32 = 50;

/// 搜索最多能翻到前 10000 条（`index + pageSize ≤ 10000`）。
const MAX_SEARCH_WINDOW: u32 = 10_000;

/// CurseForge API 客户端。克隆开销很小。
///
/// 超时、连接失败、429 与 5xx 按 [`RetryPolicy`] 重试，等待期间可被 `ctx` 取消。
/// API Key 只放在请求头里，不出现在 URL、日志与错误信息中。
#[derive(Clone)]
pub struct CurseForgeClient {
    inner: reqwest::Client,
    http: HttpClient,
    base: String,
    key: HeaderValue,
    retry: RetryPolicy,
}

impl std::fmt::Debug for CurseForgeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CurseForgeClient")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl CurseForgeClient {
    /// 使用构建期注入的 key 与官方地址创建客户端；没有 key 时返回 `None`（CurseForge 来源应隐藏）。
    pub fn from_build(http: &HttpClient) -> Option<AppResult<Self>> {
        super::api_key().map(|key| Self::with_base(http, API_BASE, key))
    }

    /// 使用指定的 API 地址（不带结尾 `/`）与 key，用于测试。
    pub fn with_base(http: &HttpClient, base: &str, key: &str) -> AppResult<Self> {
        let mut key = HeaderValue::from_str(key).map_err(|_| {
            AppError::new(ErrorCode::NetRequest).with_detail("invalid CurseForge API key")
        })?;
        key.set_sensitive(true);
        let timeouts = http.timeouts();
        let inner = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(timeouts.connect)
            .read_timeout(timeouts.read_idle)
            .build()
            .map_err(|e| AppError::new(ErrorCode::NetRequest).with_detail(e.to_string()))?;
        Ok(Self {
            inner,
            http: http.clone(),
            base: base.trim_end_matches('/').to_owned(),
            key,
            retry: RetryPolicy::default(),
        })
    }

    /// 替换重试策略（默认 1 s、2 s、4 s）。
    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// 搜索项目（`GET /mods/search`）。
    pub async fn search(&self, query: &SearchQuery, ctx: &TaskCtx) -> AppResult<Page<Mod>> {
        let url = self.url(&["mods", "search"], &query.params())?;
        self.get_json(url, ctx).await
    }

    /// 按 ID 获取项目（`GET /mods/{id}`），不存在时返回 `None`。
    pub async fn project(&self, id: u32, ctx: &TaskCtx) -> AppResult<Option<Mod>> {
        let url = self.url(&["mods", &id.to_string()], &[])?;
        Ok(self
            .get_optional::<Data<Mod>>(url, ctx)
            .await?
            .map(|d| d.data))
    }

    /// 批量获取项目（`POST /mods`），不存在的 ID 会被忽略。
    pub async fn projects(&self, ids: &[u32], ctx: &TaskCtx) -> AppResult<Vec<Mod>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "modIds": ids, "filterPcOnly": true });
        let data: Data<Vec<Mod>> = self.post_json(&["mods"], &body, ctx).await?;
        Ok(data.data)
    }

    /// 列出项目的文件（`GET /mods/{id}/files`），按发布时间从新到旧；项目不存在时返回 `NET_HTTP_STATUS`。
    pub async fn project_files(
        &self,
        id: u32,
        filter: &FileFilter,
        ctx: &TaskCtx,
    ) -> AppResult<Page<File>> {
        let url = self.url(&["mods", &id.to_string(), "files"], &filter.params())?;
        self.get_json(url, ctx).await
    }

    /// 获取单个文件（`GET /mods/{modId}/files/{fileId}`），不存在时返回 `None`。
    pub async fn file(&self, mod_id: u32, file_id: u32, ctx: &TaskCtx) -> AppResult<Option<File>> {
        let url = self.url(
            &["mods", &mod_id.to_string(), "files", &file_id.to_string()],
            &[],
        )?;
        Ok(self
            .get_optional::<Data<File>>(url, ctx)
            .await?
            .map(|d| d.data))
    }

    /// 批量获取文件（`POST /mods/files`），不存在的 ID 会被忽略。
    pub async fn files(&self, ids: &[u32], ctx: &TaskCtx) -> AppResult<Vec<File>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "fileIds": ids });
        let data: Data<Vec<File>> = self.post_json(&["mods", "files"], &body, ctx).await?;
        Ok(data.data)
    }

    /// 按指纹查找文件（`POST /fingerprints/432`），指纹见 [`super::fingerprint`]。
    pub async fn match_fingerprints(
        &self,
        fingerprints: &[u32],
        ctx: &TaskCtx,
    ) -> AppResult<FingerprintMatches> {
        if fingerprints.is_empty() {
            return Ok(FingerprintMatches::default());
        }
        let body = serde_json::json!({ "fingerprints": fingerprints });
        let game = MINECRAFT_GAME_ID.to_string();
        let data: Data<FingerprintMatches> =
            self.post_json(&["fingerprints", &game], &body, ctx).await?;
        Ok(data.data)
    }

    /// 原样取回 `GET <base>/<segments...>` 的 JSON，用于录制测试数据与排查问题。
    pub async fn get_value(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
        ctx: &TaskCtx,
    ) -> AppResult<serde_json::Value> {
        let url = self.url(segments, query)?;
        self.get_json(url, ctx).await
    }

    /// 原样取回 `POST <base>/<segments...>` 的 JSON，见 [`Self::get_value`]。
    pub async fn post_value(
        &self,
        segments: &[&str],
        body: &serde_json::Value,
        ctx: &TaskCtx,
    ) -> AppResult<serde_json::Value> {
        self.post_json(segments, body, ctx).await
    }

    /// 拼出 `<base>/<segments...>?<query>`，路径段会被百分号编码。
    fn url(&self, segments: &[&str], query: &[(&str, String)]) -> AppResult<Url> {
        let mut url = Url::parse(&self.base).map_err(|e| {
            AppError::new(ErrorCode::NetRequest)
                .with_param("url", &self.base)
                .with_detail(e.to_string())
        })?;
        url.path_segments_mut()
            .map_err(|()| AppError::new(ErrorCode::NetRequest).with_param("url", &self.base))?
            .pop_if_empty()
            .extend(segments);
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        Ok(url)
    }

    async fn get_json<T: DeserializeOwned>(&self, url: Url, ctx: &TaskCtx) -> AppResult<T> {
        match self.get_optional(url.clone(), ctx).await? {
            Some(value) => Ok(value),
            None => Err(status_error(url.as_str(), StatusCode::NOT_FOUND)),
        }
    }

    async fn get_optional<T: DeserializeOwned>(
        &self,
        url: Url,
        ctx: &TaskCtx,
    ) -> AppResult<Option<T>> {
        let Some(bytes) = self.request(Method::GET, &url, None, ctx).await? else {
            return Ok(None);
        };
        parse_json(&bytes, url.as_str()).map(Some)
    }

    async fn post_json<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        body: &serde_json::Value,
        ctx: &TaskCtx,
    ) -> AppResult<T> {
        let url = self.url(segments, &[])?;
        let body = body.to_string().into_bytes();
        match self.request(Method::POST, &url, Some(&body), ctx).await? {
            Some(bytes) => parse_json(&bytes, url.as_str()),
            None => Err(status_error(url.as_str(), StatusCode::NOT_FOUND)),
        }
    }

    /// 发送请求并读完响应体；404 返回 `None`，其余非 2xx 返回 `NET_HTTP_STATUS`。
    async fn request(
        &self,
        method: Method,
        url: &Url,
        body: Option<&[u8]>,
        ctx: &TaskCtx,
    ) -> AppResult<Option<Vec<u8>>> {
        self.retry
            .run(ctx, |_| self.attempt(method.clone(), url, body, ctx))
            .await
    }

    async fn attempt(
        &self,
        method: Method,
        url: &Url,
        body: Option<&[u8]>,
        ctx: &TaskCtx,
    ) -> AppResult<Option<Vec<u8>>> {
        let mut request = self
            .inner
            .request(method, url.clone())
            .header("x-api-key", self.key.clone())
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        let timeouts = self.http.timeouts();
        let url = url.as_str();
        let fetch = async {
            let response = tokio::time::timeout(timeouts.response, request.send())
                .await
                .map_err(|_| timeout_error(url, "等待响应头", timeouts.response))?
                .map_err(|e| map_reqwest_error(url, &e))?;
            match response.status() {
                status if status.is_success() => {
                    self.http.body(response, url).bytes(ctx).await.map(Some)
                }
                StatusCode::NOT_FOUND => Ok(None),
                StatusCode::TOO_MANY_REQUESTS => {
                    let retry_after = response.headers().get(RETRY_AFTER).cloned();
                    tracing::debug!(url, ?retry_after, "CurseForge 速率限制，稍后重试");
                    Err(status_error(url, StatusCode::TOO_MANY_REQUESTS))
                }
                status => Err(status_error(url, status)),
            }
        };
        tokio::select! {
            biased;
            _ = ctx.cancel.cancelled() => Err(AppError::cancelled()),
            r = tokio::time::timeout(timeouts.total, fetch) => {
                r.map_err(|_| timeout_error(url, "整个请求", timeouts.total))?
            }
        }
    }
}

/// 搜索条件，空字段表示不限。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// 关键词，可为空。
    pub query: String,
    /// 顶级类别，`None` 表示所有类别。
    pub class_id: Option<ClassId>,
    /// 游戏版本。
    pub game_version: Option<String>,
    /// 加载器。
    pub loader: Option<ModLoaderType>,
    /// 排序字段，降序。
    pub sort: SortField,
    /// 跳过的条数，用于翻页。
    pub offset: u32,
    /// 每页条数，0 表示 20，超过 50 按 50 处理。
    pub limit: u32,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            class_id: None,
            game_version: None,
            loader: None,
            sort: SortField::Popularity,
            offset: 0,
            limit: 0,
        }
    }
}

impl SearchQuery {
    /// 按关键词搜索。
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![("gameId", MINECRAFT_GAME_ID.to_string())];
        if let Some(class) = self.class_id {
            params.push(("classId", class.as_i64().to_string()));
        }
        if !self.query.trim().is_empty() {
            params.push(("searchFilter", self.query.trim().to_owned()));
        }
        if let Some(version) = &self.game_version {
            params.push(("gameVersion", version.clone()));
        }
        if let Some(loader) = self.loader {
            params.push(("modLoaderType", loader.as_i64().to_string()));
        }
        params.push(("sortField", self.sort.as_i64().to_string()));
        params.push(("sortOrder", "desc".to_owned()));
        let offset = self.offset.min(MAX_SEARCH_WINDOW - 1);
        let limit = match self.limit {
            0 => 20,
            n => n.min(MAX_PAGE_SIZE),
        }
        .min(MAX_SEARCH_WINDOW - offset);
        if offset > 0 {
            params.push(("index", offset.to_string()));
        }
        params.push(("pageSize", limit.to_string()));
        params
    }
}

/// 列出项目文件时的过滤条件，空字段表示不限。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFilter {
    /// 游戏版本。
    pub game_version: Option<String>,
    /// 加载器。
    pub loader: Option<ModLoaderType>,
    /// 跳过的条数。
    pub offset: u32,
    /// 每页条数，0 表示 API 默认值（50），超过 50 按 50 处理。
    pub limit: u32,
}

impl FileFilter {
    /// 按单个游戏版本与加载器名称（`fabric` 等，原版传 `""`）过滤。
    pub fn for_instance(game_version: &str, loader: &str) -> Self {
        Self {
            game_version: Some(game_version.to_owned()),
            loader: ModLoaderType::from_loader_str(loader),
            ..Self::default()
        }
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if let Some(version) = &self.game_version {
            params.push(("gameVersion", version.clone()));
        }
        if let Some(loader) = self.loader {
            params.push(("modLoaderType", loader.as_i64().to_string()));
        }
        if self.offset > 0 {
            params.push(("index", self.offset.to_string()));
        }
        if self.limit > 0 {
            params.push(("pageSize", self.limit.min(MAX_PAGE_SIZE).to_string()));
        }
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_params_include_game_and_clamp_window() {
        let query = SearchQuery {
            class_id: Some(ClassId::Mods),
            game_version: Some("1.20.1".into()),
            loader: Some(ModLoaderType::Fabric),
            limit: 500,
            ..SearchQuery::new(" jei ")
        };
        assert_eq!(
            query.params(),
            vec![
                ("gameId", "432".to_owned()),
                ("classId", "6".to_owned()),
                ("searchFilter", "jei".to_owned()),
                ("gameVersion", "1.20.1".to_owned()),
                ("modLoaderType", "4".to_owned()),
                ("sortField", "2".to_owned()),
                ("sortOrder", "desc".to_owned()),
                ("pageSize", "50".to_owned()),
            ]
        );
        let deep = SearchQuery {
            offset: 9_990,
            limit: 50,
            ..SearchQuery::default()
        };
        let params = deep.params();
        assert!(params.contains(&("index", "9990".to_owned())));
        assert!(params.contains(&("pageSize", "10".to_owned())));
    }

    #[test]
    fn debug_output_hides_key() {
        let client =
            CurseForgeClient::with_base(&HttpClient::new().unwrap(), API_BASE, "test-api-key")
                .unwrap();
        assert!(!format!("{client:?}").contains("test-api-key"));
    }
}
