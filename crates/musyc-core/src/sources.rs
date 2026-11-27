//! 自定义源（sources/*.toml）的加载与校验。
//!
//! 扫描顺序（同 id 后者覆盖前者）：
//! 1. 仓库内置源目录（编译时 `CARGO_MANIFEST_DIR` 定位 `<repo>/sources`，存在才用）
//! 2. `Paths::sources_dir`（`MUSYC_ROOT/sources` 或 `~/.local/share/musyc/sources`；
//!    `install.sh` 会把仓库内置源复制到这里，便携/删源码后仍可用）
//! 3. `~/.config/musyc/sources`（用户自定义，永远生效）
//!
//! TOML 顶层字段：
//! - `id`（必填，唯一；内置源保留：netease / streetvoice / indie）
//! - `name`（必填，选择器里显示的名字）
//! - `desc`（可选，选择器里的简短描述）
//! - `kind`（必填）：`netease-playlist` / `streetvoice-cat` / `jamendo-playlist` / `fma-charts` / `generic-json`
//! - 其余字段按 kind 使用：
//!   - `netease-playlist`：`playlist_id`（或 `url`，自动提取 `id=`）
//!   - `streetvoice-cat`：`cat`（数字字符串，对应 `/music/browse/<cat>/recommend/latest/`，`''` = 推荐）
//!   - `jamendo-playlist`：`playlist_id`（或 `url`），`client_id`（可选，默认 VLC 公开 key `3dce8b55`），`limit`（可选，默认 50）
//!   - `fma-charts`：`url`（默认 <https://freemusicarchive.org/music/charts/this-week>）
//!   - `generic-json`：`url`，`list_path`（点路径/方括号，如 `results[0].tracks`；留空 = 根即数组或根含常见字段），
//!     `id_path` / `name_path` / `artists_path` / `album_path` / `duration_path` / `duration_unit`(`ms`|`s`) / `cover_path` / `play_url_path` / `lyric_url_path`（都按点路径）

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// 内置源 id 常量，`s` 弹选择器时内置三项永远排在自定义项之前。
pub const BUILTIN_NETEASE: &str = "netease";
pub const BUILTIN_STREETVOICE: &str = "streetvoice";
pub const BUILTIN_INDIE: &str = "indie";

/// 我以为的内置项：`Source` 视图，供 UI 直接展示与「排序」用。
#[derive(Debug, Clone)]
pub struct BuiltinSource {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

pub const BUILTIN_SOURCES: [BuiltinSource; 3] = [
    BuiltinSource {
        id: BUILTIN_NETEASE,
        name: "网易原创",
        desc: "网易原创榜 · 网易云歌单 2884035",
    },
    BuiltinSource {
        id: BUILTIN_STREETVOICE,
        name: "街声推荐",
        desc: "街声推荐 30 首 · 本机 API",
    },
    BuiltinSource {
        id: BUILTIN_INDIE,
        name: "雀乐推荐",
        desc: "雀乐最新 10 期",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    NeteasePlaylist,
    StreetVoiceCat,
    JamendoPlaylist,
    FmaCharts,
    GenericJson,
    /// Audius 热门榜（免费 Open API，无需登录）。
    AudiusTrending,
}

impl SourceKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "netease-playlist" | "netease" => Some(Self::NeteasePlaylist),
            "streetvoice-cat" | "streetvoice" => Some(Self::StreetVoiceCat),
            "jamendo-playlist" | "jamendo" => Some(Self::JamendoPlaylist),
            "fma-charts" | "fma" => Some(Self::FmaCharts),
            "generic-json" | "generic" => Some(Self::GenericJson),
            "audius-trending" | "audius" => Some(Self::AudiusTrending),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NeteasePlaylist => "netease-playlist",
            Self::StreetVoiceCat => "streetvoice-cat",
            Self::JamendoPlaylist => "jamendo-playlist",
            Self::FmaCharts => "fma-charts",
            Self::GenericJson => "generic-json",
            Self::AudiusTrending => "audius-trending",
        }
    }
}

