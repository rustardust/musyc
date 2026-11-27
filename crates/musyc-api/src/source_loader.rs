//! 自定义源（sources/*.toml）的运行时加载：按 kind 拉取、探测并映射成 `Song`。
//!
//! 所有在线源都在后台任务里用同一个 `Source` / `Song` 视图，避免 TUI 直接关心解析细节。

use std::time::Duration;

use musyc_core::model::{Song, SourceKind};
use musyc_core::source_fetch::{as_array, as_string, get_path, to_millis};
use musyc_core::sources::{Source, SourceKind as Kind};
use musyc_core::{Error, Result};
use serde_json::Value;

mod audius;
mod fma;
mod json;

use audius::*;
use fma::*;
use json::*;

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const LIST_TIMEOUT: Duration = Duration::from_secs(20);
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/131.0 Safari/537.36 musyc/0.1";

#[derive(Debug, Clone, Default)]
pub struct LoadedSource {
    pub source_id: String,
    pub name: String,
    pub desc: String,
    pub songs: Vec<Song>,
}

/// 按 kind 拉取源数据并组装 Song 列表；网络错误直接返回给调用方。
pub async fn load(source: &Source, http: &reqwest::Client) -> Result<Vec<Song>> {
    let songs = match source.kind {
        Kind::NeteasePlaylist => {
            return Err(Error::Api(
                "netease-playlist 源走 ApiClient 快速通道".into(),
            ));
        }
        Kind::StreetVoiceCat => {
            return Err(Error::Api("streetvoice-cat 源走本机 API 快速通道".into()));
        }
        Kind::JamendoPlaylist => fetch_jamendo(source, http).await?,
        Kind::FmaCharts => fetch_fma(source, http).await?,
        Kind::GenericJson => fetch_generic_json(source, http).await?,
        Kind::AudiusTrending => fetch_audius(source, http).await?,
    };
    trace_source(source, &songs);
    Ok(songs)
}

fn trace_source(source: &Source, songs: &[Song]) {
    tracing::info!(source = %source.id, kind = %source.kind.as_str(), count = songs.len(), "自定义源加载完成");
}

/// 统一组装：设置来源 id + 降级封面/直链，默认副标题 "name (desc)"。
pub fn build(source: &Source, mut songs: Vec<Song>) -> Vec<Song> {
    for song in &mut songs {
        if song.source_id.is_none() {
            song.source_id = Some(source.id.clone());
        }
        if song.artists.is_empty() {
            song.artists.push("未知歌手".into());
        }
        if song.name.trim().is_empty() {
            song.name = "未命名歌曲".into();
        }
        song.fee = 0;
    }
    // 同一列表里出现重复 id 时保留第一条（= 上游返回顺序的前者）。
    let mut seen = std::collections::HashSet::new();
    songs.retain(|s| seen.insert(s.id.clone()));
    songs
}

/// 探活：HEAD / Range 首包判断直链是否真的能播。
/// 只要求 2xx / 206，失败的源（如 fileUrl 返回 403/404）在列表里直接剔除。
async fn probe_url(http: &reqwest::Client, url: &str) -> bool {
    // 先按 HEAD 探；个别文件服务器不支持 HEAD，再退到 Range GET 首字节。
    if let Ok(resp) = http.head(url).timeout(PROBE_TIMEOUT).send().await {
        if resp.status().is_success() || resp.status().as_u16() == 206 {
            return true;
        }
        // 405 / 501：服务器不支持 HEAD，走 Range GET
        if resp.status().as_u16() != 405 && resp.status().as_u16() != 501 {
            return false;
        }
    }
    http.get(url)
        .header("Range", "bytes=0-0")
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .map(|r| r.status().is_success() || r.status().as_u16() == 206)
        .unwrap_or(false)
}

/// 并发跑一批探活，最多 10 个并发，结果与入参顺序对齐。
async fn probe_all(http: &reqwest::Client, urls: &[String]) -> Vec<bool> {
    if urls.is_empty() {
        return Vec::new();
    }
    let mut out = vec![false; urls.len()];
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));
    let mut set = tokio::task::JoinSet::new();
    for (index, url) in urls.iter().cloned().enumerate() {
        let http = http.clone();
        let permit = semaphore.clone();
        set.spawn(async move {
            // 拿到名额再发请求，真正限制并发数
            let _permit = permit.acquire().await;
            let ok = probe_url(&http, &url).await;
            (index, ok)
        });
    }
    while let Some(Ok((index, ok))) = set.join_next().await {
        out[index] = ok;
    }
    out
}

