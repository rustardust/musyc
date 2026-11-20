use super::*;

impl ApiClient {
    /// 街声即时榜（前端已不使用）。
    pub async fn streetvoice_realtime(&self) -> Result<Vec<Song>> {
        self.streetvoice_list("realtime").await
    }

    /// 街声推荐前 30 首。
    pub async fn streetvoice_recommend(&self) -> Result<Vec<Song>> {
        self.streetvoice_list("recommend").await
    }

    /// 街声分类（如 7 = Alternative）前 30 首。
    pub async fn streetvoice_cat(&self, cat: &str) -> Result<Vec<Song>> {
        let v = self
            .get_local(&format!("musyc/catalog/streetvoice/cat/{cat}"), vec![])
            .await?;
        Ok(dto::array_of(&v, &["songs"])
            .iter()
            .filter_map(|x| dto::source_song_from(x, SourceKind::StreetVoice))
            .collect())
    }

    pub async fn streetvoice_detail(&self, id: &str) -> Result<StreetVoiceDetail> {
        let v = self
            .get_local(&format!("musyc/catalog/streetvoice/{id}"), vec![])
            .await?;
        Ok(StreetVoiceDetail {
            duration_ms: v.get("duration_ms").and_then(Value::as_u64).unwrap_or(0),
            album: v
                .get("album")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            cover_url: v
                .get("cover_url")
                .and_then(Value::as_str)
                .map(str::to_string),
            lyrics: v
                .get("lyrics")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            lyrics_is_lrc: v
                .get("lyrics_is_lrc")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }
}
