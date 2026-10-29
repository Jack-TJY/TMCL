//! Modrinth API 客户端：`User-Agent`、速率限制、重试、取消与错误码映射。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use reqwest::header::{CONTENT_TYPE, HeaderMap, RETRY_AFTER};
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use tmcl_common::{AppError, AppResult, ErrorCode, TaskCtx};
use tmcl_net::{HttpClient, RetryPolicy};

use super::models::{
    HashAlgorithm, Project, ProjectDependencies, ProjectType, SearchIndex, SearchResponse, Version,
};

/// Modrinth API v2 的地址。
pub const API_BASE: &str = "https://api.modrinth.com/v2";

/// Modrinth 要求能识别出应用与维护者的 `User-Agent`：`<GitHub 用户名>/TMCL/<版本>`。
pub const USER_AGENT: &str = concat!("Jack-TJY/TMCL/", env!("CARGO_PKG_VERSION"));

/// 单次因速率限制等待的上限；Modrinth 的窗口是 60 秒。
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

/// 搜索每页最多 100 条（API 限制）。
const MAX_SEARCH_LIMIT: u32 = 100;

/// Modrinth API 客户端，克隆后共享同一个速率限制状态。
///
/// 速率限制：每个响应都会读取 `X-Ratelimit-Remaining` / `X-Ratelimit-Reset`，额度用完时下一个
/// 请求先等到窗口重置；收到 429 时按响应头（或 `Retry-After`）等待后重试。
/// 超时、连接失败、429 与 5xx 按 [`RetryPolicy`] 重试，等待期间都可被 `ctx` 取消。
#[derive(Debug, Clone)]
pub struct ModrinthClient {
    inner: reqwest::Client,
    http: HttpClient,
    base: String,
    retry: RetryPolicy,
    limiter: Arc<RateLimiter>,
}

impl ModrinthClient {
    /// 使用官方地址创建客户端，超时设置沿用 `http`。
    pub fn new(http: &HttpClient) -> AppResult<Self> {
        Self::with_base(http, API_BASE)
    }

