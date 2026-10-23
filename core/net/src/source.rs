//! 下载源与镜像：把官方 URL 改写为镜像 URL，并给出按优先级排列的候选地址。
//!
//! 下载项里始终记录官方 URL。下载时由 [`Sources::candidates`] 按策略给出
//! “首选镜像 → 官方源”的候选列表，前一个失败（HTTP 错误、连接失败、校验
//! 不一致等）就换下一个，见 `download.rs`。镜像只认领自己支持的地址，
//! 不支持的地址直接走官方源。

use std::fmt::Debug;
use std::sync::Arc;

use reqwest::Url;
use tmcl_common::config::DownloadConfig;

/// 官方源的名称，也是 `download.source` 的取值之一。
pub const OFFICIAL: &str = "official";
/// `download.source` 的默认取值：先默认镜像，后官方源。
pub const AUTO: &str = "auto";
/// BMCLAPI 镜像的名称。
pub const BMCLAPI: &str = "bmclapi";
/// 只镜像模组加载器（Forge、NeoForge、Fabric）的 BMCLAPI 镜像名称。
pub const BMCLAPI_LOADERS: &str = "bmclapi-loaders";

/// `auto` 策略使用的默认镜像：只让加载器走 BMCLAPI，Mojang 的文件走官方源。
///
/// 依据 `docs/research/mirrors.md` 的实测：国内直连时官方的 Mojang 内容最快
/// 最稳，而官方 Forge/Fabric maven 无法访问；BMCLAPI 的加载器文件稳定，
/// 但 Mojang 的小文件常超时。维护者于 2026-10-23 选定。
pub const DEFAULT_MIRROR: Option<&str> = Some(BMCLAPI_LOADERS);

const BMCLAPI_ROOT: &str = "https://bmclapi2.bangbang93.com";

/// 一个下载源。
pub trait DownloadSource: Debug + Send + Sync {
    /// 源的名称，即 `download.source` 中使用的值。
    fn name(&self) -> &str;

    /// 把官方 URL 改写为本源的 URL；本源不提供该文件时返回 `None`。
    fn rewrite(&self, url: &Url) -> Option<Url>;
}

/// 官方源：原样返回。
#[derive(Debug, Clone, Copy, Default)]
pub struct Official;

impl DownloadSource for Official {
    fn name(&self) -> &str {
        OFFICIAL
    }

    fn rewrite(&self, url: &Url) -> Option<Url> {
        Some(url.clone())
    }
}

/// 按 URL 前缀替换的镜像。规则按顺序匹配，第一条命中的生效。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixMirror {
    name: String,
    rules: Vec<(String, String)>,
}

impl PrefixMirror {
    /// 用 `(官方前缀, 镜像前缀)` 规则创建镜像。前缀不带结尾的 `/`。
    pub fn new<F, T>(name: impl Into<String>, rules: impl IntoIterator<Item = (F, T)>) -> Self
    where
        F: Into<String>,
        T: Into<String>,
    {
        Self {
            name: name.into(),
            rules: rules
                .into_iter()
                .map(|(from, to)| (from.into(), to.into()))
                .collect(),
        }
    }

    /// BMCLAPI（<https://bmclapi2.bangbang93.com>），规则取自其文档
    /// <https://bmclapidoc.bangbang93.com>。NeoForge 只镜像 `forge` 与
    /// `neoforge` 两个构件；Quilt 被文档标为暂不可用，未收录。
    pub fn bmclapi() -> Self {
        let root = BMCLAPI_ROOT;
        let mut rules = vec![
            ("https://piston-meta.mojang.com", root.to_owned()),
            ("https://piston-data.mojang.com", root.to_owned()),
            ("https://launchermeta.mojang.com", root.to_owned()),
            ("https://launcher.mojang.com", root.to_owned()),
            ("https://libraries.minecraft.net", format!("{root}/maven")),
            (
                "https://resources.download.minecraft.net",
                format!("{root}/assets"),
            ),
        ];
        rules.extend(loader_rules());
        Self::new(BMCLAPI, rules)
    }