async fn fetch_jamendo(source: &Source, http: &reqwest::Client) -> Result<Vec<Song>> {
    let url = format!(
        "https://api.jamendo.com/v3.0/playlists/tracks/?client_id={}&id={}&track_type=single+albumtrack&format=json&limit={}",
        urlencode(&source.client_id),
        urlencode(&source.playlist_id),
        source.limit
    );
    let json = get_json(http, &url, &[]).await?;
    if json
        .pointer("/headers/status")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.eq_ignore_ascii_case("success"))
    {
        return Err(Error::Api(format!(
            "Jamendo 返回 {:?}",
            json.pointer("/headers/status")
        )));
    }
    let tracks = json
        .pointer("/results/0/tracks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    build_jamendo(source, tracks, http).await
}

async fn build_jamendo(
    source: &Source,
    tracks: Vec<Value>,
    http: &reqwest::Client,
) -> Result<Vec<Song>> {
    let mut out: Vec<Song> = Vec::with_capacity(tracks.len());
    for track in &tracks {
        let Some(id) = as_string(track.get("id").unwrap_or(&Value::Null)) else {
            continue;
        };
        let name = as_string(track.get("name").unwrap_or(&Value::Null))
            .unwrap_or_else(|| "（无标题）".into());
        let artists: Vec<String> = track
            .get("artist_name")
            .and_then(Value::as_str)
            .map(|s| vec![s.to_string()])
            .unwrap_or_default();
        let album = as_string(track.get("album_name").unwrap_or(&Value::Null)).unwrap_or_default();
        let duration = as_string(track.get("duration").unwrap_or(&Value::Null))
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
            .saturating_mul(1000);
        let cover = as_string(track.get("album_image").unwrap_or(&Value::Null))
            .or_else(|| as_string(track.get("image").unwrap_or(&Value::Null)));
        let play = as_string(track.get("audio").unwrap_or(&Value::Null))
            .or_else(|| as_string(track.get("audiodownload").unwrap_or(&Value::Null)));
        out.push(Song {
            id,
            name,
            artists,
            album,
            album_id: None,
            duration_ms: duration,
            cover_url: cover,
            fee: 0,
            source: SourceKind::Custom,
            play_url: play,
            lyric_url: None,
            source_id: Some(source.id.clone()),
            origin: None,
        });
    }
    // 并发探活，失败的整条剔除
    let urls: Vec<String> = out.iter().filter_map(|s| s.play_url.clone()).collect();
    let probed = probe_all(http, &urls).await;
    let mut keep = Vec::with_capacity(out.len());
    let mut i = 0;
    for song in out {
        if song.play_url.is_some() {
            let ok = probed.get(i).copied().unwrap_or(false);
            i += 1;
            if !ok {
                continue;
            }
        }
        keep.push(song);
    }
    Ok(build(source, keep))
}

async fn fetch_fma(source: &Source, http: &reqwest::Client) -> Result<Vec<Song>> {
    let html = get_text(http, &source.url, fma_headers()).await?;
    let items = parse_fma_items(&html);
    if items.is_empty() {
        return Err(Error::Api("FMA 页面里没有找到 data-track-info".into()));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let play_url = item.file_url.clone();
        let artists: Vec<String> = if item.artist_name.trim().is_empty() {
            Vec::new()
        } else {
            vec![item.artist_name.clone()]
        };
        out.push(Song {
            id: item.id.to_string(),
            name: item.title.clone(),
            artists,
            album: item.album_title.clone(),
            album_id: None,
            duration_ms: 0,
            cover_url: item.cover_url,
            fee: 0,
            source: SourceKind::Custom,
            play_url: Some(play_url).filter(|u| !u.trim().is_empty()),
            lyric_url: None,
            source_id: Some(source.id.clone()),
            origin: None,
        });
    }
    let urls: Vec<String> = out.iter().filter_map(|s| s.play_url.clone()).collect();
    let probed = probe_all(http, &urls).await;
    let mut i = 0;
    let keep: Vec<Song> = out
        .into_iter()
        .filter(|song| {
            if song.play_url.is_none() {
                return true;
            }
            let ok = probed.get(i).copied().unwrap_or(false);
            i += 1;
            ok
        })
        .collect();
    Ok(build(source, keep))
}
