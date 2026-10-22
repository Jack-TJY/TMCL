//! 构建期注入的配置。值由 `build.rs` 写入 `OUT_DIR`，见 AGENTS.md §11。

/// 构建时写入的原始值；未配置时为空字符串。
const MS_CLIENT_ID: &str = include_str!(concat!(env!("OUT_DIR"), "/ms_client_id"));

/// 微软登录使用的 Azure 应用 Client ID；构建时未配置则为 `None`。
///
/// 返回值不得写入日志或错误详情。
pub fn ms_client_id() -> Option<&'static str> {
    (!MS_CLIENT_ID.is_empty()).then_some(MS_CLIENT_ID)
}

/// 微软登录是否可用，即构建时是否注入了 Client ID。
pub fn ms_login_enabled() -> bool {
    ms_client_id().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_matches_client_id() {
        assert_eq!(ms_login_enabled(), ms_client_id().is_some());
    }

    #[test]
    fn injected_id_is_trimmed_guid() {
        if let Some(id) = ms_client_id() {
            assert_eq!(id.len(), 36);
            assert_eq!(id.trim(), id);
        }
    }
}