/// 一个源文件的原始 TOML 表示（宽松解析，kind 再决定校验哪些字段）。
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct SourceFile {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub kind: String,
    /// 通用 URL（fma-charts / generic-json 用；netease-playlist / jamendo-playlist 也接受，用来提取 id）。
    pub url: String,
    /// 网易 / Jamendo 歌单 id。
    pub playlist_id: String,
    /// 街声分类 id（对应 /music/browse/<cat>/…；空 = 推荐）。
    pub cat: String,
    /// Jamendo client_id；留空用 VLC 公开 key。
    pub client_id: String,
    /// Jamendo 返回条数，默认 50。
    pub limit: Option<u32>,
    /// Audius 时间范围：`week`（默认）/ `month` / `allTime`。
    pub time: String,
    // generic-json 字段映射（点路径）。
    pub list_path: String,
    pub id_path: String,
    pub name_path: String,
    pub artists_path: String,
    pub album_path: String,
    pub duration_path: String,
    /// `ms`（默认）或 `s`。
    pub duration_unit: String,
    pub cover_path: String,
    pub play_url_path: String,
    pub lyric_url_path: String,
}

/// 校验并规整后的源。
#[derive(Debug, Clone)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub kind: SourceKind,
    pub file: PathBuf,
    /// 按 kind 规整后的字段。
    pub playlist_id: String,
    pub cat: String,
    pub client_id: String,
    pub limit: u32,
    pub time: String,
    pub url: String,
    pub list_path: String,
    pub id_path: String,
    pub name_path: String,
    pub artists_path: String,
    pub album_path: String,
    pub duration_path: String,
    pub duration_unit: DurationUnit,
    pub cover_path: String,
    pub play_url_path: String,
    pub lyric_url_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DurationUnit {
    #[default]
    Milliseconds,
    Seconds,
}

impl DurationUnit {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "s" | "sec" | "seconds" => Self::Seconds,
            _ => Self::Milliseconds,
        }
    }
}

impl Source {
    /// `jamendo-playlist` 没填 client_id 时用 VLC 公开的那把 key。
    pub const JAMENDO_PUBLIC_CLIENT_ID: &'static str = "3dce8b55";

    fn validate(raw: SourceFile, file: &Path) -> Result<Self> {
        let id = raw.id.trim().to_string();
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(Error::Config(format!(
                "{}: id 只能用小写字母 / 数字 / - / _（当前 {:?}）",
                file.display(),
                raw.id
            )));
        }
        let name = raw.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::Config(format!("{}: 缺少 name", file.display())));
        }
        let kind = SourceKind::parse(&raw.kind).ok_or_else(|| {
            Error::Config(format!(
                "{}: 未知 kind {:?}（支持 netease-playlist / streetvoice-cat / jamendo-playlist / fma-charts / generic-json）",
                file.display(),
                raw.kind
            ))
        })?;

        let url = raw.url.trim().to_string();
        let mut playlist_id = raw.playlist_id.trim().to_string();
        if playlist_id.is_empty() && !url.is_empty() {
            // 从 url 里提取 id= 或 /playlist/<id>/
            playlist_id = extract_id(&url).unwrap_or_default();
        }
        let client_id = {
            let trimmed = raw.client_id.trim();
            if trimmed.is_empty() {
                Self::JAMENDO_PUBLIC_CLIENT_ID.to_string()
            } else {
                trimmed.to_string()
            }
        };
        let limit = raw.limit.unwrap_or(50).clamp(1, 200);
        let time = {
            let trimmed = raw.time.trim();
            match trimmed {
                "month" | "allTime" | "week" => trimmed.to_string(),
                "" => "week".to_string(),
                other => {
                    return Err(Error::Config(format!(
                        "{}: time 只能是 week / month / allTime（当前 {other:?}）",
                        file.display()
                    )));
                }
            }
        };
        match kind {
            SourceKind::NeteasePlaylist => {
                if playlist_id.is_empty() {
                    return Err(Error::Config(format!(
                        "{}: netease-playlist 需要 playlist_id（或 url 里带 ?id=）",
                        file.display()
                    )));
                }
            }
            SourceKind::JamendoPlaylist => {
                if playlist_id.is_empty() {
                    return Err(Error::Config(format!(
                        "{}: jamendo-playlist 需要 playlist_id（或 url 里带 /playlist/<id>/）",
                        file.display()
                    )));
                }
            }
            SourceKind::GenericJson => {
                if url.is_empty() {
                    return Err(Error::Config(format!(
                        "{}: generic-json 需要 url",
                        file.display()
                    )));
                }
            }
            SourceKind::FmaCharts | SourceKind::StreetVoiceCat | SourceKind::AudiusTrending => {}
        }
        let fma_url = if matches!(kind, SourceKind::FmaCharts) && url.is_empty() {
            "https://freemusicarchive.org/music/charts/this-week".to_string()
        } else {
            url
        };

        Ok(Self {
            id,
            name,
            desc: raw.desc.trim().to_string(),
            kind,
            file: file.to_path_buf(),
            playlist_id,
            cat: raw.cat.trim().to_string(),
            client_id,
            limit,
            time,
            url: fma_url,
            list_path: raw.list_path.trim().to_string(),
            id_path: raw.id_path.trim().to_string(),
            name_path: raw.name_path.trim().to_string(),
            artists_path: raw.artists_path.trim().to_string(),
            album_path: raw.album_path.trim().to_string(),
            duration_path: raw.duration_path.trim().to_string(),
            duration_unit: DurationUnit::parse(&raw.duration_unit),
            cover_path: raw.cover_path.trim().to_string(),
            play_url_path: raw.play_url_path.trim().to_string(),
            lyric_url_path: raw.lyric_url_path.trim().to_string(),
        })
    }
}

