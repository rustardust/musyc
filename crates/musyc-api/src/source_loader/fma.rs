use super::*;

pub(super) struct FmaItem {
    pub(super) id: u64,
    pub(super) title: String,
    pub(super) artist_name: String,
    pub(super) album_title: String,
    pub(super) file_url: String,
    pub(super) cover_url: Option<String>,
}

pub(super) fn parse_fma_items(html: &str) -> Vec<FmaItem> {
    let mut out = Vec::new();
    for cap in regex_lite_track_info(html) {
        let Ok(v) = serde_json::from_str::<Value>(cap) else {
            continue;
        };
        let id = v.get("id").and_then(Value::as_u64).unwrap_or(0);
        if id == 0 {
            continue;
        }
        let cover = v
            .get("trackImage")
            .and_then(as_string)
            .or_else(|| v.get("albumImage").and_then(as_string))
            .or_else(|| v.get("imageUrl").and_then(as_string));
        out.push(FmaItem {
            id,
            title: as_string(v.get("title").unwrap_or(&Value::Null))
                .unwrap_or_else(|| "未命名".into()),
            artist_name: as_string(v.get("artistName").unwrap_or(&Value::Null)).unwrap_or_default(),
            album_title: as_string(v.get("albumTitle").unwrap_or(&Value::Null)).unwrap_or_default(),
            file_url: as_string(v.get("fileUrl").unwrap_or(&Value::Null)).unwrap_or_default(),
            cover_url: cover,
        });
    }
    out
}

pub(super) fn regex_lite_track_info(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("data-track-info='") {
        let after = &rest[start + "data-track-info='".len()..];
        match after.find('\'') {
            Some(end) => {
                out.push(&after[..end]);
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
    out
}

pub(super) async fn fetch_generic_json(
    source: &Source,
    http: &reqwest::Client,
) -> Result<Vec<Song>> {
    let json = get_json(http, &source.url, source_generic_headers(source)).await?;
    let array = match get_path(&json, &source.list_path) {
        Some(v) => as_array(
            v,
            &["songs", "result", "results", "data", "tracks", "items"],
        ),
        None => as_array(
            &json,
            &["songs", "result", "results", "data", "tracks", "items"],
        ),
    };
    if array.is_empty() && !source.list_path.is_empty() {
        return Err(Error::Api(format!(
            "generic-json 在 {:?} 处没找到数组",
            source.list_path
        )));
    }
    let mut out = Vec::with_capacity(array.len().min(source.limit as usize));
    for item in &array {
        let id = pick_string(item, &source.id_path)
            .or_else(|| item.get("id").and_then(as_string))
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let name = pick_string(item, &source.name_path)
            .or_else(|| pick_string(item, "title"))
            .unwrap_or_else(|| "未命名".into());
        let artists = artists_from(item, &source.artists_path);
        let album = pick_string(item, &source.album_path).unwrap_or_default();
        let duration_ms = to_millis(
            pick_value(item, &source.duration_path),
            source.duration_unit,
        );
        let cover_url = pick_string(item, &source.cover_path);
        let play_url = pick_string(item, &source.play_url_path);
        let lyric_url = pick_string(item, &source.lyric_url_path);
        out.push(Song {
            id,
            name,
            artists,
            album,
            album_id: None,
            duration_ms,
            cover_url,
            fee: 0,
            source: SourceKind::Custom,
            play_url,
            lyric_url,
            source_id: Some(source.id.clone()),
            origin: None,
        });
        if out.len() >= source.limit as usize {
            break;
        }
    }
    Ok(build(source, out))
}