    /// 只含 [`PrefixMirror::bmclapi`] 中模组加载器规则的镜像。
    pub fn bmclapi_loaders() -> Self {
        Self::new(BMCLAPI_LOADERS, loader_rules())
    }
}

/// BMCLAPI 中 Forge、NeoForge、Fabric 的改写规则。
fn loader_rules() -> Vec<(&'static str, String)> {
    let root = BMCLAPI_ROOT;
    vec![
        (
            "https://files.minecraftforge.net/maven",
            format!("{root}/maven"),
        ),
        ("https://maven.minecraftforge.net", format!("{root}/maven")),
        (
            "https://maven.neoforged.net/releases/net/neoforged/forge",
            format!("{root}/maven/net/neoforged/forge"),
        ),
        (
            "https://maven.neoforged.net/releases/net/neoforged/neoforge",
            format!("{root}/maven/net/neoforged/neoforge"),
        ),
        ("https://maven.fabricmc.net", format!("{root}/maven")),
        ("https://meta.fabricmc.net", format!("{root}/fabric-meta")),
    ]
}

impl DownloadSource for PrefixMirror {
    fn name(&self) -> &str {
        &self.name
    }

    fn rewrite(&self, url: &Url) -> Option<Url> {
        let url = url.as_str();
        self.rules.iter().find_map(|(from, to)| {
            let rest = url.strip_prefix(from.as_str())?;
            // 只在路径边界处匹配，避免 `https://a.net` 命中 `https://a.net.evil`。
            if !(rest.is_empty() || rest.starts_with(['/', '?'])) {
                return None;
            }
            Url::parse(&format!("{to}{rest}")).ok()
        })
    }
}

/// 一个候选下载地址。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// 下载源名称。
    pub source: String,
    /// 改写后的地址。
    pub url: String,
}

/// 已登记的镜像与 `download.source` 策略。
#[derive(Debug, Clone)]
pub struct Sources {
    policy: String,
    default_mirror: Option<String>,
    mirrors: Vec<Arc<dyn DownloadSource>>,
}

impl Default for Sources {
    /// `auto` 策略，登记内置镜像。
    fn default() -> Self {
        Self::new(AUTO)
    }
}

impl Sources {
    /// 用给定策略（`auto`、`official` 或镜像名）创建，登记内置镜像。
    pub fn new(policy: impl Into<String>) -> Self {
        Self {
            policy: policy.into(),
            default_mirror: DEFAULT_MIRROR.map(str::to_owned),
            mirrors: vec![
                Arc::new(PrefixMirror::bmclapi()),
                Arc::new(PrefixMirror::bmclapi_loaders()),
            ],
        }
    }

    /// 只用官方源。
    pub fn official_only() -> Self {
        Self::new(OFFICIAL)
    }

    /// 由 `config.json` 的 `download` 段创建。
    pub fn from_config(config: &DownloadConfig) -> Self {
        Self::new(config.source.clone())
    }

    /// 登记镜像；同名镜像被替换。
    pub fn add_mirror(&mut self, mirror: impl DownloadSource + 'static) {
        self.remove_mirror(mirror.name());
        self.mirrors.push(Arc::new(mirror));
    }

    /// 移除镜像，返回是否存在。
    pub fn remove_mirror(&mut self, name: &str) -> bool {
        let before = self.mirrors.len();
        self.mirrors.retain(|m| m.name() != name);
        self.mirrors.len() != before
    }

    /// 设置 `auto` 策略使用的默认镜像。
    pub fn set_default_mirror(&mut self, name: Option<&str>) {
        self.default_mirror = name.map(str::to_owned);
    }

    /// 已登记的镜像名称。
    pub fn mirror_names(&self) -> impl Iterator<Item = &str> {
        self.mirrors.iter().map(|m| m.name())
    }