    /// 使用指定的 API 地址（不带结尾 `/`），用于测试。
    pub fn with_base(http: &HttpClient, base: &str) -> AppResult<Self> {
        let timeouts = http.timeouts();
        // `tmcl_net::HttpClient` 固定使用 `TMCL/<版本>`，这里单独建一个带 Modrinth 要求的
        // `User-Agent` 的连接池；响应体仍交给 `HttpClient::body` 按同样的超时读取。
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
            retry: RetryPolicy::default(),
            limiter: Arc::new(RateLimiter::default()),
        })
    }

    /// 替换重试策略（默认 1 s、2 s、4 s）。
    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// 搜索项目（`GET /search`）。
    pub async fn search(&self, query: &SearchQuery, ctx: &TaskCtx) -> AppResult<SearchResponse> {
        let url = self.url(&["search"], &query.params())?;
        self.get_json(url, ctx).await
    }

    /// 按 ID 或短名获取项目（`GET /project/{id}`），不存在时返回 `None`。
    pub async fn project(&self, id_or_slug: &str, ctx: &TaskCtx) -> AppResult<Option<Project>> {
        let url = self.url(&["project", id_or_slug], &[])?;
        self.get_optional(url, ctx).await
    }

    /// 批量获取项目（`GET /projects`），不存在的 ID 会被忽略。
    pub async fn projects(&self, ids: &[String], ctx: &TaskCtx) -> AppResult<Vec<Project>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let url = self.url(&["projects"], &[("ids", json_array(ids))])?;
        self.get_json(url, ctx).await
    }

    /// 列出项目的版本（`GET /project/{id}/version`），按发布时间从新到旧。
    pub async fn project_versions(
        &self,
        id_or_slug: &str,
        filter: &VersionFilter,
        ctx: &TaskCtx,
    ) -> AppResult<Vec<Version>> {
        let url = self.url(&["project", id_or_slug, "version"], &filter.params())?;
        self.get_json(url, ctx).await
    }

    /// 按 ID 获取版本（`GET /version/{id}`），不存在时返回 `None`。
    pub async fn version(&self, id: &str, ctx: &TaskCtx) -> AppResult<Option<Version>> {
        let url = self.url(&["version", id], &[])?;
        self.get_optional(url, ctx).await
    }

    /// 批量获取版本（`GET /versions`），不存在的 ID 会被忽略。
    pub async fn versions(&self, ids: &[String], ctx: &TaskCtx) -> AppResult<Vec<Version>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let url = self.url(&["versions"], &[("ids", json_array(ids))])?;
        self.get_json(url, ctx).await
    }

    /// 项目所有版本依赖到的项目与指定版本（`GET /project/{id}/dependencies`）。
    pub async fn project_dependencies(
        &self,
        id_or_slug: &str,
        ctx: &TaskCtx,
    ) -> AppResult<ProjectDependencies> {
        let url = self.url(&["project", id_or_slug, "dependencies"], &[])?;
        self.get_json(url, ctx).await
    }

    /// 按文件哈希查找所属版本（`GET /version_file/{hash}`），不是 Modrinth 上的文件时返回 `None`。
    pub async fn version_from_hash(
        &self,
        hash: &str,
        algorithm: HashAlgorithm,
        ctx: &TaskCtx,
    ) -> AppResult<Option<Version>> {
        let url = self.url(
            &["version_file", hash],
            &[("algorithm", algorithm.as_api_str().to_owned())],
        )?;
        self.get_optional(url, ctx).await
    }

    /// 批量按文件哈希查找所属版本（`POST /version_files`），返回“哈希 → 版本”，
    /// 找不到的哈希不出现在结果中。用于识别已安装的资源。
    pub async fn versions_from_hashes(
        &self,
        hashes: &[String],
        algorithm: HashAlgorithm,
        ctx: &TaskCtx,
    ) -> AppResult<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({
            "hashes": hashes,
            "algorithm": algorithm.as_api_str(),
        });
        self.post_json(&["version_files"], &body, ctx).await
    }

    /// 批量检查更新（`POST /version_files/update`）：对每个文件哈希，返回其项目中符合
    /// `filter` 的最新版本。结果版本与原文件相同即为已是最新；找不到的哈希不出现在结果中。
    ///
    /// Modrinth 的这个接口不支持 `featured` 过滤，会被忽略。
    pub async fn latest_versions_from_hashes(
        &self,
        hashes: &[String],
        algorithm: HashAlgorithm,
        filter: &VersionFilter,
        ctx: &TaskCtx,
    ) -> AppResult<HashMap<String, Version>> {
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({
            "hashes": hashes,
            "algorithm": algorithm.as_api_str(),
            "loaders": filter.loaders,
            "game_versions": filter.game_versions,
        });
        self.post_json(&["version_files", "update"], &body, ctx)
            .await
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
        self.limiter.acquire(ctx).await?;
        let mut request = self.inner.request(method, url.clone());
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
            self.limiter.update(response.headers());
            match response.status() {
                status if status.is_success() => {
                    self.http.body(response, url).bytes(ctx).await.map(Some)
                }
                StatusCode::NOT_FOUND => Ok(None),
                StatusCode::TOO_MANY_REQUESTS => {
                    let wait = self.limiter.exhaust(response.headers());
                    tracing::debug!(url, ?wait, "Modrinth 速率限制，等待后重试");
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

/// 搜索条件。同一字段内的多个取值是“或”关系（`categories` 除外，是“且”）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery {
    /// 关键词，可为空。
    pub query: String,
    /// 项目类型。
    pub project_type: Option<ProjectType>,
    /// 游戏版本，满足任一即可。
    pub game_versions: Vec<String>,
    /// 加载器（如 `fabric`），满足任一即可。
    pub loaders: Vec<String>,
    /// 分类（如 `optimization`），需全部满足。
    pub categories: Vec<String>,
    /// 排序方式。
    pub index: SearchIndex,
    /// 跳过的条数，用于翻页。
    pub offset: u32,
    /// 每页条数，0 表示使用 API 默认值（10），超过 100 按 100 处理。
    pub limit: u32,
}

impl SearchQuery {
    /// 按关键词搜索。
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    /// `facets` 参数：外层数组为“且”，内层为“或”。
    pub fn facets(&self) -> Vec<Vec<String>> {
        let mut facets = Vec::new();
        if let Some(kind) = self.project_type.and_then(ProjectType::as_api_str) {
            facets.push(vec![format!("project_type:{kind}")]);
        }
        let any_of = |prefix: &str, values: &[String]| -> Option<Vec<String>> {
            (!values.is_empty()).then(|| values.iter().map(|v| format!("{prefix}:{v}")).collect())
        };
        facets.extend(any_of("versions", &self.game_versions));
        facets.extend(any_of("categories", &self.loaders));
        facets.extend(
            self.categories
                .iter()
                .map(|c| vec![format!("categories:{c}")]),
        );
        facets
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if !self.query.is_empty() {
            params.push(("query", self.query.clone()));
        }
        let facets = self.facets();
        if !facets.is_empty() {
            params.push(("facets", serde_json::json!(facets).to_string()));
        }
        params.push(("index", self.index.as_api_str().to_owned()));
        if self.offset > 0 {
            params.push(("offset", self.offset.to_string()));
        }
        if self.limit > 0 {
            params.push(("limit", self.limit.min(MAX_SEARCH_LIMIT).to_string()));
        }
        params
    }
}

/// 列出项目版本时的过滤条件，空字段表示不限。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionFilter {
    /// 加载器，满足任一即可。
    pub loaders: Vec<String>,
    /// 游戏版本，满足任一即可。
    pub game_versions: Vec<String>,
    /// 只要精选（`true`）或只要非精选（`false`）的版本。
    pub featured: Option<bool>,
}

impl VersionFilter {
    /// 按单个游戏版本与加载器过滤，例如 `("1.20.1", "fabric")`。
    pub fn for_instance(game_version: &str, loader: &str) -> Self {
        Self {
            loaders: vec![loader.to_owned()],
            game_versions: vec![game_version.to_owned()],
            featured: None,
        }
    }

    fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if !self.loaders.is_empty() {
            params.push(("loaders", json_array(&self.loaders)));
        }
        if !self.game_versions.is_empty() {
            params.push(("game_versions", json_array(&self.game_versions)));
        }
        if let Some(featured) = self.featured {
            params.push(("featured", featured.to_string()));
        }
        params
    }
}

