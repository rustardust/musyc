//! Audius 热门榜（免费 Open API，无需登录 / 本机服务）。
//!
//! 列表：`GET https://api.audius.co/v1/tracks/trending?time=week&app_name=musyc`
//! 播放：`GET https://api.audius.co/v1/tracks/{id}/stream?app_name=musyc`
//! （302 到 creatornode CDN，直接交给播放器跟随跳转。）

use musyc_core::model::{Song, SourceKind};
use musyc_core::source_fetch::{as_string, get_path};
use musyc_core::sources::Source;
use musyc_core::{Error, Result};
use serde_json::Value;

const API_BASE: &str = "https://api.audius.co/v1";
const APP_NAME: &str = "musyc";

pub(super) async fn fetch_audius(source: &Source, http: &reqwest::Client) -> Result<Vec<Song>> {
    let url = format!(
        "{API_BASE}/tracks/trending?time={}&app_name={APP_NAME}&limit={}",
        source.time, source.limit
    );
    let json = super::get_json(http, &url, &[]).await?;
    let tracks = get_path(&json, "data")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| Error::Api("Audius 返回缺少 data 数组".into()))?;

    let songs: Vec<Song> = tracks.iter().filter_map(|track| song_from(source, track)).collect();
    if songs.is_empty() {
        return Err(Error::Api("Audius 热门榜为空".into()));
    }
    Ok(songs)
}

fn song_from(source: &Source, track: &Value) -> Option<Song> {
    let id = as_string(track.get("id")?)?;
    let name = as_string(track.get("title")?)?;
    let artist = track
        .pointer("/user/name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Audius")
        .to_string();
    let duration_ms = track
        .get("duration")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_mul(1000);
    let cover_url = track
        .pointer("/artwork/480x480")
        .and_then(Value::as_str)
        .map(str::to_string);
    let play_url = format!("{API_BASE}/tracks/{id}/stream?app_name={APP_NAME}");

    Some(Song {
        id,
        name,
        artists: vec![artist],
        album: String::new(),
        album_id: None,
        duration_ms,
        cover_url,
        fee: 0,
        source: SourceKind::Audius,
        play_url: Some(play_url),
        lyric_url: None,
        source_id: Some(source.id.clone()),
        origin: None,
    })
}