    /// 当前策略下优先使用的镜像。策略为未知名称时按 `auto` 处理。
    fn preferred(&self) -> Option<&dyn DownloadSource> {
        let name = match self.policy.as_str() {
            OFFICIAL => return None,
            AUTO => self.default_mirror.as_deref()?,
            name => name,
        };
        let found = self.mirrors.iter().find(|m| m.name() == name);
        if found.is_none() && self.policy != AUTO {
            tracing::warn!(source = %self.policy, "未知的下载源，按 auto 处理");
            let fallback = self.default_mirror.as_deref()?;
            return self
                .mirrors
                .iter()
                .find(|m| m.name() == fallback)
                .map(|m| &**m);
        }
        found.map(|m| &**m)
    }

    /// 按尝试顺序给出 `url` 的候选地址：首选镜像（若它提供该文件）在前，
    /// 官方源总在最后。无法解析的 URL 只返回原地址。
    pub fn candidates(&self, url: &str) -> Vec<Candidate> {
        let official = Candidate {
            source: OFFICIAL.to_owned(),
            url: url.to_owned(),
        };
        let Ok(parsed) = Url::parse(url) else {
            return vec![official];
        };
        let mirrored = self
            .preferred()
            .and_then(|m| Some((m.name(), m.rewrite(&parsed)?)))
            .filter(|(_, rewritten)| rewritten.as_str() != url);
        match mirrored {
            Some((name, rewritten)) => vec![
                Candidate {
                    source: name.to_owned(),
                    url: rewritten.into(),
                },
                official,
            ],
            None => vec![official],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bmclapi(url: &str) -> Option<String> {
        PrefixMirror::bmclapi()
            .rewrite(&Url::parse(url).unwrap())
            .map(String::from)
    }

    #[test]
    fn bmclapi_rewrite_table() {
        let cases = [
            (
                "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
                "https://bmclapi2.bangbang93.com/mc/game/version_manifest_v2.json",
            ),
            (
                "https://piston-meta.mojang.com/v1/packages/abc/1.20.1.json",
                "https://bmclapi2.bangbang93.com/v1/packages/abc/1.20.1.json",
            ),
            (
                "https://launchermeta.mojang.com/v1/packages/abc/1.12.2.json",
                "https://bmclapi2.bangbang93.com/v1/packages/abc/1.12.2.json",
            ),
            (
                "https://piston-data.mojang.com/v1/objects/0c3e/client.jar",
                "https://bmclapi2.bangbang93.com/v1/objects/0c3e/client.jar",
            ),
            (
                "https://launcher.mojang.com/v1/objects/0f27/client.jar",
                "https://bmclapi2.bangbang93.com/v1/objects/0f27/client.jar",
            ),
            (
                "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
                "https://bmclapi2.bangbang93.com/maven/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
            ),
            (
                "https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                "https://bmclapi2.bangbang93.com/assets/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
            ),
            (
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar",
                "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-installer.jar",
            ),
            (
                "https://files.minecraftforge.net/maven/net/minecraftforge/forge/x.jar",
                "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/x.jar",
            ),
            (
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/20.4.1/x.jar",
                "https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge/20.4.1/x.jar",
            ),
            (
                "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar",
                "https://bmclapi2.bangbang93.com/maven/net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar",
            ),
            (
                "https://meta.fabricmc.net/v2/versions/loader",
                "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions/loader",
            ),
            (
                "https://meta.fabricmc.net/v2/versions/loader?x=1",
                "https://bmclapi2.bangbang93.com/fabric-meta/v2/versions/loader?x=1",
            ),
        ];
        for (from, to) in cases {
            assert_eq!(bmclapi(from).as_deref(), Some(to), "{from}");
        }
    }

    #[test]
    fn unsupported_hosts_are_not_rewritten() {
        assert_eq!(bmclapi("https://example.com/a.jar"), None);
        assert_eq!(bmclapi("https://maven.fabricmc.net.evil.com/a.jar"), None);
        assert_eq!(
            bmclapi("https://maven.neoforged.net/releases/net/neoforged/other/1/x.jar"),
            None
        );
        assert_eq!(bmclapi("https://meta.quiltmc.org/v3/versions"), None);
    }

    #[test]
    fn official_returns_input() {
        let url = Url::parse("https://libraries.minecraft.net/a.jar").unwrap();
        assert_eq!(Official.rewrite(&url), Some(url));
    }

    fn sources(urls: &[Candidate]) -> Vec<&str> {
        urls.iter().map(|c| c.source.as_str()).collect()
    }

    const LIB: &str = "https://libraries.minecraft.net/a/b/1/b-1.jar";

    const FORGE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/x.jar";

    #[test]
    fn auto_mirrors_only_loaders_by_default() {
        let s = Sources::new(AUTO);
        assert_eq!(sources(&s.candidates(LIB)), [OFFICIAL]);
        assert_eq!(
            sources(
                &s.candidates("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
            ),
            [OFFICIAL]
        );
        let c = s.candidates(FORGE);
        assert_eq!(sources(&c), [BMCLAPI_LOADERS, OFFICIAL]);
        assert_eq!(
            c[0].url,
            "https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/x.jar"
        );
    }

    #[test]
    fn loader_mirror_is_a_subset_of_bmclapi() {
        let full = PrefixMirror::bmclapi();
        let loaders = PrefixMirror::bmclapi_loaders();
        for (from, to) in &loaders.rules {
            assert!(full.rules.contains(&(from.clone(), to.clone())), "{from}");
        }
        assert_eq!(full.rules.len(), loaders.rules.len() + 6);
    }

    #[test]
    fn auto_without_default_mirror_is_official() {
        let mut s = Sources::new(AUTO);
        s.set_default_mirror(None);
        assert_eq!(sources(&s.candidates(FORGE)), [OFFICIAL]);
    }

    #[test]
    fn auto_uses_default_mirror_then_official() {
        let mut s = Sources::new(AUTO);
        s.set_default_mirror(Some(BMCLAPI));
        let c = s.candidates(LIB);
        assert_eq!(sources(&c), [BMCLAPI, OFFICIAL]);
        assert_eq!(
            c[0].url,
            "https://bmclapi2.bangbang93.com/maven/a/b/1/b-1.jar"
        );
        assert_eq!(c[1].url, LIB);
    }

    #[test]
    fn named_mirror_falls_back_to_official() {
        let s = Sources::new(BMCLAPI);
        assert_eq!(sources(&s.candidates(LIB)), [BMCLAPI, OFFICIAL]);
        // 镜像不提供的地址只走官方源。
        assert_eq!(sources(&s.candidates("https://example.com/x")), [OFFICIAL]);
    }

    #[test]
    fn official_policy_ignores_mirrors() {
        let mut s = Sources::official_only();
        s.set_default_mirror(Some(BMCLAPI));
        assert_eq!(sources(&s.candidates(LIB)), [OFFICIAL]);
    }

    #[test]
    fn unknown_policy_behaves_like_auto() {
        let mut s = Sources::new("nope");
        assert_eq!(sources(&s.candidates(LIB)), [OFFICIAL]);
        assert_eq!(sources(&s.candidates(FORGE)), [BMCLAPI_LOADERS, OFFICIAL]);
        s.set_default_mirror(Some(BMCLAPI));
        assert_eq!(sources(&s.candidates(LIB)), [BMCLAPI, OFFICIAL]);
    }

    #[test]
    fn mirrors_can_be_added_and_removed() {
        let mut s = Sources::new("local");
        s.add_mirror(PrefixMirror::new(
            "local",
            [("https://libraries.minecraft.net", "http://127.0.0.1:1/m")],
        ));
        assert_eq!(
            s.mirror_names().collect::<Vec<_>>(),
            [BMCLAPI, BMCLAPI_LOADERS, "local"]
        );
        assert_eq!(
            s.candidates(LIB)[0].url,
            "http://127.0.0.1:1/m/a/b/1/b-1.jar"
        );
        assert!(s.remove_mirror("local"));
        assert!(!s.remove_mirror("local"));
        assert_eq!(sources(&s.candidates(LIB)), [OFFICIAL]);
    }

    #[test]
    fn invalid_url_is_passed_through() {
        let s = Sources::new(BMCLAPI);
        assert_eq!(
            s.candidates("not a url"),
            [Candidate {
                source: OFFICIAL.to_owned(),
                url: "not a url".to_owned()
            }]
        );
    }
}