/// `https://x/playlist/12345/slug` 或 `...?id=12345` → `12345`。
fn extract_id(url: &str) -> Option<String> {
    if let Some(idx) = url.find("id=") {
        let rest = &url[idx + 3..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() {
            return Some(digits);
        }
    }
    let segments: Vec<&str> = url.split('/').collect();
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "playlist"
            && let Some(next) = segments.get(i + 1)
        {
            let digits: String = next.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() {
                return Some(digits);
            }
        }
    }
    None
}

/// 按扫描顺序加载全部源；同 id 后者覆盖。返回值按 id 排序（稳定、可预测）。
pub fn load_sources(dirs: &[PathBuf]) -> Vec<Source> {
    let mut by_id: HashMap<String, Source> = HashMap::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
            .collect();
        paths.sort();
        for path in paths {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                tracing::warn!(path = ?path, "读取源文件失败，跳过");
                continue;
            };
            match toml::from_str::<SourceFile>(&raw) {
                Ok(file) => match Source::validate(file, &path) {
                    Ok(source) => {
                        by_id.insert(source.id.clone(), source);
                    }
                    Err(e) => tracing::warn!(%e, "源文件校验失败，跳过"),
                },
                Err(e) => tracing::warn!(path = ?path, %e, "源文件解析失败，跳过"),
            }
        }
    }
    let mut out: Vec<Source> = by_id.into_values().collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// 仓库内置源目录：编译时定位（CARGO_MANIFEST_DIR 为 musyc-core 的 crate 目录）。
fn builtin_dir() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("sources"))
        .filter(|p| p.is_dir())
}

/// 便携版：返回子目录下全部 TOML；用于把安装后的二进制定位到随包分发的 sources/。
pub fn discover(paths: &crate::paths::Paths) -> Vec<Source> {
    let mut dirs = vec![paths.sources_dir.clone(), paths.user_sources_dir.clone()];
    if let Some(builtin) = builtin_dir() {
        // 内置源放最前：随仓库分发的源优先被扫描，同 id 仍可由用户目录覆盖
        dirs.insert(0, builtin);
    }
    load_sources(&dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> Source {
        let file: SourceFile = toml::from_str(toml_src).expect("toml");
        Source::validate(file, Path::new("/tmp/source.toml")).expect("validate")
    }

    #[test]
    fn extracts_id_from_url_query() {
        let s = parse(
            r#"id = "netease-fma"
name = "测试"
kind = "netease-playlist"
url = "https://music.163.com/#/playlist?id=5059642708"
"#,
        );
        assert_eq!(s.playlist_id, "5059642708");
    }

    #[test]
    fn extracts_id_from_jamendo_url() {
        let s = parse(
            r#"id = "jamendo-ssw"
name = "测试"
kind = "jamendo-playlist"
url = "https://www.jamendo.com/playlist/500608899/singer-songwriter"
"#,
        );
        assert_eq!(s.playlist_id, "500608899");
        assert_eq!(s.client_id, Source::JAMENDO_PUBLIC_CLIENT_ID);
    }

    #[test]
    fn fma_defaults_to_this_week() {
        let s = parse(
            r#"id = "fma-this-week"
name = "FMA"
kind = "fma-charts"
"#,
        );
        assert_eq!(s.url, "https://freemusicarchive.org/music/charts/this-week");
    }

    #[test]
    fn rejects_bad_id() {
        let file: SourceFile = toml::from_str(
            r#"id = "Bad Id"
name = "x"
kind = "fma-charts"
"#,
        )
        .unwrap();
        assert!(Source::validate(file, Path::new("/tmp/s.toml")).is_err());
    }
}
