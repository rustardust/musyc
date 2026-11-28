use super::*;

impl ApiClient {
    /// 雀乐最新 20 期，歌曲随歌单一起返回。
    pub async fn indie_catalog(&self) -> Result<Vec<dto::IndiePlaylist>> {
        let v = self.get_local("musyc/catalog/indie", vec![]).await?;
        Ok(dto::array_of(&v, &["playlists"])
            .iter()
            .filter_map(dto::indie_playlist_from)
            .collect())
    }
}
