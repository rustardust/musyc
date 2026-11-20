use super::*;

impl App {
    pub(super) fn load_cover(&mut self, song: &Song) {
        let key = song.key();
        if self.cover.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        if let Some(art) = self.cover_cache.get(&key) {
            self.cover = Some((key, art.clone()));
            return;
        }
        self.cover = None;
        self.cover_seq += 1;
        let tx = self.tx.clone();
        let http = self.http.clone();
        let cover_url = song.cover_url.clone();
        let local_path = (song.source == SourceKind::Local).then(|| PathBuf::from(&song.id));
        tokio::spawn(async move {
            let bytes = if let Some(path) = local_path {
                tokio::task::spawn_blocking(move || musyc_library::read_cover(&path))
                    .await
                    .ok()
                    .flatten()
            } else if let Some(url) = cover_url.filter(|u| !u.trim().is_empty()) {
                match http.get(&url).send().await {
                    Ok(resp) => resp.bytes().await.ok().map(|b| b.to_vec()),
                    Err(_) => None,
                }
            } else {
                None
            };
            let Some(bytes) = bytes else {
                let _ = tx.send(Event::Cover {
                    key,
                    result: Err("没有封面".into()),
                });
                return;
            };
            let result = match tokio::task::spawn_blocking(move || decode_cover(&bytes)).await {
                Ok(Some(art)) => Ok(art),
                Ok(None) => Err("封面解码失败".into()),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(Event::Cover { key, result });
        });
    }
}
