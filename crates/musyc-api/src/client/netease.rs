use super::*;

impl ApiClient {
    pub async fn search_songs(&self, keywords: &str, limit: u32, offset: u32) -> Result<Vec<Song>> {
        let v = self
            .get(
                "cloudsearch",
                vec![
                    ("keywords", keywords.to_string()),
                    ("type", "1".into()),
                    ("limit", limit.to_string()),
                    ("offset", offset.to_string()),
                ],
            )
            .await?;
        let songs = v
            .pointer("/result/songs")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(dto::song_from).collect::<Vec<_>>())
            .unwrap_or_default();
        if songs.is_empty() {
            // 回退旧接口（字段是 artists / album）
            let v = self
                .get(
                    "search",
                    vec![
                        ("keywords", keywords.to_string()),
                        ("type", "1".into()),
                        ("limit", limit.to_string()),
                        ("offset", offset.to_string()),
                    ],
                )
                .await?;
            return Ok(v
                .pointer("/result/songs")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(dto::song_from).collect())
                .unwrap_or_default());
        }
        Ok(songs)
    }

    pub async fn search_playlists(&self, keywords: &str, limit: u32) -> Result<Vec<Playlist>> {
        let v = self
            .get(
                "cloudsearch",
                vec![
                    ("keywords", keywords.to_string()),
                    ("type", "1000".into()),
                    ("limit", limit.to_string()),
                ],
            )
            .await?;
        Ok(v.pointer("/result/playlists")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(dto::playlist_from).collect())
            .unwrap_or_default())
    }

    pub async fn song_detail(&self, ids: &[String]) -> Result<Vec<Song>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // 一次请求太多 ID 会让 URL 过长，分块查询并保持原顺序。
        const CHUNK: usize = 100;
        let mut songs = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(CHUNK) {
            let v = self
                .get("song/detail", vec![("ids", chunk.join(","))])
                .await?;
            songs.extend(
                dto::array_of(&v, &["songs"])
                    .iter()
                    .filter_map(dto::song_from),
            );
        }
        Ok(songs)
    }

    pub async fn song_url(&self, id: &str, quality: Quality) -> Result<Option<SongUrl>> {
        match self.song_url_v1(id, quality).await {
            Ok(Some(url)) => Ok(Some(url)),
            // song/url/v1 被上游 -460 风控时不要直接放弃：旧接口 song/url?id&br=
            // 返回的是标准音质（br=128010），至少能让歌放出声来。
            Ok(None) | Err(_) => self.song_url_legacy(id).await,
        }
    }

    pub async fn lyric(&self, id: &str) -> Result<RawLyric> {
        let v = match self.get("lyric/new", vec![("id", id.to_string())]).await {
            Ok(v) => v,
            Err(_) => {
                let old = self.get("lyric", vec![("id", id.to_string())]).await?;
                return Ok(RawLyric {
                    lrc: old
                        .pointer("/lrc/lyric")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    tlyric: old
                        .pointer("/tlyric/lyric")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                });
            }
        };
        Ok(RawLyric {
            lrc: v
                .pointer("/lrc/lyric")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            tlyric: v
                .pointer("/tlyric/lyric")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }

    pub async fn playlist_meta(&self, id: &str) -> Result<Playlist> {
        let v = self
            .get("playlist/detail", vec![("id", id.to_string())])
            .await?;
        v.pointer("/playlist")
            .and_then(dto::playlist_from)
            .ok_or_else(|| Error::Api("歌单不存在".into()))
    }

    pub async fn playlist_tracks(&self, id: &str, limit: u32, offset: u32) -> Result<Vec<Song>> {
        // `/playlist/track/all` 一次拉上千首很容易被上游重置，按 100 首分页。
        const PAGE: u32 = 100;
        let limit = limit.max(1);
        let mut songs = Vec::new();
        let mut offset = offset;
        while (songs.len() as u32) < limit {
            let want = PAGE.min(limit - songs.len() as u32);
            let v = self
                .get(
                    "playlist/track/all",
                    vec![
                        ("id", id.to_string()),
                        ("limit", want.to_string()),
                        ("offset", offset.to_string()),
                    ],
                )
                .await?;
            let page = dto::array_of(&v, &["songs"]);
            let count = page.len();
            songs.extend(page.iter().filter_map(dto::song_from));
            if count < want as usize {
                break;
            }
            offset += count as u32;
        }
        songs.truncate(limit as usize);
        Ok(songs)
    }

    /// 网易原创榜前 N 首（榜单 ID 2884035）。
    pub async fn original_chart(&self, limit: u32) -> Result<Vec<Song>> {
        self.playlist_tracks("2884035", limit, 0).await
    }
}