/// 进程内的速率限制状态，由同一客户端的所有克隆共享。
#[derive(Debug, Default)]
struct RateLimiter {
    state: Mutex<LimitState>,
}

#[derive(Debug, Default)]
struct LimitState {
    /// 当前窗口剩余的请求数，未知时为 `None`。
    remaining: Option<u64>,
    /// 当前窗口的重置时刻。
    reset_at: Option<Instant>,
}

impl RateLimiter {
    /// 额度用完时等到窗口重置；可被取消。
    async fn acquire(&self, ctx: &TaskCtx) -> AppResult<()> {
        while let Some(wait) = self.reserve() {
            tracing::debug!(?wait, "Modrinth 请求额度已用完，等待窗口重置");
            tokio::select! {
                biased;
                _ = ctx.cancel.cancelled() => return Err(AppError::cancelled()),
                _ = tokio::time::sleep(wait) => {}
            }
        }
        Ok(())
    }

    /// 占用一个额度；没有额度时返回需要等待的时间。
    fn reserve(&self) -> Option<Duration> {
        let mut state = lock(&self.state);
        let now = Instant::now();
        match state.reset_at {
            Some(reset_at) if reset_at > now => match state.remaining {
                Some(0) => return Some(reset_at - now),
                Some(n) => state.remaining = Some(n - 1),
                None => {}
            },
            _ => *state = LimitState::default(),
        }
        None
    }

    /// 按响应头更新剩余额度与重置时刻。
    fn update(&self, headers: &HeaderMap) {
        let remaining = header_u64(headers, "x-ratelimit-remaining");
        let reset = header_u64(headers, "x-ratelimit-reset");
        if let (Some(remaining), Some(reset)) = (remaining, reset) {
            let reset = Duration::from_secs(reset).min(MAX_RATE_LIMIT_WAIT);
            *lock(&self.state) = LimitState {
                remaining: Some(remaining),
                reset_at: Some(Instant::now() + reset),
            };
        }
    }

    /// 收到 429：清空额度，按 `X-Ratelimit-Reset` 或 `Retry-After`（至少 1 秒）设定重置时刻。
    fn exhaust(&self, headers: &HeaderMap) -> Duration {
        let wait = header_u64(headers, "x-ratelimit-reset")
            .or_else(|| header_u64(headers, RETRY_AFTER.as_str()))
            .map_or(Duration::from_secs(1), Duration::from_secs)
            .clamp(Duration::from_secs(1), MAX_RATE_LIMIT_WAIT);
        *lock(&self.state) = LimitState {
            remaining: Some(0),
            reset_at: Some(Instant::now() + wait),
        };
        wait
    }
}

fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.trim().parse().ok()
}

fn json_array(values: &[String]) -> String {
    serde_json::json!(values).to_string()
}

pub(crate) fn parse_json<T: DeserializeOwned>(bytes: &[u8], url: &str) -> AppResult<T> {
    serde_json::from_slice(bytes).map_err(|e| {
        AppError::new(ErrorCode::CommonJson)
            .with_param("url", url)
            .with_detail(e.to_string())
    })
}

pub(crate) fn status_error(url: &str, status: StatusCode) -> AppError {
    AppError::new(ErrorCode::NetHttpStatus)
        .with_param("url", url)
        .with_param("status", status.as_u16())
}

pub(crate) fn timeout_error(url: &str, stage: &str, limit: Duration) -> AppError {
    AppError::new(ErrorCode::NetTimeout)
        .with_param("url", url)
        .with_detail(format!("{stage}超过 {} 秒", limit.as_secs_f64()))
}

/// 与 `tmcl_net` 相同的 `reqwest` 错误映射。
pub(crate) fn map_reqwest_error(url: &str, err: &reqwest::Error) -> AppError {
    let code = if err.is_timeout() {
        ErrorCode::NetTimeout
    } else if err.is_connect() {
        ErrorCode::NetConnect
    } else {
        ErrorCode::NetRequest
    };
    AppError::new(code)
        .with_param("url", url)
        .with_detail(err.to_string())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_facets_combine_or_within_and_across_fields() {
        let query = SearchQuery {
            project_type: Some(ProjectType::Mod),
            game_versions: vec!["1.20.1".into(), "1.20.2".into()],
            loaders: vec!["fabric".into()],
            categories: vec!["optimization".into(), "utility".into()],
            ..SearchQuery::new("sodium")
        };
        assert_eq!(
            serde_json::json!(query.facets()).to_string(),
            r#"[["project_type:mod"],["versions:1.20.1","versions:1.20.2"],["categories:fabric"],["categories:optimization"],["categories:utility"]]"#
        );
    }

    #[test]
    fn search_params_skip_defaults_and_clamp_limit() {
        let params = SearchQuery::default().params();
        assert_eq!(params, vec![("index", "relevance".to_owned())]);
        let query = SearchQuery {
            offset: 20,
            limit: 500,
            index: SearchIndex::Downloads,
            ..SearchQuery::new("a")
        };
        assert_eq!(
            query.params(),
            vec![
                ("query", "a".to_owned()),
                ("index", "downloads".to_owned()),
                ("offset", "20".to_owned()),
                ("limit", "100".to_owned()),
            ]
        );
    }

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().unwrap());
        }
        map
    }

    #[test]
    fn limiter_waits_only_when_quota_is_used_up() {
        let limiter = RateLimiter::default();
        assert_eq!(limiter.reserve(), None);
        limiter.update(&headers(&[
            ("x-ratelimit-remaining", "1"),
            ("x-ratelimit-reset", "30"),
        ]));
        assert_eq!(limiter.reserve(), None);
        let wait = limiter.reserve().expect("额度已用完");
        assert!(wait > Duration::from_secs(29) && wait <= Duration::from_secs(30));
    }

    #[test]
    fn limiter_forgets_quota_after_reset() {
        let limiter = RateLimiter::default();
        limiter.update(&headers(&[
            ("x-ratelimit-remaining", "0"),
            ("x-ratelimit-reset", "0"),
        ]));
        assert_eq!(limiter.reserve(), None);
    }

    #[test]
    fn exhaust_prefers_ratelimit_reset_then_retry_after() {
        let limiter = RateLimiter::default();
        let reset = headers(&[("x-ratelimit-reset", "7"), ("retry-after", "3")]);
        assert_eq!(limiter.exhaust(&reset), Duration::from_secs(7));
        assert_eq!(
            limiter.exhaust(&headers(&[("retry-after", "3")])),
            Duration::from_secs(3)
        );
        assert_eq!(limiter.exhaust(&HeaderMap::new()), Duration::from_secs(1));
        assert_eq!(
            limiter.exhaust(&headers(&[("x-ratelimit-reset", "3600")])),
            MAX_RATE_LIMIT_WAIT
        );
        assert!(limiter.reserve().is_some());
    }
}
