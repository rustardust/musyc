//! 歌单来源的统一入口。
//!
//! 首页槽位、自定义源、Audius 以及将来的「本地歌单」都只通过
//! [`load_playlist`] 取回 `Vec<Song>`，UI 不再关心每个来源怎么解析。
//! 新增来源 = 在 `sources/*.toml` 里加一行 kind，或在此加一个
//! [`PlaylistRequest`] 分支。

use musyc_core::model::Song;
use musyc_core::sources::{Source, SourceKind as Kind};
use musyc_core::{Error, Result};

use crate::ApiClient;
use crate::source_loader;

/// 一次歌单加载请求。
pub enum PlaylistRequest<'a> {
    /// 内置来源（netease / streetvoice / indie）。
    Builtin(&'a str),
    /// `sources/*.toml` 定义的来源。
    Custom(&'a Source),
}

/// 按请求取回一个歌单的全部歌曲。
pub async fn load_playlist(
    request: PlaylistRequest<'_>,
    api: &ApiClient,
    http: &reqwest::Client,
) -> Result<Vec<Song>> {
    match request {
        PlaylistRequest::Builtin(id) => load_builtin(id, api).await,
        PlaylistRequest::Custom(source) => match source.kind {
            // 网易歌单：复用官方直链接口。
            Kind::NeteasePlaylist => api.playlist_tracks(&source.playlist_id, 1000, 0).await,
            // 街声分类：走本机转发（HLS → MP3）。
            Kind::StreetVoiceCat => {
                let cat = source.cat.trim();
                if cat.is_empty() || cat == "recommend" {
                    api.streetvoice_recommend().await
                } else {
                    api.streetvoice_cat(cat).await
                }
            }
            // 其余来源（Jamendo / FMA / generic-json / Audius）统一在
            // source_loader 里按 kind 分派。
            _ => source_loader::load(source, http).await,
        },
    }
}

async fn load_builtin(id: &str, api: &ApiClient) -> Result<Vec<Song>> {
    use musyc_core::sources::{BUILTIN_INDIE, BUILTIN_NETEASE, BUILTIN_STREETVOICE};
    match id {
        BUILTIN_NETEASE => api.original_chart(30).await,
        BUILTIN_STREETVOICE => api.streetvoice_recommend().await,
        BUILTIN_INDIE => {
            // 雀乐：目录里最新一期的歌曲，避免把十期扁平成超长队列。
            let catalog = api.indie_catalog().await?;
            Ok(catalog
                .into_iter()
                .next()
                .map(|p| p.songs)
                .unwrap_or_default())
        }
        other => Err(Error::Api(format!("未知内置来源：{other}"))),
    }
}
